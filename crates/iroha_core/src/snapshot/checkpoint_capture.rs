//! Original checkpoint images retained from finalized State publication.
//!
//! The canonical image is captured before visibility from the finalized frozen overlay.
//! Its original publication owner retains those bytes while a bounded, nonblocking handoff
//! requests durable persistence outside every State/World writer and commit fence. Only the
//! writer's acknowledgement permits visibility. One queued job and one writer-owned job
//! bound work; local queue or I/O refusal retries the same original without reexecution.
//! Startup replay explicitly pumps this same consumer using its original certified overlay.
//! Private pending artifacts are promoted only after original visibility or strict authenticated
//! restart reconciliation; startup promotion never runs retention or rolling maintenance.

use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    },
};

use super::{CapturedStateSnapshot, SnapshotCaptureError};
use crate::state::{StateBlock, WorldReadOnly};

pub(crate) mod encoding;

/// Bounded local durability observations retain the same frozen State for periodic retry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CheckpointDeferral {
    /// Startup has armed the writer, but its successful-recovery handshake is pending.
    #[error("checkpoint at height {height} awaits its writer startup")]
    WorkerStarting {
        /// Exact original checkpoint height.
        height: u64,
    },
    /// The bounded writer queue has no slot for this original image yet.
    #[error("checkpoint at height {height} awaits bounded writer capacity")]
    QueueFull {
        /// Exact original checkpoint height.
        height: u64,
    },
    /// The configured writer is no longer available; the original image remains held.
    #[error("checkpoint at height {height} has no available writer")]
    WorkerUnavailable {
        /// Exact original checkpoint height.
        height: u64,
    },
    /// The original image is queued or being durably written outside publication fences.
    #[error("checkpoint at height {height} awaits its durable acknowledgement")]
    DurabilityPending {
        /// Exact original checkpoint height.
        height: u64,
    },
    /// Local persistence refused; its exact image can be retried after repair.
    #[error("checkpoint at height {height} persistence refused: {kind}")]
    Persistence {
        /// Exact original checkpoint height.
        height: u64,
        /// Original I/O category, or invalid data for a failed authenticated artifact check.
        kind: std::io::ErrorKind,
    },
}

#[derive(Clone)]
struct Sink {
    sender: mpsc::SyncSender<Arc<CheckpointJob>>,
    ready: Arc<AtomicU8>,
    max_payload_bytes: NonZeroUsize,
    active: bool,
}

/// Local optional consumer of the original publisher's checkpoint images.
#[derive(Default)]
pub(crate) struct CapturePublisher(parking_lot::Mutex<Option<Sink>>);

impl CapturePublisher {
    /// Arm the bounded consumer before certified replay or live drivers start.
    pub(super) fn arm(
        &self,
        max_payload_bytes: NonZeroUsize,
    ) -> mpsc::Receiver<Arc<CheckpointJob>> {
        let (sender, receiver) = mpsc::sync_channel(1);
        *self.0.lock() = Some(Sink {
            sender,
            ready: Arc::new(AtomicU8::new(0)),
            max_payload_bytes,
            active: false,
        });
        receiver
    }

    /// Enable forward captures synchronously before the live driver can publish.
    /// The consumer may still be awaiting the successful-startup signal.
    pub(crate) fn activate(&self) {
        if let Some(sink) = self.0.lock().as_mut() {
            sink.active = true;
        }
    }

    #[cfg(test)]
    pub(super) fn start(
        &self,
        max_payload_bytes: NonZeroUsize,
    ) -> mpsc::Receiver<Arc<CheckpointJob>> {
        let receiver = self.arm(max_payload_bytes);
        self.activate();
        self.writer_ready();
        receiver
    }

    /// Mark a consumer ready: either the synchronous certified replay owner or the live worker.
    pub(super) fn writer_ready(&self) {
        if let Some(sink) = self.0.lock().as_ref() {
            sink.ready.store(1, Ordering::Release);
        }
    }

    /// Stop future captures before the receiver is dropped at shutdown.
    pub(super) fn stop(&self) {
        if let Some(sink) = self.0.lock().take() {
            sink.ready.store(2, Ordering::Release);
        }
    }

    /// Prepare only a checkpoint height of the committed policy, retaining its exact bytes.
    pub(crate) fn prepare(
        &self,
        state: &StateBlock<'_>,
    ) -> Result<Option<PreparedCheckpoint>, SnapshotCaptureError> {
        let Some(sink) = self.0.lock().clone().filter(|sink| sink.active) else {
            return Ok(None);
        };
        let Some(tip) = *state.native_execution_tip.get() else {
            return Ok(None);
        };
        let Some(Some(parent)) = *state.native_execution_tip.original_undo() else {
            return Ok(None);
        };
        if parent.height().checked_add(1) != Some(tip.height()) {
            return Err(SnapshotCaptureError::Selection(
                "checkpoint publication has no exact native predecessor".into(),
            ));
        }
        let world = state.world();
        let Some(policy) = crate::runtime::checkpoint::committed_policy(
            world.runtime_selection(),
            WorldReadOnly::parameters(world),
        ) else {
            return Ok(None);
        };
        if !policy.is_checkpoint(parent.creation_time_ms(), tip.creation_time_ms()) {
            return Ok(None);
        }
        let snapshot =
            CapturedStateSnapshot::capture_finalized(state, sink.max_payload_bytes.get())?;
        Ok(Some(PreparedCheckpoint {
            job: Arc::new(CheckpointJob {
                capture: CheckpointCapture {
                    snapshot,
                    height: tip.height(),
                    committed_ms: tip.creation_time_ms(),
                },
                phase: parking_lot::Mutex::new(CheckpointPhase::Captured),
                owner_alive: AtomicBool::new(true),
            }),
            sender: sink.sender,
            ready: sink.ready,
        }))
    }
}

/// One move-only original image; no live State view survives its preparation.
pub(super) struct CheckpointCapture {
    pub(super) snapshot: CapturedStateSnapshot,
    pub(super) height: u64,
    pub(super) committed_ms: u64,
}

/// Bounded ownership shared by the original publication and its one writer job.
/// The frozen image is never cloned or recaptured.
pub(super) struct CheckpointJob {
    pub(super) capture: CheckpointCapture,
    phase: parking_lot::Mutex<CheckpointPhase>,
    owner_alive: AtomicBool,
}

enum CheckpointPhase {
    Captured,
    Queued,
    Failed(Option<CheckpointWriteError>),
    /// Keep the exact permanent cause beside its original image after reporting recovery.
    Corrupt {
        _original: CheckpointWriteError,
        diagnostic: Option<String>,
    },
    Durable,
    Visible,
}

impl CheckpointJob {
    pub(super) fn needs_write(&self) -> bool {
        matches!(
            &*self.phase.lock(),
            CheckpointPhase::Queued | CheckpointPhase::Failed(None)
        )
    }

    pub(super) fn acknowledge(&self, result: Result<(), CheckpointWriteError>) {
        *self.phase.lock() = match result {
            Ok(()) => CheckpointPhase::Durable,
            Err(original) if !original.is_retryable() => {
                let diagnostic = Some(original.to_string());
                CheckpointPhase::Corrupt {
                    _original: original,
                    diagnostic,
                }
            }
            Err(error) => CheckpointPhase::Failed(Some(error)),
        };
    }

    /// Export one original writer failure, leaving this exact image retryable.
    pub(super) fn take_refusal(
        &self,
    ) -> Option<crate::state::storage_transactions::TransactionsBlockError> {
        match &mut *self.phase.lock() {
            CheckpointPhase::Failed(original) => original
                .take()
                .map(|error| error.into_publication(self.capture.height)),
            CheckpointPhase::Corrupt { diagnostic, .. } => Some(diagnostic.take().map_or(
                crate::state::storage_transactions::TransactionsBlockError::PublicationRecoveryRequired,
                crate::state::storage_transactions::TransactionsBlockError::CheckpointIntegrity,
            )),
            _ => None,
        }
    }

    pub(super) fn visible(&self) -> bool {
        matches!(&*self.phase.lock(), CheckpointPhase::Visible)
    }

    pub(super) fn abandoned(&self) -> bool {
        !self.owner_alive.load(Ordering::Acquire) && !self.visible()
    }
}

/// Original checkpoint custody. Durability permits publication; visibility alone permits
/// retention. A local refusal leaves both this owner and its exact frozen bytes intact.
pub(crate) struct PreparedCheckpoint {
    job: Arc<CheckpointJob>,
    sender: mpsc::SyncSender<Arc<CheckpointJob>>,
    ready: Arc<AtomicU8>,
}

impl PreparedCheckpoint {
    /// Observe only bounded in-memory acknowledgement while the publication fence is held.
    pub(crate) fn durability(
        &self,
    ) -> Result<(), crate::state::storage_transactions::TransactionsBlockError> {
        let height = self.job.capture.height;
        let mut phase = self.job.phase.lock();
        match &mut *phase {
            CheckpointPhase::Durable | CheckpointPhase::Visible => Ok(()),
            CheckpointPhase::Failed(original) => Err(match original.take() {
                Some(original) => original.into_publication(height),
                None => {
                    crate::state::storage_transactions::TransactionsBlockError::CheckpointDeferred(
                        CheckpointDeferral::DurabilityPending { height },
                    )
                }
            }),
            CheckpointPhase::Corrupt { diagnostic, .. } => Err(diagnostic.take().map_or(
                crate::state::storage_transactions::TransactionsBlockError::PublicationRecoveryRequired,
                crate::state::storage_transactions::TransactionsBlockError::CheckpointIntegrity,
            )),
            _ if self.ready.load(Ordering::Acquire) == 0 => Err(
                crate::state::storage_transactions::TransactionsBlockError::CheckpointDeferred(
                    CheckpointDeferral::WorkerStarting { height },
                ),
            ),
            _ if self.ready.load(Ordering::Acquire) == 2 => Err(
                crate::state::storage_transactions::TransactionsBlockError::CheckpointDeferred(
                    CheckpointDeferral::WorkerUnavailable { height },
                ),
            ),
            _ => Err(
                crate::state::storage_transactions::TransactionsBlockError::CheckpointDeferred(
                    CheckpointDeferral::DurabilityPending { height },
                ),
            ),
        }
    }

    /// Queue after every physical writer and commit fence has been released. Never wait for
    /// queue space or disk I/O, and never release the original image on a local refusal.
    pub(crate) fn request(&self) -> Result<(), CheckpointDeferral> {
        let height = self.job.capture.height;
        match self.ready.load(Ordering::Acquire) {
            0 => return Err(CheckpointDeferral::WorkerStarting { height }),
            2 => return Err(CheckpointDeferral::WorkerUnavailable { height }),
            _ => {}
        }
        let mut phase = self.job.phase.lock();
        match &*phase {
            CheckpointPhase::Captured => {}
            // The caller already received this attempt's original refusal. Do not replace
            // it with another observation or duplicate a writer-owned queue entry.
            CheckpointPhase::Failed(_) | CheckpointPhase::Corrupt { .. } => return Ok(()),
            CheckpointPhase::Queued => {
                return Err(CheckpointDeferral::DurabilityPending { height });
            }
            CheckpointPhase::Durable | CheckpointPhase::Visible => return Ok(()),
        }
        match self.sender.try_send(Arc::clone(&self.job)) {
            Ok(()) => {
                *phase = CheckpointPhase::Queued;
                Err(CheckpointDeferral::DurabilityPending { height })
            }
            Err(mpsc::TrySendError::Full(_)) => Err(CheckpointDeferral::QueueFull { height }),
            Err(mpsc::TrySendError::Disconnected(_)) => {
                Err(CheckpointDeferral::WorkerUnavailable { height })
            }
        }
    }

    /// Mark the exact acknowledged image visible after publication releases every fence.
    pub(crate) fn publish(self) {
        let mut phase = self.job.phase.lock();
        assert!(
            matches!(&*phase, CheckpointPhase::Durable),
            "checkpoint visibility requires durable acknowledgement"
        );
        *phase = CheckpointPhase::Visible;
    }
}

impl Drop for PreparedCheckpoint {
    fn drop(&mut self) {
        self.job.owner_alive.store(false, Ordering::Release);
    }
}

#[cfg(test)]
pub(super) mod tests;

/// Original writer failures are non-wire and move exactly once through publication.
#[derive(Debug, thiserror::Error)]
pub(super) enum CheckpointWriteError {
    #[error(transparent)]
    Io(std::io::Error),
    #[error(transparent)]
    Execution(crate::execution_attempt::ExecutionDeferred),
    #[error(transparent)]
    Read(super::TryReadError),
    #[error(transparent)]
    Capture(super::SnapshotCaptureError),
}
impl CheckpointWriteError {
    /// Local resources or I/O can be retried; authenticated integrity violations cannot.
    fn is_retryable(&self) -> bool {
        use super::{SnapshotCaptureError as Capture, TryReadError as Read};
        match self {
            Self::Io(_) | Self::Execution(_) => true,
            Self::Capture(Capture::Read(_) | Capture::Changed(_) | Capture::BoundedEncoding(_)) => {
                true
            }
            Self::Capture(Capture::Runtime(original)) => matches!(
                original.as_ref(),
                crate::state::LaneLifecycleError::NposPolicy(
                    crate::execution_attempt::ExecutionAttemptError::Deferred(_)
                )
            ),
            Self::Capture(_) => false,
            Self::Read(original) => match original {
                Read::IO(_, _)
                | Read::StateParameterCell(_)
                | Read::StateParameterDecode(_)
                | Read::StateParameterIngress(_)
                | Read::StateWorldInitialization(_)
                | Read::StateAdmission(_)
                | Read::StateRead(_)
                | Read::StateVmInitialization(_)
                | Read::StateExecutionDeferred(_)
                | Read::StateNativeLaneCustody(_)
                | Read::StateNativeSchedule(_)
                | Read::StateBeaconSession(_)
                | Read::StateNativeAmx(_)
                | Read::SnapshotResourceLimit(_)
                | Read::PayloadAllocation(_)
                | Read::PayloadAllocatorFailure { .. }
                | Read::ReconciliationConfiguration(_)
                | Read::ZkConfigInstall(_)
                | Read::Kura(_) => true,
                Read::StateRootVerification(original) => matches!(
                    original,
                    crate::state::WorldStateVerificationError::Acquisition(_)
                        | crate::state::WorldStateVerificationError::KeyedAdmission(_)
                        | crate::state::WorldStateVerificationError::KeyedDeferred(_)
                ),
                Read::StateEvidence(_) | Read::StateSccp(_) => original.halts_restore(),
                _ => false,
            },
        }
    }

    pub(super) fn into_publication(
        self,
        height: u64,
    ) -> crate::state::storage_transactions::TransactionsBlockError {
        use crate::state::storage_transactions::TransactionsBlockError as Error;
        if !self.is_retryable() {
            return Error::CheckpointIntegrity(self.to_string());
        }
        match self {
            Self::Execution(original) => Error::ExecutionDeferred(original),
            Self::Read(original) => Error::CheckpointReadDeferred(original),
            Self::Capture(original) => original.into(),
            Self::Io(original) => Error::CheckpointDeferred(CheckpointDeferral::Persistence {
                height,
                kind: original.kind(),
            }),
        }
    }
    #[cfg(test)]
    pub(super) fn kind(&self) -> std::io::ErrorKind {
        match self {
            Self::Io(error) => error.kind(),
            _ => std::io::ErrorKind::InvalidData,
        }
    }
}
impl From<std::io::Error> for CheckpointWriteError {
    fn from(original: std::io::Error) -> Self {
        if original.kind() == std::io::ErrorKind::InvalidData {
            Self::Read(super::TryReadError::SnapshotGenerationInvalid {
                path: std::path::PathBuf::new(),
                reason: original.to_string(),
            })
        } else {
            Self::Io(original)
        }
    }
}
impl From<super::TryWriteError> for CheckpointWriteError {
    fn from(error: super::TryWriteError) -> Self {
        match error {
            super::TryWriteError::CommitEvidenceResourceDeferred { reason, .. } => {
                Self::Execution(reason)
            }
            super::TryWriteError::PayloadAllocation(original) => Self::Execution(original.into()),
            super::TryWriteError::PayloadAllocatorFailure { requested_bytes } => {
                Self::Read(super::TryReadError::PayloadAllocatorFailure { requested_bytes })
            }
            super::TryWriteError::RestartValidation(original) => original.into(),
            super::TryWriteError::Capture(original) => Self::Capture(original),
            super::TryWriteError::IO(original, _) => Self::Io(original),
            // Prepare permanent integrity diagnostics on the writer, before the
            // publication fence observes and moves the original refusal.
            other => Self::Read(super::TryReadError::SnapshotGenerationInvalid {
                path: std::path::PathBuf::new(),
                reason: other.to_string(),
            }),
        }
    }
}
impl From<super::TryReadError> for CheckpointWriteError {
    fn from(error: super::TryReadError) -> Self {
        match error {
            super::TryReadError::PayloadAllocation(original) => Self::Execution(original.into()),
            super::TryReadError::StateExecutionDeferred(original) => Self::Execution(original),
            super::TryReadError::StateVmInitialization(original) => {
                match crate::execution_attempt::ExecutionDeferred::try_from_vm_error(original) {
                    Ok(original) => Self::Execution(original),
                    Err(original) => {
                        Self::Read(super::TryReadError::StateVmInitialization(original))
                    }
                }
            }
            super::TryReadError::IO(original, _) => Self::Io(original),
            other => Self::Read(other),
        }
    }
}

impl From<super::SnapshotCaptureError> for CheckpointWriteError {
    fn from(original: super::SnapshotCaptureError) -> Self {
        Self::Capture(original)
    }
}
impl From<norito::json::Error> for CheckpointWriteError {
    fn from(original: norito::json::Error) -> Self {
        Self::Read(super::TryReadError::Serialization(original))
    }
}
impl From<std::str::Utf8Error> for CheckpointWriteError {
    fn from(original: std::str::Utf8Error) -> Self {
        Self::Read(super::TryReadError::SnapshotGenerationInvalid {
            path: std::path::PathBuf::new(),
            reason: original.to_string(),
        })
    }
}
impl From<String> for CheckpointWriteError {
    fn from(reason: String) -> Self {
        Self::Read(super::TryReadError::SnapshotGenerationInvalid {
            path: std::path::PathBuf::new(),
            reason,
        })
    }
}
impl From<crate::kura::Error> for CheckpointWriteError {
    fn from(original: crate::kura::Error) -> Self {
        super::TryWriteError::ExactKuraBoundary(original).into()
    }
}

impl From<super::SnapshotMerkleError> for CheckpointWriteError {
    fn from(original: super::SnapshotMerkleError) -> Self {
        super::merkle_err_to_try_read(original, std::path::Path::new("")).into()
    }
}
impl From<crate::state::WorldStateVerificationError> for CheckpointWriteError {
    fn from(original: crate::state::WorldStateVerificationError) -> Self {
        match original {
            crate::state::WorldStateVerificationError::KeyedDeferred(original) => {
                Self::Execution(original)
            }
            other => Self::Read(super::TryReadError::StateRootVerification(other)),
        }
    }
}
