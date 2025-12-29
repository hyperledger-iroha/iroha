//! ZK lane: capture IVM formal traces, verify in the background, and report.
//!
//! This module provides a lightweight, non-forking background worker that can
//! receive formal execution traces from IVM runs, verify constraints and
//! Merkle-authenticated logs, and report outcomes. It does not mutate WSV and
//! is intended purely for diagnostics/telemetry.
//!
//! Integration points:
//! - Overlay builder: after running IVM to collect queued ISIs, capture the
//!   formal trace and submit a job if ZK is enabled in config.
//! - Node startup: call `start()` once to initialize the worker.

#![deny(missing_docs)]

#[cfg(test)]
use iroha_crypto::HashOf;
use iroha_crypto::{Hash, streaming::TransportCapabilityResolutionSnapshot};
use ivm::zk::{Constraint, MemEvent, RegEvent, RegisterState, StepEntry};
use norito::streaming::CapabilityFlags;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
use std::{num::NonZeroU64, sync::Arc};
use tokio::sync::mpsc;

/// Task carrying a single IVM execution's formal trace and metadata.
#[derive(Clone)]
pub struct ZkTask {
    /// Transaction hash if available (zero when not applicable).
    pub tx_hash: Option<Hash>,
    /// Code hash of the executed bytecode.
    pub code_hash: [u8; 32],
    /// Full program bytes executed to produce this trace.
    pub program: Arc<Vec<u8>>,
    /// Optional block header associated with this trace (for warnings/events).
    /// If not provided, the ZK lane will emit a warning with a minimal header
    /// carrying height=1.
    pub header: Option<iroha_data_model::block::BlockHeader>,
    /// Expanded register trace.
    pub trace: Vec<RegisterState>,
    /// Logged constraints encountered during execution.
    pub constraints: Vec<Constraint>,
    /// Memory access log with Merkle proofs.
    pub mem_log: Vec<MemEvent>,
    /// Register access log with Merkle proofs.
    pub reg_log: Vec<RegEvent>,
    /// Per-step Merkle roots of registers and memory.
    pub step_log: Vec<StepEntry>,
    /// Transport capabilities negotiated for the session (if available).
    pub transport_capabilities: Option<TransportCapabilityResolutionSnapshot>,
    /// Negotiated feature flags advertised by the peer (if available).
    pub negotiated_capabilities: Option<CapabilityFlags>,
}

impl ZkTask {
    /// Compute a stable digest of the task contents for idempotence/logging.
    pub fn digest(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(self.code_hash);
        if let Some(tx) = &self.tx_hash {
            h.update(tx.as_ref());
        }
        h.update((self.program.len() as u64).to_le_bytes());
        if !self.program.is_empty() {
            let sample = if self.program.len() <= 64 {
                &self.program
            } else {
                &self.program[..64]
            };
            h.update(sample);
        }
        // Keep digest bounded and deterministic: include sizes and first/last PCs.
        let cycles = self.trace.len() as u64;
        h.update(cycles.to_le_bytes());
        let constraints = self.constraints.len() as u64;
        h.update(constraints.to_le_bytes());
        let mem = self.mem_log.len() as u64;
        h.update(mem.to_le_bytes());
        let regs = self.reg_log.len() as u64;
        h.update(regs.to_le_bytes());
        if let Some(first) = self.step_log.first() {
            h.update(first.pc.to_le_bytes());
            h.update(first.reg_root.as_ref().as_ref());
            h.update(first.mem_root.as_ref().as_ref());
        }
        if let Some(last) = self.step_log.last() {
            h.update(last.pc.to_le_bytes());
            h.update(last.reg_root.as_ref().as_ref());
            h.update(last.mem_root.as_ref().as_ref());
        }
        if let Some(caps) = &self.transport_capabilities {
            h.update(caps.hpke_suite.suite_id().to_le_bytes());
            h.update([u8::from(caps.use_datagram)]);
            h.update(caps.max_segment_datagram_size.to_le_bytes());
            h.update(caps.fec_feedback_interval_ms.to_le_bytes());
            h.update([caps.privacy_bucket_granularity as u8]);
        }
        if let Some(flags) = self.negotiated_capabilities {
            h.update(flags.bits().to_le_bytes());
        }
        h.finalize().into()
    }
}

/// Result of background verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZkOutcome {
    /// All constraints and Merkle paths verified.
    Verified,
    /// Verification failed (constraint or path mismatch).
    Rejected,
}

/// Handle for submitting ZK tasks to the background worker.
#[derive(Clone)]
pub struct ZkLaneHandle(mpsc::Sender<ZkTask>);

impl ZkLaneHandle {
    /// Submit a task; returns false if the lane is not running or channel is full.
    pub fn submit(&self, task: ZkTask) -> bool {
        self.0.try_send(task).is_ok()
    }
}

static GLOBAL_SENDER: OnceLock<ZkLaneHandle> = OnceLock::new();
static EVENTS: OnceLock<crate::EventsSender> = OnceLock::new();

fn process_batch(batch: &mut Vec<ZkTask>) {
    for job in batch.drain(..) {
        let dig = job.digest();
        let outcome =
            ivm::zk::verify_trace(&job.trace, &job.constraints, &job.mem_log, &job.reg_log)
                .map(|()| ZkOutcome::Verified)
                .unwrap_or(ZkOutcome::Rejected);
        #[cfg(feature = "zk-preverify")]
        if matches!(outcome, ZkOutcome::Verified) {
            if let Some(header) = &job.header {
                let artifact =
                    crate::zk::make_trace_digest_artifact(job.code_hash, job.tx_hash.as_ref(), dig);
                crate::zk::queue_trace_proof(header.height().get(), artifact);
                crate::zk::queue_trace_for_proving(
                    header.height().get(),
                    crate::zk::TraceForProving::from_task(&job, dig),
                );
            }
        }
        let transport_desc = job.transport_capabilities.as_ref().map(|caps| {
            format!(
                "suite={}, datagram={}, max_dgram={}, feedback_ms={}, privacy={:?}",
                caps.hpke_suite.suite_id(),
                caps.use_datagram,
                caps.max_segment_datagram_size,
                caps.fec_feedback_interval_ms,
                caps.privacy_bucket_granularity
            )
        });
        let negotiated_desc = job
            .negotiated_capabilities
            .map(|flags| format!("0x{:x}", flags.bits()));
        iroha_logger::info!(
            code_hash = %hex::encode(job.code_hash),
            tx_hash = job.tx_hash.as_ref().map(|hash| hex::encode(hash.as_ref())),
            transport = transport_desc.as_deref(),
            negotiated_capabilities = negotiated_desc.as_deref(),
            digest = %hex::encode(dig),
            outcome = ?outcome,
            cycles = job.trace.len(),
            constraints = job.constraints.len(),
            mem_events = job.mem_log.len(),
            reg_events = job.reg_log.len(),
            "zk_lane: verified formal trace"
        );
        if let Some(es) = EVENTS.get() {
            let header = job.header.unwrap_or_else(|| {
                iroha_data_model::block::BlockHeader::new(
                    NonZeroU64::new(1).expect("non-zero constant"),
                    None,
                    None,
                    None,
                    0,
                    0,
                )
            });
            let (kind, details) = match outcome {
                ZkOutcome::Verified => (
                    "zk_trace_verified",
                    format!(
                        "trace verified: code_hash={}, digest={}, transport={:?}, negotiated={:?}",
                        hex::encode(job.code_hash),
                        hex::encode(dig),
                        transport_desc,
                        negotiated_desc
                    ),
                ),
                ZkOutcome::Rejected => (
                    "zk_trace_rejected",
                    format!(
                        "trace rejected: code_hash={}, digest={}, transport={:?}, negotiated={:?}",
                        hex::encode(job.code_hash),
                        hex::encode(dig),
                        transport_desc,
                        negotiated_desc
                    ),
                ),
            };
            let warn = iroha_data_model::events::pipeline::PipelineWarning {
                header,
                kind: kind.to_string(),
                details,
            };
            let _ = es.send(iroha_data_model::events::EventBox::from(
                iroha_data_model::events::pipeline::PipelineEventBox::Warning(warn),
            ));
        }
    }
}

/// Try to submit a task through a globally registered lane, if present.
pub fn try_submit(task: ZkTask) -> bool {
    GLOBAL_SENDER
        .get()
        .is_some_and(|handle| handle.submit(task))
}

/// Register a global events sender to receive ZK verification warnings.
/// No-op if already registered.
pub fn register_events_sender(sender: crate::EventsSender) {
    let _ = EVENTS.set(sender);
}

/// Start the background ZK lane if Halo2 verification is enabled.
///
/// Returns an optional handle and a `tokio` task `JoinHandle` wrapped for
/// supervisor registration. If the lane is already running, returns the
/// existing handle and no new task.
pub fn start(
    cfg: &iroha_config::parameters::actual::Halo2,
) -> Option<(ZkLaneHandle, tokio::task::JoinHandle<()>)> {
    if !cfg.enabled {
        return None;
    }
    if let Some(existing) = GLOBAL_SENDER.get() {
        // Already started in this process.
        return Some((existing.clone(), tokio::spawn(async {})));
    }
    // Use a small bounded channel; config integration can be added if needed.
    let (tx, mut rx) = mpsc::channel::<ZkTask>(128);
    let handle = ZkLaneHandle(tx.clone());
    let _ = GLOBAL_SENDER.set(handle.clone());

    let max_batch = cfg.verifier_max_batch.max(1) as usize;
    let task = tokio::spawn(async move {
        let mut pending: Vec<ZkTask> = Vec::with_capacity(max_batch);
        loop {
            tokio::select! {
                biased;
                maybe = rx.recv() => {
                    if let Some(job) = maybe {
                        pending.push(job);
                        if pending.len() >= max_batch { process_batch(&mut pending); }
                    } else {
                        // Channel closed; drain and exit
                        if !pending.is_empty() { process_batch(&mut pending); }
                        break;
                    }
                }
                // Periodically drain small batches to avoid unbounded latency
                () = tokio::time::sleep(std::time::Duration::from_millis(5)) => {
                    if !pending.is_empty() { process_batch(&mut pending); }
                }
            }
        }
    });
    Some((handle, task))
}

#[cfg(test)]
mod tests {
    //! Minimal unit tests for `ZkLane` helpers.
    use super::*;

    #[test]
    fn digest_changes_with_roots_and_sizes() {
        fn mk(n: usize) -> ZkTask {
            let mut task = ZkTask {
                tx_hash: None,
                code_hash: [0xAB; 32],
                program: Arc::new(vec![0, 1, 2, 3]),
                header: None,
                trace: Vec::new(),
                constraints: Vec::new(),
                mem_log: Vec::new(),
                reg_log: Vec::new(),
                step_log: Vec::new(),
                transport_capabilities: None,
                negotiated_capabilities: None,
            };
            for i in 0..n {
                task.trace.push(RegisterState {
                    pc: i as u64,
                    gpr: [0; 256],
                    tags: [false; 256],
                });
            }
            task.step_log.push(StepEntry {
                pc: 0,
                reg_root: HashOf::from_untyped_unchecked(Hash::prehashed([1; 32])),
                mem_root: HashOf::from_untyped_unchecked(Hash::prehashed([2; 32])),
            });
            task.step_log.push(StepEntry {
                pc: n as u64,
                reg_root: HashOf::from_untyped_unchecked(Hash::prehashed([3; 32])),
                mem_root: HashOf::from_untyped_unchecked(Hash::prehashed([4; 32])),
            });
            task
        }
        let a = mk(3).digest();
        let b = mk(4).digest();
        assert_ne!(a, b);
    }

    #[cfg(feature = "zk-preverify")]
    #[test]
    fn process_batch_enqueues_digest_and_trace_jobs() {
        use iroha_data_model::block::BlockHeader;
        use std::num::NonZeroU64;

        crate::zk::reset_trace_proof_state_for_tests();
        crate::zk::reset_trace_proving_state_for_tests();

        let header = BlockHeader::new(
            NonZeroU64::new(42).expect("non-zero"),
            None,
            None,
            None,
            0,
            0,
        );
        let program = Arc::new(vec![0x13, 0x37, 0xC0, 0xDE]);
        let trace = vec![
            RegisterState {
                pc: 0,
                gpr: [0u64; 256],
                tags: [false; 256],
            },
            RegisterState {
                pc: 4,
                gpr: [0u64; 256],
                tags: [false; 256],
            },
        ];
        let task = ZkTask {
            tx_hash: None,
            code_hash: [0xDA; 32],
            program: Arc::clone(&program),
            header: Some(header.clone()),
            trace,
            constraints: Vec::new(),
            mem_log: Vec::new(),
            reg_log: Vec::new(),
            step_log: Vec::new(),
            transport_capabilities: None,
            negotiated_capabilities: None,
        };
        let expected_digest = task.digest();

        let mut batch = vec![task];
        process_batch(&mut batch);
        assert!(batch.is_empty(), "processing drains the batch");

        let proving_jobs = crate::zk::collect_traces_for_proving(header.height().get());
        assert_eq!(proving_jobs.len(), 1, "trace job enqueued for proving");

        let proofs = crate::zk::collect_trace_proofs_for_height(header.height().get());
        assert_eq!(proofs.len(), 1, "digest artifact recorded");
        let artifact = &proofs[0];
        assert_eq!(artifact.backend, "zk-trace/digest-v1");
        assert_eq!(artifact.proof, expected_digest.to_vec());
    }
}
