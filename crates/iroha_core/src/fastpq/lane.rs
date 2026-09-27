//! Canonical masked FASTPQ proving for finalized full-domain transfer statements.
use crate::{
    fastpq::{FastpqWitnessContext, quantity_statement_from_finalized_transcripts},
    kura::{FastpqProofEnqueueResult, FastpqProofSnapshot, Kura},
};
use fastpq_prover::{
    DigestExecutionV1, MetalOverrides, apply_metal_overrides,
    offline_compact::{self, ExpectedStatement, ProvingError, ProvingLimits, VerificationLimits},
    set_metal_queue_policy,
};
use iroha_config::parameters::actual::{Fastpq, FastpqExecutionMode, FastpqPoseidonMode};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{BlockHeader, consensus::ExecWitness},
    fastpq::{FastpqArtifactIdentityDescriptionV1, FastpqPublicTransferStatementV1},
};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_logger::{debug, info, warn};
use std::{
    sync::{
        Arc, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use tokio::sync::mpsc;
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
                "fastpq lane: queueing background prover job while backend is initialising"
            );
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
/// Canonical artifact bytes and their independently recomputed verified identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastpqProofOutput {
    /// Complete canonical ordinary compact artifact, including its public statement.
    pub proof_bytes: Vec<u8>,
    /// Recomputed content identity and ordered AIR row commitments.
    pub identity: FastpqArtifactIdentityDescriptionV1,
}
/// Prover abstraction for exact finalized public statements.
pub trait FastpqProofEngine: Send + Sync + 'static {
    /// Prove the complete original quantities, identities and ordered occurrences.
    ///
    /// # Errors
    /// Returns errors for invalid statements, resource bounds or proving failures.
    fn prove(
        &self,
        statement: &FastpqPublicTransferStatementV1,
    ) -> Result<FastpqProofOutput, ProvingError>;
}
struct RealProofEngine {
    proving: ProvingLimits,
    verification: VerificationLimits,
}
impl FastpqProofEngine for RealProofEngine {
    fn prove(
        &self,
        statement: &FastpqPublicTransferStatementV1,
    ) -> Result<FastpqProofOutput, ProvingError> {
        let expected = expected_statement(statement)?;
        let proof_bytes = offline_compact::prove_quantity_ordinary_artifact(
            statement,
            expected,
            self.proving,
            self.verification,
        )?;
        let verified = offline_compact::verify_quantity_ordinary_artifact(
            &proof_bytes,
            expected,
            self.verification,
        )?;
        Ok(FastpqProofOutput {
            proof_bytes,
            identity: verified.identity().clone(),
        })
    }
}
fn expected_statement(
    statement: &FastpqPublicTransferStatementV1,
) -> fastpq_prover::Result<ExpectedStatement> {
    let encoded = norito::encode_canonical(statement).map_err(fastpq_prover::Error::Encode)?;
    Ok(ExpectedStatement {
        inputs: statement.public_inputs,
        ordering_hash: statement.ordering_hash,
        public_statement_digest: Hash::new(encoded).into(),
    })
}
struct RegisteredFastpqLane {
    generation: u64,
    handle: FastpqLaneHandle,
    shutdown: ShutdownSignal,
}
#[derive(Default)]
struct FastpqLaneRegistry {
    generation: u64,
    current: Option<RegisteredFastpqLane>,
}
struct FastpqLaneGenerationLease {
    generation: u64,
}
impl Drop for FastpqLaneGenerationLease {
    fn drop(&mut self) {
        // The async worker and any live `spawn_blocking` operation share this lease. A
        // supervisor abort can therefore drop the worker without making a replacement lane
        // visible until the detached blocking operation has actually stopped.
        clear_generation(self.generation);
    }
}
static GLOBAL_LANE: OnceLock<Mutex<FastpqLaneRegistry>> = OnceLock::new();
#[cfg(test)]
static TEST_ENGINE: OnceLock<Arc<dyn FastpqProofEngine>> = OnceLock::new();
fn global_lane() -> &'static Mutex<FastpqLaneRegistry> {
    GLOBAL_LANE.get_or_init(|| Mutex::new(FastpqLaneRegistry::default()))
}
fn lock_global_lane() -> MutexGuard<'static, FastpqLaneRegistry> {
    match global_lane().lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn!("fastpq lane registry mutex was poisoned; recovering registry state");
            poisoned.into_inner()
        }
    }
}
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
    start_with_options(cfg, backpressure, kura, None)
}
/// Start the FASTPQ prover lane with queue integration and a node shutdown signal.
pub fn start_with_backpressure_and_shutdown(
    cfg: &Fastpq,
    backpressure: Option<crate::queue::BackpressureHandle>,
    kura: Option<Arc<Kura>>,
    shutdown: ShutdownSignal,
) -> Option<(FastpqLaneHandle, tokio::task::JoinHandle<()>)> {
    start_with_options(cfg, backpressure, kura, Some(shutdown))
}
fn start_with_options(
    cfg: &Fastpq,
    backpressure: Option<crate::queue::BackpressureHandle>,
    kura: Option<Arc<Kura>>,
    external_shutdown: Option<ShutdownSignal>,
) -> Option<(FastpqLaneHandle, tokio::task::JoinHandle<()>)> {
    let cfg = cfg.clone();
    start_with_builder(backpressure, kura, external_shutdown, move || {
        build_engine(&cfg)
    })
}
fn start_with_builder(
    backpressure: Option<crate::queue::BackpressureHandle>,
    kura: Option<Arc<Kura>>,
    external_shutdown: Option<ShutdownSignal>,
    build_engine: impl FnOnce() -> Option<Arc<dyn FastpqProofEngine>> + Send + 'static,
) -> Option<(FastpqLaneHandle, tokio::task::JoinHandle<()>)> {
    let mut registry = lock_global_lane();
    if registry.current.is_some() {
        return None;
    }
    registry.generation = registry
        .generation
        .checked_add(1)
        .expect("FASTPQ lane generation exhausted");
    let generation = registry.generation;
    let (tx, rx) = mpsc::channel::<FastpqWitnessJob>(32);
    let ready = Arc::new(AtomicBool::new(false));
    let handle = FastpqLaneHandle {
        tx,
        backpressure,
        ready: Arc::clone(&ready),
    };
    let shutdown = ShutdownSignal::new();
    registry.current = Some(RegisteredFastpqLane {
        generation,
        handle: handle.clone(),
        shutdown: shutdown.clone(),
    });
    // A new generation must start fail-closed. `build_engine` performs the one hardware
    // preflight for this generation and enables the digest path only after it succeeds.
    crate::fastpq::set_poseidon_digest_acceleration_enabled(false);
    drop(registry);
    let generation_lease = Arc::new(FastpqLaneGenerationLease { generation });
    let task = spawn_worker(
        rx,
        ready,
        kura,
        generation_lease,
        shutdown,
        external_shutdown,
        build_engine,
    );
    Some((handle, task))
}
/// Submit a prover job if the lane is running.
pub fn try_submit(job: FastpqWitnessJob) -> bool {
    let handle = lock_global_lane()
        .current
        .as_ref()
        .map(|registered| registered.handle.clone());
    handle.is_some_and(|handle| handle.submit(job))
}
/// Request shutdown of the active FASTPQ lane, if any.
pub fn shutdown() {
    let shutdown = lock_global_lane()
        .current
        .as_ref()
        .map(|registered| registered.shutdown.clone());
    if let Some(shutdown) = shutdown {
        shutdown.send();
    }
}
fn clear_generation(generation: u64) {
    let mut registry = lock_global_lane();
    if registry
        .current
        .as_ref()
        .is_some_and(|registered| registered.generation == generation)
    {
        if let Some(registered) = registry.current.take() {
            registered.handle.ready.store(false, Ordering::Release);
        }
    }
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
    let digest_execution = match configured_digest_execution(cfg) {
        Ok(execution) => execution,
        Err(err) => {
            warn!(%err, "fastpq lane: required digest device failed preflight");
            return None;
        }
    };
    let mut verification = VerificationLimits::default();
    verification.transport.max_wire_bytes = verification
        .transport
        .max_wire_bytes
        .min(usize::try_from(cfg.proof_sidecar_max_bytes.get()).unwrap_or(usize::MAX));
    Some(Arc::new(RealProofEngine {
        proving: ProvingLimits {
            digest_execution,
            ..ProvingLimits::default()
        },
        verification,
    }))
}
fn configured_digest_execution(cfg: &Fastpq) -> fastpq_prover::Result<DigestExecutionV1> {
    let required_device = matches!(cfg.execution_mode, FastpqExecutionMode::Gpu)
        || matches!(cfg.poseidon_mode, FastpqPoseidonMode::Gpu);
    if !required_device {
        crate::fastpq::set_poseidon_digest_acceleration_enabled(false);
        return Ok(DigestExecutionV1::Cpu);
    }
    #[cfg(feature = "fastpq-gpu")]
    {
        let backend = if cfg!(target_os = "macos") {
            fastpq_prover::Digest384GpuBackendV1::Metal
        } else {
            fastpq_prover::Digest384GpuBackendV1::Cuda
        };
        fastpq_prover::preflight_digest384_continuation_v1(backend).map_err(|error| {
            fastpq_prover::Error::NativeDigestExecution {
                details: error.to_string(),
            }
        })?;
        let enabled = crate::fastpq::poseidon_digest_acceleration_configured(cfg)
            && fastpq_prover::preflight_bn254_poseidon_word_batches();
        crate::fastpq::set_poseidon_digest_acceleration_enabled(enabled);
        Ok(DigestExecutionV1::Device(backend))
    }
    #[cfg(not(feature = "fastpq-gpu"))]
    Err(fastpq_prover::Error::NativeDigestExecution {
        details: "required FASTPQ digest device support is not compiled".into(),
    })
}
fn spawn_worker(
    mut rx: mpsc::Receiver<FastpqWitnessJob>,
    ready: Arc<AtomicBool>,
    kura: Option<Arc<Kura>>,
    generation_lease: Arc<FastpqLaneGenerationLease>,
    shutdown: ShutdownSignal,
    external_shutdown: Option<ShutdownSignal>,
    build_engine: impl FnOnce() -> Option<Arc<dyn FastpqProofEngine>> + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let engine_generation_lease = Arc::clone(&generation_lease);
        let mut engine_task = tokio::task::spawn_blocking(move || {
            let _generation_lease = engine_generation_lease;
            build_engine()
        });
        let engine_result = tokio::select! {
            result = &mut engine_task => Some(result),
            () = wait_for_shutdown(shutdown.clone(), external_shutdown.clone()) => None,
        };
        let Some(engine_result) = engine_result else {
            rx.close();
            // `spawn_blocking` work cannot be cancelled after it starts. Keep this generation
            // registered until initialisation actually finishes so a retry cannot race global
            // Metal/prover setup from the retiring worker.
            let _ = engine_task.await;
            return;
        };
        let engine = match engine_result {
            Ok(Some(engine)) => engine,
            Ok(None) => {
                warn!("fastpq lane: failed to initialise prover backend; lane disabled");
                rx.close();
                return;
            }
            Err(err) => {
                warn!(
                    ?err,
                    "fastpq lane: prover backend initialisation task panicked"
                );
                rx.close();
                return;
            }
        };
        ready.store(true, Ordering::Release);
        loop {
            let job = tokio::select! {
                job = rx.recv() => job,
                () = wait_for_shutdown(shutdown.clone(), external_shutdown.clone()) => {
                    rx.close();
                    None
                },
            };
            let Some(job) = job else {
                break;
            };
            let engine = Arc::clone(&engine);
            let kura = kura.clone();
            let prove_shutdown = shutdown.clone();
            let prove_external_shutdown = external_shutdown.clone();
            let prove_generation_lease = Arc::clone(&generation_lease);
            let mut prove_task = tokio::task::spawn_blocking(move || {
                let _generation_lease = prove_generation_lease;
                process_job(
                    &engine,
                    &job,
                    kura.as_deref(),
                    &prove_shutdown,
                    prove_external_shutdown.as_ref(),
                );
            });
            tokio::select! {
                result = &mut prove_task => {
                    if let Err(err) = result {
                        warn!(?err, "fastpq lane: prover task panicked");
                    }
                }
                () = wait_for_shutdown(shutdown.clone(), external_shutdown.clone()) => {
                    rx.close();
                    // Proof work may persist a sidecar before returning. Await it before releasing
                    // the generation so no old worker can write after a same-process restart.
                    if let Err(err) = prove_task.await {
                        warn!(?err, "fastpq lane: prover task panicked during shutdown");
                    }
                    break;
                }
            }
        }
        ready.store(false, Ordering::Release);
    })
}
async fn wait_for_shutdown(shutdown: ShutdownSignal, external: Option<ShutdownSignal>) {
    if let Some(external) = external {
        tokio::select! {
            () = shutdown.receive() => {}
            () = external.receive() => {}
        }
    } else {
        shutdown.receive().await;
    }
}
fn metal_overrides_from_config(cfg: &Fastpq) -> MetalOverrides {
    MetalOverrides {
        max_in_flight: cfg.metal_max_in_flight,
        threadgroup_size: cfg.metal_threadgroup_width,
        dispatch_trace: cfg.metal_trace,
        debug_enum: cfg.metal_debug_enum,
    }
}
fn process_job(
    engine: &Arc<dyn FastpqProofEngine>,
    job: &FastpqWitnessJob,
    kura: Option<&Kura>,
    shutdown: &ShutdownSignal,
    external_shutdown: Option<&ShutdownSignal>,
) {
    if shutdown_requested(shutdown, external_shutdown) {
        return;
    }
    if job.witness.fastpq_transcripts.is_empty() && job.witness.fastpq_batches.is_empty() {
        debug!(
            height = job.height,
            view = job.view,
            "fastpq lane: witness contains no transcripts"
        );
        return;
    }
    let statements = match statements_for_job(job) {
        Ok(statements) => statements,
        Err(err) => {
            warn!(
                height = job.height,
                view = job.view,
                ?err,
                "fastpq lane: failed to construct canonical statements"
            );
            return;
        }
    };
    if statements.is_empty() {
        debug!(
            height = job.height,
            view = job.view,
            "fastpq lane: no statements produced from witness"
        );
        return;
    }
    let batch_count = statements.len();
    let job_started = Instant::now();
    let mut proved = 0usize;
    let mut failed = 0usize;
    let mut persisted = 0usize;
    let mut transition_count = 0usize;
    for (idx, (entry_hash, statement)) in statements.into_iter().enumerate() {
        if shutdown_requested(shutdown, external_shutdown) {
            break;
        }
        let entry_hash_hex = hex::encode(entry_hash.as_ref());
        transition_count = transition_count.saturating_add(statement.transitions.len());
        let started = Instant::now();
        let proof_result = engine.prove(&statement);
        // `spawn_blocking` continues after its async JoinHandle is aborted. In particular,
        // the node supervisor may stop waiting for this lane after its shutdown timeout.
        // Discard a proof completed after either shutdown signal so the detached task cannot
        // enqueue a sidecar after the node has begun shutting down.
        if shutdown_requested(shutdown, external_shutdown) {
            debug!(
                height = job.height,
                view = job.view,
                batch_index = idx,
                "fastpq lane: discarding proof result completed during shutdown"
            );
            break;
        }
        match proof_result {
            Ok(output) => {
                proved = proved.saturating_add(1);
                if let Some(kura) = kura {
                    if let Ok(batch_index) = u32::try_from(idx) {
                        let snapshot = FastpqProofSnapshot::from_statement(
                            job.height,
                            job.block_hash,
                            entry_hash,
                            batch_index,
                            &statement,
                            output.identity.clone(),
                        );
                        if shutdown_requested(shutdown, external_shutdown) {
                            break;
                        }
                        match kura.enqueue_fastpq_proof_snapshot_unless(snapshot, || {
                            shutdown_requested(shutdown, external_shutdown)
                        }) {
                            FastpqProofEnqueueResult::Enqueued { .. } => {
                                persisted = persisted.saturating_add(1);
                            }
                            FastpqProofEnqueueResult::RejectedShutdown => {
                                debug!(
                                    height = job.height,
                                    view = job.view,
                                    entry_hash = entry_hash_hex,
                                    "fastpq lane: proof snapshot enqueue cancelled during shutdown"
                                );
                                break;
                            }
                            result => {
                                warn!(
                                    height = job.height,
                                    view = job.view,
                                    entry_hash = entry_hash_hex,
                                    ?result,
                                    "fastpq lane: proof snapshot was not enqueued for persistence"
                                );
                            }
                        }
                    } else {
                        kura.record_fastpq_missing_entry_hash();
                        warn!(
                            height = job.height,
                            view = job.view,
                            batch_index = idx,
                            "fastpq lane: missing entry hash; proof snapshot not persisted"
                        );
                    }
                }
                debug!(
                    height = job.height,
                    view = job.view,
                    entry_hash = entry_hash_hex,
                    transitions = statement.transitions.len(),
                    proof_bytes = output.proof_bytes.len(),
                    artifact_digest = ?output.identity.artifact_digest,
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
fn shutdown_requested(
    shutdown: &ShutdownSignal,
    external_shutdown: Option<&ShutdownSignal>,
) -> bool {
    shutdown.is_sent() || external_shutdown.is_some_and(ShutdownSignal::is_sent)
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
    use super::*;
    use crate::fastpq::{
        DigestAccelerationTestGuard, FASTPQ_CANONICAL_PARAMETER_SET, FastpqPublicInputsTemplate,
        authority_digest, batches_from_bundles, transition_batch_to_dto,
    };
    use fastpq_prover::TransitionBatch;
    use iroha_data_model::fastpq::{
        TransferDeltaTranscript, TransferTranscript, TransferTranscriptBundle,
    };
    use iroha_model_base::domain::DomainId;
    use iroha_primitives::numeric::Quantity;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use std::{collections::BTreeMap, sync::atomic::AtomicBool, time::Duration};
    static LANE_REGISTRY_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    fn gpu_execution_cpu_poseidon_config() -> Fastpq {
        Fastpq {
            execution_mode: FastpqExecutionMode::Gpu,
            poseidon_mode: FastpqPoseidonMode::Cpu,
            proof_sidecar_queue_cap:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_QUEUE_CAP,
            proof_sidecar_max_bytes:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_MAX_BYTES,
            proof_sidecar_max_retries:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_MAX_RETRIES,
            device_class: None,
            chip_family: None,
            gpu_kind: None,
            metal_queue_fanout: None,
            metal_queue_column_threshold: None,
            metal_max_in_flight: None,
            metal_threadgroup_width: None,
            metal_trace: iroha_config::parameters::defaults::zk::fastpq::METAL_TRACE,
            metal_debug_enum: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_ENUM,
        }
    }
    #[tokio::test]
    async fn lane_processes_transcripts_with_mock_engine() {
        use tokio::time::{Instant, sleep};
        let _registry_lock = LANE_REGISTRY_TEST_LOCK.lock().await;
        let _digest_guard = DigestAccelerationTestGuard::new();
        let calls = Arc::new(std::sync::Mutex::new(0usize));
        install_test_engine(Arc::new(MockEngine {
            calls: Arc::clone(&calls),
        }));
        let cfg = Fastpq {
            execution_mode: FastpqExecutionMode::Cpu,
            poseidon_mode: FastpqPoseidonMode::Cpu,
            proof_sidecar_queue_cap:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_QUEUE_CAP,
            proof_sidecar_max_bytes:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_MAX_BYTES,
            proof_sidecar_max_retries:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_MAX_RETRIES,
            device_class: None,
            chip_family: None,
            gpu_kind: None,
            metal_queue_fanout: None,
            metal_queue_column_threshold: None,
            metal_max_in_flight: None,
            metal_threadgroup_width: None,
            metal_trace: iroha_config::parameters::defaults::zk::fastpq::METAL_TRACE,
            metal_debug_enum: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_ENUM,
        };
        let (handle, task) = start(&cfg).expect("lane starts");
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
            context: FastpqWitnessContext {
                public_inputs: Some(template),
                tx_set_hash: Some(tx_set_hash),
                entry_dataspaces: BTreeMap::new(),
                _source_inventory: None,
            },
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
        shutdown();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("fastpq lane stops after shutdown")
            .expect("fastpq worker joins cleanly");
    }
    #[tokio::test]
    async fn worker_start_does_not_wait_for_backend_initialisation() {
        use std::time::Instant as StdInstant;
        use tokio::time::sleep;
        let (_tx, rx) = mpsc::channel::<FastpqWitnessJob>(1);
        let ready = Arc::new(AtomicBool::new(false));
        let started_at = StdInstant::now();
        let task = spawn_worker(
            rx,
            Arc::clone(&ready),
            None,
            Arc::new(FastpqLaneGenerationLease { generation: 0 }),
            ShutdownSignal::new(),
            None,
            || {
                std::thread::sleep(Duration::from_millis(200));
                None
            },
        );
        assert!(
            started_at.elapsed() < Duration::from_millis(100),
            "worker startup waited for backend initialisation"
        );
        assert!(!ready.load(Ordering::Acquire));
        sleep(Duration::from_millis(250)).await;
        assert!(!ready.load(Ordering::Acquire));
        task.await.expect("worker task joins");
    }
    #[tokio::test]
    async fn new_lane_generation_starts_with_digest_acceleration_disabled() {
        let _registry_lock = LANE_REGISTRY_TEST_LOCK.lock().await;
        let _digest_guard = DigestAccelerationTestGuard::new();
        crate::fastpq::set_poseidon_digest_acceleration_enabled(true);

        let (_handle, task) =
            start_with_builder(None, None, None, || None).expect("lane generation registers");
        assert!(
            !crate::fastpq::poseidon_digest_acceleration_enabled(),
            "a new lane must stay on the CPU digest path until its one preflight succeeds"
        );

        task.await.expect("failed worker joins cleanly");
    }
    #[tokio::test]
    async fn failed_backend_initialisation_allows_lane_retry() {
        use tokio::time::{Instant, sleep};
        let _registry_lock = LANE_REGISTRY_TEST_LOCK.lock().await;
        let _digest_guard = DigestAccelerationTestGuard::new();
        let (_failed_handle, failed_task) =
            start_with_builder(None, None, None, || None).expect("failed lane attempt registers");
        failed_task.await.expect("failed worker joins cleanly");
        assert!(
            lock_global_lane().current.is_none(),
            "failed generation must release the global lane registration"
        );

        let calls = Arc::new(std::sync::Mutex::new(0usize));
        let retry_calls = Arc::clone(&calls);
        let (retry_handle, retry_task) = start_with_builder(None, None, None, move || {
            Some(Arc::new(MockEngine { calls: retry_calls }))
        })
        .expect("lane retry registers");
        let deadline = Instant::now() + Duration::from_secs(1);
        while !retry_handle.is_ready_for_test() {
            assert!(
                Instant::now() < deadline,
                "retried lane did not become ready"
            );
            sleep(Duration::from_millis(10)).await;
        }
        shutdown();
        tokio::time::timeout(Duration::from_secs(1), retry_task)
            .await
            .expect("retried lane observes shutdown")
            .expect("retried worker joins cleanly");
    }
    #[tokio::test]
    async fn external_shutdown_closes_idle_lane_receiver() {
        use tokio::time::{Instant, sleep};
        let _registry_lock = LANE_REGISTRY_TEST_LOCK.lock().await;
        let _digest_guard = DigestAccelerationTestGuard::new();
        let external_shutdown = ShutdownSignal::new();
        let calls = Arc::new(std::sync::Mutex::new(0usize));
        let (handle, task) =
            start_with_builder(None, None, Some(external_shutdown.clone()), move || {
                Some(Arc::new(MockEngine { calls }))
            })
            .expect("lane registers");
        let deadline = Instant::now() + Duration::from_secs(1);
        while !handle.is_ready_for_test() {
            assert!(Instant::now() < deadline, "lane did not become ready");
            sleep(Duration::from_millis(10)).await;
        }

        external_shutdown.send();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("idle lane exits on node shutdown")
            .expect("worker joins cleanly");
        assert!(
            lock_global_lane().current.is_none(),
            "shutdown generation must release the global lane registration"
        );
        assert!(
            !handle.submit(FastpqWitnessJob {
                block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0xCD; 32]
                )),
                height: 1,
                view: 0,
                witness: ExecWitness::default(),
                context: FastpqWitnessContext::default(),
            }),
            "closed lane receiver must reject submissions"
        );
    }
    #[tokio::test]
    async fn shutdown_keeps_generation_until_blocking_initialisation_finishes() {
        let _registry_lock = LANE_REGISTRY_TEST_LOCK.lock().await;
        let _digest_guard = DigestAccelerationTestGuard::new();
        let external_shutdown = ShutdownSignal::new();
        // Await startup without blocking this test's single Tokio thread.
        // Dropping the release sender also frees the blocking initializer if
        // an assertion fails, so runtime teardown cannot hang on the fixture.
        let (started, started_rx) = tokio::sync::oneshot::channel();
        let (release, release_rx) = std::sync::mpsc::channel();
        let (_handle, task) =
            start_with_builder(None, None, Some(external_shutdown.clone()), move || {
                if started.send(()).is_ok() {
                    let _ = release_rx.recv();
                }
                None
            })
            .expect("lane registers");
        tokio::time::timeout(Duration::from_secs(5), started_rx)
            .await
            .expect("blocking setup starts without blocking the runtime")
            .expect("blocking setup reports startup");
        external_shutdown.send();
        tokio::task::yield_now().await;
        assert!(
            lock_global_lane().current.is_some(),
            "retiring generation must remain registered while blocking setup is alive"
        );
        assert!(!task.is_finished());

        release
            .send(())
            .expect("blocking setup remains alive until explicitly released");
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("lane exits once blocking setup returns")
            .expect("worker joins cleanly");
        assert!(lock_global_lane().current.is_none());
    }
    #[tokio::test]
    async fn aborted_worker_releases_generation_after_blocking_initialisation_finishes() {
        use tokio::time::{Instant, sleep};
        let _registry_lock = LANE_REGISTRY_TEST_LOCK.lock().await;
        let _digest_guard = DigestAccelerationTestGuard::new();
        let external_shutdown = ShutdownSignal::new();
        // Await startup without blocking this test's single Tokio thread.
        // Dropping the release sender also frees the blocking initializer if
        // an assertion fails, so runtime teardown cannot hang on the fixture.
        let (started, started_rx) = tokio::sync::oneshot::channel();
        let (release, release_rx) = std::sync::mpsc::channel();
        let (_handle, task) =
            start_with_builder(None, None, Some(external_shutdown.clone()), move || {
                if started.send(()).is_ok() {
                    let _ = release_rx.recv();
                }
                None
            })
            .expect("lane registers");
        tokio::time::timeout(Duration::from_secs(5), started_rx)
            .await
            .expect("blocking setup starts without blocking the runtime")
            .expect("blocking setup reports startup");
        external_shutdown.send();
        task.abort();
        let join_error = task.await.expect_err("aborted worker reports cancellation");
        assert!(join_error.is_cancelled());
        assert!(
            lock_global_lane().current.is_some(),
            "detached blocking setup must retain its generation lease"
        );

        release
            .send(())
            .expect("blocking setup remains alive until explicitly released");
        let deadline = Instant::now() + Duration::from_secs(1);
        while lock_global_lane().current.is_some() {
            assert!(
                Instant::now() < deadline,
                "completed detached setup did not release the lane generation"
            );
            sleep(Duration::from_millis(10)).await;
        }
    }
    #[test]
    fn handle_buffers_jobs_while_backend_is_initialising() {
        let (tx, mut rx) = mpsc::channel(1);
        let handle = FastpqLaneHandle {
            tx,
            backpressure: None,
            ready: Arc::new(AtomicBool::new(false)),
        };
        let job = FastpqWitnessJob {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAB; 32])),
            height: 42,
            view: 7,
            witness: ExecWitness::default(),
            context: FastpqWitnessContext::default(),
        };

        assert!(handle.submit(job));
        let queued = rx.try_recv().expect("pre-ready job is buffered");
        assert_eq!(queued.height, 42);
        assert_eq!(queued.view, 7);
    }
    #[derive(Clone)]
    struct MockEngine {
        calls: Arc<std::sync::Mutex<usize>>,
    }
    impl FastpqProofEngine for MockEngine {
        fn prove(
            &self,
            statement: &FastpqPublicTransferStatementV1,
        ) -> Result<FastpqProofOutput, ProvingError> {
            *self.calls.lock().unwrap() += 1;

            let proof_bytes = b"mock-fastpq-proof".to_vec();
            Ok(FastpqProofOutput {
                identity: mock_identity(statement, &proof_bytes),
                proof_bytes,
            })
        }
    }
    struct ShutdownDuringProofEngine {
        shutdown: ShutdownSignal,
    }
    impl FastpqProofEngine for ShutdownDuringProofEngine {
        fn prove(
            &self,
            _statement: &FastpqPublicTransferStatementV1,
        ) -> Result<FastpqProofOutput, ProvingError> {
            self.shutdown.send();
            let proof_bytes = b"proof-completed-after-shutdown".to_vec();
            Ok(FastpqProofOutput {
                identity: mock_identity(_statement, &proof_bytes),
                proof_bytes,
            })
        }
    }
    fn sample_bundle() -> TransferTranscriptBundle {
        let mut bundle = TransferTranscriptBundle {
            entry_hash: Hash::prehashed([0x11; 32]),
            transcripts: vec![TransferTranscript {
                batch_hash: Hash::prehashed([0x22; 32]),
                deltas: vec![TransferDeltaTranscript {
                    from_account: (*ALICE_ID).clone(),
                    to_account: (*BOB_ID).clone(),
                    asset_definition:
                        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                            DomainId::try_new("wonderland", "universal").unwrap(),
                            "rose".parse().unwrap(),
                        ),
                    amount: Quantity::from(10u32),
                    from_balance_before: Quantity::from(100u32),
                    from_balance_after: Quantity::from(90u32),
                    to_balance_before: Quantity::from(5u32),
                    to_balance_after: Quantity::from(15u32),
                    from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
                    to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
                }],
                authority_digest: authority_digest(&ALICE_ID),
                poseidon_preimage_digest: None,
            }],
        };
        let transcript = &mut bundle.transcripts[0];
        transcript.poseidon_preimage_digest = Some(crate::fastpq::poseidon_preimage_digest(
            &transcript.deltas[0],
            &transcript.batch_hash,
        ));
        bundle
    }
    fn sample_batches(bundle: &TransferTranscriptBundle) -> Vec<TransitionBatch> {
        batches_from_bundles(
            FASTPQ_CANONICAL_PARAMETER_SET,
            FastpqPublicInputsTemplate {
                dsid: [0; 16],
                slot: 0,
                old_root: [0; 32],
                new_root: [0; 32],
                perm_root: [0; 32],
            },
            [0; 32],
            [bundle],
        )
        .expect("sample FASTPQ batch")
    }
    #[test]
    fn proof_completed_during_shutdown_is_not_enqueued() {
        let lane_shutdown = ShutdownSignal::new();
        let supervisor_shutdown = ShutdownSignal::new();
        let engine: Arc<dyn FastpqProofEngine> = Arc::new(ShutdownDuringProofEngine {
            shutdown: supervisor_shutdown.clone(),
        });
        let bundle = sample_bundle();
        let batches = sample_batches(&bundle);
        let template = FastpqPublicInputsTemplate {
            dsid: [0; 16],
            slot: 0,
            old_root: [0; 32],
            new_root: [0; 32],
            perm_root: [0; 32],
        };
        // This fixture has an internal transcript and no external transaction wires.
        // Supply the real empty-wire commitment so admission reaches the prover.
        let entrypoints: [iroha_data_model::transaction::TransactionEntrypoint; 0] = [];
        let tx_set_hash =
            iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(&entrypoints)
                .expect("canonical empty transaction-wire commitment")
                .into();
        let job = FastpqWitnessJob {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAC; 32])),
            height: 7,
            view: 3,
            witness: ExecWitness {
                fastpq_transcripts: vec![bundle],
                fastpq_batches: batches.iter().map(transition_batch_to_dto).collect(),
                ..ExecWitness::default()
            },
            context: FastpqWitnessContext {
                public_inputs: Some(template),
                tx_set_hash: Some(tx_set_hash),
                entry_dataspaces: BTreeMap::new(),
                _source_inventory: None,
            },
        };
        let admitted = statements_for_job(&job).expect("shutdown fixture reaches the prover");
        assert_eq!(admitted.len(), 1);
        assert_eq!(admitted[0].1.public_inputs.tx_set_hash, tx_set_hash);
        let kura = Kura::blank_kura_for_testing();

        process_job(
            &engine,
            &job,
            Some(&kura),
            &lane_shutdown,
            Some(&supervisor_shutdown),
        );

        assert!(supervisor_shutdown.is_sent());
        assert!(!lane_shutdown.is_sent());
        assert_eq!(
            kura.fastpq_proof_queue_len_for_testing(),
            0,
            "a detached proof must not persist after shutdown"
        );
    }
    #[test]
    fn maps_config_to_metal_overrides() {
        let cfg = Fastpq {
            execution_mode: FastpqExecutionMode::Gpu,
            poseidon_mode: FastpqPoseidonMode::Gpu,
            proof_sidecar_queue_cap:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_QUEUE_CAP,
            proof_sidecar_max_bytes:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_MAX_BYTES,
            proof_sidecar_max_retries:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_MAX_RETRIES,
            device_class: None,
            chip_family: None,
            gpu_kind: None,
            metal_queue_fanout: None,
            metal_queue_column_threshold: None,
            metal_max_in_flight: Some(8),
            metal_threadgroup_width: Some(256),
            metal_trace: true,
            metal_debug_enum: true,
        };
        let overrides = metal_overrides_from_config(&cfg);
        assert_eq!(overrides.max_in_flight, Some(8));
        assert_eq!(overrides.threadgroup_size, Some(256));
        assert!(overrides.dispatch_trace);
        assert!(overrides.debug_enum);
    }
    fn mock_identity(
        statement: &FastpqPublicTransferStatementV1,
        bytes: &[u8],
    ) -> FastpqArtifactIdentityDescriptionV1 {
        use iroha_data_model::fastpq::{
            FastpqCommitmentDescriptionV1, FastpqOrderedCompactAirCommitmentsV1, FastpqProofKindV1,
        };
        FastpqArtifactIdentityDescriptionV1 {
            proof_kind: FastpqProofKindV1::OrdinaryCompact,
            profile_id: offline_compact::quantity_profile_id(),
            public_statement_digest: expected_statement(statement)
                .unwrap()
                .public_statement_digest,
            artifact_digest: Hash::new(bytes).into(),
            inner_bundle_digest: Hash::new(b"mock inner bundle").into(),
            artifact_bytes: bytes.len() as u64,
            commitments: FastpqCommitmentDescriptionV1::OrderedCompactAir(
                FastpqOrderedCompactAirCommitmentsV1 {
                    segment_count: 1,
                    segment_air_row_roots: vec![
                        iroha_data_model::privacy::GoldilocksDigest384V1::new([0x31; 6]).unwrap(),
                    ],
                },
            ),
        }
    }
    fn sample_job() -> FastpqWitnessJob {
        let bundle = sample_bundle();
        let entry_hash = bundle.entry_hash;
        FastpqWitnessJob {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAA; 32])),
            height: 42,
            view: 7,
            witness: ExecWitness {
                fastpq_transcripts: vec![bundle],
                ..ExecWitness::default()
            },
            context: FastpqWitnessContext {
                public_inputs: Some(FastpqPublicInputsTemplate {
                    dsid: [1; 16],
                    slot: 23,
                    old_root: [2; 32],
                    new_root: [3; 32],
                    perm_root: [4; 32],
                }),
                tx_set_hash: Some([5; 32]),
                entry_dataspaces: BTreeMap::from([(entry_hash, [6; 16])]),
                _source_inventory: None,
            },
        }
    }
    #[test]
    fn finalized_job_preserves_full_quantities_context_and_original_entry_identity() {
        let mut job = sample_job();
        let transcript = &mut job.witness.fastpq_transcripts[0].transcripts[0];
        let delta = &mut transcript.deltas[0];
        delta.from_balance_before = Quantity::from(u128::MAX);
        delta.from_balance_after = delta.from_balance_before.try_sub(&delta.amount).unwrap();
        transcript.poseidon_preimage_digest = Some(crate::fastpq::poseidon_preimage_digest(
            delta,
            &transcript.batch_hash,
        ));
        let original = norito::encode_canonical(&job.witness.fastpq_transcripts).unwrap();
        let statements = statements_for_job(&job).unwrap();
        assert_eq!(statements.len(), 1);
        let (entry, statement) = &statements[0];
        assert_eq!(*entry, job.witness.fastpq_transcripts[0].entry_hash);
        assert_eq!(statement.public_inputs.dsid, [6; 16]);
        assert_eq!(statement.public_inputs.slot, 23);
        assert_eq!(statement.public_inputs.perm_root, [4; 32]);
        assert_eq!(statement.public_inputs.tx_set_hash, [5; 32]);
        assert_eq!(
            statement.transcripts[0].deltas[0].from_balance_before,
            Quantity::from(u128::MAX)
        );
        assert_eq!(statement.transitions.len(), 2);
        assert_eq!(
            norito::encode_canonical(&job.witness.fastpq_transcripts).unwrap(),
            original
        );
    }
    #[test]
    fn finalized_job_requires_source_context_and_rejects_missing_digest_without_repair() {
        let original = sample_job();
        for mutation in 0..5 {
            let mut job = original.clone();
            match mutation {
                0 => job.context.public_inputs = None,
                1 => job.context.tx_set_hash = None,
                2 => job.context.tx_set_hash = Some([0; 32]),
                3 => {
                    job.witness.fastpq_transcripts[0].transcripts[0].poseidon_preimage_digest = None
                }
                _ => {
                    job.witness.fastpq_batches = sample_batches(&job.witness.fastpq_transcripts[0])
                        .iter()
                        .map(transition_batch_to_dto)
                        .collect();
                    job.witness.fastpq_transcripts.clear();
                }
            }
            let before = norito::encode_canonical(&job.witness).unwrap();
            assert!(statements_for_job(&job).is_err());
            assert_eq!(norito::encode_canonical(&job.witness).unwrap(), before);
        }
        let mut empty = original;
        empty.witness = ExecWitness::default();
        assert!(statements_for_job(&empty).unwrap().is_empty());
    }
    #[test]
    fn precomputed_batches_cannot_override_finalized_statement_or_entry() {
        let mut job = sample_job();
        let expected = statements_for_job(&job).unwrap();
        let mut batch = sample_batches(&job.witness.fastpq_transcripts[0]).remove(0);
        batch.public_inputs.tx_set_hash = [0xE1; 32];
        batch.public_inputs.dsid = [0xE2; 16];
        batch.metadata.clear();
        job.witness.fastpq_batches = vec![transition_batch_to_dto(&batch)];
        assert_eq!(statements_for_job(&job).unwrap(), expected);
    }
    #[test]
    fn statement_expectations_are_canonical_and_bind_every_ambient_layout() {
        let statement = statements_for_job(&sample_job()).unwrap().remove(0).1;
        let canonical = norito::encode_canonical(&statement).unwrap();
        let expected = expected_statement(&statement).unwrap();
        assert_eq!(
            expected.public_statement_digest,
            <[u8; 32]>::from(Hash::new(&canonical))
        );
        for flags in
            (u8::MIN..=u8::MAX).filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
        {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            assert_eq!(expected_statement(&statement).unwrap(), expected);
            assert_eq!(norito::core::effective_decode_flags(), Some(flags));
        }
        let mut changed = statement;
        changed.ordering_hash[0] ^= 1;
        assert_ne!(expected_statement(&changed).unwrap(), expected);
    }
    #[test]
    fn real_engine_enforces_artifact_output_limit_before_proof_work() {
        let statement = statements_for_job(&sample_job()).unwrap().remove(0).1;
        let mut verification = VerificationLimits::default();
        verification.transport.max_wire_bytes = 0;
        let engine = RealProofEngine {
            proving: ProvingLimits::default(),
            verification,
        };
        assert!(engine.prove(&statement).is_err());
    }
    #[test]
    fn cpu_policy_uses_canonical_producer_without_device_readiness() {
        let _digest_guard = DigestAccelerationTestGuard::new();
        let mut cfg = gpu_execution_cpu_poseidon_config();
        cfg.execution_mode = FastpqExecutionMode::Cpu;
        assert_eq!(
            configured_digest_execution(&cfg).unwrap(),
            DigestExecutionV1::Cpu
        );
    }
    #[cfg(not(feature = "fastpq-gpu"))]
    #[test]
    fn explicit_device_policy_without_device_feature_fails_closed() {
        let _digest_guard = DigestAccelerationTestGuard::new();
        let mut cfg = gpu_execution_cpu_poseidon_config();
        assert!(configured_digest_execution(&cfg).is_err());
        cfg.execution_mode = FastpqExecutionMode::Cpu;
        cfg.poseidon_mode = FastpqPoseidonMode::Gpu;
        assert!(configured_digest_execution(&cfg).is_err());
    }
}
fn statements_for_job(
    job: &FastpqWitnessJob,
) -> fastpq_prover::Result<Vec<(Hash, FastpqPublicTransferStatementV1)>> {
    let invalid = |details: &str| fastpq_prover::Error::InvalidTraceShape {
        details: details.to_owned(),
    };
    if job.witness.fastpq_transcripts.is_empty() {
        return if job.witness.fastpq_batches.is_empty() {
            Ok(Vec::new())
        } else {
            Err(invalid(
                "FASTPQ proving requires original finalized transcript bundles",
            ))
        };
    }
    let public_inputs = job
        .context
        .public_inputs
        .ok_or_else(|| invalid("FASTPQ source public inputs are missing"))?;
    let tx_set_hash = job
        .context
        .tx_set_hash
        .filter(|hash| *hash != [0; 32])
        .ok_or_else(|| invalid("FASTPQ source transaction-set commitment is missing"))?;
    let verification = VerificationLimits::default();
    let proving = ProvingLimits::default();
    job.witness
        .fastpq_transcripts
        .iter()
        .map(|bundle| {
            let mut inputs = public_inputs.with_tx_set_hash(tx_set_hash);
            inputs.dsid = job
                .context
                .entry_dataspaces
                .get(&bundle.entry_hash)
                .copied()
                .unwrap_or(inputs.dsid);
            let (statement, witnesses) = quantity_statement_from_finalized_transcripts(
                inputs,
                &bundle.transcripts,
                verification.public_statement,
                proving.private_smt,
            )?
            .into_parts();
            drop(witnesses);
            Ok((bundle.entry_hash, statement))
        })
        .collect()
}
