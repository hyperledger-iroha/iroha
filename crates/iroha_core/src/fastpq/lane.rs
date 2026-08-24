//! FASTPQ prover lane: converts execution witnesses into transition batches and
//! drives the Stage 6 prover in the background.
use crate::{
    fastpq::{
        ENTRY_HASH_METADATA_KEY, FASTPQ_CANONICAL_PARAMETER_SET, FastpqWitnessContext,
        TranscriptBatchError, batches_from_bundles, batches_from_exec_witness,
    },
    kura::{FastpqProofEnqueueResult, FastpqProofSnapshot, Kura},
};
#[cfg(feature = "fastpq-gpu")]
use fastpq_prover::Planner;
use fastpq_prover::{
    ExecutionMode as ProverExecutionMode, MetalOverrides,
    PoseidonExecutionMode as ProverPoseidonMode, Prover, TransitionBatch, apply_metal_overrides,
    set_metal_queue_policy,
};
use iroha_config::parameters::actual::{Fastpq, FastpqExecutionMode, FastpqPoseidonMode};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::{BlockHeader, consensus::ExecWitness};
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
    let mode = map_execution_mode(cfg.execution_mode);
    let poseidon_mode = map_poseidon_mode(cfg.poseidon_mode);
    let (mode, poseidon_mode) = preflight_prover_modes(cfg, mode, poseidon_mode)?;
    match Prover::canonical_with_modes(FASTPQ_CANONICAL_PARAMETER_SET, mode, poseidon_mode) {
        Ok(prover) => Some(Arc::new(RealProofEngine { prover })),
        Err(err) => {
            warn!(?err, "fastpq lane: failed to construct canonical prover");
            None
        }
    }
}
#[cfg(not(feature = "fastpq-gpu"))]
fn preflight_prover_modes(
    _cfg: &Fastpq,
    mode: ProverExecutionMode,
    poseidon_mode: ProverPoseidonMode,
) -> Option<(ProverExecutionMode, ProverPoseidonMode)> {
    if matches!(mode, ProverExecutionMode::Gpu) || matches!(poseidon_mode, ProverPoseidonMode::Gpu)
    {
        warn!(
            "fastpq lane: GPU execution requested but GPU support is not compiled; lane disabled"
        );
        return None;
    }
    Some((mode, poseidon_mode))
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
        preflight_execution_gpu_backend,
        fastpq_prover::preflight_poseidon_gpu_backend,
        fastpq_prover::preflight_bn254_poseidon_word_batches,
    )
}
#[cfg(feature = "fastpq-gpu")]
fn preflight_execution_gpu_backend() -> bool {
    let Some(params) = Prover::canonical_parameter_sets()
        .iter()
        .find(|params| params.name == FASTPQ_CANONICAL_PARAMETER_SET)
    else {
        warn!(
            parameter = FASTPQ_CANONICAL_PARAMETER_SET,
            "fastpq lane: canonical parameters unavailable during GPU preflight"
        );
        return false;
    };
    let planner = Planner::new(params);
    let trace_log = params.trace_log_size.min(4);
    let trace_len = 1usize << trace_log;
    let mut gpu_coefficients = vec![
        (0..trace_len)
            .map(|index| u64::try_from(index).expect("preflight index fits u64") + 1)
            .collect::<Vec<_>>(),
    ];
    let mut cpu_coefficients = gpu_coefficients.clone();
    planner.ifft_columns(&mut cpu_coefficients);
    // The pending APIs report dispatch failure instead of using the planner's
    // ordinary CPU fallback, which makes them suitable for a fail-closed probe.
    let Some(ifft) = planner.ifft_gpu_pending(&mut gpu_coefficients) else {
        return false;
    };
    if ifft.wait().is_err() || gpu_coefficients != cpu_coefficients {
        return false;
    }

    let cpu_lde = planner.lde_columns(&cpu_coefficients);
    let Some(lde) = planner.lde_gpu_pending(&gpu_coefficients) else {
        return false;
    };
    matches!(lde.wait(), Ok(Some(gpu_lde)) if gpu_lde == cpu_lde)
}
#[cfg(feature = "fastpq-gpu")]
fn preflight_prover_modes_with_preflights(
    cfg: &Fastpq,
    mode: ProverExecutionMode,
    poseidon_mode: ProverPoseidonMode,
    execution_preflight: impl FnOnce() -> bool,
    poseidon_preflight: impl FnOnce() -> bool,
    digest_preflight: impl FnOnce() -> bool,
) -> Option<(ProverExecutionMode, ProverPoseidonMode)> {
    preflight_digest_acceleration(cfg, digest_preflight);
    if matches!(mode, ProverExecutionMode::Gpu) {
        let started_at = Instant::now();
        let execution_ok = execution_preflight();
        info!(
            ok = execution_ok,
            elapsed_ms = started_at.elapsed().as_millis(),
            "fastpq lane: FFT/LDE GPU preflight completed"
        );
        if !execution_ok {
            warn!("fastpq lane: GPU execution backend failed preflight; lane disabled");
            return None;
        }
    }
    if matches!(poseidon_mode, ProverPoseidonMode::Gpu) {
        let started_at = Instant::now();
        let poseidon_ok = poseidon_preflight();
        info!(
            ok = poseidon_ok,
            elapsed_ms = started_at.elapsed().as_millis(),
            "fastpq lane: Poseidon GPU preflight completed"
        );
        if !poseidon_ok {
            warn!("fastpq lane: GPU Poseidon backend failed preflight; lane disabled");
            return None;
        }
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
        debug_fused: cfg.metal_debug_fused,
    }
}
fn map_execution_mode(mode: FastpqExecutionMode) -> ProverExecutionMode {
    match mode {
        FastpqExecutionMode::Cpu => ProverExecutionMode::Cpu,
        FastpqExecutionMode::Gpu => ProverExecutionMode::Gpu,
    }
}
fn map_poseidon_mode(mode: FastpqPoseidonMode) -> ProverPoseidonMode {
    match mode {
        FastpqPoseidonMode::Cpu => ProverPoseidonMode::Cpu,
        FastpqPoseidonMode::Gpu => ProverPoseidonMode::Gpu,
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
        if shutdown_requested(shutdown, external_shutdown) {
            break;
        }
        let entry_hash = entry_hash_for_batch(idx, &job.witness, &batch);
        let entry_hash_hex = entry_hash
            .map(|hash| hex::encode(hash.as_ref()))
            .unwrap_or_else(|| "unknown".to_string());
        transition_count = transition_count.saturating_add(batch.transitions.len());
        let started = Instant::now();
        let proof_result = engine.prove(&batch);
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
                    if let Some((entry_hash, batch_index)) = entry_hash.and_then(|entry_hash| {
                        let batch_index = u32::try_from(idx).ok()?;
                        Some((entry_hash, batch_index))
                    }) {
                        let snapshot = FastpqProofSnapshot::compact_from_batch(
                            job.height,
                            job.block_hash,
                            entry_hash,
                            batch_index,
                            &batch,
                            output.trace_commitment,
                            output.proof_digest,
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
fn shutdown_requested(
    shutdown: &ShutdownSignal,
    external_shutdown: Option<&ShutdownSignal>,
) -> bool {
    shutdown.is_sent() || external_shutdown.is_some_and(ShutdownSignal::is_sent)
}
fn entry_hash_for_batch(
    idx: usize,
    witness: &ExecWitness,
    batch: &fastpq_prover::TransitionBatch,
) -> Option<Hash> {
    let bundle_entry_hash = witness
        .fastpq_transcripts
        .get(idx)
        .map(|bundle| bundle.entry_hash)?;
    let bytes = batch.metadata.get(ENTRY_HASH_METADATA_KEY)?;
    let digest: [u8; 32] = bytes.as_slice().try_into().ok()?;
    let metadata_entry_hash = Hash::prehashed(digest);
    if bundle_entry_hash != metadata_entry_hash {
        return None;
    }
    Some(metadata_entry_hash)
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
        FastpqPublicInputsTemplate, authority_digest, batches_from_bundles, transition_batch_to_dto,
    };
    use iroha_data_model::domain::DomainId;
    use iroha_data_model::fastpq::{
        TransferDeltaTranscript, TransferTranscript, TransferTranscriptBundle,
    };
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
            metal_debug_fused: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_FUSED,
        }
    }
    #[tokio::test]
    async fn lane_processes_transcripts_with_mock_engine() {
        use tokio::time::{Instant, sleep};
        let _registry_lock = LANE_REGISTRY_TEST_LOCK.lock().await;
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
            metal_debug_fused: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_FUSED,
        };
        let (handle, task) = {
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
            context: FastpqWitnessContext {
                public_inputs: Some(template),
                tx_set_hash: Some(tx_set_hash),
                entry_dataspaces: BTreeMap::new(),
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
        let digest_lock = super::super::DIGEST_ACCELERATION_TEST_LOCK
            .lock()
            .expect("digest acceleration test lock poisoned");
        let previous = crate::fastpq::poseidon_digest_acceleration_enabled();
        crate::fastpq::set_poseidon_digest_acceleration_enabled(true);

        let (_handle, task) =
            start_with_builder(None, None, None, || None).expect("lane generation registers");
        assert!(
            !crate::fastpq::poseidon_digest_acceleration_enabled(),
            "a new lane must stay on the CPU digest path until its one preflight succeeds"
        );

        crate::fastpq::set_poseidon_digest_acceleration_enabled(previous);
        drop(digest_lock);
        task.await.expect("failed worker joins cleanly");
    }
    #[tokio::test]
    async fn failed_backend_initialisation_allows_lane_retry() {
        use tokio::time::{Instant, sleep};
        let _registry_lock = LANE_REGISTRY_TEST_LOCK.lock().await;
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
        let external_shutdown = ShutdownSignal::new();
        let started = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));
        let worker_started = Arc::clone(&started);
        let worker_release = Arc::clone(&release);
        let (_handle, task) =
            start_with_builder(None, None, Some(external_shutdown.clone()), move || {
                worker_started.wait();
                worker_release.wait();
                None
            })
            .expect("lane registers");
        started.wait();
        external_shutdown.send();
        tokio::task::yield_now().await;
        assert!(
            lock_global_lane().current.is_some(),
            "retiring generation must remain registered while blocking setup is alive"
        );
        assert!(!task.is_finished());

        release.wait();
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
        let external_shutdown = ShutdownSignal::new();
        let started = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));
        let worker_started = Arc::clone(&started);
        let worker_release = Arc::clone(&release);
        let (_handle, task) =
            start_with_builder(None, None, Some(external_shutdown.clone()), move || {
                worker_started.wait();
                worker_release.wait();
                None
            })
            .expect("lane registers");
        started.wait();
        external_shutdown.send();
        task.abort();
        let join_error = task.await.expect_err("aborted worker reports cancellation");
        assert!(join_error.is_cancelled());
        assert!(
            lock_global_lane().current.is_some(),
            "detached blocking setup must retain its generation lease"
        );

        release.wait();
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
    fn job_context_rebinds_prebuilt_non_root_public_inputs() {
        let bundle = sample_bundle();
        let mut batches = sample_batches(&bundle);
        batches[0].public_inputs.dsid = [0xA1; 16];
        batches[0].public_inputs.slot = 1;
        batches[0].public_inputs.perm_root = [0xA2; 32];
        batches[0].public_inputs.tx_set_hash = [0xA3; 32];
        let template = FastpqPublicInputsTemplate {
            dsid: [0xB1; 16],
            slot: 23,
            old_root: [0xB2; 32],
            new_root: [0xB3; 32],
            perm_root: [0xB4; 32],
        };
        let tx_set_hash = [0xB5; 32];
        let entry_dsid = [0xB6; 16];
        let entry_hash = bundle.entry_hash;
        let job = FastpqWitnessJob {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAA; 32])),
            height: 42,
            view: 7,
            witness: ExecWitness {
                fastpq_transcripts: vec![bundle],
                fastpq_batches: batches.iter().map(transition_batch_to_dto).collect(),
                ..ExecWitness::default()
            },
            context: FastpqWitnessContext {
                public_inputs: Some(template),
                tx_set_hash: Some(tx_set_hash),
                entry_dataspaces: BTreeMap::from([(entry_hash, entry_dsid)]),
            },
        };

        let rebound = batches_for_job(&job).expect("prebuilt batch binds to finalized context");
        assert_eq!(rebound[0].public_inputs.dsid, entry_dsid);
        assert_eq!(rebound[0].public_inputs.slot, template.slot);
        assert_eq!(rebound[0].public_inputs.perm_root, template.perm_root);
        assert_eq!(rebound[0].public_inputs.tx_set_hash, tx_set_hash);
        assert_eq!(
            rebound[0].public_inputs.old_root, batches[0].public_inputs.old_root,
            "transfer SMT roots remain transcript-bound"
        );
        assert_eq!(
            rebound[0].public_inputs.new_root, batches[0].public_inputs.new_root,
            "transfer SMT roots remain transcript-bound"
        );
    }
    #[test]
    fn entry_hash_for_batch_accepts_matching_bundle_and_metadata() {
        let bundle = sample_bundle();
        let batches = sample_batches(&bundle);
        let witness = ExecWitness {
            fastpq_transcripts: vec![bundle.clone()],
            ..ExecWitness::default()
        };

        assert_eq!(
            entry_hash_for_batch(0, &witness, &batches[0]),
            Some(bundle.entry_hash)
        );
    }
    #[test]
    fn entry_hash_for_batch_rejects_conflicting_bundle_and_metadata() {
        let bundle = sample_bundle();
        let mut batches = sample_batches(&bundle);
        batches[0].metadata.insert(
            ENTRY_HASH_METADATA_KEY.into(),
            Hash::prehashed([0x99; 32]).as_ref().to_vec(),
        );
        let witness = ExecWitness {
            fastpq_transcripts: vec![bundle],
            ..ExecWitness::default()
        };

        assert_eq!(entry_hash_for_batch(0, &witness, &batches[0]), None);
    }
    #[test]
    fn entry_hash_for_batch_rejects_missing_metadata_even_with_bundle() {
        let bundle = sample_bundle();
        let mut batches = sample_batches(&bundle);
        batches[0].metadata.remove(ENTRY_HASH_METADATA_KEY);
        let witness = ExecWitness {
            fastpq_transcripts: vec![bundle],
            ..ExecWitness::default()
        };

        assert_eq!(entry_hash_for_batch(0, &witness, &batches[0]), None);
    }
    #[test]
    fn entry_hash_for_batch_rejects_malformed_metadata_even_with_bundle() {
        let bundle = sample_bundle();
        let mut batches = sample_batches(&bundle);
        batches[0]
            .metadata
            .insert(ENTRY_HASH_METADATA_KEY.into(), vec![0x11; 31]);
        let witness = ExecWitness {
            fastpq_transcripts: vec![bundle],
            ..ExecWitness::default()
        };

        assert_eq!(entry_hash_for_batch(0, &witness, &batches[0]), None);
    }
    #[test]
    fn entry_hash_for_batch_rejects_proof_only_metadata_identity() {
        let bundle = sample_bundle();
        let batches = sample_batches(&bundle);
        let witness = ExecWitness::default();

        assert_eq!(entry_hash_for_batch(0, &witness, &batches[0]), None);
    }
    #[test]
    #[cfg(feature = "fastpq-gpu")]
    fn prover_poseidon_preflight_failure_disables_explicit_gpu_lane() {
        let _digest_lock = super::super::DIGEST_ACCELERATION_TEST_LOCK
            .lock()
            .expect("digest acceleration test lock poisoned");
        let previous = crate::fastpq::poseidon_digest_acceleration_enabled();
        crate::fastpq::set_poseidon_digest_acceleration_enabled(false);
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
            metal_max_in_flight: None,
            metal_threadgroup_width: None,
            metal_trace: iroha_config::parameters::defaults::zk::fastpq::METAL_TRACE,
            metal_debug_enum: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_ENUM,
            metal_debug_fused: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_FUSED,
        };
        let digest_preflight_calls = std::cell::Cell::new(0usize);
        let preflight = preflight_prover_modes_with_preflights(
            &cfg,
            ProverExecutionMode::Gpu,
            ProverPoseidonMode::Gpu,
            || true,
            || false,
            || {
                digest_preflight_calls.set(digest_preflight_calls.get() + 1);
                true
            },
        );
        assert!(
            preflight.is_none(),
            "explicit GPU preflight must fail closed"
        );
        assert!(
            crate::fastpq::poseidon_digest_acceleration_enabled(),
            "BN254 digest acceleration records its own successful preflight before lane disable"
        );
        assert_eq!(
            digest_preflight_calls.get(),
            1,
            "one lane initialisation must run the digest hardware preflight exactly once"
        );
        crate::fastpq::set_poseidon_digest_acceleration_enabled(previous);
    }
    #[test]
    #[cfg(feature = "fastpq-gpu")]
    fn execution_gpu_preflight_failure_disables_lane_with_cpu_poseidon() {
        let _digest_lock = super::super::DIGEST_ACCELERATION_TEST_LOCK
            .lock()
            .expect("digest acceleration test lock poisoned");
        let previous = crate::fastpq::poseidon_digest_acceleration_enabled();
        let cfg = gpu_execution_cpu_poseidon_config();
        let poseidon_preflight_called = std::cell::Cell::new(false);

        let preflight = preflight_prover_modes_with_preflights(
            &cfg,
            ProverExecutionMode::Gpu,
            ProverPoseidonMode::Cpu,
            || false,
            || {
                poseidon_preflight_called.set(true);
                true
            },
            || panic!("CPU Poseidon must not preflight digest acceleration"),
        );

        assert!(
            preflight.is_none(),
            "forced GPU execution must fail closed when FFT/LDE preflight fails"
        );
        assert!(
            !poseidon_preflight_called.get(),
            "CPU Poseidon must not mask or replace the execution backend preflight"
        );
        crate::fastpq::set_poseidon_digest_acceleration_enabled(previous);
    }
    #[test]
    #[cfg(not(feature = "fastpq-gpu"))]
    fn forced_gpu_execution_without_gpu_feature_disables_lane() {
        let cfg = gpu_execution_cpu_poseidon_config();

        assert!(
            preflight_prover_modes(&cfg, ProverExecutionMode::Gpu, ProverPoseidonMode::Cpu,)
                .is_none(),
            "forced GPU execution must fail closed when GPU support is not compiled"
        );
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
    struct ShutdownDuringProofEngine {
        shutdown: ShutdownSignal,
    }
    impl FastpqProofEngine for ShutdownDuringProofEngine {
        fn prove(
            &self,
            _batch: &fastpq_prover::TransitionBatch,
        ) -> fastpq_prover::Result<FastpqProofOutput> {
            self.shutdown.send();
            let proof_bytes = b"proof-completed-after-shutdown".to_vec();
            Ok(FastpqProofOutput {
                proof_digest: Hash::new(&proof_bytes),
                trace_commitment: Hash::new(b"shutdown-trace-commitment"),
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
        }
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
        assert_eq!(
            entry_hash_for_batch(
                0,
                &ExecWitness {
                    fastpq_transcripts: vec![bundle.clone()],
                    ..ExecWitness::default()
                },
                &batches[0]
            ),
            Some(bundle.entry_hash),
            "fixture must carry a persistable entry identity"
        );
        let template = FastpqPublicInputsTemplate {
            dsid: [0; 16],
            slot: 0,
            old_root: [0; 32],
            new_root: [0; 32],
            perm_root: [0; 32],
        };
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
                tx_set_hash: Some([0; 32]),
                entry_dataspaces: BTreeMap::new(),
            },
        };
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
    let mut batches = match batches_from_exec_witness(&job.witness) {
        Ok(batches) => batches,
        Err(TranscriptBatchError::MissingFastpqBatches) => Vec::new(),
        Err(err) => return Err(err),
    };
    if batches.is_empty() && !job.witness.fastpq_transcripts.is_empty() {
        let Some(public_inputs) = job.context.public_inputs else {
            return Err(TranscriptBatchError::MissingFastpqBatches);
        };
        let Some(tx_set_hash) = job.context.tx_set_hash else {
            return Err(TranscriptBatchError::MissingFastpqBatches);
        };
        batches = batches_from_bundles(
            FASTPQ_CANONICAL_PARAMETER_SET,
            public_inputs,
            tx_set_hash,
            job.witness.fastpq_transcripts.iter(),
        )?;
    }
    if job.witness.fastpq_transcripts.is_empty() {
        return Ok(batches);
    }
    let Some(public_inputs) = job.context.public_inputs else {
        return Err(TranscriptBatchError::MissingFastpqBatches);
    };
    let Some(tx_set_hash) = job.context.tx_set_hash else {
        return Err(TranscriptBatchError::MissingFastpqBatches);
    };
    for (bundle, batch) in job
        .witness
        .fastpq_transcripts
        .iter()
        .zip(batches.iter_mut())
    {
        batch.public_inputs.dsid = job
            .context
            .entry_dataspaces
            .get(&bundle.entry_hash)
            .copied()
            .unwrap_or(public_inputs.dsid);
        batch.public_inputs.slot = public_inputs.slot;
        batch.public_inputs.perm_root = public_inputs.perm_root;
        batch.public_inputs.tx_set_hash = tx_set_hash;
    }
    Ok(batches)
}
