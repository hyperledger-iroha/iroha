//! FASTPQ prover lane: converts execution witnesses into transition batches and
//! drives the Stage 6 prover in the background.

use std::{
    sync::{Arc, OnceLock},
    time::Instant,
};

use fastpq_prover::{
    ExecutionMode as ProverExecutionMode, MetalOverrides,
    PoseidonExecutionMode as ProverPoseidonMode, Prover, apply_metal_overrides,
    set_metal_queue_policy,
};
use iroha_config::parameters::actual::{Fastpq, FastpqExecutionMode, FastpqPoseidonMode};
#[cfg(test)]
use iroha_crypto::Hash;
use iroha_crypto::HashOf;
use iroha_data_model::block::{BlockHeader, consensus::ExecWitness};
use iroha_logger::{debug, info, warn};
use tokio::sync::mpsc;

use crate::fastpq::{
    ENTRY_HASH_METADATA_KEY, FASTPQ_CANONICAL_PARAMETER_SET, batches_from_exec_witness,
};

/// Handle used to submit FASTPQ prover jobs.
#[derive(Clone)]
pub struct FastpqLaneHandle(mpsc::Sender<FastpqWitnessJob>);

impl FastpqLaneHandle {
    /// Submit a prover job to the lane.
    pub fn submit(&self, job: FastpqWitnessJob) -> bool {
        self.0.try_send(job).is_ok()
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
}

/// Trait abstracting over the FASTPQ prover backend so tests can inject mocks.
pub trait FastpqProofEngine: Send + Sync + 'static {
    /// Prove the supplied transition batch.
    ///
    /// # Errors
    /// Returns an error when the prover backend fails to generate a proof.
    fn prove(&self, batch: &fastpq_prover::TransitionBatch) -> fastpq_prover::Result<()>;
}

struct RealProofEngine {
    prover: Prover,
}

impl FastpqProofEngine for RealProofEngine {
    fn prove(&self, batch: &fastpq_prover::TransitionBatch) -> fastpq_prover::Result<()> {
        self.prover.prove(batch).map(|_| ())
    }
}

static GLOBAL_SENDER: OnceLock<FastpqLaneHandle> = OnceLock::new();

#[cfg(test)]
static TEST_ENGINE: OnceLock<Arc<dyn FastpqProofEngine>> = OnceLock::new();

/// Start the FASTPQ prover lane. Returns the handle and the spawned task when successful.
pub fn start(cfg: &Fastpq) -> Option<(FastpqLaneHandle, tokio::task::JoinHandle<()>)> {
    if let Some(existing) = GLOBAL_SENDER.get() {
        return Some((existing.clone(), tokio::spawn(async {})));
    }
    let engine = if let Some(engine) = build_engine(cfg) {
        engine
    } else {
        warn!("fastpq lane: failed to initialise prover backend; lane disabled");
        return None;
    };
    let (tx, mut rx) = mpsc::channel::<FastpqWitnessJob>(32);
    let handle = FastpqLaneHandle(tx.clone());
    if GLOBAL_SENDER.set(handle.clone()).is_err() {
        return Some((GLOBAL_SENDER.get().unwrap().clone(), tokio::spawn(async {})));
    }
    let parameter = Arc::<str>::from(FASTPQ_CANONICAL_PARAMETER_SET);
    let task = tokio::spawn(async move {
        while let Some(job) = rx.recv().await {
            let engine = Arc::clone(&engine);
            let parameter = Arc::clone(&parameter);
            if let Err(err) =
                tokio::task::spawn_blocking(move || process_job(&engine, &parameter, &job)).await
            {
                warn!(?err, "fastpq lane: prover task panicked");
            }
        }
    });
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
    match Prover::canonical_with_modes(FASTPQ_CANONICAL_PARAMETER_SET, mode, poseidon_mode) {
        Ok(prover) => Some(Arc::new(RealProofEngine { prover })),
        Err(err) => {
            warn!(?err, "fastpq lane: failed to construct canonical prover");
            None
        }
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

fn process_job(engine: &Arc<dyn FastpqProofEngine>, parameter: &Arc<str>, job: &FastpqWitnessJob) {
    if job.witness.fastpq_transcripts.is_empty() && job.witness.fastpq_batches.is_empty() {
        debug!(
            height = job.height,
            view = job.view,
            "fastpq lane: witness contains no transcripts"
        );
        return;
    }
    let batches = match batches_from_exec_witness(&job.witness, parameter.as_ref()) {
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
    for (idx, batch) in batches.into_iter().enumerate() {
        let entry_hash = entry_hash_hex(idx, &job.witness, &batch);
        let started = Instant::now();
        match engine.prove(&batch) {
            Ok(()) => {
                info!(
                    height = job.height,
                    view = job.view,
                    entry_hash,
                    transitions = batch.transitions.len(),
                    elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0,
                    "fastpq lane: generated proof"
                );
            }
            Err(err) => {
                warn!(
                    height = job.height,
                    view = job.view,
                    entry_hash,
                    ?err,
                    "fastpq lane: prover error"
                );
            }
        }
    }
}

fn entry_hash_hex(
    idx: usize,
    witness: &ExecWitness,
    batch: &fastpq_prover::TransitionBatch,
) -> String {
    if let Some(bundle) = witness.fastpq_transcripts.get(idx) {
        return hex::encode(bundle.entry_hash.as_ref());
    }
    batch
        .metadata
        .get(ENTRY_HASH_METADATA_KEY)
        .map_or_else(|| "unknown".to_string(), hex::encode)
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
    use iroha_data_model::fastpq::{
        TransferDeltaTranscript, TransferTranscript, TransferTranscriptBundle,
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{ALICE_ID, BOB_ID};

    use super::*;

    #[tokio::test]
    async fn lane_processes_transcripts_with_mock_engine() {
        use tokio::time::{Duration, Instant, sleep};

        let calls = Arc::new(std::sync::Mutex::new(0usize));
        install_test_engine(Arc::new(MockEngine {
            calls: Arc::clone(&calls),
        }));
        let cfg = Fastpq {
            execution_mode: FastpqExecutionMode::Cpu,
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
        let _ = start(&cfg);
        let witness = ExecWitness {
            reads: Vec::new(),
            writes: Vec::new(),
            fastpq_transcripts: vec![sample_bundle()],
            fastpq_batches: Vec::new(),
        };
        let job = FastpqWitnessJob {
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAA; 32])),
            height: 42,
            view: 7,
            witness,
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

    #[derive(Clone)]
    struct MockEngine {
        calls: Arc<std::sync::Mutex<usize>>,
    }

    impl FastpqProofEngine for MockEngine {
        fn prove(&self, batch: &fastpq_prover::TransitionBatch) -> fastpq_prover::Result<()> {
            *self.calls.lock().unwrap() += 1;
            let _ = &batch.parameter;
            Ok(())
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
                    asset_definition: "rose#wonderland".parse().unwrap(),
                    amount: Numeric::from(10u32),
                    from_balance_before: Numeric::from(100u32),
                    from_balance_after: Numeric::from(90u32),
                    to_balance_before: Numeric::from(5u32),
                    to_balance_after: Numeric::from(15u32),
                    from_merkle_proof: None,
                    to_merkle_proof: None,
                }],
                authority_digest: super::authority_digest(&ALICE_ID),
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
