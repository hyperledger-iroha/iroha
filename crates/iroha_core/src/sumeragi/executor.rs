//! The node's executor for the Sumeragi driver: executes blocks on the committed [`State`],
//! applies committed blocks and builds payloads (`specs/sumeragi.md` §4, §6.10, §12.2,
//! §12.3 O3/O4).
//!
//! State allows one writer at a time and an executed overlay borrows the state, so all work
//! runs on one dedicated thread that owns the state and keeps at most **one live overlay**
//! (the most recent execution). [`StateExecutor`] is the driver-facing handle: it forwards
//! each call to that thread and waits for the answer.
//!
//! - `execute` runs only on the applied tip (the core requests an execution once its parent's
//!   `CommitBlock` was emitted, and the driver applies in order); otherwise it answers `None`
//!   and the driver parks the request (never `Failed` for that).
//! - A local condition (storage, admission, a panic) is `Failed` and retried; everything else
//!   the validator rejects is `Invalid` — a deterministic verdict every honest node reaches.
//! - `prepare` pins the original matching overlay, capture and one certified staged frame until
//!   publication. A missing speculative overlay may execute again only before preparation.
//! - `commit` retains that owner on reversible durable-authorization refusal. Once the one-shot
//!   State apply begins, any failure requires recovery; the worker cannot re-execute or retry
//!   partially consumed publication. Successful repeated completion emits no duplicate events.

/// Move-only authority issued inside the original native execution worker.
/// No decoder, clone or public constructor can recreate this proof of origin.
pub(crate) struct NativeExecutionAuthorization {
    state: usize,
    tip: crate::state::native_execution_tip::NativeExecutionTipRecord,
    parent: Option<(Hash32, Hash32)>,
    telemetry_origin: CommitTelemetryOrigin,
}

/// Local observations of an authenticated execution; never protocol authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommitTelemetryOrigin {
    /// A newly committed execution emits its transition observations once.
    Forward,
    /// Startup re-execution restores gauges without recounting historical transitions.
    HistoricalReplay,
}
impl NativeExecutionAuthorization {
    /// The startup module can transfer only its own original signed-genesis execution.
    pub(super) fn from_genesis(original: super::startup::GenesisExecutionAuthorization) -> Self {
        let (state, tip, telemetry_origin) = original.into_parts();
        Self {
            state,
            tip,
            parent: None,
            telemetry_origin,
        }
    }

    /// Observe the local origin retained beside this exact execution authorization.
    pub(crate) fn telemetry_origin(&self) -> CommitTelemetryOrigin {
        self.telemetry_origin
    }

    /// Return fixed claims only for the exact State that owns the execution.
    pub(crate) fn for_state(
        &self,
        state: &State,
    ) -> Result<
        (
            crate::state::native_execution_tip::NativeExecutionTipRecord,
            Option<(Hash32, Hash32)>,
        ),
        String,
    > {
        if self.state != std::ptr::from_ref(state) as usize {
            return Err("native execution authorization belongs to another State".into());
        }
        Ok((self.tip, self.parent))
    }
}

use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, mpsc},
    thread::JoinHandle,
    time::Duration,
};

use iroha_data_model::{
    account::AccountId,
    block::{BlockHeader as IrohaHeader, CommitCertificate, SignedBlock},
    events::EventBox,
    parameter::system::ConsensusMode,
    transaction::TransactionEntrypoint,
};
use iroha_model_base::peer::PeerId;
use iroha_sumeragi::{
    api::{ApplicationControlContext, ControlWitnessContext, ExecOutcome},
    availability::{AvailabilityFrame, AvailableBody, PayloadBytes},
    message::{ApplicationControl, Qc},
    types::{AppliedConfig, ControlWitness, Hash32, PublicKey},
};

use super::{
    block_store::{StagedBlock, Staging},
    commitment::{
        ExecutionResultCommitment, encode_result_preimage, execution_result, result_of_preimage,
    },
    driver::traits::{Executor, PublicationError},
    lanes,
    network_topology::Topology,
    payload::{self, Assembly},
    schedule::{self, NativeExecutionInputs},
};
use crate::{
    EventsSender,
    block::{BlockValidationError, CommittedBlock, ValidBlock},
    query::native_context_archive::{
        NativeContextArchive, NativeContextArchiveError, PreparedNativeContext,
    },
    queue::Queue,
    state::{
        NativeLaneStateProof, NativeLaneStateProofError, State, StateBlock, StateReadOnly,
        WorldReadOnly,
    },
};

/// Payload bytes kept free for the block's non-transaction fields when selecting.
const PAYLOAD_OVERHEAD: usize = 64 * 1024;

/// What the executor thread needs.
#[derive(Clone)]
pub struct ExecutorContext {
    /// The node's state.
    pub state: Arc<State>,
    /// Mandatory original-pool archive of complete context values from each native execution.
    pub native_context_archive: Arc<NativeContextArchive>,
    /// The transaction queue (peeked by the builder, cleaned after apply); `None` until the
    /// node attaches it ([`StateExecutor::attach_queue`]), e.g. during startup replay.
    pub queue: Option<Arc<Queue>>,
    /// Hand-off of executed blocks to the block store.
    pub staging: Staging,
    /// Pipeline and state events of applied blocks.
    pub events: EventsSender,
    /// The genesis account.
    pub genesis_account: AccountId,
    /// The chain's consensus mode.
    pub consensus_mode: ConsensusMode,
    /// The applied tip: its height and core block hash.
    pub applied: (u64, Hash32),
    /// The driver's cryptography: the keys of each newly scheduled committee are admitted
    /// (their proofs of possession verified) as blocks commit.
    pub crypto: Option<Arc<super::crypto::BlsCrypto>>,
    /// The applied tip published to the node's lane instances (`specs/sumeragi_lanes.md` §3.2).
    pub applied_watch: Arc<crate::sumeragi::lanes::global::AppliedWatch>,
    /// The node's committed lane blocks, which merged global blocks execute (§4.3).
    pub lane_blocks: Arc<dyn crate::sumeragi::lanes::merge::LaneBlockSource>,
}

/// Configured SoraFS archives captured from the exact committed State before apply completes.
///
/// The executor is their single capture producer. A failed capture keeps the committed decision
/// pending, so the driver cannot acknowledge it or advance to another height before recovery.
#[derive(Clone, Default)]
pub struct FinalizedArchives {
    /// Finalized provider assignments and completions.
    pub provider_ingest:
        Option<Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>>,
    /// Finalized provider reputation state.
    pub reputation: Option<Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>>,
}

impl FinalizedArchives {
    fn capture(&self, view: &impl StateReadOnly) -> Result<(), String> {
        if let Some(archive) = &self.provider_ingest {
            archive
                .capture_certified_view(view, view.kura())
                .map_err(|error| format!("provider-ingest archive capture failed: {error}"))?;
        }
        if let Some(archive) = &self.reputation {
            archive
                .capture_certified_view(view, view.kura())
                .map_err(|error| format!("reputation archive capture failed: {error}"))?;
        }
        Ok(())
    }
}

/// One State publication awaiting durable archive capture and its remaining notifications.
struct PendingCommit {
    telemetry_origin: CommitTelemetryOrigin,
    native_contexts: PreparedNativeContext,
    header: iroha_sumeragi::message::BlockHeader,
    availability: AvailabilityFrame,
    source: iroha_sumeragi::availability::AvailabilitySource,
    qc: Qc,
    state_hash: iroha_crypto::HashOf<IrohaHeader>,
    next: AppliedConfig,
    hashes: Vec<iroha_crypto::HashOf<TransactionEntrypoint>>,
    events: Vec<EventBox>,
}

impl PendingCommit {
    fn matches(&self, block: &AvailableBody, qc: &Qc) -> bool {
        self.header == *block.header()
            && self.availability == *block.availability()
            && self.source == *block.source()
            && self.qc == *qc
    }
}

/// Borrowed original Worker execution for component assertions and refusal controls.
///
/// This view grants no certificate or publication capability. Mutating any sealed source
/// makes the production preparation boundary reject that same retained execution.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub struct PendingExecutionView<'borrow, 'state> {
    /// The exact originally executed carrier before a certificate is attached.
    pub block: &'borrow mut ValidBlock,
    /// The original overlay retained by the Worker.
    pub state: &'borrow mut StateBlock<'state>,
    /// The exact witness retained with the original R.
    pub witness: &'borrow mut iroha_data_model::block::consensus::ExecWitness,
}

/// Immutable observation of the actual certificate-authorized publication overlay.
/// No source mutation or publication capability is exposed by this test view.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub struct PreparedExecutionView<'borrow, 'state> {
    /// The exact originally executed carrier with its native certificate.
    pub block: &'borrow CommittedBlock,
    /// The original prepared overlay whose projected snapshot will be published.
    pub state: &'borrow StateBlock<'state>,
    /// The original execution witness, retained with certified R.
    pub witness: &'borrow iroha_data_model::block::consensus::ExecWitness,
}

#[cfg(any(test, feature = "iroha-core-tests"))]
type PreparedInspection = Box<
    dyn for<'borrow, 'state> FnOnce(Result<PreparedExecutionView<'borrow, 'state>, String>) + Send,
>;

#[cfg(any(test, feature = "iroha-core-tests"))]
type PendingInspection = Box<
    dyn for<'borrow, 'state> FnOnce(Result<PendingExecutionView<'borrow, 'state>, String>) + Send,
>;

enum Request {
    #[cfg(any(test, feature = "iroha-core-tests"))]
    InspectPending(Hash32, PendingInspection),
    #[cfg(any(test, feature = "iroha-core-tests"))]
    InspectPrepared(Hash32, PreparedInspection),
    Execute(AvailableBody, Hash32, mpsc::SyncSender<Option<ExecOutcome>>),
    Discard(u64, Vec<Hash32>),
    Prepare(
        AvailableBody,
        Qc,
        CommitTelemetryOrigin,
        mpsc::SyncSender<Result<Option<Hash32>, PublicationError>>,
    ),
    Commit(
        AvailableBody,
        Qc,
        mpsc::SyncSender<Result<AppliedConfig, PublicationError>>,
    ),
    Build(
        u64,
        u64,
        u32,
        mpsc::SyncSender<Result<(Option<PayloadBytes>, bool), PublicationError>>,
    ),
    BuildControl(
        ControlWitnessContext,
        mpsc::SyncSender<Result<(ControlWitness, bool), PublicationError>>,
    ),
    DriveControl(
        ApplicationControlContext,
        mpsc::SyncSender<Result<Option<ApplicationControl>, PublicationError>>,
    ),
    ReceiveControl(
        PublicKey,
        ApplicationControl,
        mpsc::SyncSender<Result<(), PublicationError>>,
    ),
    AttachBeacon {
        instance: Hash32,
        local_bls: Option<[u8; 48]>,
        signer: Option<Arc<dyn crate::beacon::GlobalThresholdBeaconPartialSignerV1>>,
        reply: mpsc::SyncSender<
            Result<super::epoch_beacon::producer::NativeBeaconReadiness, PublicationError>,
        >,
    },
    AttachAttestation {
        verifier: super::attestation::NativePastaVerifier,
        custody:
            Option<Arc<crate::zk::kagemusha_v1_recursion::KagemushaMintFinalityLocalAuthorityV1>>,
        publisher: super::attestation::NativeAttestationPublisher,
        reply: mpsc::SyncSender<Result<(), PublicationError>>,
    },
    Reject(u64, u64, Hash32),
    AttachQueue(Arc<Queue>),
    AttachFinalizedArchives(FinalizedArchives, mpsc::SyncSender<Result<(), String>>),
}

/// Pure owner check before a certificate enters a queued request or retained publication.
/// Cryptographic validity cannot authorize allocation from another or uncharged pool.
fn require_qc_witness_admission(
    qc: &Qc,
    budget: &iroha_allocation::AllocationBudget,
) -> Result<(), PublicationError> {
    if qc
        .attestation_witness
        .as_ref()
        .is_some_and(|witness| !witness.admitted_to(budget))
    {
        return Err(PublicationError::Retryable(
            "commit witness requires admission to the original State pool".into(),
        ));
    }
    Ok(())
}

/// Only the exact verified frame and payload from this State pool may enter execution.
fn require_body_admission(
    body: &AvailableBody,
    budget: &iroha_allocation::AllocationBudget,
) -> Result<(), PublicationError> {
    if !body.admitted_to(budget) {
        return Err(PublicationError::RecoveryRequired(
            "available body belongs to another State pool".into(),
        ));
    }
    Ok(())
}

/// The driver-facing handle of the executor thread.
pub struct StateExecutor {
    /// The same pool as the worker, checked before retaining a queued certificate.
    execution_budget: iroha_allocation::AllocationBudget,
    requests: mpsc::SyncSender<Request>,
    _thread: JoinHandle<()>,
}

impl StateExecutor {
    /// Spawn the executor thread.
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn spawn(context: ExecutorContext) -> std::io::Result<Self> {
        // One serialized worker can retain at most one waiting request. The driver
        // already owns bounded control ingress and the original publication retry.
        let (requests, rx) = mpsc::sync_channel(1);
        let execution_budget = context.state.ivm_execution_budget();
        let thread = super::threads::sumeragi_thread_builder("sumeragi-state-exec")
            .spawn(move || run(&context, &rx))?;
        Ok(Self {
            execution_budget,
            requests,
            _thread: thread,
        })
    }

    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn inspect_pending<R: Send + 'static>(
        &self,
        block_hash: Hash32,
        inspect: impl for<'borrow, 'state> FnOnce(PendingExecutionView<'borrow, 'state>) -> R
        + Send
        + 'static,
    ) -> Result<R, String> {
        self.call(|reply| {
            Request::InspectPending(
                block_hash,
                Box::new(move |original| {
                    let result = original.and_then(|original| {
                        catch_unwind(AssertUnwindSafe(|| inspect(original)))
                            .map_err(|_| "pending execution inspection panicked".to_owned())
                    });
                    let _ = reply.send(result);
                }),
            )
        })
        .ok_or_else(|| "original execution Worker stopped".to_owned())?
    }

    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn inspect_prepared<R: Send + 'static>(
        &self,
        block_hash: Hash32,
        inspect: impl for<'borrow, 'state> FnOnce(PreparedExecutionView<'borrow, 'state>) -> R
        + Send
        + 'static,
    ) -> Result<R, String> {
        self.call(|reply| {
            Request::InspectPrepared(
                block_hash,
                Box::new(move |original| {
                    let result = original.and_then(|original| {
                        catch_unwind(AssertUnwindSafe(|| inspect(original)))
                            .map_err(|_| "prepared execution inspection panicked".to_owned())
                    });
                    let _ = reply.send(result);
                }),
            )
        })
        .ok_or_else(|| "original execution Worker stopped".to_owned())?
    }

    fn call<T>(&self, request: impl FnOnce(mpsc::SyncSender<T>) -> Request) -> Option<T> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.requests.send(request(tx)).ok()?;
        rx.recv().ok()
    }
}

impl StateExecutor {
    /// Attach the transaction queue: the builder reads it and applied blocks clean it.
    pub fn attach_queue(&self, queue: Arc<Queue>) {
        let _ = self.requests.send(Request::AttachQueue(queue));
    }

    /// Attach the configured archives once, after replay and before starting the driver.
    /// This synchronously captures the reconciled tip before the executor acknowledges binding;
    /// the replayed tip published by startup replay is that tip, not an execution.
    ///
    /// # Errors
    /// The executor is unavailable, already bound, recovering or executing beyond the applied
    /// tip, or the exact tip cannot be captured.
    pub fn attach_finalized_archives(&self, archives: FinalizedArchives) -> Result<(), String> {
        self.call(|reply| Request::AttachFinalizedArchives(archives, reply))
            .unwrap_or_else(|| Err("executor thread stopped".into()))
    }

    /// Attach the actual runtime beacon custodian once before the driver starts.
    /// Startup replay consumes the signed witness and needs no local producer.
    ///
    /// # Errors
    /// A producer is already attached or the serialized executor stopped.
    pub(crate) fn attach_beacon(
        &self,
        instance: Hash32,
        local_bls: Option<[u8; 48]>,
        signer: Option<Arc<dyn crate::beacon::GlobalThresholdBeaconPartialSignerV1>>,
    ) -> Result<super::epoch_beacon::producer::NativeBeaconReadiness, PublicationError> {
        self.call(|reply| Request::AttachBeacon {
            instance,
            local_bls,
            signer,
            reply,
        })
        .unwrap_or_else(|| Err(control::stopped()))
    }

    /// Attach provisioned generation custody and its original-pool mailbox before startup.
    ///
    /// # Errors
    /// Rejects replacement of an existing signer or a stopped serialized worker.
    pub(crate) fn attach_attestation(
        &self,
        verifier: super::attestation::NativePastaVerifier,
        custody: Option<
            Arc<crate::zk::kagemusha_v1_recursion::KagemushaMintFinalityLocalAuthorityV1>,
        >,
        publisher: super::attestation::NativeAttestationPublisher,
    ) -> Result<(), PublicationError> {
        self.call(|reply| Request::AttachAttestation {
            verifier,
            custody,
            publisher,
            reply,
        })
        .unwrap_or_else(|| Err(control::stopped()))
    }

    /// Re-apply a block Kura already holds (startup replay): execute it on the applied tip
    /// and require the certified result. The caller must admit any decoded witness to the
    /// original State pool before this retained handoff; KuraBlockStore does that explicitly.
    ///
    /// # Errors
    /// The block does not re-execute to its certified result, or a local failure.
    pub fn replay(&mut self, block: &AvailableBody, commit_qc: &Qc) -> Result<(), String> {
        match self
            .prepare_with_origin(block, commit_qc, CommitTelemetryOrigin::HistoricalReplay)
            .map_err(|error| error.to_string())?
        {
            Some(result) if result == commit_qc.result => {}
            Some(_) => return Err("replayed block diverges from its certified result".into()),
            None => return Err("replayed block no longer executes".into()),
        }
        self.commit(block, commit_qc)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn prepare_with_origin(
        &mut self,
        block: &AvailableBody,
        commit_qc: &Qc,
        origin: CommitTelemetryOrigin,
    ) -> Result<Option<Hash32>, PublicationError> {
        require_body_admission(block, &self.execution_budget)?;
        require_qc_witness_admission(commit_qc, &self.execution_budget)?;
        self.call(|reply| Request::Prepare(block.clone(), commit_qc.clone(), origin, reply))
            .unwrap_or_else(|| {
                Err(PublicationError::RecoveryRequired(
                    "executor thread stopped".into(),
                ))
            })
    }
}

impl Executor for StateExecutor {
    fn execute(&mut self, block: &AvailableBody, block_hash: &Hash32) -> Option<ExecOutcome> {
        let outcome = match require_body_admission(block, &self.execution_budget) {
            Err(error) => Some(ExecOutcome::Failed(error.to_string())),
            Ok(()) => self
                .call(|reply| Request::Execute(block.clone(), *block_hash, reply))
                .unwrap_or_else(|| Some(ExecOutcome::Failed("executor thread stopped".into()))),
        };
        // The core reports only `LocalFault::ExecutorFailed { height }` and retries; the
        // local reason is logged here, once per failed answer.
        if let Some(ExecOutcome::Failed(reason)) = &outcome {
            iroha_logger::warn!(
                height = block.header().height,
                view = block.header().origin_view,
                %reason,
                "sumeragi: local execution failure"
            );
        }
        outcome
    }

    fn discard(&mut self, height: u64, keep: &[Hash32]) {
        let _ = self.requests.send(Request::Discard(height, keep.to_vec()));
    }

    fn prepare(
        &mut self,
        block: &AvailableBody,
        commit_qc: &Qc,
    ) -> Result<Option<Hash32>, PublicationError> {
        self.prepare_with_origin(block, commit_qc, CommitTelemetryOrigin::Forward)
    }

    fn commit(
        &mut self,
        block: &AvailableBody,
        commit_qc: &Qc,
    ) -> Result<AppliedConfig, PublicationError> {
        require_body_admission(block, &self.execution_budget)?;
        require_qc_witness_admission(commit_qc, &self.execution_budget)?;
        self.call(|reply| Request::Commit(block.clone(), commit_qc.clone(), reply))
            .unwrap_or_else(|| {
                Err(PublicationError::RecoveryRequired(
                    "executor thread stopped".into(),
                ))
            })
    }

    fn build(
        &mut self,
        height: u64,
        view: u64,
        max_bytes: u32,
        _exec_budget_ms: u32,
    ) -> Result<(Option<PayloadBytes>, bool), PublicationError> {
        self.call(|reply| Request::Build(height, view, max_bytes, reply))
            .unwrap_or_else(|| {
                Err(PublicationError::RecoveryRequired(
                    "payload builder stopped".into(),
                ))
            })
    }

    fn build_control_witness(
        &mut self,
        context: &ControlWitnessContext,
    ) -> Result<(ControlWitness, bool), PublicationError> {
        self.call(|reply| Request::BuildControl(*context, reply))
            .unwrap_or_else(|| Err(control::stopped()))
    }

    fn drive_control(
        &mut self,
        context: &ApplicationControlContext,
    ) -> Result<Option<ApplicationControl>, PublicationError> {
        self.call(|reply| Request::DriveControl(*context, reply))
            .unwrap_or_else(|| Err(control::stopped()))
    }

    fn receive_application_control(
        &mut self,
        from: &PublicKey,
        message: &ApplicationControl,
    ) -> Result<(), PublicationError> {
        self.call(|reply| Request::ReceiveControl(from.clone(), message.clone(), reply))
            .unwrap_or_else(|| Err(control::stopped()))
    }

    fn reject(&mut self, height: u64, view: u64, block_hash: &Hash32) {
        let _ = self
            .requests
            .send(Request::Reject(height, view, *block_hash));
    }
}

/// A read handle into the exact funded World graph; cloning a decoded epoch is forbidden here.
struct ScheduledAuthority {
    schedule: schedule::RetainedConsensusSchedule,
    height: u64,
}
impl std::ops::Deref for ScheduledAuthority {
    type Target = schedule::ScheduledConfig;
    fn deref(&self) -> &Self::Target {
        self.schedule
            .ready(self.height)
            .expect("immutable ready slot checked at acquisition")
    }
}

/// Original completed execution inputs advance once through proof and result preparation.
enum FinishingPhase {
    ContextProof {
        inputs: iroha_allocation::RetainedPayload<NativeExecutionInputs>,
        refusal: Option<NativeLaneStateProofError>,
    },
    Ready(iroha_allocation::RetainedPayload<ExecutionResultCommitment>),
    /// A one-shot deterministic transition failed or unwound; only recovery may release it.
    Consuming,
}
impl FinishingPhase {
    fn ready(&self) -> Option<&iroha_allocation::RetainedPayload<ExecutionResultCommitment>> {
        match self {
            Self::Ready(commitment) => Some(commitment),
            Self::ContextProof { .. } | Self::Consuming => None,
        }
    }
}

/// One completed execution retained while its proof or result encoding awaits local resources.
/// No retry of either phase re-executes transactions or creates another overlay.
struct Finishing<'s> {
    block_hash: Hash32,
    height: u64,
    header: iroha_sumeragi::message::BlockHeader,
    availability: AvailabilityFrame,
    source: iroha_sumeragi::availability::AvailabilitySource,
    valid: ValidBlock,
    overlay: Box<StateBlock<'s>>,
    witness: iroha_data_model::block::consensus::ExecWitness,
    phase: FinishingPhase,
    applied_config: AppliedConfig,
    committee: Vec<PeerId>,
    events: Vec<EventBox>,
    /// Original typed source of the latest refusal; never a cached invalid verdict.
    encoding_refusal: Option<super::commitment::ResultPreimageError>,
    native_contexts: Option<PreparedNativeContext>,
    archive_refusal: Option<NativeContextArchiveError>,
    world_cut_refusal:
        Option<crate::state::world_projection::world_state_accumulator::world_state_cut::CutError>,
}

/// The executed overlay of one block.
struct Live<'s> {
    telemetry_origin: Option<CommitTelemetryOrigin>,
    native_contexts: Option<PreparedNativeContext>,
    block_hash: Hash32,
    height: u64,
    header: iroha_sumeragi::message::BlockHeader,
    availability: AvailabilityFrame,
    source: iroha_sumeragi::availability::AvailabilitySource,
    phase: PublicationPhase,
    overlay: Option<Box<StateBlock<'s>>>,
    witness: iroha_data_model::block::consensus::ExecWitness,
    result: Hash32,
    /// Complete original canonical epoch result and its exact source-bound allocation ledger.
    commitment: iroha_allocation::RetainedPayload<ExecutionResultCommitment>,
    /// Exact original witness/signature progress, retained independently of durable encoding.
    attestation: local_attestation::Progress,
    applied_config: AppliedConfig,
    committee: Vec<PeerId>,
    events: Vec<EventBox>,
}

/// The one original execution's progress; no phase reconstructs a predecessor owner.
enum PublicationPhase {
    Executed {
        valid: ValidBlock,
        preimage: iroha_allocation::ChargedBuffer<u8>,
    },
    /// Each successful encoding remains owned while a later allocation is refused.
    EncodingCertificate {
        valid: ValidBlock,
        preimage: iroha_allocation::ChargedBuffer<u8>,
        header_wire: Option<iroha_allocation::ChargedBuffer<u8>>,
        qc_wire: Option<iroha_allocation::ChargedBuffer<u8>>,
        availability_wire: Option<iroha_allocation::ChargedBuffer<u8>>,
        qc: Qc,
        refusal: Option<super::commitment::ResultPreimageError>,
    },
    /// All four original byte allocations await shared-control admission.
    Certifying {
        valid: ValidBlock,
        parts: Option<iroha_data_model::block::ChargedCertificateParts>,
        qc: Qc,
        refusal: Option<iroha_data_model::block::CertificateAdmissionError>,
    },
    Prepared {
        committed: CommittedBlock,
        staged: Arc<StagedBlock>,
        qc: Qc,
        state_events: Option<Vec<EventBox>>,
    },
    /// A conversion or consuming apply is in progress; unwind requires recovery.
    Consuming,
    Published {
        staged: Arc<StagedBlock>,
        qc: Qc,
    },
}

#[path = "executor_control.rs"]
mod control;
#[path = "executor_attestation.rs"]
mod local_attestation;

/// Exact validation identity permitting transaction isolation; a control refusal never sets it.
#[derive(Clone, Copy)]
struct QuarantineContext {
    height: u64,
    view: u64,
    block_hash: Hash32,
    pulse_context: iroha_data_model::consensus::GlobalThresholdBeaconPulseContextV1,
}

struct GlobalPayloadSource {
    block: SignedBlock,
    attest: bool,
    hashes: Vec<iroha_crypto::HashOf<TransactionEntrypoint>>,
}

struct GlobalPayloadBuild {
    height: u64,
    view: u64,
    max_bytes: u32,
    job: super::driver::payload_build::PayloadBuild<GlobalPayloadSource>,
}

struct Worker<'s> {
    payload_build: Option<GlobalPayloadBuild>,
    context: &'s ExecutorContext,
    state: &'s State,
    applied: (u64, Hash32),
    live: Option<Live<'s>>,
    finishing: Option<Finishing<'s>>,
    /// Verdicts of executions whose overlay is gone, by block hash (bounded per height).
    results: BTreeMap<
        Hash32,
        (
            iroha_sumeragi::availability::AvailabilitySource,
            ExecOutcome,
        ),
    >,
    /// The transactions of the last payload this node built, for the quarantine.
    last_built: Option<(u64, u64, Vec<iroha_crypto::HashOf<TransactionEntrypoint>>)>,
    queue: Option<Arc<Queue>>,
    /// The process-lifetime partial owner; view changes never replace it.
    beacon: Option<super::epoch_beacon::producer::NativeBeaconProducer>,
    /// Sole local Pasta custodian and one original-pool receipt publisher.
    attestation: Option<local_attestation::Custody>,
    /// Only an exact control-free transaction rejection permits queue isolation.
    quarantine_context: Option<QuarantineContext>,
    /// A consuming publication cannot be retried on this worker, even after an unwind.
    recovery: Option<String>,
    archives: Option<FinalizedArchives>,
    pending_commit: Option<PendingCommit>,
}

fn run(context: &ExecutorContext, requests: &mpsc::Receiver<Request>) {
    let state = Arc::clone(&context.state);
    let mut worker = Worker {
        payload_build: None,
        context,
        state: &state,
        applied: context.applied,
        live: None,
        finishing: None,
        results: BTreeMap::new(),
        last_built: None,
        queue: context.queue.clone(),
        recovery: None,
        beacon: None,
        archives: None,
        pending_commit: None,
        attestation: None,
        quarantine_context: None,
    };
    while let Ok(request) = requests.recv() {
        worker.serve(request);
    }
}

impl<'s> Worker<'s> {
    fn serve(&mut self, request: Request) {
        match request {
            #[cfg(any(test, feature = "iroha-core-tests"))]
            Request::InspectPending(block_hash, inspect) => {
                let original =
                    self.live
                        .as_mut()
                        .ok_or_else(|| "no original pending execution".to_owned())
                        .and_then(|live| {
                            if live.block_hash != block_hash {
                                return Err(
                                    "pending inspection belongs to a different execution".into()
                                );
                            }
                            let PublicationPhase::Executed { valid, .. } = &mut live.phase else {
                                return Err("certified publication cannot be mutated".into());
                            };
                            let state = live.overlay.as_deref_mut().ok_or_else(|| {
                                "original pending overlay was consumed".to_owned()
                            })?;
                            Ok(PendingExecutionView {
                                block: valid,
                                state,
                                witness: &mut live.witness,
                            })
                        });
                inspect(original);
            }
            #[cfg(any(test, feature = "iroha-core-tests"))]
            Request::InspectPrepared(block_hash, inspect) => {
                let original = self
                    .live
                    .as_ref()
                    .ok_or_else(|| "no original prepared execution".to_owned())
                    .and_then(|live| {
                        if live.block_hash != block_hash {
                            return Err(
                                "prepared inspection belongs to a different execution".into()
                            );
                        }
                        let PublicationPhase::Prepared {
                            committed,
                            state_events: Some(_),
                            ..
                        } = &live.phase
                        else {
                            return Err("original execution is not prepared for publication".into());
                        };
                        let state = live
                            .overlay
                            .as_deref()
                            .ok_or_else(|| "original prepared overlay was consumed".to_owned())?;
                        Ok(PreparedExecutionView {
                            block: committed,
                            state,
                            witness: &live.witness,
                        })
                    });
                inspect(original);
            }
            Request::Execute(block, block_hash, reply) => {
                let _ = reply.send(self.execute(&block, block_hash));
            }
            Request::Discard(height, keep) => self.discard(height, &keep),
            Request::Prepare(block, qc, origin, reply) => {
                let _ = reply.send(self.prepare_with_origin(&block, &qc, origin));
            }
            Request::Commit(block, qc, reply) => {
                let _ = reply.send(self.commit(&block, &qc));
            }
            Request::Build(height, view, max_bytes, reply) => {
                let _ = reply.send(self.build(height, view, max_bytes));
            }
            Request::BuildControl(context, reply) => {
                let _ = reply.send(self.build_control_witness(&context));
            }
            Request::DriveControl(context, reply) => {
                let _ = reply.send(self.drive_control(&context));
            }
            Request::ReceiveControl(from, message, reply) => {
                let _ = reply.send(self.receive_application_control(&from, &message));
            }
            Request::AttachBeacon {
                instance,
                local_bls,
                signer,
                reply,
            } => {
                let _ = reply.send(self.attach_beacon(instance, local_bls, signer));
            }
            Request::AttachAttestation {
                verifier,
                custody,
                publisher,
                reply,
            } => {
                let _ = reply.send(self.attach_attestation(verifier, custody, publisher));
            }
            Request::Reject(height, view, block_hash) => self.reject(height, view, block_hash),
            Request::AttachQueue(queue) => self.queue = Some(queue),
            Request::AttachFinalizedArchives(archives, reply) => {
                let _ = reply.send(self.bind_finalized_archives(archives));
            }
        }
    }

    /// Bind the configured archives once, at the applied tip and before any execution beyond
    /// it. Startup replay leaves its last block published: that is the applied tip itself, not
    /// an execution. The binding captures the exact certified tip State (a no-op when startup
    /// reconciliation already captured it), and every later commit captures its own height, so
    /// no height is skipped or captured twice.
    fn bind_finalized_archives(&mut self, archives: FinalizedArchives) -> Result<(), String> {
        if self.archives.is_some() {
            return Err("finalized archives are already bound".into());
        }
        if let Some(reason) = &self.recovery {
            return Err(format!(
                "finalized archives cannot bind during recovery: {reason}"
            ));
        }
        let (height, block_hash) = self.applied;
        if self.pending_commit.is_some()
            || self.finishing.is_some()
            || self.live.as_ref().is_some_and(|live| {
                (live.height, live.block_hash) != (height, block_hash)
                    || !matches!(live.phase, PublicationPhase::Published { .. })
            })
        {
            return Err(
                "finalized archives must be bound before execution beyond the applied tip".into(),
            );
        }
        let view = self.state.view();
        if u64::try_from(view.height()).ok() != Some(height) {
            return Err(format!(
                "committed State height {} differs from the applied tip {height}",
                view.height()
            ));
        }
        archives.capture(&view)?;
        drop(view);
        self.archives = Some(archives);
        Ok(())
    }

    /// Answer an `Execute` (O4): the live overlay or a remembered verdict, `None` while the
    /// parent is not applied, otherwise a fresh execution.
    fn execute(&mut self, block: &AvailableBody, block_hash: Hash32) -> Option<ExecOutcome> {
        if self.pending_commit.is_some() {
            return None;
        }
        if let Some(reason) = &self.recovery {
            return Some(ExecOutcome::Failed(reason.clone()));
        }
        if let Some(live) = &self.live {
            if live.block_hash == block_hash {
                if live.header != *block.header()
                    || live.availability != *block.availability()
                    || live.source != *block.source()
                {
                    return Some(invalid(
                        block.header().height,
                        &"execution retry changes its original header",
                    ));
                }
                return Some(self.finish_local_attestation());
            }
        }
        if let Some((source, outcome)) = self.results.get(&block_hash) {
            return Some(if source == block.source() {
                outcome.clone()
            } else {
                invalid(
                    block.header().height,
                    &"execution retry changes its original authority",
                )
            });
        }
        if !self.parent_applied(block) {
            return None;
        }
        let outcome = self.run_execution(block, block_hash);
        if !matches!(outcome, ExecOutcome::Valid(_) | ExecOutcome::Failed(_)) {
            self.remember(block.source().clone(), block_hash, outcome.clone());
        }
        Some(outcome)
    }

    fn parent_applied(&self, block: &AvailableBody) -> bool {
        block.header().height == self.applied.0.saturating_add(1)
            && block.header().parent_hash == self.applied.1
    }

    fn remember(
        &mut self,
        source: iroha_sumeragi::availability::AvailabilitySource,
        block_hash: Hash32,
        outcome: ExecOutcome,
    ) {
        self.results.insert(block_hash, (source, outcome));
        // The core keeps at most a handful of bodies per height (§8.4); stay bounded.
        while self.results.len() > 16 {
            let Some(oldest) = self
                .results
                .iter()
                .min_by_key(|(_, (source, _))| source.height())
                .map(|(hash, _)| *hash)
            else {
                break;
            };
            self.results.remove(&oldest);
        }
    }

    /// Execute `block` on the applied tip, keeping the overlay as the live one.
    fn run_execution(&mut self, block: &AvailableBody, block_hash: Hash32) -> ExecOutcome {
        self.run_execution_with_encoder(block, block_hash, encode_result_preimage)
    }

    fn run_execution_with_encoder(
        &mut self,
        block: &AvailableBody,
        block_hash: Hash32,
        encode: impl FnOnce(
            &iroha_allocation::RetainedPayload<ExecutionResultCommitment>,
            &iroha_allocation::AllocationBudget,
        ) -> Result<
            iroha_allocation::ChargedBuffer<u8>,
            super::commitment::ResultPreimageError,
        >,
    ) -> ExecOutcome {
        self.run_execution_with_finisher(block, block_hash, |worker| {
            worker.finish_execution_with_encoder(encode)
        })
    }

    /// The callback receives the sole completed original, before any proof scratch allocation.
    fn run_execution_with_finisher(
        &mut self,
        block: &AvailableBody,
        block_hash: Hash32,
        finish: impl FnOnce(&mut Self) -> ExecOutcome,
    ) -> ExecOutcome {
        if self.publication_pending() {
            return ExecOutcome::Failed(
                "the original prepared publication is still retained".into(),
            );
        }
        if let Some(original) = &self.finishing {
            if original.block_hash == block_hash {
                if original.header != *block.header()
                    || original.availability != *block.availability()
                    || original.source != *block.source()
                {
                    return invalid(
                        block.header().height,
                        &"result retry changes its original header",
                    );
                }
                return finish(self);
            }
        }
        // Invalidate the public receipt before releasing its exact original overlay.
        if let Err(error) = self.clear_local_attestation() {
            return ExecOutcome::Failed(error);
        }
        // An explicitly superseded candidate releases its original private execution.
        self.finishing = None;
        // The state admits one overlay: drop the previous one first.
        if let Some(previous) = self.live.take() {
            // Its overlay is gone; a later `prepare` executes again.
            self.results.remove(&previous.block_hash);
        }
        let height = block.header().height;
        let iroha_block = match payload::decode(block.payload().as_slice()) {
            Ok(block) => block,
            Err(error @ payload::PayloadError::DecodeResource)
                if !cfg!(all(test, sumeragi_core_mutation = "HC8")) =>
            {
                // The caller retains the original available owner. Failed is retried and
                // never enters the deterministic negative-result cache.
                return ExecOutcome::Failed(error.to_string());
            }
            Err(error) => return invalid(height, &error),
        };
        if self.state.view().latest_block().is_none() {
            return ExecOutcome::Failed("the applied parent block is not available".into());
        }
        let Some(scheduled) = self.scheduled(height) else {
            return ExecOutcome::Failed(format!("no scheduled configuration for height {height}"));
        };
        let configured = match scheduled.height_config() {
            Ok(config) => config,
            Err(error) => return ExecOutcome::Failed(format!("invalid retained epoch: {error}")),
        };
        if block.source().config() != &configured || block.header().epoch != configured.epoch.id {
            return invalid(
                height,
                &"available custody does not bind the complete scheduled authority",
            );
        }
        self.quarantine_context = None;
        // Decode solely inside the original header/payload validation boundary.
        let pulse_context = control::pulse_context(block.header());
        let boundary_attestation = height == configured.epoch.last_height;
        let cadence = Duration::from_millis(scheduled.params.block_time_ms);
        if !proposal_matches_header(iroha_block.header(), block) {
            return invalid(
                height,
                &"the payload's height or view differs from the header",
            );
        }
        if block.header().attest != (boundary_attestation || attestation_required(&iroha_block)) {
            return invalid(
                height,
                &"the attestation flag differs from the payload's rule",
            );
        }
        // Merged lane blocks execute after the block's own transactions (§4.3 of
        // `specs/sumeragi_lanes.md`); the node waits for its lane stores within `E_max`.
        let expansion = match lanes::merge::expand(
            self.state,
            &iroha_block,
            &*self.context.lane_blocks,
            Duration::from_millis(scheduled.params.exec_budget_ms),
        ) {
            Ok(expansion) => expansion,
            Err(lanes::merge::MergeError::Pending(reason)) => {
                return ExecOutcome::Failed(reason);
            }
            Err(error @ lanes::merge::MergeError::Invalid(_)) => return invalid(height, &error),
            Err(lanes::merge::MergeError::Storage(error)) => {
                let reason = format!("lane storage during execution: {error}");
                if !matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) {
                    self.recovery = Some(reason.clone());
                }
                return ExecOutcome::Failed(reason);
            }
        };
        let committee = scheduled
            .epoch
            .committee
            .iter()
            .map(|member| member.validator.clone())
            .collect::<Vec<_>>();
        let topology = Topology::new(committee.clone());
        let validated = catch_unwind(AssertUnwindSafe(|| {
            ValidBlock::validate_sumeragi_block(
                iroha_block,
                &topology,
                &self.context.genesis_account,
                cadence,
                self.context.consensus_mode,
                expansion,
                block.header(),
                block.payload().as_slice(),
                self.state,
            )
        }));
        let Ok(validated) = validated else {
            return ExecOutcome::Failed("block validation panicked".into());
        };
        let mut events = Vec::new();
        let (valid, mut overlay) = match validated.unpack(|event| events.push(event.into())) {
            Ok(executed) => executed,
            Err((_, error)) => {
                // Native validation attaches a rejection only after checking the original
                // header/payload source. Local refusals and unbound sources attach none.
                // Rejection is an observation, not a committed transaction outcome.
                for event in events {
                    let _ = self.context.events.send(event);
                }
                if block.header().control_witness.is_empty()
                    && control::transaction_rejection(&error)
                {
                    self.quarantine_context = Some(QuarantineContext {
                        height,
                        view: block.header().origin_view,
                        block_hash,
                        pulse_context,
                    });
                }
                return classify(height, &error);
            }
        };
        if let Err(error) = overlay.take_sumeragi_lanes() {
            return classify_lane_step(height, &error);
        }
        let inputs = match overlay.take_sumeragi_execution_inputs() {
            Ok(inputs) => inputs,
            Err(error) => return classify(height, &BlockValidationError::from(error)),
        };
        let applied_config = match inputs.get().schedule.applied_config() {
            Ok(config) => config,
            Err(error) => return invalid(height, &error),
        };
        let Some(witness) = overlay.take_exec_witness() else {
            return ExecOutcome::Failed("the execution witness was not captured".into());
        };
        self.finishing = Some(Finishing {
            block_hash,
            height,
            header: block.header().clone(),
            availability: block.availability().clone(),
            source: block.source().clone(),
            valid,
            overlay,
            witness,
            phase: FinishingPhase::ContextProof {
                inputs,
                refusal: None,
            },
            applied_config,
            committee,
            events,
            encoding_refusal: None,
            native_contexts: None,
            archive_refusal: None,
            world_cut_refusal: None,
        });
        finish(self)
    }

    /// Construct the mandatory context proof from the same original witness and input owner.
    fn prepare_original_result(&mut self) -> Result<(), String> {
        let original = self
            .finishing
            .as_ref()
            .expect("original completed execution");
        match &original.phase {
            FinishingPhase::Ready(_) => return Ok(()),
            FinishingPhase::Consuming => {
                let reason = "original result transition requires recovery".to_owned();
                self.recovery = Some(reason.clone());
                return Err(reason);
            }
            FinishingPhase::ContextProof { .. } => {}
        }
        let budget = self.state.ivm_execution_budget();
        let proof = match NativeLaneStateProof::from_witness(&original.witness, &budget) {
            Ok(proof) => proof,
            Err(error) => {
                let reason = error.to_string();
                if !error.is_local_refusal() {
                    self.recovery = Some(format!(
                        "original context proof requires recovery: {reason}"
                    ));
                }
                let FinishingPhase::ContextProof { refusal, .. } =
                    &mut self.finishing.as_mut().unwrap().phase
                else {
                    unreachable!("original proof phase")
                };
                *refusal = Some(error);
                return Err(reason);
            }
        };
        // The complete World state before and after this execution and its emitted events
        // (§4.1, Appendix E, E51), read from the same sealed original overlay.
        let transition = match self
            .finishing
            .as_ref()
            .expect("original completed execution")
            .overlay
            .world_state_transition()
        {
            Ok(transition) => transition,
            Err(error) => {
                let reason = format!("original World state transition requires recovery: {error}");
                self.recovery = Some(reason.clone());
                return Err(reason);
            }
        };
        // Retain only original journal-touched native hashes at precisely this R.
        // Local refusal leaves the same completed overlay and one-shot inputs live.
        if let Err(error) = self
            .finishing
            .as_mut()
            .unwrap()
            .overlay
            .capture_original_world_cut(transition.world_state_root)
        {
            let reason = error.to_string();
            if matches!(
                &error,
                crate::state::world_projection::world_state_accumulator::world_state_cut::CutError::Invalid(_)
            ) {
                self.recovery = Some(format!("original World cut requires recovery: {reason}"));
            }
            self.finishing.as_mut().unwrap().world_cut_refusal = Some(error);
            return Err(reason);
        }
        self.finishing.as_mut().unwrap().world_cut_refusal = None;
        let original = self.finishing.as_mut().unwrap();
        let FinishingPhase::ContextProof { inputs, .. } =
            std::mem::replace(&mut original.phase, FinishingPhase::Consuming)
        else {
            unreachable!("original proof inputs")
        };
        let commitment = execution_result(
            &original.witness,
            original.valid.as_ref(),
            &transition,
            inputs,
            proof,
        )
        .map_err(|error| {
            let reason = format!("original result requires recovery: {error}");
            self.recovery = Some(reason.clone());
            reason
        })?;
        if top_ups_without_flag(commitment.get(), original.header.attest) {
            let reason = "executed top-ups without the attestation flag".to_owned();
            self.recovery = Some(reason.clone());
            return Err(reason);
        }
        original.phase = FinishingPhase::Ready(commitment);
        Ok(())
    }

    /// Capture complete original values before any State publication. Retry reuses the same
    /// overlay and retained R; once admitted, the exact charged projection is never encoded twice.
    fn prepare_original_context_archive(&mut self) -> Result<(), String> {
        let original = self
            .finishing
            .as_mut()
            .expect("original completed execution");
        if original.native_contexts.is_some() {
            return Ok(());
        }
        match self.context.native_context_archive.prepare(
            &original.overlay,
            original.valid.as_ref(),
            original.phase.ready().expect("original prepared result"),
            &original.witness,
        ) {
            Ok(projection) => {
                original.native_contexts = Some(projection);
                original.archive_refusal = None;
                Ok(())
            }
            Err(error) => {
                let reason = error.to_string();
                if !error.is_local_refusal() {
                    self.recovery = Some(format!(
                        "original native context archive requires recovery: {reason}"
                    ));
                }
                original.archive_refusal = Some(error);
                Err(reason)
            }
        }
    }

    /// Retry only proof or encoding of the original completed execution.
    fn finish_execution_with_encoder(
        &mut self,
        encode: impl FnOnce(
            &iroha_allocation::RetainedPayload<ExecutionResultCommitment>,
            &iroha_allocation::AllocationBudget,
        ) -> Result<
            iroha_allocation::ChargedBuffer<u8>,
            super::commitment::ResultPreimageError,
        >,
    ) -> ExecOutcome {
        if let Err(reason) = self.prepare_original_result() {
            return ExecOutcome::Failed(reason);
        }
        if let Err(reason) = self.prepare_original_context_archive() {
            return ExecOutcome::Failed(reason);
        }
        let original = self
            .finishing
            .as_ref()
            .expect("original completed execution");
        let budget = self.state.ivm_execution_budget();
        let preimage = match encode(
            original.phase.ready().expect("original prepared result"),
            &budget,
        ) {
            Ok(preimage) => preimage,
            Err(error) => {
                // TODO: driver retry dispatch must wait on this retained original
                // capacity observation; diagnostic text is not a release source.
                let message = error.to_string();
                if !error.is_local_refusal() {
                    self.recovery = Some(format!(
                        "original result encoding invariant failed; recovery required: {message}"
                    ));
                }
                self.finishing.as_mut().unwrap().encoding_refusal = Some(error);
                return ExecOutcome::Failed(message);
            }
        };
        let result = result_of_preimage(preimage.as_slice());
        let original = self.finishing.take().unwrap();
        let FinishingPhase::Ready(commitment) = original.phase else {
            unreachable!("encoded original result")
        };
        self.live = Some(Live {
            telemetry_origin: None,
            native_contexts: Some(
                original
                    .native_contexts
                    .expect("original preapply context projection"),
            ),
            block_hash: original.block_hash,
            height: original.height,
            header: original.header,
            availability: original.availability,
            source: original.source,
            phase: PublicationPhase::Executed {
                valid: original.valid,
                preimage,
            },
            overlay: Some(original.overlay),
            witness: original.witness,
            result,
            commitment,
            attestation: local_attestation::Progress::WaitingBacking(None),
            applied_config: original.applied_config,
            committee: original.committee,
            events: original.events,
        });
        self.finish_local_attestation()
    }

    /// Admit the keys of the committee scheduled for `height` into the driver's cryptography.
    fn admit_scheduled(&self, height: u64) {
        let Some(crypto) = &self.context.crypto else {
            return;
        };
        let Some(config) = self.scheduled(height) else {
            return;
        };
        for member in &config.epoch.committee {
            if let Err(error) =
                crypto.admit(member.validator.public_key(), &member.proof_of_possession)
            {
                iroha_logger::warn!(peer = %member.validator, ?error, "sumeragi: committee key not admitted");
            }
        }
    }

    fn scheduled(&self, height: u64) -> Option<ScheduledAuthority> {
        let view = self.state.view();
        let schedule = view.world().consensus_schedule();
        schedule.ready(height).ok()?;
        Some(ScheduledAuthority {
            schedule: schedule.clone(),
            height,
        })
    }

    fn discard(&mut self, height: u64, keep: &[Hash32]) {
        if self.recovery.is_some() || self.publication_pending() {
            return;
        }
        if self
            .live
            .as_ref()
            .is_some_and(|live| live.height == height && !keep.contains(&live.block_hash))
        {
            if self.clear_local_attestation().is_err() {
                return;
            }
            self.live = None;
        }
        if self.finishing.as_ref().is_some_and(|original| {
            original.height == height && !keep.contains(&original.block_hash)
        }) {
            self.finishing = None;
        }
        self.results
            .retain(|hash, (source, _)| source.height() != height || keep.contains(hash));
    }

    fn publication_pending(&self) -> bool {
        self.pending_commit.is_some()
            || self.live.as_ref().is_some_and(|live| {
                matches!(
                    live.phase,
                    PublicationPhase::EncodingCertificate { .. }
                        | PublicationPhase::Certifying { .. }
                        | PublicationPhase::Prepared { .. }
                        | PublicationPhase::Consuming
                )
            })
    }

    /// Pin the original execution and exactly one certified frame for durable append.
    fn prepare(
        &mut self,
        block: &AvailableBody,
        qc: &Qc,
    ) -> Result<Option<Hash32>, PublicationError> {
        self.prepare_with_origin(block, qc, CommitTelemetryOrigin::Forward)
    }

    fn prepare_with_origin(
        &mut self,
        block: &AvailableBody,
        qc: &Qc,
        origin: CommitTelemetryOrigin,
    ) -> Result<Option<Hash32>, PublicationError> {
        self.prepare_with_encoder(block, qc, origin, |part, budget| {
            super::commitment::encode_certificate_part(
                part,
                budget,
                super::commitment::MAX_RESULT_PREIMAGE_BYTES,
            )
        })
    }

    fn prepare_with_encoder(
        &mut self,
        block: &AvailableBody,
        qc: &Qc,
        origin: CommitTelemetryOrigin,
        mut encode: impl FnMut(
            super::commitment::CertificatePart<'_>,
            &iroha_allocation::AllocationBudget,
        ) -> Result<
            iroha_allocation::ChargedBuffer<u8>,
            super::commitment::ResultPreimageError,
        >,
    ) -> Result<Option<Hash32>, PublicationError> {
        if let Some(reason) = &self.recovery {
            return Err(PublicationError::RecoveryRequired(reason.clone()));
        }
        require_qc_witness_admission(qc, &self.state.ivm_execution_budget())?;
        match catch_unwind(AssertUnwindSafe(|| {
            self.prepare_inner(block, qc, origin, &mut encode)
        })) {
            Ok(result) => result.map_err(|error| match &self.recovery {
                Some(reason) => PublicationError::RecoveryRequired(reason.clone()),
                None => PublicationError::Retryable(error),
            }),
            Err(_) => {
                let reason = "publication preparation panicked; recovery required".to_owned();
                self.recovery = Some(reason.clone());
                Err(PublicationError::RecoveryRequired(reason))
            }
        }
    }

    fn prepare_inner(
        &mut self,
        block: &AvailableBody,
        qc: &Qc,
        origin: CommitTelemetryOrigin,
        encode: &mut impl FnMut(
            super::commitment::CertificatePart<'_>,
            &iroha_allocation::AllocationBudget,
        ) -> Result<
            iroha_allocation::ChargedBuffer<u8>,
            super::commitment::ResultPreimageError,
        >,
    ) -> Result<Option<Hash32>, String> {
        if let Some(pending) = &self.pending_commit {
            return if pending.matches(block, qc) && pending.telemetry_origin == origin {
                Ok(Some(pending.qc.result))
            } else {
                Err("another committed decision is awaiting archive capture".into())
            };
        }
        let block_hash = qc.block_hash;
        if qc.kind != iroha_sumeragi::message::VoteKind::Commit
            || qc.height != block.header().height
            || qc.instance != block.header().instance
            || qc.epoch != block.header().epoch
            || qc.attest != block.header().attest
            || super::commitment::chain_hash(&iroha_sumeragi::preimage::block_hash_preimage(
                block.header(),
            )) != block_hash
        {
            return Err("commit certificate does not bind the exact requested block".into());
        }
        // A published execution already owns this exact checked certificate; its old
        // schedule slot may have retired. Every new certificate is checked independently.
        let authenticated = self.live.as_ref().is_some_and(|live| {
            live.block_hash == block_hash
                && live.header == *block.header()
                && live.availability == *block.availability()
                && live.source == *block.source()
                && matches!(&live.phase,
                    PublicationPhase::Prepared { qc: original, .. }
                    | PublicationPhase::Published { qc: original, .. } if original == qc)
        });
        if !authenticated {
            self.verify_prepared_certificate(block, qc)?;
        }
        let reusable = self
            .live
            .as_ref()
            .is_some_and(|live| live.block_hash == block_hash);
        if !reusable {
            if self.publication_pending() {
                return Err("cannot replace the original prepared publication".into());
            }
            if !self.parent_applied(block) {
                return Err("the committed block's parent is not applied".into());
            }
            self.results.remove(&block_hash);
            match self.run_execution(block, block_hash) {
                ExecOutcome::Valid(_) => {}
                ExecOutcome::Invalid => return Ok(None),
                ExecOutcome::Failed(reason) => return Err(reason),
                ExecOutcome::Cancelled => return Err("execution cancelled".into()),
            }
        }
        // Reusing an executed overlay must complete the same receipt publication too.
        match self.finish_local_attestation() {
            ExecOutcome::Valid(_) => {}
            ExecOutcome::Failed(reason) => return Err(reason),
            _ => return Err("original attestation did not complete".into()),
        }
        let live = self.live.as_mut().ok_or("no executed overlay to prepare")?;
        if live.header != *block.header()
            || live.availability != *block.availability()
            || live.source != *block.source()
        {
            return Err("prepared header differs from original execution".into());
        }
        if live
            .telemetry_origin
            .is_some_and(|original| original != origin)
        {
            return Err("prepared execution telemetry origin cannot be replaced".into());
        }
        match &live.phase {
            PublicationPhase::Prepared {
                staged,
                qc: original,
                ..
            }
            | PublicationPhase::Published {
                staged,
                qc: original,
            } => {
                if original != qc {
                    return Err("prepared certificate cannot be replaced".into());
                }
                self.context.staging.stage(Arc::clone(staged));
                return Ok(Some(live.result));
            }
            PublicationPhase::Consuming => return Err("publication requires recovery".into()),
            PublicationPhase::EncodingCertificate { qc: original, .. }
            | PublicationPhase::Certifying { qc: original, .. }
                if original != qc =>
            {
                return Err("original certificate parts cannot be replaced".into());
            }
            PublicationPhase::EncodingCertificate { .. }
            | PublicationPhase::Certifying { .. }
            | PublicationPhase::Executed { .. } => {}
        }
        // A disagreement reports the original result, never a fresh execution of the same source.
        if live.result != qc.result {
            return Ok(Some(live.result));
        }
        let budget = self.state.ivm_execution_budget();
        if let PublicationPhase::Executed { valid, .. } = &live.phase {
            live.overlay
                .as_ref()
                .ok_or("original pending overlay was consumed")?
                .verify_sumeragi_execution_witness(valid.as_ref(), &live.witness)?;
        }
        live.telemetry_origin = Some(origin);
        if matches!(live.phase, PublicationPhase::Executed { .. }) {
            let PublicationPhase::Executed { valid, preimage } =
                std::mem::replace(&mut live.phase, PublicationPhase::Consuming)
            else {
                unreachable!("original executed phase")
            };
            live.phase = PublicationPhase::EncodingCertificate {
                valid,
                preimage,
                header_wire: None,
                qc_wire: None,
                availability_wire: None,
                qc: qc.clone(),
                refusal: None,
            };
        }
        if let PublicationPhase::EncodingCertificate {
            header_wire,
            qc_wire,
            availability_wire,
            refusal,
            ..
        } = &mut live.phase
        {
            for (slot, part) in [
                (
                    header_wire,
                    super::commitment::CertificatePart::Header(block.header()),
                ),
                (qc_wire, super::commitment::CertificatePart::Qc(qc)),
                (
                    availability_wire,
                    super::commitment::CertificatePart::Availability(&live.availability),
                ),
            ] {
                if slot.is_some() {
                    continue;
                }
                match encode(part, &budget) {
                    Ok(bytes) => {
                        *slot = Some(bytes);
                        *refusal = None;
                    }
                    Err(error) => {
                        let message = error.to_string();
                        if !error.is_local_refusal() {
                            self.recovery =
                                Some(format!("original certificate encoding failed: {message}"));
                        }
                        *refusal = Some(error);
                        return Err(message);
                    }
                }
            }
            let PublicationPhase::EncodingCertificate {
                valid,
                preimage,
                header_wire,
                qc_wire,
                availability_wire,
                qc,
                ..
            } = std::mem::replace(&mut live.phase, PublicationPhase::Consuming)
            else {
                unreachable!("original encoded parts")
            };
            live.phase = PublicationPhase::Certifying {
                valid,
                parts: Some(iroha_data_model::block::ChargedCertificateParts {
                    consensus_header: header_wire.expect("retained original header"),
                    commit_qc: qc_wire.expect("retained original QC"),
                    result_preimage: preimage,
                    availability: availability_wire.expect("retained original availability frame"),
                }),
                qc,
                refusal: None,
            };
        }
        let PublicationPhase::Certifying { parts, refusal, .. } = &mut live.phase else {
            unreachable!("original charged certificate parts")
        };
        let certificate = match CommitCertificate::from_charged_owner(
            parts.take().expect("same retained buffers"),
            &budget,
        ) {
            Ok(certificate) => certificate,
            Err((original, error)) => {
                let message = error.to_string();
                if !error.is_local_refusal() {
                    self.recovery = Some(format!("original certificate source changed: {message}"));
                }
                *parts = Some(original);
                *refusal = Some(error);
                return Err(message);
            }
        };
        let PublicationPhase::Certifying { valid, .. } =
            std::mem::replace(&mut live.phase, PublicationPhase::Consuming)
        else {
            unreachable!("original certificate phase")
        };
        // TODO: admit this one durable-frame allocation from the production physical pool;
        // retaining it through retries is not complete resource funding.
        let staged = Arc::new(StagedBlock {
            block_hash,
            executed: Arc::new(
                valid
                    .as_ref()
                    .clone()
                    .with_commit_certificate(Some(certificate)),
            ),
        });
        let committed = valid
            .commit_unchecked()
            .unpack(|event| live.events.push(event.into()));
        live.phase = PublicationPhase::Prepared {
            committed,
            staged: Arc::clone(&staged),
            qc: qc.clone(),
            state_events: None,
        };
        self.context.staging.stage(staged);
        Ok(Some(live.result))
    }

    /// Publish the same prepared owner after its exact frame is durable.
    fn commit(
        &mut self,
        block: &AvailableBody,
        qc: &Qc,
    ) -> Result<AppliedConfig, PublicationError> {
        self.commit_with(block, qc, StateBlock::try_publish)
    }

    // One original State boundary. Tests inject failures on either side of actual visibility
    // without replacing validation, authorization, metadata preparation or its original owner.
    fn commit_with(
        &mut self,
        block: &AvailableBody,
        qc: &Qc,
        publish: impl FnOnce(&mut StateBlock<'s>) -> crate::state::StatePublicationOutcome,
    ) -> Result<AppliedConfig, PublicationError> {
        if let Some(reason) = &self.recovery {
            return Err(PublicationError::RecoveryRequired(reason.clone()));
        }
        require_qc_witness_admission(qc, &self.state.ivm_execution_budget())?;
        match catch_unwind(AssertUnwindSafe(|| self.commit_inner(block, qc, publish))) {
            Ok(Err(error)) if self.recovery.is_some() => {
                let reason = format!("publication requires recovery: {error}");
                self.recovery = Some(reason.clone());
                Err(PublicationError::RecoveryRequired(reason))
            }
            Ok(result) => result.map_err(PublicationError::Retryable),
            Err(_) => {
                let reason = "publication panicked; recovery required".to_owned();
                self.recovery = Some(reason.clone());
                Err(PublicationError::RecoveryRequired(reason))
            }
        }
    }

    fn commit_inner(
        &mut self,
        block: &AvailableBody,
        qc: &Qc,
        publish: impl FnOnce(&mut StateBlock<'s>) -> crate::state::StatePublicationOutcome,
    ) -> Result<AppliedConfig, String> {
        if let Some(pending) = &self.pending_commit {
            if !pending.matches(block, qc) {
                return Err("another committed decision is awaiting archive capture".into());
            }
            return self.finish_commit();
        }
        let live = self
            .live
            .as_mut()
            .ok_or("commit without its prepared overlay")?;
        if live.block_hash != qc.block_hash
            || live.result != qc.result
            || live.header != *block.header()
            || live.availability != *block.availability()
            || live.source != *block.source()
        {
            return Err("commit differs from its original prepared execution".into());
        }
        match &live.phase {
            PublicationPhase::Published { qc: original, .. } if original == qc => {
                return Ok(live.applied_config.clone());
            }
            PublicationPhase::Prepared { qc: original, .. } if original == qc => {}
            _ => return Err("commit without its exact prepared certificate".into()),
        }
        let PublicationPhase::Prepared {
            committed,
            staged,
            state_events,
            ..
        } = &mut live.phase
        else {
            unreachable!("checked prepared phase")
        };
        let certificate = staged
            .executed
            .commit_certificate()
            .ok_or("prepared frame lost its certificate")?;
        if !certificate.admitted_to(&self.state.ivm_execution_budget()) {
            self.recovery = Some("prepared certificate lost its original pool custody".into());
            return Err("prepared certificate lost its original pool custody".into());
        }
        let overlay = live
            .overlay
            .as_mut()
            .ok_or("prepared publication lost its original overlay")?;
        if state_events.is_none() {
            // A normal authorization refusal retains the same original and can retry
            // after append. Metadata finalization itself is one-shot and may unwind.
            let native_execution = NativeExecutionAuthorization {
                telemetry_origin: live
                    .telemetry_origin
                    .expect("original prepared telemetry origin"),
                state: std::ptr::from_ref(self.state) as usize,
                tip: crate::state::native_execution_tip::NativeExecutionTipRecord {
                    height: live.header.height,
                    creation_time_ms: u64::try_from(
                        committed.as_ref().header().creation_time().as_millis(),
                    )
                    .expect("block creation time fits u64"),
                    iroha_hash: committed.as_ref().hash(),
                    core_hash: live.block_hash.0,
                    result: live.result.0,
                },
                parent: Some((live.header.parent_hash, live.header.parent_result)),
            };
            overlay.authorize_sumeragi_output_publication(
                committed,
                &live.witness,
                certificate,
                native_execution,
            )?;
            self.recovery =
                Some("finalizing original metadata; recovery required on failure".into());
            *state_events = Some(
                overlay
                    .apply_without_execution_with_sumeragi_commit(
                        committed,
                        certificate,
                        std::mem::take(&mut live.committee),
                    )
                    .map_err(|error| error.to_string())?,
            );
        }
        self.recovery = Some("publishing original State; recovery required on failure".into());
        match publish(overlay) {
            crate::state::StatePublicationOutcome::Published => {}
            crate::state::StatePublicationOutcome::Deferred(reason) => {
                self.recovery = None;
                return Err(reason.to_string());
            }
            crate::state::StatePublicationOutcome::RecoveryRequired(reason) => {
                return Err(reason.to_string());
            }
        }
        let state_events = state_events
            .take()
            .expect("original finalized State events");
        // Invalidate the local signing receipt before its original overlay is released.
        // A poisoned mailbox after visibility is a recovery condition, never a fresh execution.
        if let Some(custody) = &self.attestation {
            if !custody.publisher.discard(0, &[]) {
                return Err("native attestation mailbox requires recovery".into());
            }
        }
        // Retire only after the complete original State and its retained post-effects
        // have published. State's Drop releases siblings before refunds and notices.
        drop(live.overlay.take().expect("original published overlay"));
        let PublicationPhase::Prepared {
            committed,
            staged,
            qc,
            ..
        } = std::mem::replace(&mut live.phase, PublicationPhase::Consuming)
        else {
            unreachable!("original prepared owner")
        };
        self.pending_commit = Some(PendingCommit {
            telemetry_origin: live
                .telemetry_origin
                .expect("original prepared telemetry origin"),
            native_contexts: live
                .native_contexts
                .take()
                .expect("original preapply context projection"),
            header: live.header.clone(),
            availability: live.availability.clone(),
            source: live.source.clone(),
            qc: qc.clone(),
            state_hash: committed.as_ref().hash(),
            next: live.applied_config.clone(),
            hashes: committed
                .as_ref()
                .external_entrypoints_slice()
                .iter()
                .map(TransactionEntrypoint::hash)
                .collect(),
            events: std::mem::take(&mut live.events)
                .into_iter()
                .chain(state_events)
                .collect(),
        });
        live.phase = PublicationPhase::Published { staged, qc };
        self.recovery = None;
        self.finish_commit()
    }

    /// Retry durable archive capture without publishing State or notifications twice.
    fn finish_commit(&mut self) -> Result<AppliedConfig, String> {
        let pending = self
            .pending_commit
            .as_ref()
            .ok_or_else(|| "no committed decision is awaiting completion".to_owned())?;
        {
            let view = self.state.view();
            let height = u64::try_from(view.height()).map_err(|_| "State height exceeds u64")?;
            if height != pending.header.height
                || view.latest_block_hash() != Some(pending.state_hash)
            {
                return Err("State changed while committed archive capture was pending".into());
            }
            self.context
                .native_context_archive
                .publish(&pending.native_contexts)
                .map_err(|error| {
                    format!("original native context archive publication failed: {error}")
                })?;
            if let Some(archives) = &self.archives {
                archives.capture(&view)?;
            }
        }
        let pending = self
            .pending_commit
            .take()
            .ok_or_else(|| "committed completion disappeared".to_owned())?;
        let height = pending.header.height;
        self.applied = (height, pending.qc.block_hash);
        self.context
            .applied_watch
            .publish(height, pending.state_hash);
        self.results
            .retain(|_, (source, _)| source.height() > height);
        if let Some(queue) = &self.queue {
            queue.remove_committed_hashes(pending.hashes, None);
        }
        self.admit_scheduled(height.saturating_add(1));
        self.admit_scheduled(height.saturating_add(2));
        for event in pending.events {
            let _ = self.context.events.send(event);
        }
        Ok(pending.next)
    }

    /// Build a payload for `(height, view)` over the applied tip (§6.10).
    fn build(
        &mut self,
        height: u64,
        view: u64,
        max_bytes: u32,
    ) -> Result<(Option<PayloadBytes>, bool), PublicationError> {
        if let Some(reason) = &self.recovery {
            return Err(PublicationError::RecoveryRequired(reason.clone()));
        }
        if self.pending_commit.is_some()
            || self.publication_pending()
            || height != self.applied.0.saturating_add(1)
        {
            return Ok((None, false));
        }
        if self.payload_build.as_ref().is_some_and(|build| {
            (build.height, build.view, build.max_bytes) != (height, view, max_bytes)
        }) {
            self.payload_build = None;
        }
        if self.payload_build.is_some() {
            return self.finish_payload_build();
        }
        let Some(parent) = self.state.view().latest_block() else {
            return Ok((None, false));
        };
        let Some(scheduled) = self.scheduled(height) else {
            return Ok((None, false));
        };
        let boundary_attestation = height == scheduled.epoch.authorization.last_height;
        let Some(queue) = &self.queue else {
            return Ok((None, boundary_attestation));
        };
        let max_bytes = usize::try_from(max_bytes).unwrap_or(usize::MAX);
        // Certified lane blocks come first: they reserve their share of the block's capacity
        // (`specs/sumeragi_lanes.md` §4.2).
        let merges =
            match lanes::merge::propose(&self.state.view(), &*self.context.lane_blocks, height) {
                Ok(merges) => merges,
                Err(error) => {
                    let reason = format!("lane storage during payload selection: {error}");
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) {
                        return Err(PublicationError::Retryable(reason));
                    }
                    self.recovery = Some(reason.clone());
                    return Err(PublicationError::RecoveryRequired(reason));
                }
            };
        let mut selected = payload::select(
            self.state,
            queue,
            max_bytes.saturating_sub(PAYLOAD_OVERHEAD),
            merges.transactions,
        );
        // Only real work may activate the pulse signer. A pulse cannot create a block.
        if selected.is_empty() && merges.merges.is_empty() {
            return Ok((None, false));
        }
        let assembly = Assembly {
            parent: &parent,
            view,
            cadence: Duration::from_millis(scheduled.params.block_time_ms),
        };
        while !selected.is_empty() || !merges.merges.is_empty() {
            let block =
                match payload::assemble_with_merges(self.state, assembly, &selected, &merges) {
                    Ok(block) => block,
                    Err(error) => {
                        iroha_logger::warn!(height, %error, "sumeragi: payload assembly failed");
                        return Err(PublicationError::Retryable(format!(
                            "payload assembly: {error}"
                        )));
                    }
                };
            match block.resultless_proposal_wire_len() {
                Ok(length) if length <= max_bytes => {
                    let source = GlobalPayloadSource {
                        attest: boundary_attestation || attestation_required(&block),
                        hashes: selected.iter().map(|tx| tx.hash_as_entrypoint()).collect(),
                        block,
                    };
                    self.payload_build = Some(GlobalPayloadBuild {
                        height,
                        view,
                        max_bytes: max_bytes as u32,
                        job: super::driver::payload_build::PayloadBuild::new(
                            source,
                            self.state.ivm_execution_budget(),
                            max_bytes,
                        ),
                    });
                    return self.finish_payload_build();
                }
                Ok(_) if !selected.is_empty() => {
                    selected.pop();
                }
                Ok(_) => {
                    return Err(PublicationError::Retryable(
                        "mandatory lane merges exceed payload limit".into(),
                    ));
                }
                Err(error) => {
                    return Err(PublicationError::RecoveryRequired(format!(
                        "canonical payload length: {error}"
                    )));
                }
            }
        }
        Ok((None, boundary_attestation))
    }

    fn finish_payload_build(&mut self) -> Result<(Option<PayloadBytes>, bool), PublicationError> {
        let GlobalPayloadBuild {
            height,
            view,
            max_bytes,
            job,
        } = self.payload_build.take().ok_or_else(|| {
            PublicationError::RecoveryRequired("payload source disappeared".into())
        })?;
        match job.finish(
            |source| source.block.resultless_proposal_wire_len(),
            |source, writer| source.block.write_resultless_proposal_wire(writer),
        ) {
            Ok((source, payload)) => {
                self.last_built = Some((height, view, source.hashes));
                Ok((Some(payload), source.attest))
            }
            Err((job, error)) => {
                let retry = error.is_local_refusal();
                self.payload_build = Some(GlobalPayloadBuild {
                    height,
                    view,
                    max_bytes,
                    job,
                });
                let reason = format!("canonical payload admission: {error:?}");
                Err(if retry {
                    PublicationError::Retryable(reason)
                } else {
                    PublicationError::RecoveryRequired(reason)
                })
            }
        }
    }

    /// Retain queue ownership after a rejected proposal until the offending original
    /// transaction can be proved without forging a replacement native source header.
    fn reject(&mut self, height: u64, view: u64, block_hash: Hash32) {
        if self.recovery.is_some() || self.publication_pending() {
            return;
        }
        if self.quarantine_context.is_some_and(|checked| {
            (checked.height, checked.view, checked.block_hash) == (height, view, block_hash)
        }) {
            self.quarantine_context = None;
        }
        // TODO: an admission-isolation primitive must identify the exact offending transaction
        // under the retained original source before removal can be authorized. Subset execution
        // with a newly synthesized header is not the original consensus source.
    }
}

/// The iroha header of a decoded payload must match the certified core header.
fn proposal_matches_header(header: IrohaHeader, block: &AvailableBody) -> bool {
    header.height().get() == block.header().height
        && header.view_change_index() == block.header().origin_view
}

/// Whether a block requires commit attestations (§3.7, KAGEMUSHA mint finality): it carries a
/// KAGEMUSHA V1 top-up, which admission confines to single-instruction transactions.
/// The native transaction layout has no optional admission mode that can disable this seal.
///
/// The caller additionally requires a seal at every authenticated epoch boundary,
/// for each nonempty boundary block. This predicate checks only the transaction-dependent rule.
#[must_use]
pub fn attestation_required(block: &SignedBlock) -> bool {
    block.external_entrypoints_slice().iter().any(|entrypoint| {
        let TransactionEntrypoint::External(tx) = entrypoint else {
            return false;
        };
        tx.instructions()
            .explicit_instructions()
            .any(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::kagemusha_v1::TopUpKagemushaV1>()
                    .is_some()
            })
    })
}

/// Executed top-ups in a block that does not require attestations: the static rule missed a
/// top-up path, so the block cannot be finalized with mint finality.
fn top_ups_without_flag(commitment: &ExecutionResultCommitment, attest: bool) -> bool {
    commitment.execution.kagemusha_top_up_count > 0 && !attest
}

/// A deterministically invalid block at `height`, logged with its reason.
fn invalid(height: u64, reason: &dyn std::fmt::Display) -> ExecOutcome {
    iroha_logger::warn!(height, %reason, "sumeragi: block is invalid");
    ExecOutcome::Invalid
}

/// Custody allocator refusal is local; a semantic lane transition defect is deterministic.
fn classify_lane_step(height: u64, error: &lanes::step::LaneStepError) -> ExecOutcome {
    match error {
        lanes::step::LaneStepError::CustodyAllocation => ExecOutcome::Failed(error.to_string()),
        _ => invalid(height, error),
    }
}

/// Local conditions are `Failed` (retried); every other rejection is deterministic.
fn classify(height: u64, error: &BlockValidationError) -> ExecOutcome {
    local_failure(error).map_or_else(|| invalid(height, error), ExecOutcome::Failed)
}

/// The local condition behind `error`, if it is one (not a property of the block).
fn local_failure(error: &BlockValidationError) -> Option<String> {
    match error {
        BlockValidationError::LaneStorage(error) => Some(format!("lane storage: {error}")),
        BlockValidationError::StateStorageAdmission(reason) => {
            Some(format!("World storage admission: {reason}"))
        }
        BlockValidationError::EvidencePreparation(reason) => {
            Some(format!("consensus penalty preparation: {reason}"))
        }
        BlockValidationError::ExecutionDeferred(reason) => {
            Some(format!("execution deferred: {reason}"))
        }
        BlockValidationError::BlockHashAdmission(reason) => {
            Some(format!("block-hash admission: {reason}"))
        }
        BlockValidationError::MembershipAdmission(reason) => {
            Some(format!("membership admission: {reason}"))
        }
        BlockValidationError::DaIndexHydration(reason) => {
            Some(format!("DA index hydration: {reason}"))
        }
        BlockValidationError::LocalStorageRecoveryRequired { reason } => {
            Some(format!("local storage recovery: {reason}"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty input cannot obtain available custody; a genuinely available zero-transaction
    /// carrier is invalid before any execution overlay or state publication is created.
    #[test]
    fn empty_and_encoded_zero_transaction_payloads_are_invalid_without_state_work() {
        use iroha_sumeragi::{
            availability::PayloadBytes, message::ByteAdmissionError, preimage::payload_hash,
        };
        use std::collections::BTreeSet;

        publication_tests::with_worker(|chain, worker, _blocks, _events| {
            let original = publication_tests::proposal(chain, worker);
            let state_height = worker.state.view().height();
            let before = worker.state.ivm_execution_budget().reserved_bytes();
            let budget = worker.state.ivm_execution_budget();
            let empty = iroha_allocation::ChargedBuffer::new(0, &budget).unwrap();
            let (empty_owner, error) = PayloadBytes::from_charged(empty, &budget)
                .err()
                .expect("empty payload never obtains available custody");
            assert!(matches!(error, ByteAdmissionError::Length { length: 0 }));
            assert_eq!(worker.state.view().height(), state_height);
            assert!(worker.live.is_none());
            assert!(worker.finishing.is_none());
            drop(empty_owner);
            assert_eq!(worker.state.ivm_execution_budget().reserved_bytes(), before);

            let proposal = payload::decode(original.payload().as_slice()).unwrap();
            let zero_transaction_wire =
                iroha_data_model::block::builder::BlockBuilder::new(proposal.header())
                    .build(BTreeSet::new())
                    .encode_wire()
                    .unwrap();
            let mut header = original.header().clone();
            header.payload_hash = payload_hash(
                &**worker.context.crypto.as_ref().unwrap(),
                &zero_transaction_wire,
            );
            header.payload_len = u32::try_from(zero_transaction_wire.len()).unwrap();
            let block = chain.author_payload(header, zero_transaction_wire);
            let hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
            assert!(matches!(
                worker.execute(&block, hash),
                Some(ExecOutcome::Invalid)
            ));
            assert_eq!(
                worker.state.view().height(),
                state_height,
                "no synthesized block or overlay committed"
            );
            assert!(worker.live.is_none());
            assert!(worker.finishing.is_none());
            assert!(worker.context.staging.get(&hash).is_none());
        });
    }

    #[test]
    fn lane_custody_allocation_refusal_is_local_and_semantic_errors_remain_invalid() {
        use lanes::step::LaneStepError;
        assert!(matches!(
            classify_lane_step(2, &LaneStepError::CustodyAllocation),
            ExecOutcome::Failed(_)
        ));
        for error in [
            LaneStepError::Custody(lanes::step::CustodyViolation::StakeBinding),
            LaneStepError::NotAdvanced,
            LaneStepError::MissingLane(iroha_model_base::topology::LaneId::new(1)),
        ] {
            assert!(matches!(
                classify_lane_step(2, &error),
                ExecOutcome::Invalid
            ));
        }
    }

    /// Local conditions are retried (`Failed`); a property of the block is `Invalid`.
    #[test]
    fn classification_table() {
        let local = [
            BlockValidationError::LaneStorage(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "original lane read waits for capacity",
            )),
            BlockValidationError::StateStorageAdmission(
                mv::storage::AdmittedStorageError::Changed.into(),
            ),
            BlockValidationError::EvidencePreparation(
                crate::state::EvidencePreparationError::Allocator {
                    requested_bytes: 64,
                },
            ),
            BlockValidationError::ExecutionDeferred(
                ivm::error::ExecutionDeferral::ActiveMemoryCapacity.into(),
            ),
            BlockValidationError::DaIndexHydration("cold".into()),
            BlockValidationError::LocalStorageRecoveryRequired {
                reason: "disk".into(),
            },
        ];
        for error in &local {
            assert!(
                matches!(classify(2, error), ExecOutcome::Failed(_)),
                "{error:?} is local"
            );
        }
        let invalid = [
            BlockValidationError::InvalidGenesis(crate::block::InvalidGenesisError::InvalidHeader),
            BlockValidationError::ExecutionContextInvalid("bad".into()),
            BlockValidationError::HasCommittedTransactions,
        ];
        for error in &invalid {
            assert!(
                matches!(classify(2, error), ExecOutcome::Invalid),
                "{error:?} is a property of the block"
            );
        }
    }
}

#[cfg(test)]
mod archive_tests;
#[cfg(test)]
#[path = "executor_publication_tests.rs"]
mod publication_tests;

#[cfg(test)]
mod native_execution_authorization_tests {
    use super::*;

    #[test]
    fn original_execution_authorization_is_bound_to_its_actual_state() {
        use crate::{kura::Kura, query::store::LiveQueryStore, state::World};
        let original = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let foreign = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let token = NativeExecutionAuthorization {
            telemetry_origin: CommitTelemetryOrigin::Forward,
            state: std::ptr::from_ref(&original) as usize,
            tip: crate::state::native_execution_tip::NativeExecutionTipRecord {
                height: 1,
                creation_time_ms: 0,
                iroha_hash: iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
                    b"origin identity test",
                )),
                core_hash: [1; 32],
                result: [2; 32],
            },
            parent: None,
        };
        assert!(token.for_state(&original).is_ok());
        assert!(token.for_state(&foreign).is_err());
    }
}
