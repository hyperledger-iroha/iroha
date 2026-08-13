//! Serialized production height runner for the authoritative Sumeragi v2 reducer.
//!
//! This module owns exactly one reducer/effect executor at a time. It opens the
//! immutable context and safety WAL before processing network traffic, routes
//! authenticated control and body messages, schedules bounded proposal work,
//! and performs an explicit Kura-authorized rollover after application.

use std::{
    collections::BTreeSet,
    num::{NonZeroU64, NonZeroUsize},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use super::v2_core::{
    CanonicalIdentityProjection, EventTag, Generation, IDENTITY_DOMAIN_DURABLE_ARTIFACT,
    IDENTITY_KIND_FINALITY_ARTIFACT, ProductionSuccessorPredecessorBindingProjection,
    ProductionSuccessorStartupLifecycleProjection,
    ProductionTerminalApplicationWithoutSuccessorActivationProjection,
    SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP, SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP,
    SUCCESSOR_LIFECYCLE_RETRY_COMPLETE_TIP, SUCCESSOR_LIFECYCLE_SNAPSHOT_BOOTSTRAP,
    SUCCESSOR_STAGE_NONE, check_production_successor_startup_lifecycle_transition,
    check_production_terminal_application_transition,
    production_successor_predecessor_binding_kernel,
};
#[cfg(test)]
use iroha_config::parameters::actual::SUMERAGI_V2_CONFIG_FORMAT_VERSION;
use iroha_config::parameters::actual::{NodeRole, SumeragiV2Config, sumeragi_v2_timing_ms};
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    Encode as _,
    account::AccountId,
    block::{BlockHeader, SignedBlock, consensus_v2 as wire},
    events::{EventBox, pipeline::PipelineEventBox},
    peer::PeerId,
};
use thiserror::Error;

#[cfg(test)]
use super::v2_recovery::RecoveredCompleteTipActivationAuthority;

use super::{
    FairV2Ingress, FairV2IngressBarrierBypass, FairV2IngressCapacityError,
    FairV2IngressDequeueDisposition, FairV2IngressOwnershipEvidence, GenesisWithPubKey,
    InboundBlockMessage, SumeragiWorker,
    message::{BlockMessage, CanonicalExecutedBlockNeedV1},
    output_guard::{ConsensusOutputGuard, ConsensusOutputPermit},
    serviced_candidate_store::LeaderWireLifecycleStoreGate,
    v2::{
        AdapterEffect, AdapterFingerprints, DeferredAdmissionOrdinalSource, LocalProposalDirective,
        ServicedCandidateCapacityGeometry, SignRequest, SumeragiV2Adapter,
    },
    v2_apply::{
        LaneReservationReconciliationPlanning, V2ApplyService, V2ReservationLifecycleError,
        apply_lane_reservation_reconciliation_plan,
        persist_preflighted_historical_autonomous_lane_recoveries, plan_lane_reservation_ownership,
        preflight_historical_autonomous_lane_recovery,
        validate_installed_historical_autonomous_lane_recoveries,
    },
    v2_block_sync::{
        CommitCertificateAdmissionError, V2BlockSyncDiscovery, V2BlockSyncError, V2BlockSyncServer,
    },
    v2_body_store::{BlockSignaturePolicy, V2BodyStore},
    v2_candidate::{
        CandidateAssemblyOutcome, CandidateAttachments, CandidateLimits, CandidateParent,
        CandidateRequest, V2CandidateAssembler, candidate_block_has_proposal_work,
    },
    v2_chunks::{EncodedV2Payload, encode_payload},
    v2_effects::{
        EffectExecutorStep, EffectQueueConfig, EffectTransportError, PendingKuraApplyRecoveryStage,
        PostFinalityCleanupTarget, V2EffectExecutor,
        certified_body_request_is_superseded_after_decision,
        network_ingress_is_certified_fence_escape, v2_ingress_head_can_drain,
    },
    v2_lane_work::{
        AuthenticatedGenesisNexusAmxContext, CanonicalExecutedBlockRecovery, GlobalBodyLockOutcome,
        HistoricalRecoveryServiceOutcome, LaneApplicationEvidenceRepairPlanning,
        MergeSidecarDeferralDisposition, RetainedMergeSidecars, V2LaneIngressOutcome,
        V2LaneWorkAdapter, V2LaneWorkEffect, V2LaneWorkError, V2LaneWorkLimits,
        apply_lane_application_evidence_repair,
        persist_canonical_historical_recovery_payload_custody,
        plan_lane_application_evidence_repair, require_validator_storage_platform,
    },
    v2_lifecycle_coordinator::{
        CompleteTipPredecessorStorageErrorV1, RetiredRecoveredCompleteTipActivationAuthorityV1,
    },
    v2_lifecycle_recovery::{
        AutonomousLifecycleDeferredTerminalRecoveryHandoff, reconcile_autonomous_lifecycle_startup,
        reconcile_pending_autonomous_lifecycle_terminal_outcomes,
    },
    v2_npos::V2NposVrfLifecycle,
    v2_recovery::{
        DurableSuccessorActivationAuthority, DurableV2PredecessorIdentity,
        RecoveredSuccessorActivationAuthority, SnapshotSuccessorActivationAuthority,
        build_verified_successor, recover_active_height_with_plan,
        successor_block_refinement_projection, successor_context_refinement_projection,
    },
    v2_runtime::{NetworkIngressError, RuntimeQueueConfig, SerializedV2Runtime},
    v2_transport::AuthenticatedCertifiedBodyRequest,
    v2_worker::{
        CertifiedServeAdmission, CertifiedServeIngressGate, CertifiedServeNegativeOutcome,
        CertifiedServePrepareError, ExactFanoutOwnership, KuraReplicaAdvertRefreshOwner,
        ProductionV2Services, V2CleanupSupervisor, durable_exact_output_handoff_owner_pair,
    },
};
use crate::{
    kura::{AutonomousLifecycleProcessGenerationClaim, Kura, KuraV2CommitReceipt},
    merge_sidecar::{
        CertifiedMergeSidecarClosedPrefix, CertifiedMergeSidecarMessage, MergeSidecarLimits,
        MergeSigningGuardLimits,
    },
    native_amx::NativeAmxMessage,
    queue::{GlobalQueueSelectionLease, Queue},
    state::{PendingCertifiedMergeSelection, State},
};

const IDLE_POLL: Duration = Duration::from_millis(10);
const CANDIDATE_WORK_RECHECK: Duration = Duration::from_millis(100);
const PENDING_TIP_RECOVERY_DEADLINE_ROUNDS: u32 = 3;

/// Move-only authority for binding runner-owned lifecycle execution dependencies.
///
/// The future runner cutover will mint this private seal immediately before it
/// moves Queue, archive, and event ownership into recovered startup. Sumeragi
/// siblings may name the consumed type but cannot manufacture production
/// authority for caller-selected dependencies.
#[must_use = "the runner dependency permit must enter recovered lifecycle startup"]
pub(in crate::sumeragi) struct RecoveredLifecycleOwnerFactoryDependencyPermitV1 {
    _seal: RecoveredLifecycleOwnerFactoryDependencyPermitSealV1,
    local_signer: KeyPair,
}

struct RecoveredLifecycleOwnerFactoryDependencyPermitSealV1;

impl Drop for RecoveredLifecycleOwnerFactoryDependencyPermitSealV1 {
    fn drop(&mut self) {}
}

impl RecoveredLifecycleOwnerFactoryDependencyPermitV1 {
    // TODO: Mint this private permit at the atomic runner/owner cutover which
    // moves the runner's exact Queue, archives, and EventsSender into startup.
    #[cfg_attr(not(test), allow(dead_code))]
    fn mint_for_recovered_runner(local_signer: KeyPair) -> Self {
        Self {
            _seal: RecoveredLifecycleOwnerFactoryDependencyPermitSealV1,
            local_signer,
        }
    }

    #[cfg(test)]
    /// Mint the same sealed dependency permit for production-shaped unit tests.
    pub(in crate::sumeragi) fn for_test(local_signer: KeyPair) -> Self {
        Self::mint_for_recovered_runner(local_signer)
    }

    /// Consume the runner seal into its factory-owned local signer.
    pub(in crate::sumeragi) fn into_local_signer(self) -> KeyPair {
        self.local_signer
    }
}

/// Runner-private one-shot authority for activating a launched lifecycle height.
///
/// The permit retains the exact process readiness flag and fair-ingress Arc.
/// Its status authority is either the currently recovered height, an applied
/// predecessor handoff, or audited-snapshot bootstrap. CompleteTip uses the
/// separate authority below because its retired predecessor must remain joined
/// to the launched H+1 owner until this exact publication boundary.
#[must_use = "runner activation authority must be consumed by the launched lifecycle"]
pub(in crate::sumeragi) struct ProductionLifecycleRunnerActivationV1 {
    _seal: ProductionLifecycleRunnerActivationSealV1,
    ingress_ready: Arc<AtomicBool>,
    block_ingress: Arc<FairV2Ingress>,
    status: ProductionLifecycleRunnerStatusAuthorityV1,
}

struct ProductionLifecycleRunnerActivationSealV1;

impl Drop for ProductionLifecycleRunnerActivationSealV1 {
    fn drop(&mut self) {}
}

enum ProductionLifecycleRunnerStatusAuthorityV1 {
    CurrentHeight,
    Applied {
        expected_predecessor: DurableV2PredecessorIdentity,
        authority: DurableSuccessorActivationAuthority,
    },
    SnapshotBootstrap {
        authority: SnapshotSuccessorActivationAuthority,
    },
}

impl ProductionLifecycleRunnerActivationV1 {
    /// Mint the current-height activation at the future atomic runner cutover.
    #[cfg_attr(not(test), allow(dead_code))]
    fn current_height(ingress_ready: Arc<AtomicBool>, block_ingress: Arc<FairV2Ingress>) -> Self {
        Self {
            _seal: ProductionLifecycleRunnerActivationSealV1,
            ingress_ready,
            block_ingress,
            status: ProductionLifecycleRunnerStatusAuthorityV1::CurrentHeight,
        }
    }

    /// Mint an applied-predecessor successor activation without exposing parts.
    #[allow(dead_code)]
    fn applied(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
        expected_predecessor: DurableV2PredecessorIdentity,
        authority: DurableSuccessorActivationAuthority,
    ) -> Self {
        Self {
            _seal: ProductionLifecycleRunnerActivationSealV1,
            ingress_ready,
            block_ingress,
            status: ProductionLifecycleRunnerStatusAuthorityV1::Applied {
                expected_predecessor,
                authority,
            },
        }
    }

    /// Mint an audited-snapshot successor activation without exposing parts.
    #[allow(dead_code)]
    fn snapshot_bootstrap(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
        authority: SnapshotSuccessorActivationAuthority,
    ) -> Self {
        Self {
            _seal: ProductionLifecycleRunnerActivationSealV1,
            ingress_ready,
            block_ingress,
            status: ProductionLifecycleRunnerStatusAuthorityV1::SnapshotBootstrap { authority },
        }
    }

    /// Open the exact retained ingress, publish status, then release readiness.
    pub(in crate::sumeragi) fn open_and_publish(
        self,
        launched_ingress: &Arc<FairV2Ingress>,
        successor: wire::SumeragiV2Status,
    ) -> Result<ProductionLifecycleActivatedRunnerAuthorityV1, V2RunnerError> {
        self.ingress_ready.store(false, Ordering::Release);
        if !Arc::ptr_eq(&self.block_ingress, launched_ingress) {
            self.block_ingress.close();
            return Err(V2RunnerError::LifecycleActivationIngressMismatch);
        }
        self.block_ingress.open().map_err(ingress_capacity_error)?;
        let publication = match self.status {
            ProductionLifecycleRunnerStatusAuthorityV1::CurrentHeight => {
                super::status::set_v2_status(successor);
                Ok(())
            }
            ProductionLifecycleRunnerStatusAuthorityV1::Applied {
                expected_predecessor,
                authority,
            } => super::status::activate_v2_successor_height(
                expected_predecessor,
                authority,
                successor,
            )
            .map_err(V2RunnerError::from),
            ProductionLifecycleRunnerStatusAuthorityV1::SnapshotBootstrap { authority } => {
                super::status::activate_snapshot_bootstrap_v2_height(authority, successor)
                    .map_err(V2RunnerError::from)
            }
        };
        if let Err(error) = publication {
            self.block_ingress.close();
            return Err(error);
        }
        self.ingress_ready.store(true, Ordering::Release);
        Ok(ProductionLifecycleActivatedRunnerAuthorityV1 {
            _seal: ProductionLifecycleActivatedRunnerAuthoritySealV1,
            ingress_ready: self.ingress_ready,
            block_ingress: self.block_ingress,
        })
    }

    #[cfg(test)]
    pub(in crate::sumeragi) fn current_height_for_test(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
    ) -> Self {
        Self::current_height(ingress_ready, block_ingress)
    }
}

/// Runner-private activation half for an exact launched CompleteTip successor.
#[must_use = "CompleteTip runner activation must consume its launched retirement join"]
pub(in crate::sumeragi) struct ProductionLifecycleCompleteTipRunnerActivationV1 {
    _seal: ProductionLifecycleCompleteTipRunnerActivationSealV1,
    ingress_ready: Arc<AtomicBool>,
    block_ingress: Arc<FairV2Ingress>,
}

struct ProductionLifecycleCompleteTipRunnerActivationSealV1;

impl Drop for ProductionLifecycleCompleteTipRunnerActivationSealV1 {
    fn drop(&mut self) {}
}

impl ProductionLifecycleCompleteTipRunnerActivationV1 {
    /// Mint only at the future branch which binds retired H to launched H+1.
    #[cfg_attr(not(test), allow(dead_code))]
    fn mint_for_recovered_runner(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
    ) -> Self {
        Self {
            _seal: ProductionLifecycleCompleteTipRunnerActivationSealV1,
            ingress_ready,
            block_ingress,
        }
    }

    /// Publish only through the still-sealed retired CompleteTip authority.
    pub(in crate::sumeragi) fn open_and_publish(
        self,
        launched_ingress: &Arc<FairV2Ingress>,
        retirement: RetiredRecoveredCompleteTipActivationAuthorityV1,
        successor: wire::SumeragiV2Status,
    ) -> Result<ProductionLifecycleActivatedRunnerAuthorityV1, V2RunnerError> {
        self.ingress_ready.store(false, Ordering::Release);
        if !Arc::ptr_eq(&self.block_ingress, launched_ingress) {
            self.block_ingress.close();
            return Err(V2RunnerError::LifecycleActivationIngressMismatch);
        }
        if !retirement.authorizes_successor_status(&successor) {
            self.block_ingress.close();
            return Err(V2RunnerError::CompleteTipSuccessorAuthorityInvalid {
                predecessor: retirement.predecessor(),
            });
        }
        self.block_ingress.open().map_err(ingress_capacity_error)?;
        if let Err(error) =
            super::status::activate_recovered_complete_tip_v2_height(retirement, successor)
        {
            self.block_ingress.close();
            return Err(error.into());
        }
        self.ingress_ready.store(true, Ordering::Release);
        Ok(ProductionLifecycleActivatedRunnerAuthorityV1 {
            _seal: ProductionLifecycleActivatedRunnerAuthoritySealV1,
            ingress_ready: self.ingress_ready,
            block_ingress: self.block_ingress,
        })
    }

    #[cfg(test)]
    fn for_test(ingress_ready: Arc<AtomicBool>, block_ingress: Arc<FairV2Ingress>) -> Self {
        Self::mint_for_recovered_runner(ingress_ready, block_ingress)
    }
}

/// Move-only post-activation ownership of runner readiness and exact ingress.
///
/// The activated lifecycle stack retains this authority until finalization.
/// Dropping it first clears readiness and closes ingress, so the later durable
/// gate teardown cannot leave a carrierless queue advertised as live.
#[must_use = "activated runner authority must remain with the lifecycle height"]
pub(in crate::sumeragi) struct ProductionLifecycleActivatedRunnerAuthorityV1 {
    _seal: ProductionLifecycleActivatedRunnerAuthoritySealV1,
    ingress_ready: Arc<AtomicBool>,
    block_ingress: Arc<FairV2Ingress>,
}

struct ProductionLifecycleActivatedRunnerAuthoritySealV1;

impl Drop for ProductionLifecycleActivatedRunnerAuthoritySealV1 {
    fn drop(&mut self) {}
}

impl ProductionLifecycleActivatedRunnerAuthorityV1 {
    /// Consume the exact readiness owner before lifecycle gate retirement.
    #[allow(dead_code)]
    pub(in crate::sumeragi) fn retire(
        self,
        launched_ingress: &Arc<FairV2Ingress>,
    ) -> Result<(), V2RunnerError> {
        self.ingress_ready.store(false, Ordering::Release);
        self.block_ingress.close();
        if !Arc::ptr_eq(&self.block_ingress, launched_ingress) {
            return Err(V2RunnerError::LifecycleActivationIngressMismatch);
        }
        Ok(())
    }
}

impl Drop for ProductionLifecycleActivatedRunnerAuthorityV1 {
    fn drop(&mut self) {
        self.ingress_ready.store(false, Ordering::Release);
        self.block_ingress.close();
    }
}

/// Process-local borrow key for driving an activated lifecycle stack.
///
/// Only the serialized runner can mint this key. Repeated mutable borrows keep
/// owner, executor, and services inside the activated type state and cannot
/// move any of them into a shadow scheduler.
#[must_use = "the active runner borrow key must remain with the height loop"]
pub(in crate::sumeragi) struct ProductionLifecycleActiveRunnerBorrowV1 {
    _seal: ProductionLifecycleActiveRunnerBorrowSealV1,
}

struct ProductionLifecycleActiveRunnerBorrowSealV1;

impl Drop for ProductionLifecycleActiveRunnerBorrowSealV1 {
    fn drop(&mut self) {}
}

impl ProductionLifecycleActiveRunnerBorrowV1 {
    /// Mint beside the activated owner at the future atomic runner cutover.
    #[allow(dead_code)]
    fn mint_for_recovered_runner() -> Self {
        Self {
            _seal: ProductionLifecycleActiveRunnerBorrowSealV1,
        }
    }

    /// Mint the same opaque runner borrow for a production-shaped lifecycle test.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test() -> Self {
        Self::mint_for_recovered_runner()
    }
}

/// Cadence-derived process-local deadline for closed-ingress interrupted-tip recovery.
#[derive(Clone, Copy, Debug)]
struct PendingTipRecoveryDeadline {
    started_at: Instant,
    deadline: Instant,
    timeout: Duration,
}

impl PendingTipRecoveryDeadline {
    fn new(started_at: Instant, round_timeout: Duration) -> Result<Self, V2RunnerError> {
        let timeout = round_timeout
            .checked_mul(PENDING_TIP_RECOVERY_DEADLINE_ROUNDS)
            .ok_or(V2RunnerError::InvalidLimits)?;
        let deadline = started_at
            .checked_add(timeout)
            .ok_or(V2RunnerError::InvalidLimits)?;
        Ok(Self {
            started_at,
            deadline,
            timeout,
        })
    }

    fn expired(self, now: Instant) -> bool {
        now >= self.deadline
    }

    fn remaining(self, now: Instant) -> Duration {
        self.deadline.saturating_duration_since(now)
    }

    fn elapsed(self, now: Instant) -> Duration {
        now.saturating_duration_since(self.started_at)
    }
}

/// Exact reducer facts which own one local proposal-side work item.
///
/// A higher PrepareQC can replace the lock without changing [`EventTag`].
/// Tagging local work by the runtime incarnation alone would therefore let a
/// delayed rejection or preparation completion for the old subject mutate the
/// new lock's scheduling state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LocalProposalOwner {
    tag: EventTag,
    locked_body: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    decided_subject: Option<wire::BlockSubject>,
}

impl From<LocalProposalDirective> for LocalProposalOwner {
    fn from(directive: LocalProposalDirective) -> Self {
        Self {
            tag: directive.tag(),
            locked_body: directive.locked_body(),
            decided_subject: directive.decided_subject(),
        }
    }
}

impl LocalProposalOwner {
    /// Return whether this owner installs the first exact lock for prior
    /// unlocked proposal work from the same reducer incarnation.
    fn installs_first_exact_lock_for(self, prior: Self, subject: wire::BlockSubject) -> bool {
        prior.tag == self.tag
            && prior.decided_subject == self.decided_subject
            && prior.locked_body.is_none()
            && self.locked_body.is_some_and(|(round, locked_subject)| {
                round.height == self.tag.height()
                    && round.view == self.tag.view()
                    && locked_subject == subject
            })
    }
}

#[derive(Debug)]
struct PendingLocalEvents {
    owner: LocalProposalOwner,
    subject: wire::BlockSubject,
    events: Vec<PipelineEventBox>,
}

/// Queue ownership retained until the exact local proposal is decided or abandoned.
///
/// The ordinary transaction remains in the queue while this process-local fence prevents the
/// autonomous lane producer from reserving the same hash. The fence follows only the first exact
/// lock for this proposal subject and is released on every replacement, rejection, or decision.
#[derive(Debug)]
struct PendingGlobalSelection {
    owner: LocalProposalOwner,
    subject: wire::BlockSubject,
    _lease: GlobalQueueSelectionLease,
}

#[derive(Clone, Copy, Debug)]
struct CandidateWorkWait {
    owner: LocalProposalOwner,
    started_at: Instant,
    next_retry: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReplayedProposalSign {
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
}

/// Fallible construction ownership of an applied predecessor's successor.
///
/// Starting construction changes the predecessor's durable diagnostic witness
/// from `Queued` to `Running`. Only a successfully verified successor context
/// can bind this token into [`PendingSuccessorActivation`].
#[derive(Debug)]
struct PendingSuccessorConstruction {
    predecessor: DurableV2PredecessorIdentity,
}

impl PendingSuccessorConstruction {
    fn begin(predecessor: DurableV2PredecessorIdentity) -> Result<Self, V2RunnerError> {
        super::status::begin_v2_successor_activation(predecessor)?;
        Ok(Self { predecessor })
    }

    fn bind(
        self,
        authority: DurableSuccessorActivationAuthority,
    ) -> Result<PendingSuccessorActivation, V2RunnerError> {
        let binding = ProductionSuccessorPredecessorBindingProjection {
            expected_predecessor: self.predecessor.refinement_projection(),
            authority_predecessor: authority.predecessor().refinement_projection(),
            successor_context_id: super::v2_recovery::successor_context_refinement_projection(
                authority.successor_context_id(),
            ),
        };
        if !production_successor_predecessor_binding_kernel(binding) {
            return Err(V2RunnerError::SuccessorPredecessorAuthorityMismatch {
                expected: self.predecessor,
                actual: authority.predecessor(),
            });
        }
        Ok(PendingSuccessorActivation::Applied {
            expected_predecessor: self.predecessor,
            authority,
        })
    }
}

/// One-shot ownership of an authenticated successor's activation handoff.
///
/// Construction failure simply drops this token, leaving the predecessor's
/// `Running` work stage visible. The outer runner failure guard then closes
/// output and requires restart; only [`Self::publish`] can claim activation.
#[derive(Debug)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum PendingSuccessorActivation {
    /// Uninterrupted rollover whose published Applied predecessor owns the
    /// Running handoff.
    Applied {
        expected_predecessor: DurableV2PredecessorIdentity,
        authority: DurableSuccessorActivationAuthority,
    },
    /// Process restart after recovery authenticated an exact complete durable
    /// tip; the process-local predecessor registry was intentionally cleared.
    RecoveredCompleteTip {
        authority: RetiredRecoveredCompleteTipActivationAuthorityV1,
    },
    /// First executable height derived from an authenticated audited snapshot.
    /// This carries no historical CommitQC or Kura finality receipt.
    SnapshotBootstrap {
        authority: SnapshotSuccessorActivationAuthority,
    },
}

impl PendingSuccessorActivation {
    fn recovered(
        authority: RecoveredSuccessorActivationAuthority,
        local_signer: &KeyPair,
    ) -> Result<Self, V2RunnerError> {
        let (transition, authority_kind, status_height) = match &authority {
            RecoveredSuccessorActivationAuthority::CompleteTip(authority) => (
                SUCCESSOR_LIFECYCLE_RETRY_COMPLETE_TIP,
                SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP,
                authority.predecessor().height(),
            ),
            RecoveredSuccessorActivationAuthority::SnapshotBootstrap(authority) => (
                SUCCESSOR_LIFECYCLE_SNAPSHOT_BOOTSTRAP,
                SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP,
                authority.snapshot_anchor_height(),
            ),
        };
        let published_height = super::status::v2_status().map_or(0, |status| status.height);
        let lifecycle = ProductionSuccessorStartupLifecycleProjection {
            transition_kind: transition,
            authority_kind,
            status_height,
            stage_before: SUCCESSOR_STAGE_NONE,
            stage_after: SUCCESSOR_STAGE_NONE,
            published_height_before: published_height,
            published_height_after: published_height,
            restart_required_before: false,
            restart_required_after: false,
        };
        let Some(checked_lifecycle) =
            check_production_successor_startup_lifecycle_transition(lifecycle)
        else {
            return Err(V2RunnerError::SuccessorRefinementRejected);
        };
        let _authorized_lifecycle = checked_lifecycle.into_projection();
        Ok(match authority {
            RecoveredSuccessorActivationAuthority::CompleteTip(authority) => {
                let expected_predecessor = authority.predecessor();
                // TODO: The generic sealed owner/launch cutover must make every
                // live height use this canonical lifecycle target. This
                // restart-only bridge authenticates and consumes the same
                // target without claiming that broader replacement complete.
                let retired = authority
                    .into_canonical_predecessor_storage(local_signer)?
                    .retire()?;
                if retired.predecessor() != expected_predecessor {
                    return Err(V2RunnerError::SuccessorPredecessorAuthorityMismatch {
                        expected: expected_predecessor,
                        actual: retired.predecessor(),
                    });
                }
                Self::RecoveredCompleteTip { authority: retired }
            }
            RecoveredSuccessorActivationAuthority::SnapshotBootstrap(authority) => {
                Self::SnapshotBootstrap { authority }
            }
        })
    }

    /// Reauthenticate retained restart storage before constructing live H+1 services.
    fn preflight_recovered_startup(&self) -> Result<(), V2RunnerError> {
        match self {
            Self::RecoveredCompleteTip { authority }
                if !authority.authorizes_retained_successor() =>
            {
                Err(V2RunnerError::CompleteTipSuccessorAuthorityInvalid {
                    predecessor: authority.predecessor(),
                })
            }
            Self::Applied { .. }
            | Self::RecoveredCompleteTip { .. }
            | Self::SnapshotBootstrap { .. } => Ok(()),
        }
    }

    /// Bind the prepared status to its retained restart authority before ingress opens.
    fn preflight_ingress_open(
        &self,
        successor: &wire::SumeragiV2Status,
    ) -> Result<(), V2RunnerError> {
        match self {
            Self::RecoveredCompleteTip { authority }
                if !authority.authorizes_successor_status(successor) =>
            {
                Err(V2RunnerError::CompleteTipSuccessorAuthorityInvalid {
                    predecessor: authority.predecessor(),
                })
            }
            Self::Applied { .. }
            | Self::RecoveredCompleteTip { .. }
            | Self::SnapshotBootstrap { .. } => Ok(()),
        }
    }

    fn publish(self, successor: wire::SumeragiV2Status) -> Result<(), V2RunnerError> {
        match self {
            Self::Applied {
                expected_predecessor,
                authority,
            } => {
                super::status::activate_v2_successor_height(
                    expected_predecessor,
                    authority,
                    successor,
                )?;
            }
            Self::RecoveredCompleteTip { authority } => {
                super::status::activate_recovered_complete_tip_v2_height(authority, successor)?;
            }
            Self::SnapshotBootstrap { authority } => {
                super::status::activate_snapshot_bootstrap_v2_height(authority, successor)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocalValidationDisposition {
    Ignored,
    RetryNonEmpty,
    FatalNonEmpty,
}

#[derive(Default, Debug)]
struct LocalProposalState {
    attempted: Option<LocalProposalOwner>,
    submitted: Option<(LocalProposalOwner, wire::BlockSubject)>,
    non_empty_retry: Option<LocalProposalOwner>,
    candidate_work_wait: Option<CandidateWorkWait>,
    pending_events: Option<PendingLocalEvents>,
    global_selection: Option<PendingGlobalSelection>,
}

impl LocalProposalState {
    fn from_replayed_proposal(
        replayed: Option<ReplayedProposalSign>,
        current: LocalProposalDirective,
    ) -> Self {
        let owner = LocalProposalOwner::from(current);
        let replayed_owns_current = replayed.is_some_and(|replayed| {
            replayed.tag == owner.tag
                && replayed.round.height == replayed.tag.height()
                && replayed.round.view == replayed.tag.view()
                && owner.decided_subject.is_none()
                && owner
                    .locked_body
                    .is_none_or(|(locked_round, locked_subject)| {
                        replayed.round.context_id == locked_round.context_id
                            && replayed.round.height == locked_round.height
                            && replayed.subject == locked_subject
                    })
        });
        Self {
            attempted: replayed_owns_current.then_some(owner),
            ..Self::default()
        }
    }

    /// Retire every volatile item which is not owned by the exact current
    /// lock/decision snapshot. A Decision owns no further proposal work.
    fn reconcile(&mut self, owner: LocalProposalOwner) -> LocalProposalOwner {
        if owner.decided_subject.is_some() {
            *self = Self::default();
            return owner;
        }
        if let Some((candidate, subject)) = self.submitted
            && candidate != owner
        {
            if owner.installs_first_exact_lock_for(candidate, subject) {
                self.submitted = Some((owner, subject));
            } else {
                self.submitted = None;
            }
        }
        if self
            .pending_events
            .as_ref()
            .is_some_and(|pending| pending.owner != owner)
        {
            let preserve = self.pending_events.as_ref().is_some_and(|pending| {
                owner.installs_first_exact_lock_for(pending.owner, pending.subject)
            });
            if preserve {
                self.pending_events
                    .as_mut()
                    .expect("pending events were observed above")
                    .owner = owner;
            } else {
                self.pending_events = None;
            }
        }
        if self
            .global_selection
            .as_ref()
            .is_some_and(|selection| selection.owner != owner)
        {
            let preserve = self.global_selection.as_ref().is_some_and(|selection| {
                owner.installs_first_exact_lock_for(selection.owner, selection.subject)
            });
            if preserve {
                self.global_selection
                    .as_mut()
                    .expect("global selection was observed above")
                    .owner = owner;
            } else {
                self.global_selection = None;
            }
        }
        let continued_exact_work = self
            .submitted
            .is_some_and(|(candidate, _)| candidate == owner)
            || self
                .pending_events
                .as_ref()
                .is_some_and(|pending| pending.owner == owner)
            || self
                .global_selection
                .as_ref()
                .is_some_and(|selection| selection.owner == owner);
        if self.attempted.is_some_and(|candidate| candidate != owner) {
            self.attempted = continued_exact_work.then_some(owner);
        }
        if self
            .non_empty_retry
            .is_some_and(|candidate| candidate != owner)
        {
            self.non_empty_retry = None;
        }
        if self
            .candidate_work_wait
            .is_some_and(|wait| wait.owner != owner)
        {
            self.candidate_work_wait = None;
        }
        owner
    }

    /// Keep retrying deferred autonomous work for the bounded observation
    /// window, then arm one ordinary non-empty recovery retry for this owner.
    ///
    /// Expiry deliberately changes only proposal shape. It never changes the
    /// lane adapter's routing or queue-ownership policy.
    fn defer_candidate_work(
        &mut self,
        owner: LocalProposalOwner,
        now: Instant,
        wait_bound: Duration,
    ) {
        if self.non_empty_retry == Some(owner) {
            self.candidate_work_wait = None;
            return;
        }
        let started_at = self
            .candidate_work_wait
            .filter(|wait| wait.owner == owner)
            .map_or(now, |wait| wait.started_at);
        if now.saturating_duration_since(started_at) < wait_bound {
            self.candidate_work_wait = Some(CandidateWorkWait {
                owner,
                started_at,
                next_retry: deadline_after(now, CANDIDATE_WORK_RECHECK),
            });
            return;
        }
        self.non_empty_retry = Some(owner);
        self.candidate_work_wait = None;
    }

    /// Retire an armed recovery retry which completed assembly without finding
    /// any publishable work. A later retry must cross a fresh bounded
    /// observation window instead of re-running full assembly every runner
    /// poll.
    fn retire_unsubmitted_non_empty_retry(&mut self, owner: LocalProposalOwner) -> bool {
        let owner = self.reconcile(owner);
        if self.non_empty_retry != Some(owner) {
            return false;
        }
        self.non_empty_retry = None;
        self.candidate_work_wait = None;
        true
    }

    fn handle_validation_rejection(
        &mut self,
        owner: LocalProposalOwner,
        expected_round: wire::ConsensusRound,
        rejected_round: wire::ConsensusRound,
        rejected_subject: wire::BlockSubject,
    ) -> LocalValidationDisposition {
        let owner = self.reconcile(owner);
        if expected_round != rejected_round || self.submitted != Some((owner, rejected_subject)) {
            return LocalValidationDisposition::Ignored;
        }
        if self
            .pending_events
            .as_ref()
            .is_some_and(|pending| pending.owner == owner && pending.subject == rejected_subject)
        {
            self.pending_events = None;
        }
        if self.global_selection.as_ref().is_some_and(|selection| {
            selection.owner == owner && selection.subject == rejected_subject
        }) {
            self.global_selection = None;
        }
        if self.non_empty_retry == Some(owner) {
            return LocalValidationDisposition::FatalNonEmpty;
        }
        self.attempted = None;
        self.non_empty_retry = Some(owner);
        self.submitted = None;
        self.candidate_work_wait = None;
        LocalValidationDisposition::RetryNonEmpty
    }

    /// Abandon a candidate whose lane-local ownership could not be bound
    /// before the body was submitted, then give the exact owner one ordinary
    /// non-empty retry. The caller releases the candidate's selection lease
    /// before crossing this boundary, without publishing a body or events.
    fn handle_candidate_binding_rejection(
        &mut self,
        owner: LocalProposalOwner,
    ) -> LocalValidationDisposition {
        let owner = self.reconcile(owner);
        if self.non_empty_retry == Some(owner) {
            return LocalValidationDisposition::FatalNonEmpty;
        }
        self.attempted = None;
        self.non_empty_retry = Some(owner);
        self.candidate_work_wait = None;
        LocalValidationDisposition::RetryNonEmpty
    }

    fn take_prepared_events(
        &mut self,
        owner: LocalProposalOwner,
        prepared_tag: EventTag,
        prepared_subject: wire::BlockSubject,
    ) -> Option<Vec<PipelineEventBox>> {
        let owner = self.reconcile(owner);
        let matches = self.pending_events.as_ref().is_some_and(|pending| {
            pending.owner == owner
                && pending.owner.tag == prepared_tag
                && pending.subject == prepared_subject
        });
        matches.then(|| {
            self.pending_events
                .take()
                .expect("matching pending events were observed above")
                .events
        })
    }
}

/// Run the v2-only worker until shutdown or a fail-closed error.
pub(super) fn run(worker: SumeragiWorker) {
    let mut status_clear = V2StatusClearGuard::new();
    let ingress_ready = Arc::clone(&worker.ingress_ready);
    let block_ingress = Arc::clone(&worker.block_rx);
    let output_guard = Arc::clone(&worker.output_guard);
    let _ingress_clear = V2IngressClearGuard::new(Arc::clone(&ingress_ready), block_ingress);
    // Declared after ingress cleanup so reverse-order unwinding closes the
    // process output gate before readiness state is released.
    let mut failure_guard = V2RunnerFailureGuard::new(Arc::clone(&output_guard));
    match run_inner(worker) {
        Ok(()) => {
            failure_guard.disarm();
            status_clear.clear_on_drop();
        }
        Err(error) => {
            output_guard.activate_restart_required();
            super::status::mark_v2_restart_required();
            iroha_logger::error!(%error, "authoritative Sumeragi v2 runner stopped fail-closed");
        }
    }
    ingress_ready.store(false, Ordering::Release);
}

/// Latch process-lifetime restart recovery when the runner exits abnormally.
///
/// In particular, this guard covers panics before production services exist;
/// those services therefore cannot be relied upon to poison the shared guard
/// during their own abnormal drop.
struct V2RunnerFailureGuard {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}

impl V2RunnerFailureGuard {
    fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for V2RunnerFailureGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.output_guard.close_admission_for_restart();
        if !std::thread::panicking() {
            self.output_guard.activate_restart_required();
        }
    }
}

struct V2IngressClearGuard {
    ingress_ready: Arc<AtomicBool>,
    block_ingress: Arc<FairV2Ingress>,
}

impl V2IngressClearGuard {
    fn new(ingress_ready: Arc<AtomicBool>, block_ingress: Arc<FairV2Ingress>) -> Self {
        ingress_ready.store(false, Ordering::Release);
        block_ingress.close();
        Self {
            ingress_ready,
            block_ingress,
        }
    }
}

impl Drop for V2IngressClearGuard {
    fn drop(&mut self) {
        self.ingress_ready.store(false, Ordering::Release);
        self.block_ingress.close();
    }
}

struct CertifiedServeIngressBinding {
    ingress_ready: Arc<AtomicBool>,
    block_ingress: Arc<FairV2Ingress>,
    gate: Option<CertifiedServeIngressGate>,
}

impl CertifiedServeIngressBinding {
    fn bind(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
        gate: CertifiedServeIngressGate,
    ) -> Result<Self, V2RunnerError> {
        block_ingress
            .bind_certified_serve_gate(gate.clone())
            .map_err(V2RunnerError::Service)?;
        Ok(Self {
            ingress_ready,
            block_ingress,
            gate: Some(gate),
        })
    }

    fn retire(&mut self) -> Result<(), V2RunnerError> {
        let Some(gate) = self.gate.as_ref() else {
            return Ok(());
        };
        close_ingress_for_rollover(&self.ingress_ready, &self.block_ingress);
        self.block_ingress
            .unbind_certified_serve_gate(gate)
            .map_err(V2RunnerError::Service)?;
        self.gate = None;
        Ok(())
    }
}

impl Drop for CertifiedServeIngressBinding {
    fn drop(&mut self) {
        if let Err(error) = self.retire() {
            iroha_logger::error!(
                %error,
                "failed to retire the per-height certified Serve ingress gate"
            );
        }
    }
}

/// Per-height binding of durable generic leader-wire ownership to fair ingress.
struct LeaderWireIngressBinding {
    ingress_ready: Arc<AtomicBool>,
    block_ingress: Arc<FairV2Ingress>,
    gate: Option<Arc<LeaderWireLifecycleStoreGate>>,
}

impl LeaderWireIngressBinding {
    fn bind(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
        gate: Arc<LeaderWireLifecycleStoreGate>,
        restore: super::serviced_candidate_store::LeaderWireLifecycleRestore,
        lifecycle_ordinals: super::v2_runtime::RuntimeLifecycleOrdinalSource,
        context_id: wire::HeightContextId,
        height: wire::Height,
    ) -> Result<Self, V2RunnerError> {
        block_ingress
            .bind_leader_wire_lifecycle_gate(
                Arc::clone(&gate),
                restore,
                lifecycle_ordinals,
                context_id,
                height,
            )
            .map_err(V2RunnerError::Service)?;
        Ok(Self {
            ingress_ready,
            block_ingress,
            gate: Some(gate),
        })
    }

    fn retire(&mut self) -> Result<(), V2RunnerError> {
        let Some(gate) = self.gate.as_ref() else {
            return Ok(());
        };
        close_ingress_for_rollover(&self.ingress_ready, &self.block_ingress);
        self.block_ingress
            .unbind_leader_wire_lifecycle_gate(gate)
            .map_err(V2RunnerError::Service)?;
        self.gate = None;
        Ok(())
    }
}

impl Drop for LeaderWireIngressBinding {
    fn drop(&mut self) {
        if let Err(error) = self.retire() {
            iroha_logger::error!(
                %error,
                "failed to retire the per-height durable leader-wire lifecycle gate"
            );
        }
    }
}

include!("v2_runner/height_ingress_bindings.rs");
include!("v2_runner/lifecycle_terminal_recovery.rs");

#[allow(clippy::too_many_lines)]
fn run_inner(worker: SumeragiWorker) -> Result<(), V2RunnerError> {
    let SumeragiWorker {
        config,
        common_config,
        events_sender,
        state,
        queue,
        kura,
        provider_ingest_finalized_archive,
        reputation_finalized_archive,
        startup_replay_plan,
        mut startup_replay_inventory_guard,
        network,
        genesis_network,
        block_rx,
        lane_relay_rx,
        wake_rx,
        shutdown_signal,
        ingress_ready,
        output_guard,
        consensus_frame_byte_capacity,
        block_sync_frame_byte_capacity,
    } = worker;

    // Reject an unsupported voting host before any recovery or durable
    // consensus constructor can touch validator storage. Observers remain
    // available for sync and query service on other platforms.
    require_validator_storage_platform(
        config.role == NodeRole::Validator,
        crate::kura::sumeragi_v2_validator_storage_supported(),
    )?;

    let GenesisWithPubKey {
        genesis,
        public_key: genesis_public_key,
        block_cadence,
        v2_bootstrap,
    } = genesis_network;
    let genesis_body = genesis.map(|block| block.0);
    let recovery = output_guard
        .begin_fail_stop_operation()
        .ok_or(V2RunnerError::RestartRequired)?;
    let recovered = recover_active_height_with_plan(
        kura.as_ref(),
        state.as_ref(),
        v2_bootstrap,
        genesis_public_key.clone(),
        startup_replay_plan,
    )?;
    startup_replay_inventory_guard.finish();
    recovery.complete();
    let mut pending_kura_apply = recovered.pending_kura_apply();
    let (
        mut verified_context,
        context_store,
        mut signature_policy,
        _lifecycle_storage_authority,
        _authenticated_genesis,
        recovered_successor_activation,
        mut staged_genesis_nexus_amx_context,
    ) = recovered.into_parts();
    let local_peer = common_config.peer.id().clone();
    // Height-local roster membership is read-only and must precede the first
    // lifecycle mutation. It controls global duty for this frozen height, not
    // the immutable process capability: a configured validator may be absent
    // now and later rotate into a global roster, or still serve an independently
    // frozen lane descriptor which names its key.
    let _initial_local_validator =
        local_validator_index(verified_context.context(), &local_peer, config.role)?;
    // Claim the process generation before constructing any height-local lane
    // service.  The claim is rooted in the authenticated recovered chain
    // context and the immutable local Kura identity; later startup lifecycle
    // reconciliation consumes this same process-lifetime claim rather than
    // allowing each height adapter to invent a generation.
    let _lifecycle_process_generation = claim_runner_lifecycle_process_generation(
        config.role,
        kura.as_ref(),
        verified_context.context(),
        &local_peer,
    )?;
    // A process which recovered durable v2 height ownership may be behind its
    // peers. Probe that exact active context immediately, then retain eager
    // discovery only while an authenticated discovered CommitQC acquires or
    // coalesces with serialized reducer ownership. Ordinary live finality
    // clears the hint, so this does not add permanent all-to-all traffic.
    let mut eager_block_sync =
        recovered_successor_activation.is_some() || pending_kura_apply.is_some();
    // Reconcile exactly once, after an interrupted canonical tip (if any) has
    // completed State application. Running before that boundary could mistake
    // the tip's not-yet-published membership for a losing lane proposal.
    let mut reservation_reconciliation_pending = true;
    let genesis_account = AccountId::new(genesis_public_key);
    let mut first_height_genesis = genesis_body;
    let mut block_sync_server = None;
    // The first-release cadence comes from authenticated signed-genesis or
    // snapshot startup metadata and remains immutable for this process.
    // In particular, fresh startup cannot read the uncommitted base State here:
    // its placeholder cadence predates execution of signed genesis.
    let block_cadence_ms = u64::try_from(block_cadence.as_millis())?;
    let (round_timeout_ms, retransmit_interval_ms) = sumeragi_v2_timing_ms(block_cadence_ms)?;
    let round_timeout = Duration::from_millis(round_timeout_ms);
    let retransmit_interval = Duration::from_millis(retransmit_interval_ms);
    validate_deadline_duration(round_timeout)?;
    validate_deadline_duration(retransmit_interval)?;
    let mut cleanup_supervisor = V2CleanupSupervisor::default();
    let recovered_activation_guard = recovered_successor_activation
        .as_ref()
        .map(|_| {
            output_guard
                .begin_fail_stop_operation()
                .ok_or(V2RunnerError::RestartRequired)
        })
        .transpose()?;
    let mut pending_successor_activation = recovered_successor_activation
        .map(|authority| PendingSuccessorActivation::recovered(authority, &common_config.key_pair))
        .transpose()?;
    if let Some(activation) = pending_successor_activation.as_ref() {
        // CompleteTip has already retired H and authenticated H+1 here. Reopen
        // that exact retained successor frame before any H+1 adapter, worker,
        // clock, or output construction; final status binding is repeated at
        // the ingress-publication boundary below.
        activation.preflight_recovered_startup()?;
    }
    if let Some(guard) = recovered_activation_guard {
        guard.complete();
    }
    let mut liveness_watchdog = super::status::V2LivenessWatchdog::default();
    let deferred_admission_ordinals = DeferredAdmissionOrdinalSource::new(0);
    let mut retained_merge_sidecars: Option<RetainedMergeSidecars> = None;
    let kura_replica_advert_refresh = Arc::new(
        KuraReplicaAdvertRefreshOwner::from_kura(kura.as_ref(), Instant::now())
            .map_err(V2RunnerError::Service)?,
    );

    loop {
        cleanup_supervisor.reap_finished();
        if output_guard.restart_required() {
            return Err(V2RunnerError::RestartRequired);
        }
        if shutdown_signal.is_sent() {
            return Ok(());
        }
        let context = verified_context.context().clone();
        close_ingress_for_rollover(&ingress_ready, &block_rx);
        block_rx
            .configure_roster_for_context(
                context
                    .roster
                    .iter()
                    .map(|validator| validator.validator.clone()),
                &context.network_id,
                context.da_layout,
            )
            .map_err(ingress_capacity_error)?;
        super::status::set_v2_network_ingress(context.id(), context.height, &block_rx);
        let validator_set_pops = verified_context.proofs_of_possession().to_vec();
        let shared_config = config.v2_config(block_cadence, context.mode)?;
        let fingerprints = adapter_fingerprints(&local_peer, &shared_config);
        let control_queue_capacity = usize::try_from(shared_config.limits.control_queue_capacity)?;
        let body_queue_capacity = usize::try_from(shared_config.limits.body_queue_capacity)?;
        let chunk_queue_capacity = usize::try_from(shared_config.limits.chunk_queue_capacity)?;
        let certified_request_capacity =
            usize::try_from(shared_config.limits.certified_request_capacity)?;
        let effect_work_capacity = usize::try_from(shared_config.limits.effect_work_capacity)?;
        validate_deadline_duration(CANDIDATE_WORK_RECHECK)?;
        let runtime_queue = runtime_queue_config(&shared_config)?;
        let effect_queue = effect_queue_config(&shared_config)?;
        let serviced_candidate_capacity_geometry = ServicedCandidateCapacityGeometry::new(
            usize::try_from(shared_config.limits.runtime_command_capacity)?,
            effect_work_capacity,
        );
        let lane_work_limits = lane_work_limits(
            &shared_config,
            network.reply_route_source_capacity(),
            consensus_frame_byte_capacity,
            block_sync_frame_byte_capacity,
            retransmit_interval,
            round_timeout,
        )?;
        let candidate_limits = candidate_limits(&context, &shared_config)?;
        let local_validator = local_validator_index(&context, &local_peer, config.role)?;
        let mut npos_vrf = V2NposVrfLifecycle::open(
            &context,
            state.as_ref(),
            local_validator,
            &common_config.key_pair,
        )?;
        let new_block_sync_server = block_sync_server
            .is_none()
            .then(|| V2BlockSyncServer::new(context.network_id, certified_request_capacity))
            .transpose()?;
        let mut block_sync = V2BlockSyncDiscovery::new(
            context.clone(),
            local_peer.clone(),
            certified_request_capacity,
        )?;
        let consensus_key_hash: [u8; 32] =
            Hash::new(common_config.key_pair.public_key().encode()).into();
        let storage_root = kura.sumeragi_v2_storage_root();
        let wal_path = storage_root
            .join("wal")
            .join(format!("{:020}.wal", context.height));
        // Complete every pure or validation-only height preflight before any
        // WAL, body-store, chunk-store, or lane-work constructor can mutate
        // durable state. Publish the newly validated in-memory server only
        // after the full preflight succeeds.
        if let Some(server) = new_block_sync_server {
            block_sync_server = Some(server);
        }
        let lifecycle_ordinals = ProductionV2Services::restore_lifecycle_ordinal_source(
            &context,
            storage_root.join("chunks"),
            network.reply_route_source_capacity().max(1),
            certified_request_capacity,
        )
        .map_err(V2RunnerError::Service)?;
        // Open and validate the body store before the runtime can mint a new
        // scheduler ordinal. Its independently reconstructed receipt catalog
        // is the authority for body-backed leader-wire terminals; the gate's
        // adjacent snapshot cannot validate itself after a crash.
        let mut body_store = V2BodyStore::open_with_policy(
            storage_root.join("bodies"),
            context.clone(),
            signature_policy,
        )
        .map_err(|error| {
            V2RunnerError::Effect(super::v2_effects::EffectExecutorError::BodyStore(
                error.to_string(),
            ))
        })?;
        let recovery_validator = V2ApplyService::new(
            Arc::clone(&state),
            Arc::clone(&queue),
            Arc::clone(&kura),
            provider_ingest_finalized_archive.clone(),
            reputation_finalized_archive.clone(),
            block_cadence,
            genesis_account.clone(),
            events_sender.clone(),
            validator_set_pops.clone(),
        );
        if let Some(decided_subject) = recovery_validator
            .recovered_finality_subject(&context)
            .map_err(|error| V2RunnerError::Service(error.to_string()))?
        {
            body_store
                .retain_recovered_markers_for_subject(decided_subject)
                .map_err(|error| {
                    V2RunnerError::Effect(super::v2_effects::EffectExecutorError::BodyStore(
                        error.to_string(),
                    ))
                })?;
        }
        let recovered_body_catalog = body_store.recovery_catalog().map_err(|error| {
            V2RunnerError::Effect(super::v2_effects::EffectExecutorError::BodyStore(
                error.to_string(),
            ))
        })?;
        let recovered_body_receipts = recovered_body_catalog
            .values()
            .map(|(_, receipt)| receipt.clone())
            .collect::<Vec<_>>();
        let adapter_construction = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        let wal_authority = kura
            .mint_safety_wal_directory_authority()
            .map_err(|error| V2RunnerError::Service(error.to_string()))?;
        let adapter = if pending_successor_activation.is_some() {
            // Preserve the finalized predecessor's Running handoff until the
            // complete successor stack is live. No reducer status from this
            // adapter may escape the construction boundary early.
            SumeragiV2Adapter::open_deferred_status_with_capacity_geometry(
                kura.as_ref(),
                wal_authority,
                verified_context.clone(),
                local_validator,
                Generation::INITIAL,
                consensus_key_hash,
                fingerprints,
                serviced_candidate_capacity_geometry,
                deferred_admission_ordinals.clone(),
            )
        } else {
            SumeragiV2Adapter::open_with_capacity_geometry(
                kura.as_ref(),
                wal_authority,
                verified_context.clone(),
                local_validator,
                Generation::INITIAL,
                consensus_key_hash,
                fingerprints,
                serviced_candidate_capacity_geometry,
                deferred_admission_ordinals.clone(),
            )
        };
        let (adapter, startup_effects) = adapter?;
        // Adapter replay returns plain effects; their fresh lifecycle owners
        // are minted only by `SerializedV2Runtime` below. Fold the validated
        // producer snapshot into the shared source before that constructor so
        // neither startup/runtime work nor a later Serve reservation can reuse
        // a reclaimed producer ordinal.
        if let Some(high_watermark) =
            adapter.restored_producer_continuation_ordinal_high_watermark()
        {
            lifecycle_ordinals
                .advance_past(high_watermark)
                .map_err(V2RunnerError::Service)?;
        }
        let leader_wire_roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let leader_wire_max_chunk_count = context.da_layout.max_chunk_count;
        let leader_wire_capacity = LeaderWireLifecycleStoreGate::derived_capacity(
            leader_wire_roster.len(),
            leader_wire_max_chunk_count,
        )
        .map_err(V2RunnerError::Service)?;
        let leader_wire_owner: [u8; 32] = fingerprints.node.into();
        let leader_wire_recovery_authority = adapter.leader_wire_recovery_authority()?;
        let producer_terminals = adapter.durable_producer_terminal_tokens();
        let leader_wire_storage = adapter.mint_leader_wire_store_authority(&wal_path)?;
        let (leader_wire_gate, leader_wire_restore) =
            LeaderWireLifecycleStoreGate::open_with_safety_wal_authority(
                leader_wire_storage,
                context.id(),
                context.height,
                leader_wire_owner,
                leader_wire_roster,
                leader_wire_capacity,
                leader_wire_max_chunk_count,
                leader_wire_recovery_authority,
                &producer_terminals,
                &recovered_body_receipts,
            )
            .map_err(V2RunnerError::Service)?;
        lifecycle_ordinals
            .advance_past(leader_wire_restore.scheduler_ordinal_high_watermark())
            .map_err(V2RunnerError::Service)?;
        let leader_wire_ingress_binding = LeaderWireIngressBinding::bind(
            Arc::clone(&ingress_ready),
            Arc::clone(&block_rx),
            Arc::clone(&leader_wire_gate),
            leader_wire_restore,
            lifecycle_ordinals.clone(),
            context.id(),
            context.height,
        )?;
        // The body directory may retain markers from arbitrarily many past
        // views. WAL replay is the sole authority for deciding which bounded
        // frontier can recover vote authority; re-execute only those exact
        // identities before constructing the live serialized runtime.
        let recovered_validation_authority =
            adapter.recovered_validation_authority(&startup_effects)?;
        body_store
            .retain_recovered_markers_for_authority(recovered_validation_authority)
            .map_err(|error| {
                V2RunnerError::Effect(super::v2_effects::EffectExecutorError::BodyStore(
                    error.to_string(),
                ))
            })?;
        body_store
            .revalidate_recovered_markers(|body| {
                recovery_validator.revalidate_recovered_candidate(&context, body)
            })
            .map_err(|error| {
                V2RunnerError::Effect(super::v2_effects::EffectExecutorError::BodyStore(
                    error.to_string(),
                ))
            })?;
        adapter_construction.complete();
        let runtime_construction = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        let (runtime, mut startup_effects) = SerializedV2Runtime::new_with_lifecycle_ordinals(
            adapter,
            startup_effects,
            Instant::now(),
            round_timeout,
            runtime_queue,
            lifecycle_ordinals.clone(),
        )?;
        runtime_construction.complete();
        let (mut executor, body_store) = V2EffectExecutor::open_with_body_store(
            runtime,
            body_store,
            context.clone(),
            local_peer.clone(),
            local_validator,
            Arc::clone(&output_guard),
            effect_queue,
        )?;
        if let Some(authenticated_genesis) = first_height_genesis.as_ref() {
            executor.install_authenticated_genesis_body(authenticated_genesis)?;
        }
        // A replayed ProposalIntent owns candidate work only when its exact
        // tag, round, and subject still match the current lock snapshot. Its
        // asynchronous signature completion must restore and broadcast the
        // exact durable payload before any fresh candidate work is admitted.
        let replayed_proposal = replayed_proposal_sign(&startup_effects);
        let recovering_interrupted_tip = pending_kura_apply.is_some();
        let pending_recovery_identity = pending_kura_apply;
        let initially_recovered_applied_height = pending_kura_apply.filter(|pending| {
            usize::try_from(pending.height()).is_ok_and(|height| state.committed_height() == height)
        });
        let mut authenticated_genesis_nexus_amx_context =
            staged_genesis_nexus_amx_context.map(AuthenticatedGenesisNexusAmxContext::Staged);
        if let Some(pending) = pending_kura_apply.take() {
            let pending_replay_verification = output_guard
                .begin_fail_stop_operation()
                .ok_or(V2RunnerError::RestartRequired)?;
            let replayed_genesis_nexus_amx_context =
                executor.verify_pending_kura_apply_replay(pending, &startup_effects)?;
            pending_replay_verification.complete();
            if initially_recovered_applied_height.is_none()
                && let Some(replayed) = replayed_genesis_nexus_amx_context
                && authenticated_genesis_nexus_amx_context
                    .replace(AuthenticatedGenesisNexusAmxContext::ReplayedPending(
                        replayed,
                    ))
                    .is_some()
            {
                return Err(V2RunnerError::ConflictingGenesisNexusContext);
            }
        }
        let (exact_output_service_owner, exact_output_transport_owner) =
            durable_exact_output_handoff_owner_pair();
        let durable_decided_subject = executor.local_proposal_directive()?.decided_subject();
        let mut services = ProductionV2Services::start(
            context.clone(),
            executor.current_tag(),
            durable_decided_subject,
            validator_set_pops,
            local_peer.clone(),
            local_validator,
            common_config.key_pair.clone(),
            network.clone(),
            storage_root.join("chunks"),
            body_store,
            Arc::clone(&state),
            Arc::clone(&queue),
            Arc::clone(&kura),
            provider_ingest_finalized_archive.clone(),
            reputation_finalized_archive.clone(),
            block_cadence,
            genesis_account.clone(),
            events_sender.clone(),
            effect_work_capacity,
            certified_request_capacity,
            chunk_queue_capacity,
            lifecycle_ordinals,
            Arc::clone(&output_guard),
            Arc::clone(&block_rx),
            Arc::clone(&kura_replica_advert_refresh),
            leader_wire_recovery_authority,
            exact_output_service_owner,
        )
        .map_err(V2RunnerError::Service)?;
        let certified_serve_ingress_binding = CertifiedServeIngressBinding::bind(
            Arc::clone(&ingress_ready),
            Arc::clone(&block_rx),
            services
                .certified_serve_ingress_gate()
                .map_err(V2RunnerError::Service)?,
        )?;
        let mut height_ingress_bindings = HeightIngressBindings::new(
            certified_serve_ingress_binding,
            leader_wire_ingress_binding,
        );

        // A Native receipt at the durable tip may have crossed its
        // finality/manifest/receipt boundary before WSV checkpoint and commit
        // metadata were published. Finish the exact local replay first.
        // `DurableApplyCompletion` is emitted only after post-apply metadata
        // and strict Native evidence repair both succeed, so no lane-work
        // constructor can observe or reject the recoverable intermediate
        // shape.
        if recovering_interrupted_tip {
            let recovery_deadline = PendingTipRecoveryDeadline::new(Instant::now(), round_timeout)?;
            iroha_logger::info!(
                height = context.height,
                timeout = ?recovery_deadline.timeout,
                stage = ?executor
                    .pending_kura_apply_recovery_evidence()
                    .map(|evidence| evidence.stage()),
                "started bounded Sumeragi v2 interrupted-tip recovery"
            );
            let _ = reconcile_executor_locked_body(&mut executor, &mut services)?;
            executor.consume_pending_tip_recovery_effects(
                std::mem::take(&mut startup_effects),
                &mut services,
            )?;
            while executor.durable_finality().is_none() {
                if output_guard.restart_required() {
                    return Err(V2RunnerError::RestartRequired);
                }
                if shutdown_signal.is_sent() {
                    height_ingress_bindings.retire()?;
                    services.allow_clean_shutdown();
                    return Ok(());
                }
                let now = Instant::now();
                if recovery_deadline.expired(now) {
                    executor.record_pending_tip_recovery_deadline_exceeded(&mut services)?;
                    let error = pending_tip_recovery_deadline_error(
                        output_guard.as_ref(),
                        recovery_deadline.timeout,
                        executor.pending_tip_recovery_attempts(),
                        executor
                            .pending_kura_apply_recovery_evidence()
                            .map(|evidence| evidence.stage()),
                    );
                    return Err(error);
                }
                let completions = services.drain_completions(&mut executor)?;
                let advanced = advance_pending_tip_recovery_executor(
                    &mut executor,
                    &mut services,
                    control_queue_capacity,
                )?;
                if executor.durable_finality().is_none() && completions == 0 && advanced == 0 {
                    let remaining = recovery_deadline.remaining(Instant::now());
                    if !remaining.is_zero() {
                        let _ = wake_rx.recv_timeout(remaining.min(IDLE_POLL));
                    }
                }
            }
            iroha_logger::info!(
                height = context.height,
                elapsed = ?recovery_deadline.elapsed(Instant::now()),
                attempts = executor.pending_tip_recovery_attempts(),
                stage = ?executor
                    .pending_kura_apply_recovery_evidence()
                    .map(|evidence| evidence.stage()),
                "finished bounded Sumeragi v2 interrupted-tip recovery"
            );
        }
        let recovered_applied_height = pending_recovery_identity.filter(|pending| {
            usize::try_from(pending.height()).is_ok_and(|height| {
                state.committed_height() == height
                    && state.latest_block_hash_fast() == Some(pending.block_hash())
            })
        });
        if recovering_interrupted_tip {
            // A replayed genesis projection is a pre-apply capability. The
            // completed State tip is now authoritative and the lane adapter
            // must enter through its exact post-apply recovery path.
            authenticated_genesis_nexus_amx_context = None;
        }
        if reservation_reconciliation_pending {
            let evidence_repair_queue_fence =
                LaneApplicationEvidenceRepairQueueFence::capture(queue.as_ref())?;
            loop {
                evidence_repair_queue_fence.revalidate(queue.as_ref())?;
                match plan_lane_application_evidence_repair(
                    &context,
                    state.as_ref(),
                    kura.as_ref(),
                    lane_work_limits,
                )? {
                    LaneApplicationEvidenceRepairPlanning::Ready(plan) if plan.is_empty() => {
                        break;
                    }
                    LaneApplicationEvidenceRepairPlanning::Ready(plan) => {
                        let planned_items = plan.item_count();
                        let evidence_repair = output_guard
                            .begin_fail_stop_operation()
                            .ok_or(V2RunnerError::RestartRequired)?;
                        let summary = apply_lane_application_evidence_repair(
                            state.as_ref(),
                            kura.as_ref(),
                            plan,
                        )?;
                        let progressed = summary.publication_count();
                        if planned_items == 0 || progressed == 0 {
                            return Err(V2RunnerError::Service(
                                "lane application evidence startup repair made no bounded progress"
                                    .to_owned(),
                            ));
                        }
                        evidence_repair.complete();
                        iroha_logger::info!(
                            ordinary_pairs = summary.ordinary_pairs,
                            ordinary_receipts = summary.ordinary_receipts,
                            native_carriers = summary.native_carriers,
                            native_routes = summary.native_routes,
                            merge_carriers = summary.merge_carriers,
                            "published preflighted lane application evidence before Queue startup"
                        );
                    }
                    LaneApplicationEvidenceRepairPlanning::RecoverCanonicalBodies(needs) => {
                        if needs.is_empty() {
                            return Err(V2RunnerError::Service(
                                "lane application evidence repair requested an empty body set"
                                    .to_owned(),
                            ));
                        }
                        open_ingress_for_active_height(
                            output_guard.as_ref(),
                            &ingress_ready,
                            &block_rx,
                            None,
                        )?;
                        let recovery_capacity =
                            CanonicalExecutedBlockRecovery::need_capacity(lane_work_limits);
                        let recovery_batches =
                            canonical_executed_block_recovery_batches(&needs, recovery_capacity)?;
                        for bounded_needs in recovery_batches {
                            let mut body_recovery = CanonicalExecutedBlockRecovery::new(
                                context.clone(),
                                local_peer.clone(),
                                Arc::clone(&state),
                                Arc::clone(&kura),
                                Arc::clone(&output_guard),
                                lane_work_limits,
                                bounded_needs.to_vec(),
                            )?;
                            let mut next_retry = Instant::now();
                            while body_recovery.has_pending() {
                                if output_guard.restart_required() {
                                    return Err(V2RunnerError::RestartRequired);
                                }
                                if shutdown_signal.is_sent() {
                                    height_ingress_bindings.retire()?;
                                    services.allow_clean_shutdown();
                                    return Ok(());
                                }
                                let now = Instant::now();
                                if now >= next_retry {
                                    body_recovery.service_next()?;
                                    next_retry = deadline_after(now, retransmit_interval);
                                }
                                let drained = drain_canonical_executed_block_recovery_ingress(
                                    &block_rx,
                                    &mut body_recovery,
                                    control_queue_capacity,
                                )?;
                                if drained != 0 && body_recovery.has_pending() {
                                    body_recovery.service_next()?;
                                }
                                let dispatched =
                                    dispatch_canonical_executed_block_recovery_effects(
                                        &mut body_recovery,
                                        &services,
                                        control_queue_capacity,
                                    )?;
                                if body_recovery.has_pending() && drained == 0 && dispatched == 0 {
                                    let wait = next_retry
                                        .saturating_duration_since(Instant::now())
                                        .min(IDLE_POLL);
                                    if !wait.is_zero() {
                                        let _ = wake_rx.recv_timeout(wait);
                                    }
                                }
                            }
                        }
                        close_ingress_for_rollover(&ingress_ready, &block_rx);
                    }
                }
            }
        }
        if reservation_reconciliation_pending {
            let summary = loop {
                let deferred_terminal_recovery =
                    reconcile_lifecycle_terminal_outcomes_before_queue_planning(
                        &output_guard,
                        state.as_ref(),
                        queue.as_ref(),
                        kura.as_ref(),
                        &context,
                    )?;
                let planning = plan_lane_reservation_ownership(
                    state.as_ref(),
                    queue.as_ref(),
                    kura.as_ref(),
                    &verified_context,
                    None,
                )?;
                let planning = match planning {
                    LaneReservationReconciliationPlanning::Ready(pre_lifecycle_plan) => {
                        // Classification is read-only. Discard its first receipt, reconcile every
                        // signed local lifecycle boundary while the Queue gate remains closed, then
                        // rebuild the complete plan with the exact receipt retained by that flow.
                        let planner_evidence =
                            pre_lifecycle_plan.startup_snapshot_recovery_evidence()?;
                        let lifecycle = reconcile_autonomous_lifecycle_startup(
                            state.as_ref(),
                            queue.as_ref(),
                            kura.as_ref(),
                            &context,
                            planner_evidence,
                            deferred_terminal_recovery,
                            _lifecycle_process_generation.as_ref(),
                            &local_peer,
                            &common_config.key_pair,
                        )
                        .map_err(V2RunnerError::Service)?;
                        let completed_bootstraps = lifecycle.completed_bootstraps();
                        let recovered_attempts = lifecycle.recovered_attempts();
                        let replanned = plan_lane_reservation_ownership(
                            state.as_ref(),
                            queue.as_ref(),
                            kura.as_ref(),
                            &verified_context,
                            Some(lifecycle),
                        )?;
                        if completed_bootstraps != 0 || recovered_attempts != 0 {
                            iroha_logger::info!(
                                completed_bootstraps,
                                recovered_attempts,
                                "reconciled signed autonomous lifecycle custody before Queue publication"
                            );
                        }
                        replanned
                    }
                    pending => pending,
                };
                match planning {
                    LaneReservationReconciliationPlanning::Ready(plan) => {
                        let reservation_recovery = output_guard
                            .begin_fail_stop_operation()
                            .ok_or(V2RunnerError::RestartRequired)?;
                        let summary = apply_lane_reservation_reconciliation_plan(
                            state.as_ref(),
                            queue.as_ref(),
                            kura.as_ref(),
                            plan,
                        )?;
                        reservation_recovery.complete();
                        break summary;
                    }
                    LaneReservationReconciliationPlanning::RecoverCanonicalBodies(needs) => {
                        if !queue.lane_reservation_startup_reconciliation_pending() {
                            return Err(V2RunnerError::Service(
                                "reservation body recovery was requested after the Queue startup gate opened"
                                    .to_owned(),
                            ));
                        }
                        iroha_logger::info!(
                            body_count = needs.len(),
                            "starting authenticated recovery of pruned canonical executed blocks"
                        );
                        open_ingress_for_active_height(
                            output_guard.as_ref(),
                            &ingress_ready,
                            &block_rx,
                            None,
                        )?;
                        let recovery_capacity =
                            CanonicalExecutedBlockRecovery::need_capacity(lane_work_limits);
                        let recovery_batches =
                            canonical_executed_block_recovery_batches(&needs, recovery_capacity)?;
                        for bounded_needs in recovery_batches {
                            let mut body_recovery = CanonicalExecutedBlockRecovery::new(
                                context.clone(),
                                local_peer.clone(),
                                Arc::clone(&state),
                                Arc::clone(&kura),
                                Arc::clone(&output_guard),
                                lane_work_limits,
                                bounded_needs.to_vec(),
                            )?;
                            let mut next_retry = Instant::now();
                            while body_recovery.has_pending() {
                                if output_guard.restart_required() {
                                    return Err(V2RunnerError::RestartRequired);
                                }
                                if shutdown_signal.is_sent() {
                                    height_ingress_bindings.retire()?;
                                    services.allow_clean_shutdown();
                                    return Ok(());
                                }
                                let now = Instant::now();
                                if now >= next_retry {
                                    body_recovery.service_next()?;
                                    next_retry = deadline_after(now, retransmit_interval);
                                }
                                let drained = drain_canonical_executed_block_recovery_ingress(
                                    &block_rx,
                                    &mut body_recovery,
                                    control_queue_capacity,
                                )?;
                                if drained != 0 && body_recovery.has_pending() {
                                    body_recovery.service_next()?;
                                }
                                let dispatched =
                                    dispatch_canonical_executed_block_recovery_effects(
                                        &mut body_recovery,
                                        &services,
                                        control_queue_capacity,
                                    )?;
                                if body_recovery.has_pending() && drained == 0 && dispatched == 0 {
                                    let wait = next_retry
                                        .saturating_duration_since(Instant::now())
                                        .min(IDLE_POLL);
                                    if !wait.is_zero() {
                                        let _ = wake_rx.recv_timeout(wait);
                                    }
                                }
                            }
                        }
                        close_ingress_for_rollover(&ingress_ready, &block_rx);
                        iroha_logger::info!(
                            "cached authenticated canonical executed blocks; rebuilding the complete immutable reconciliation plan"
                        );
                    }
                    LaneReservationReconciliationPlanning::InstallHistoricalAutonomousRecoveries(
                        installs,
                    ) => {
                        if installs.is_empty() {
                            return Err(V2RunnerError::Service(
                                "reservation reconciliation requested an empty historical recovery installation"
                                    .to_owned(),
                            ));
                        }
                        if !queue.lane_reservation_startup_reconciliation_pending() {
                            return Err(V2RunnerError::Service(
                                "historical reservation recovery was requested after the Queue startup gate opened"
                                    .to_owned(),
                            ));
                        }
                        let historical_recovery = output_guard
                            .begin_fail_stop_operation()
                            .ok_or(V2RunnerError::RestartRequired)?;
                        iroha_logger::info!(
                            install_count = installs.len(),
                            "installing finalized historical autonomous recovery inputs"
                        );
                        let records = installs
                            .iter()
                            .map(|install| {
                                preflight_historical_autonomous_lane_recovery(
                                    state.as_ref(),
                                    kura.as_ref(),
                                    install,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let process_generation = _lifecycle_process_generation
                            .as_ref()
                            .ok_or_else(|| {
                                V2RunnerError::Service(
                                    "historical autonomous recovery payload custody requires a validator process generation"
                                        .to_owned(),
                                )
                            })?;
                        for record in &records {
                            persist_canonical_historical_recovery_payload_custody(
                                kura.as_ref(),
                                process_generation,
                                &common_config.key_pair,
                                &local_peer,
                                record,
                            )?;
                        }
                        let _outcomes =
                            persist_preflighted_historical_autonomous_lane_recoveries(
                                kura.as_ref(),
                                &records,
                            )?;
                        validate_installed_historical_autonomous_lane_recoveries(
                            kura.as_ref(),
                            &records,
                        )?;
                        historical_recovery.complete();
                        iroha_logger::info!(
                            install_count = installs.len(),
                            "installed historical autonomous recovery inputs; rebuilding the complete immutable reconciliation plan"
                        );
                    }
                }
            };
            reservation_reconciliation_pending = false;
            if summary != Default::default() {
                iroha_logger::info!(
                    recovered = summary.recovered,
                    finalized_committed = summary.finalized_committed,
                    retained_current = summary.retained_current,
                    retained_certified = summary.retained_certified,
                    retained_pending_merge = summary.retained_pending_merge,
                    retained_historical_recovery = summary.retained_historical_recovery,
                    released_strictly_absent = summary.released_strictly_absent,
                    released_terminal_loser = summary.released_terminal_loser,
                    resumed_retirement = summary.resumed_retirement,
                    "reconciled durable lane reservations from one exact startup evidence plan"
                );
            }
        }
        let mut lane_work = construct_after_pending_tip_application_recovery(
            recovering_interrupted_tip,
            executor.durable_finality().is_some() && recovered_applied_height.is_some(),
            || {
                V2LaneWorkAdapter::new_with_output_guard_and_transport(
                    &verified_context,
                    local_peer.clone(),
                    common_config.key_pair.clone(),
                    config.role == NodeRole::Validator,
                    Arc::clone(&state),
                    Arc::clone(&kura),
                    lane_work_limits,
                    authenticated_genesis_nexus_amx_context,
                    recovered_applied_height,
                    Arc::clone(&output_guard),
                    exact_output_transport_owner,
                    retained_merge_sidecars.take(),
                    _lifecycle_process_generation.clone(),
                )
                .map_err(V2RunnerError::from)
            },
        )?;
        lane_work.install_lane_drain_queue(Arc::clone(&queue))?;
        // Signed lifecycle bootstrap, generation takeover, Queue snapshot recovery, and local
        // Kura body rehydration completed above while the adapter was still carrier-silent.
        // Activation is one-shot and independently revalidates every hydrated owner before work.
        lane_work.activate_after_lane_drain_queue_install(&queue)?;
        let mut committed_lane_status_publisher = CommittedLaneStatusPublisher::default();
        committed_lane_status_publisher.publish_if_changed(&lane_work);
        if let Some(scheduler_ordinal) = services
            .dormant_certified_serve_ingress_scheduler_ordinal()
            .map_err(V2RunnerError::Service)?
        {
            let _ = services.fail_closed_dormant_certified_serve(scheduler_ordinal);
            return Err(V2RunnerError::RestartRequired);
        }
        // Seed executor lock ownership from replay before consuming startup
        // effects. Otherwise a recovered lock would look like a live first-lock
        // transition and could retire safe work reconstructed from the same WAL.
        let _ = reconcile_executor_locked_body(&mut executor, &mut services)?;
        if !recovering_interrupted_tip {
            executor.consume_effects(std::mem::take(&mut startup_effects), &mut services)?;
        }
        let startup_directive = executor.local_proposal_directive()?;
        // Adapter construction is deliberately carrier-silent. Only the exact
        // reducer/WAL recovery directive may unlock candidate signing or the
        // decided carrier's bounded lane-completion traffic.
        lane_work.retain_merge_sidecars_for_global_view(
            startup_directive.tag().view(),
            startup_directive.locked_subject(),
            startup_directive.decided_subject(),
        )?;
        if startup_directive.decided_subject().is_none()
            && let Some((locked_round, locked)) = startup_directive.locked_body()
        {
            let _ = lane_work.mark_global_body_locked(locked_round, locked)?;
        }
        dispatch_lane_work_effects(&mut lane_work, &services, control_queue_capacity)?;
        // Startup recovery and durable constructor work must not consume the
        // live height cadence. Constructor repair already emitted the first
        // bounded Native recovery request for every marker it could own; any
        // pressure-deferred marker, or retransmission to the next authenticated
        // signer, waits for `next_lane_retransmit`. Interrupted-tip replay
        // remains permanently unarmed because the already-decided runtime is
        // consumed as soon as its local Apply finishes; the fresh successor is
        // armed normally.
        // These additions are infallible after the early representability
        // probes above.
        let height_started_at = Instant::now();
        if !recovering_interrupted_tip {
            executor.arm_live_clocks(height_started_at)?;
        }
        let mut next_block_sync_attempt =
            initial_block_sync_deadline(height_started_at, round_timeout, eager_block_sync);
        let mut next_lane_retransmit = deadline_after(height_started_at, retransmit_interval);
        let mut next_npos_vrf_retransmit = deadline_after(height_started_at, retransmit_interval);
        let initial_directive = reconcile_executor_locked_body(&mut executor, &mut services)?;
        let mut local_proposal_state =
            LocalProposalState::from_replayed_proposal(replayed_proposal, initial_directive);
        debug_assert!(!recovering_interrupted_tip || pending_successor_activation.is_none());
        let activation = pending_successor_activation
            .take()
            .map(|pending| {
                executor
                    .successor_activation_status_snapshot()
                    .map(|status| (pending, status))
            })
            .transpose()?;
        // Interrupted-tip recovery admits transport so validators can finish
        // only the exact replayed Decision's lane session. Its dedicated drain
        // below discards terminal global traffic instead of re-entering it into
        // the already-decided reducer.
        open_ingress_for_active_height(
            output_guard.as_ref(),
            &ingress_ready,
            &block_rx,
            activation,
        )?;
        if !recovering_interrupted_tip {
            broadcast_npos_vrf_messages(
                npos_vrf.take_outbound(),
                output_guard.as_ref(),
                &services,
            )?;
        }

        let mut block_sync_request = None;
        let mut admitted_discovered_commit_qc = false;

        let finality = loop {
            committed_lane_status_publisher.publish_if_changed(&lane_work);
            cleanup_supervisor.reap_finished();
            if output_guard.restart_required() {
                return Err(V2RunnerError::RestartRequired);
            }
            if shutdown_signal.is_sent() {
                height_ingress_bindings.retire()?;
                services.allow_clean_shutdown();
                return Ok(());
            }
            // Every retry/continue path returns through this edge-triggered
            // poll. It rebuilds the live overlays only at its next semantic
            // deadline or after the published height owner changes.
            liveness_watchdog.poll(Instant::now());
            if let Some(scheduler_ordinal) = services
                .dormant_certified_serve_ingress_scheduler_ordinal()
                .map_err(V2RunnerError::Service)?
            {
                let _ = services.fail_closed_dormant_certified_serve(scheduler_ordinal);
                return Err(V2RunnerError::RestartRequired);
            }
            let certified_serve_barrier = services
                .certified_serve_barrier()
                .map_err(V2RunnerError::Service)?;
            let now = Instant::now();
            if !recovering_interrupted_tip && now >= next_npos_vrf_retransmit {
                broadcast_npos_vrf_messages(
                    npos_vrf.retransmission(),
                    output_guard.as_ref(),
                    &services,
                )?;
                next_npos_vrf_retransmit = deadline_after(now, retransmit_interval);
            }

            if executor.has_retained_certified_body_response() {
                let target_ordinal = executor
                    .retained_certified_body_response_scheduler_ordinal()?
                    .ok_or_else(|| {
                        V2RunnerError::Service(
                            "retained certified body response lost its exact scheduler position"
                                .to_owned(),
                        )
                    })?;
                // Give one strict frozen predecessor first claim on newly
                // available completion capacity. Otherwise the response could
                // occupy the sole returned slot and strand that older owner.
                services.drain_exact_serve_runtime_predecessor(&mut executor, target_ordinal)?;
                if executor.older_runtime_lifecycle_predates_retained_response(
                    Instant::now(),
                    target_ordinal,
                )? {
                    // The matching PendingFetch is itself an older passive
                    // owner, so this is one bounded opportunity rather than a
                    // prerequisite for retry.
                    advance_executor_once_before_exact_serve(
                        &block_rx,
                        &mut executor,
                        &mut services,
                    )?;
                }
                if let Some(timeout_recovery_cut) = executor.timeout_recovery_lifecycle_cut()? {
                    // The retained response may predate the timeout itself,
                    // so its strict `< target` completion turn cannot admit
                    // the timeout signer's callback. Give exactly one owner
                    // from the separately frozen inclusive `<= timeout` prefix
                    // a turn before retrying the response.
                    services.drain_timeout_recovery_prefix_completion(
                        &mut executor,
                        timeout_recovery_cut,
                    )?;
                }
                let response_backpressured = match executor
                    .retry_retained_certified_body_response(&mut services)
                {
                    Ok(_) => false,
                    Err(EffectTransportError::Backpressure) => true,
                    Err(EffectTransportError::FailClosed(reason)) => {
                        return Err(V2RunnerError::Service(reason));
                    }
                    Err(error) => {
                        iroha_logger::debug!(%error, "rejected retained certified body response");
                        false
                    }
                };
                if response_backpressured {
                    // Transport capacity is not pacemaker authority. This
                    // retained response owns one bounded opportunity to admit
                    // a new authenticated certificate into the typed Progress
                    // lane. Once charged, later retries service the finite
                    // retained certificate prefix but cannot replenish it
                    // from fresh ingress and recreate the same capacity cycle.
                    if executor.retained_response_may_admit_certified_fence_escape() {
                        drain_v2_ingress(
                            &block_rx,
                            &mut executor,
                            &mut services,
                            &mut lane_work,
                            output_guard.as_ref(),
                            kura.as_ref(),
                            &common_config.key_pair,
                            block_sync_server
                                .as_mut()
                                .expect("block-sync server initialized before ingress"),
                            &mut block_sync,
                            &mut block_sync_request,
                            &mut npos_vrf,
                            V2IngressDrainMode::CertifiedFenceEscape,
                            1,
                        )?;
                    }
                    // Certificate escape is a one-shot retained-response
                    // credit. TimeoutVote production is a distinct frozen
                    // roster episode: service one eligible source on every
                    // backpressured outer turn so reaching quorum never
                    // depends on that certificate credit remaining unused.
                    drain_v2_ingress(
                        &block_rx,
                        &mut executor,
                        &mut services,
                        &mut lane_work,
                        output_guard.as_ref(),
                        kura.as_ref(),
                        &common_config.key_pair,
                        block_sync_server
                            .as_mut()
                            .expect("block-sync server initialized before ingress"),
                        &mut block_sync,
                        &mut block_sync_request,
                        &mut npos_vrf,
                        V2IngressDrainMode::TimeoutVoteEpisode,
                        1,
                    )?;
                    executor.reconcile_retained_response_certified_fence_escape_phase();
                    // Give one already-owned timeout/Progress root a turn
                    // before retrying the exact response. Ordinary work stays
                    // parked behind the retained physical carrier.
                    advance_pacemaker_once(&block_rx, &mut executor, &mut services)?;
                    executor.reconcile_retained_response_certified_fence_escape_phase();
                }
                committed_lane_status_publisher.publish_if_changed(&lane_work);
                let _ = wake_rx.recv_timeout(IDLE_POLL);
                continue;
            }

            // The network thread installs an exact certified-body ticket before
            // its carrier becomes visible in fair ingress. Give that target a
            // dedicated runner turn before completions, runtime work, lock
            // reconciliation, or any other local producer can acquire a later
            // I/O position.
            if let Some(serve_barrier) = certified_serve_barrier {
                // One exact ticket closes its finite older-owner prefix through
                // bounded turns. Each turn atomically selects at most one
                // completed causal lifecycle whose immutable ordinal is
                // strictly older than the ticket, publishes/freezes resulting
                // runtime ownership, and executes at most one serialized
                // transition. The ticket reaches target-only ingress only once
                // re-evaluation finds no older owner. Later producers cannot
                // enter the frozen prefix, and exact carrier retry retains the
                // same episode state and ticket identity.
                let mut older_predecessor_remains = false;
                let completion_evidence = services
                    .certified_serve_predecessor_completion_evidence(
                        executor.remaining_completion_capacity() != 0,
                        serve_barrier.scheduler_ordinal(),
                    )
                    .map_err(V2RunnerError::Service)?;
                if let Some(witness) = executor.exact_serve_predecessor_episode_witness(
                    Instant::now(),
                    serve_barrier.scheduler_ordinal(),
                    completion_evidence,
                )? {
                    // A passive Fetch is intentionally absent from the
                    // runnable-owner set. A completed strict predecessor is
                    // projected without consuming it; its exact local ordinal
                    // lets the runtime issue one newer episode witness before
                    // the worker claims capacity and admits the completion.
                    let _ = services
                        .observe_certified_serve_predecessor_episode_witness(serve_barrier, witness)
                        .map_err(V2RunnerError::Service)?;
                }
                let claimed_older_runtime_episode = services
                    .claim_certified_serve_runtime_episode(serve_barrier)
                    .map_err(V2RunnerError::Service)?;
                if claimed_older_runtime_episode {
                    services.drain_exact_serve_runtime_predecessor(
                        &mut executor,
                        serve_barrier.scheduler_ordinal(),
                    )?;
                    // A frozen Control/Serve prefix may still own every
                    // physical I/O unit. Yield this turn until one unit drains
                    // instead of dispatching a retained causal effect into
                    // backpressure. Once capacity exists, the queue admits
                    // only this turn's strict older lifecycle and keeps the
                    // exact target barrier installed.
                    let completion_evidence = services
                        .certified_serve_predecessor_completion_evidence(
                            executor.remaining_completion_capacity() != 0,
                            serve_barrier.scheduler_ordinal(),
                        )
                        .map_err(V2RunnerError::Service)?;
                    let predecessor_witness = executor.exact_serve_predecessor_episode_witness(
                        Instant::now(),
                        serve_barrier.scheduler_ordinal(),
                        completion_evidence,
                    )?;
                    if let Some(witness) = predecessor_witness {
                        let _ = services
                            .observe_certified_serve_predecessor_episode_witness(
                                serve_barrier,
                                witness,
                            )
                            .map_err(V2RunnerError::Service)?;
                    }
                    if predecessor_witness.is_some()
                        && services
                            .certified_serve_runtime_predecessor_capacity_available(serve_barrier)
                            .map_err(V2RunnerError::Service)?
                    {
                        if recovering_interrupted_tip {
                            let _ = advance_pending_tip_recovery_executor(
                                &mut executor,
                                &mut services,
                                1,
                            )?;
                        } else {
                            advance_executor_once_before_exact_serve(
                                &block_rx,
                                &mut executor,
                                &mut services,
                            )?;
                        }
                    }
                    if !recovering_interrupted_tip {
                        // A backpressured Serve ticket may retain the sole I/O
                        // unit indefinitely. It cannot suppress authenticated
                        // certified progress admission or an already-admitted
                        // certificate root.
                        drain_v2_ingress(
                            &block_rx,
                            &mut executor,
                            &mut services,
                            &mut lane_work,
                            output_guard.as_ref(),
                            kura.as_ref(),
                            &common_config.key_pair,
                            block_sync_server
                                .as_mut()
                                .expect("block-sync server initialized before ingress"),
                            &mut block_sync,
                            &mut block_sync_request,
                            &mut npos_vrf,
                            V2IngressDrainMode::CertifiedFenceEscape,
                            1,
                        )?;
                    }
                    let completion_evidence = services
                        .certified_serve_predecessor_completion_evidence(
                            executor.remaining_completion_capacity() != 0,
                            serve_barrier.scheduler_ordinal(),
                        )
                        .map_err(V2RunnerError::Service)?;
                    let predecessor_witness = executor.exact_serve_predecessor_episode_witness(
                        Instant::now(),
                        serve_barrier.scheduler_ordinal(),
                        completion_evidence,
                    )?;
                    if let Some(witness) = predecessor_witness {
                        let _ = services
                            .observe_certified_serve_predecessor_episode_witness(
                                serve_barrier,
                                witness,
                            )
                            .map_err(V2RunnerError::Service)?;
                    }
                    older_predecessor_remains = predecessor_witness.is_some();
                    services
                        .finish_certified_serve_runtime_episode_turn(
                            serve_barrier,
                            older_predecessor_remains,
                        )
                        .map_err(V2RunnerError::Service)?;
                }
                service_certified_serve_barrier_liveness_turn(
                    recovering_interrupted_tip,
                    claimed_older_runtime_episode,
                    |action| match action {
                        CertifiedServeBarrierLivenessAction::TimeoutVoteEpisode => {
                            // The one-shot older-runtime claim above cannot own
                            // the whole timeout-recovery episode. Admit one
                            // authenticated direct-roster TimeoutVote on every
                            // selected-Serve outer turn, including after that
                            // claim reaches Complete.
                            drain_v2_ingress(
                                &block_rx,
                                &mut executor,
                                &mut services,
                                &mut lane_work,
                                output_guard.as_ref(),
                                kura.as_ref(),
                                &common_config.key_pair,
                                block_sync_server
                                    .as_mut()
                                    .expect("block-sync server initialized before ingress"),
                                &mut block_sync,
                                &mut block_sync_request,
                                &mut npos_vrf,
                                V2IngressDrainMode::TimeoutVoteEpisode,
                                1,
                            )
                        }
                        CertifiedServeBarrierLivenessAction::TimeoutRecoveryPrefix => {
                            if let Some(timeout_recovery_cut) =
                                executor.timeout_recovery_lifecycle_cut()?
                            {
                                services.drain_timeout_recovery_prefix_completion(
                                    &mut executor,
                                    timeout_recovery_cut,
                                )?;
                            }
                            Ok(())
                        }
                        CertifiedServeBarrierLivenessAction::Pacemaker => {
                            advance_pacemaker_once(&block_rx, &mut executor, &mut services)
                        }
                    },
                )?;
                if !older_predecessor_remains {
                    // A full prefix may include an earlier Serve lifecycle whose
                    // auxiliary unit is released only after its sealed response is
                    // posted and acknowledged. Service at most one I/O completion
                    // from the prefix frozen by this target; the barrier prevents
                    // later I/O replenishment and the source-only turn excludes
                    // unrelated local completions.
                    services.drain_certified_serve_predecessor_completion(&mut executor)?;
                    if recovering_interrupted_tip {
                        drain_decided_lane_recovery_ingress(
                            &block_rx,
                            &executor,
                            &mut services,
                            &mut lane_work,
                            executor.current_tag().view(),
                            output_guard.as_ref(),
                            kura.as_ref(),
                            &common_config.key_pair,
                            block_sync_server
                                .as_mut()
                                .expect("block-sync server initialized before ingress"),
                        )?;
                    } else {
                        drain_v2_ingress(
                            &block_rx,
                            &mut executor,
                            &mut services,
                            &mut lane_work,
                            output_guard.as_ref(),
                            kura.as_ref(),
                            &common_config.key_pair,
                            block_sync_server
                                .as_mut()
                                .expect("block-sync server initialized before ingress"),
                            &mut block_sync,
                            &mut block_sync_request,
                            &mut npos_vrf,
                            V2IngressDrainMode::Ordinary,
                            1,
                        )?;
                    }
                }
                // Popping an ordinary frozen predecessor materializes the
                // barrier inside the worker queue lock. A Serve predecessor
                // retains its unit through completion posting, so the dedicated
                // source-only turn above acknowledges exactly that finite
                // prefix before another target turn.
                committed_lane_status_publisher.publish_if_changed(&lane_work);
                let _ = wake_rx.recv_timeout(IDLE_POLL);
                continue;
            }
            let Some(_certified_serve_producer_episode) = services
                .try_begin_certified_serve_producer_episode()
                .map_err(V2RunnerError::Service)?
            else {
                // Exact admission won the queue-locked race after the
                // observation above. Restart at the dedicated target turn.
                let _ = wake_rx.recv_timeout(IDLE_POLL);
                continue;
            };

            debug_assert!(startup_effects.is_empty());

            // Retry actor-owned output first, but keep servicing bounded
            // reducer and completion sources while one target is unavailable.
            // Each producer either transfers its complete fanout into the
            // corridor or retains the durable/reconstructible semantic source.
            let _ = retry_exact_output_and_apply_sidecar_admissions(
                &mut lane_work,
                &services,
                control_queue_capacity,
            )?;
            let advert_refresh = services
                .service_kura_replica_advert_refresh_turn(Instant::now())
                .map_err(V2RunnerError::Service)?;
            if advert_refresh.fanout_attempted {
                iroha_logger::debug!(
                    height = context.height,
                    probes = advert_refresh.probes,
                    retained_source = advert_refresh.retained_source,
                    scan_active = advert_refresh.scan_active,
                    "advanced bounded Kura replica-advert refresh"
                );
            }
            services.drain_completions(&mut executor)?;
            let _ = retry_exact_output_and_apply_sidecar_admissions(
                &mut lane_work,
                &services,
                control_queue_capacity,
            )?;
            if !recovering_interrupted_tip {
                let directive = reconcile_executor_locked_body(&mut executor, &mut services)?;
                local_proposal_state.reconcile(LocalProposalOwner::from(directive));
                lane_work.retain_merge_sidecars_for_global_view(
                    directive.tag().view(),
                    directive.locked_subject(),
                    directive.decided_subject(),
                )?;
                drive_merge_sidecar_recovery(&mut executor, &mut services, &mut lane_work)?;
                services
                    .replay_buffered_chunks(&mut executor)
                    .map_err(V2RunnerError::Service)?;
                while let Some(rejection) = services.take_validation_rejection() {
                    let current = executor.local_proposal_directive()?;
                    let expected_round = round_for_tag(&context, current.tag())?;
                    match local_proposal_state.handle_validation_rejection(
                        LocalProposalOwner::from(current),
                        expected_round,
                        rejection.round(),
                        rejection.subject(),
                    ) {
                        LocalValidationDisposition::Ignored => {}
                        LocalValidationDisposition::FatalNonEmpty => {
                            return Err(V2RunnerError::LocalNonEmptyRetryRejected(
                                rejection.reason().to_owned(),
                            ));
                        }
                        LocalValidationDisposition::RetryNonEmpty => {
                            iroha_logger::warn!(
                                reason = rejection.reason(),
                                "local Sumeragi v2 candidate rejected; retrying with non-empty work only"
                            );
                        }
                    }
                }

                let terminal_decision = directive.decided_subject().is_some();
                if !terminal_decision {
                    drive_block_sync(
                        Instant::now(),
                        &mut next_block_sync_attempt,
                        retransmit_interval,
                        &mut block_sync_request,
                        &mut block_sync,
                        &common_config.key_pair,
                        output_guard.as_ref(),
                        &services,
                    )?;
                }
                let discovery_was_outstanding = block_sync_request.is_some();
                drain_v2_ingress(
                    &block_rx,
                    &mut executor,
                    &mut services,
                    &mut lane_work,
                    output_guard.as_ref(),
                    kura.as_ref(),
                    &common_config.key_pair,
                    block_sync_server
                        .as_mut()
                        .expect("block-sync server initialized before ingress"),
                    &mut block_sync,
                    &mut block_sync_request,
                    &mut npos_vrf,
                    V2IngressDrainMode::Ordinary,
                    body_queue_capacity,
                )?;
                if discovery_was_outstanding && block_sync_request.is_none() {
                    // `drain_v2_ingress` retires this sole request only after
                    // the authenticated response's CommitQC is admitted to or
                    // coalesces with serialized reducer ownership. Preserve
                    // exactly that catch-up witness for the successor deadline.
                    admitted_discovered_commit_qc = true;
                }
                let _ = retry_exact_output_and_apply_sidecar_admissions(
                    &mut lane_work,
                    &services,
                    control_queue_capacity,
                )?;
                let directive = reconcile_executor_locked_body(&mut executor, &mut services)?;
                local_proposal_state.reconcile(LocalProposalOwner::from(directive));
                lane_work.retain_merge_sidecars_for_global_view(
                    directive.tag().view(),
                    directive.locked_subject(),
                    directive.decided_subject(),
                )?;
                drain_lane_relay_ingress(
                    &lane_relay_rx,
                    &mut lane_work,
                    executor.current_tag().view(),
                    control_queue_capacity,
                )?;
                drive_merge_sidecar_recovery(&mut executor, &mut services, &mut lane_work)?;
                let now = Instant::now();
                if now >= next_lane_retransmit {
                    let _ = service_historical_recovery_tick(&mut lane_work)?;
                    lane_work.schedule_autonomous_new_view_timeouts(
                        now,
                        executor.current_tag().view(),
                        round_timeout,
                    )?;
                    lane_work.schedule_retransmission()?;
                    next_lane_retransmit = deadline_after(now, retransmit_interval);
                }
                dispatch_lane_work_effects(&mut lane_work, &services, control_queue_capacity)?;
                let _ = retry_exact_output_and_apply_sidecar_admissions(
                    &mut lane_work,
                    &services,
                    control_queue_capacity,
                )?;
            } else {
                let directive = reconcile_executor_locked_body(&mut executor, &mut services)?;
                lane_work.retain_merge_sidecars_for_global_view(
                    directive.tag().view(),
                    directive.locked_subject(),
                    directive.decided_subject(),
                )?;
                drain_decided_lane_recovery_ingress(
                    &block_rx,
                    &executor,
                    &mut services,
                    &mut lane_work,
                    executor.current_tag().view(),
                    output_guard.as_ref(),
                    kura.as_ref(),
                    &common_config.key_pair,
                    block_sync_server
                        .as_mut()
                        .expect("block-sync server initialized before ingress"),
                )?;
                drain_lane_relay_ingress(
                    &lane_relay_rx,
                    &mut lane_work,
                    executor.current_tag().view(),
                    control_queue_capacity,
                )?;
                drive_merge_sidecar_recovery(&mut executor, &mut services, &mut lane_work)?;
                let now = Instant::now();
                if now >= next_lane_retransmit {
                    let _ = service_historical_recovery_tick(&mut lane_work)?;
                    lane_work.schedule_autonomous_new_view_timeouts(
                        now,
                        executor.current_tag().view(),
                        round_timeout,
                    )?;
                    lane_work.schedule_retransmission()?;
                    next_lane_retransmit = deadline_after(now, retransmit_interval);
                }
                dispatch_lane_work_effects(&mut lane_work, &services, control_queue_capacity)?;
            }

            if recovering_interrupted_tip {
                advance_pending_tip_recovery_executor(
                    &mut executor,
                    &mut services,
                    control_queue_capacity,
                )?;
            } else {
                advance_executor(
                    &block_rx,
                    &mut executor,
                    &mut services,
                    control_queue_capacity,
                )?;
                let _ = retry_exact_output_and_apply_sidecar_admissions(
                    &mut lane_work,
                    &services,
                    control_queue_capacity,
                )?;
                let directive = reconcile_executor_locked_body(&mut executor, &mut services)?;
                local_proposal_state.reconcile(LocalProposalOwner::from(directive));
                lane_work.retain_merge_sidecars_for_global_view(
                    directive.tag().view(),
                    directive.locked_subject(),
                    directive.decided_subject(),
                )?;
                if directive.decided_subject().is_none()
                    && let Some((locked_round, locked)) = directive.locked_body()
                {
                    let lock_outcome = lane_work.mark_global_body_locked(locked_round, locked)?;
                    if lock_outcome == GlobalBodyLockOutcome::Inserted && local_validator.is_some()
                    {
                        services
                            .request_locked_candidate(executor.current_tag(), locked_round, locked)
                            .map_err(V2RunnerError::Service)?;
                    }
                }
                while let Some(prepared) = services.take_prepared_candidate() {
                    let current = executor.local_proposal_directive()?;
                    if let Some(events) = local_proposal_state.take_prepared_events(
                        LocalProposalOwner::from(current),
                        prepared.tag(),
                        prepared.subject(),
                    ) {
                        let Some(_permit) = output_guard.acquire() else {
                            return Err(V2RunnerError::RestartRequired);
                        };
                        for event in events {
                            let _ = events_sender.send(EventBox::Pipeline(event));
                        }
                    }
                }
                services
                    .replay_buffered_chunks(&mut executor)
                    .map_err(V2RunnerError::Service)?;
            }

            committed_lane_status_publisher.publish_if_changed(&lane_work);
            if executor.ready_to_finish() {
                let (durable_receipt, durable_artifact) = executor
                    .durable_finality()
                    .map(|(receipt, artifact)| (receipt.clone(), artifact.clone()))
                    .ok_or_else(|| {
                        V2RunnerError::Service(
                            "ready Sumeragi v2 executor has no durable finality authority"
                                .to_owned(),
                        )
                    })?;
                height_ingress_bindings.retire()?;
                let (runtime, receipt, artifact) = executor.into_finalized_parts()?;
                let wal_retirement = output_guard
                    .begin_fail_stop_operation()
                    .ok_or(V2RunnerError::RestartRequired)?;
                let finalized = runtime.into_driver().finish_height(&receipt, &artifact)?;
                wal_retirement.complete();
                if let Some(warning) = finalized.wal_retirement_warning() {
                    iroha_logger::warn!(
                        height = receipt.height(),
                        context_id = ?receipt.context_id(),
                        block_hash = %receipt.block_hash(),
                        cleanup_target = PostFinalityCleanupTarget::SafetyWal.as_str(),
                        reason = warning,
                        "Sumeragi v2 finalized with retained local cleanup state"
                    );
                }
                committed_lane_status_publisher.publish_if_changed(&lane_work);
                debug_assert_eq!(durable_receipt.height(), receipt.height());
                debug_assert_eq!(durable_receipt.context_id(), receipt.context_id());
                debug_assert_eq!(durable_receipt.block_hash(), receipt.block_hash());
                debug_assert_eq!(durable_receipt.subject(), receipt.subject());
                debug_assert_eq!(durable_receipt.certificate(), receipt.certificate());
                debug_assert_eq!(durable_receipt.artifact_hash(), receipt.artifact_hash());
                debug_assert_eq!(durable_artifact, artifact);
                break (receipt, artifact, lane_work, services);
            }

            if recovering_interrupted_tip {
                // Global body/application recovery is local to the durable
                // Decision. Exact decided-lane votes or QCs may still wake
                // progress until their certificate and receipt are durable;
                // the recovery-specific executor rejects every
                // network-producing global reducer effect.
                committed_lane_status_publisher.publish_if_changed(&lane_work);
                let _ = wake_rx.recv_timeout(IDLE_POLL);
                continue;
            }

            schedule_local_proposal(
                candidate_limits,
                &context,
                local_validator,
                &common_config.key_pair,
                output_guard.as_ref(),
                state.as_ref(),
                &queue,
                kura.as_ref(),
                first_height_genesis.as_ref(),
                height_started_at,
                block_cadence,
                &mut local_proposal_state,
                &mut executor,
                &mut services,
                &mut lane_work,
                &npos_vrf,
                retransmit_interval,
            )?;
            dispatch_lane_work_effects(&mut lane_work, &services, control_queue_capacity)?;

            committed_lane_status_publisher.publish_if_changed(&lane_work);
            let _ = wake_rx.recv_timeout(IDLE_POLL);
        };

        let (receipt, artifact, lane_work, mut finalized_services) = finality;
        eager_block_sync =
            retain_eager_block_sync(recovering_interrupted_tip, admitted_discovered_commit_qc);
        let predecessor = DurableV2PredecessorIdentity::authenticate(&artifact, &receipt)?;
        let artifact_hash = HashOf::new(&artifact);
        let terminal_application =
            ProductionTerminalApplicationWithoutSuccessorActivationProjection {
                context_id: successor_context_refinement_projection(context.id()),
                context_height: context.height,
                receipt_context_id: successor_context_refinement_projection(receipt.context_id()),
                receipt_height: receipt.height(),
                receipt_block_hash: successor_block_refinement_projection(receipt.block_hash()),
                receipt_artifact_hash: CanonicalIdentityProjection::from_bytes(
                    IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                    IDENTITY_KIND_FINALITY_ARTIFACT,
                    *receipt.artifact_hash().as_ref(),
                ),
                artifact_context_id: successor_context_refinement_projection(artifact.context_id()),
                artifact_height: artifact.height,
                artifact_block_hash: successor_block_refinement_projection(artifact.block_hash),
                artifact_hash: CanonicalIdentityProjection::from_bytes(
                    IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                    IDENTITY_KIND_FINALITY_ARTIFACT,
                    *artifact_hash.as_ref(),
                ),
                predecessor: predecessor.refinement_projection(),
                pending_successor_activation_present: pending_successor_activation.is_some(),
            };
        let Some(checked_application) =
            check_production_terminal_application_transition(terminal_application)
        else {
            return Err(V2RunnerError::SuccessorRefinementRejected);
        };
        let _authorized_application = checked_application.into_projection();
        let activation = PendingSuccessorConstruction::begin(predecessor)?;
        let successor_construction = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        let successor =
            build_verified_successor(state.as_ref(), &context_store, &artifact, &receipt)?;
        successor_construction.complete();
        let (next_verified_context, successor_authority) = successor.into_parts();
        let next_context = next_verified_context.context().clone();
        retained_merge_sidecars = Some(rollover_finalized_height_outputs(
            lane_work,
            &finalized_services,
            &receipt,
            &artifact,
            &next_context,
            control_queue_capacity,
        )?);
        finalized_services.allow_clean_shutdown();
        let cleanup = finalized_services.finish_height(
            receipt.clone(),
            Duration::ZERO,
            &mut cleanup_supervisor,
        );
        for warning in cleanup.warnings() {
            iroha_logger::warn!(
                height = receipt.height(),
                context_id = ?receipt.context_id(),
                block_hash = %receipt.block_hash(),
                cleanup_target = warning.target().as_str(),
                reason = warning.reason(),
                "Sumeragi v2 finalized with retained local cleanup state"
            );
        }
        pending_successor_activation = Some(activation.bind(successor_authority)?);
        verified_context = next_verified_context;
        signature_policy = BlockSignaturePolicy::RotatingLeader;
        first_height_genesis = None;
        staged_genesis_nexus_amx_context = None;
    }
}

#[derive(Default)]
pub(super) struct CommittedLaneStatusPublisher {
    published_revision: Option<(u64, u64, u64)>,
}

impl CommittedLaneStatusPublisher {
    pub(super) fn publish_if_changed(&mut self, lane_work: &V2LaneWorkAdapter) -> bool {
        self.publish_if_changed_with(
            || lane_work.committed_lane_block_status_revision(),
            || lane_work.committed_lane_block_status_snapshot(),
        )
    }

    fn publish_if_changed_with(
        &mut self,
        mut observe_revision: impl FnMut() -> (u64, u64, u64),
        project: impl FnOnce() -> Vec<super::status::CommittedLaneBlockSnapshot>,
    ) -> bool {
        let revision = observe_revision();
        if self.published_revision == Some(revision) {
            return false;
        }
        let snapshot = project();
        if observe_revision() != revision {
            return false;
        }
        super::status::set_committed_lane_blocks(snapshot);
        self.published_revision = Some(revision);
        true
    }
}

fn replayed_proposal_sign(effects: &[AdapterEffect]) -> Option<ReplayedProposalSign> {
    effects.iter().find_map(|effect| match effect {
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(proposal),
        } => Some(ReplayedProposalSign {
            tag: *tag,
            round: proposal.round,
            subject: proposal.subject,
        }),
        AdapterEffect::Sign { .. }
        | AdapterEffect::Broadcast(_)
        | AdapterEffect::FetchBody { .. }
        | AdapterEffect::StoreBody { .. }
        | AdapterEffect::ValidateBody { .. }
        | AdapterEffect::Apply { .. }
        | AdapterEffect::EnterView { .. }
        | AdapterEffect::ReportEquivocation { .. }
        | AdapterEffect::ReportInvalidCertifiedBody { .. } => None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LockedBodyRecoveryPlan {
    request: Option<(EventTag, wire::ConsensusRound, wire::BlockSubject)>,
    may_repropose: bool,
}

fn locked_body_recovery_plan(
    directive: LocalProposalDirective,
    local_validator: wire::ValidatorIndex,
    attempted: Option<LocalProposalOwner>,
    can_admit_local_proposal: bool,
) -> LockedBodyRecoveryPlan {
    let owner = LocalProposalOwner::from(directive);
    let request = directive
        .decided_subject()
        .is_none()
        .then(|| directive.locked_body())
        .flatten()
        .map(|(round, subject)| (directive.tag(), round, subject));
    LockedBodyRecoveryPlan {
        request,
        may_repropose: request.is_some()
            && directive.leader() == local_validator
            && attempted != Some(owner)
            && can_admit_local_proposal,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LocalConsensusDuties {
    autonomous_lane_view: Option<wire::View>,
    global_validator: Option<wire::ValidatorIndex>,
}

fn local_consensus_duties(
    directive: LocalProposalDirective,
    global_validator: Option<wire::ValidatorIndex>,
) -> LocalConsensusDuties {
    LocalConsensusDuties {
        autonomous_lane_view: (directive.decided_subject().is_none()
            && directive.locked_body().is_none())
        .then_some(directive.tag().view()),
        global_validator,
    }
}

#[allow(clippy::too_many_arguments)]
fn schedule_local_proposal(
    candidate_limits: CandidateLimits,
    context: &wire::HeightContext,
    local_validator: Option<wire::ValidatorIndex>,
    key_pair: &KeyPair,
    output_guard: &ConsensusOutputGuard,
    state: &State,
    queue: &Arc<Queue>,
    kura: &Kura,
    genesis_body: Option<&SignedBlock>,
    height_started_at: Instant,
    block_cadence: Duration,
    proposal_state: &mut LocalProposalState,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
    npos_vrf: &V2NposVrfLifecycle,
    candidate_work_wait_bound: Duration,
) -> Result<(), V2RunnerError> {
    let directive = executor.local_proposal_directive()?;
    let duties = local_consensus_duties(directive, local_validator);
    // Lane authority is frozen independently from the successor global
    // roster. A configured validator removed from that roster must still
    // produce a lane payload when the exact current lane descriptor selects
    // it as author. The adapter rechecks voting role, route, and slot author.
    if let Some(active_view) = duties.autonomous_lane_view {
        lane_work.schedule_autonomous_lane_production(active_view, candidate_limits)?;
    }
    let Some(local_validator) = duties.global_validator else {
        return Ok(());
    };
    let owner = proposal_state.reconcile(LocalProposalOwner::from(directive));
    let recovery_plan = locked_body_recovery_plan(
        directive,
        local_validator,
        proposal_state.attempted,
        executor.can_schedule_local_proposal()?,
    );
    // Bind immutable locked-body acquisition to the current reducer
    // incarnation before observing proposal eligibility. Every validator must
    // authenticate and bind the exact carrier even when it is not the current
    // global leader or has already attempted that proposal; only the later
    // unchanged reproposal remains leader-gated.
    if let Some((tag, locked_round, locked)) = recovery_plan.request {
        services
            .request_locked_candidate(tag, locked_round, locked)
            .map_err(V2RunnerError::Service)?;
    }
    while let Some(loaded) = services.take_loaded_candidate() {
        let current = executor.local_proposal_directive()?;
        let loaded_round = loaded.round();
        let loaded_subject = loaded.subject();
        if loaded.tag() != current.tag()
            || current.locked_body() != Some((loaded_round, loaded_subject))
        {
            iroha_logger::debug!(
                loaded_height = loaded.tag().height(),
                loaded_view = loaded.tag().view(),
                current_height = current.tag().height(),
                current_view = current.tag().view(),
                loaded_subject = ?loaded.subject(),
                current_locked_subject = ?current.locked_subject(),
                "discarded stale locked-body load before Sumeragi v2 reproposal"
            );
            continue;
        }
        let canonical_wire = loaded.into_canonical_wire();
        let block = iroha_data_model::block::decode_framed_signed_block(&canonical_wire)
            .map_err(|error| V2RunnerError::Candidate(error.to_string()))?;
        if !block.is_resultless_proposal() {
            return Err(V2RunnerError::ResultBearingProposal);
        }
        let time_trigger_clock_progress_required = block
            .header()
            .creation_time()
            .checked_sub(block_cadence)
            .is_some_and(|parent_creation_time| {
                state.time_trigger_clock_progress_required_fast(parent_creation_time)
            });
        if !candidate_block_has_proposal_work(&block, time_trigger_clock_progress_required) {
            return Err(V2RunnerError::EmptyProposalWork);
        }
        let lane_binding = if context.height == 1 {
            let authenticated_genesis = genesis_body.ok_or(V2RunnerError::MissingGenesisBody)?;
            lane_work.bind_locked_genesis_body(&block, authenticated_genesis)
        } else {
            lane_work.bind_locked_global_body(&block)
        };
        if lane_binding == V2LaneIngressOutcome::Rejected {
            return Err(V2RunnerError::LaneCandidateBinding);
        }
        let current_owner = proposal_state.reconcile(LocalProposalOwner::from(current));
        let current_recovery_plan = locked_body_recovery_plan(
            current,
            local_validator,
            proposal_state.attempted,
            executor.can_schedule_local_proposal()?,
        );
        if !current_recovery_plan.may_repropose {
            continue;
        }
        // Keep the immutable bytes available under the PrepareQC round before
        // minting the later-round proposal. This preserves both admissible
        // progress branches: an already-started exact-origin recovery can
        // finish, while the current leader re-proposes the same subject
        // unchanged.
        executor.retain_locked_body_for_recovery(
            current.tag(),
            loaded_round,
            loaded_subject,
            canonical_wire.clone(),
            services,
        )?;
        submit_exact_body(
            context,
            current,
            canonical_wire,
            executor,
            services,
            proposal_state,
        )?;
        proposal_state.attempted = Some(current_owner);
        iroha_logger::debug!(
            height = current.tag().height(),
            proposal_view = current.tag().view(),
            locked_view = loaded_round.view,
            subject = ?loaded_subject,
            local_validator,
            "submitted exact locked body for local Sumeragi v2 reproposal"
        );
        return Ok(());
    }
    if directive.decided_subject().is_some() {
        return Ok(());
    }
    if directive.leader() != local_validator
        || proposal_state.attempted == Some(owner)
        || (directive.tag().view() == 0
            && height_started_at.elapsed() < block_cadence
            && context.height > 1)
    {
        return Ok(());
    }
    // Do not consume a fresh or retained candidate until the executor can
    // reserve its local StoreBody owner. Timers, retransmission, and
    // completions continue while this producer waits.
    if !executor.can_schedule_local_proposal()? {
        return Ok(());
    }
    // A locked directive can only consume its exact retained body. It must
    // never fall through to fresh candidate or genesis construction while the
    // asynchronous load is pending.
    if directive.locked_body().is_some() {
        return Ok(());
    }

    if context.height == 1 {
        let body = genesis_body.ok_or(V2RunnerError::MissingGenesisBody)?;
        // Genesis staging retains its deterministic execution image for application, while
        // consensus authenticates the canonical resultless proposal. Project exactly once at
        // that boundary; every downstream proposal path remains strict about result-bearing data.
        submit_exact_body(
            context,
            directive,
            canonical_height_one_proposal_wire(body)?,
            executor,
            services,
            proposal_state,
        )?;
        proposal_state.attempted = Some(owner);
    } else {
        if proposal_state
            .candidate_work_wait
            .is_some_and(|wait| wait.owner == owner && Instant::now() < wait.next_retry)
        {
            return Ok(());
        }
        let parent_body = if let Some(anchor) = &context.snapshot_bootstrap {
            let parent_height = NonZeroUsize::new(usize::try_from(anchor.snapshot_height)?)
                .ok_or(V2RunnerError::InvalidSnapshotBootstrapParent)?;
            if kura.get_block(parent_height).is_some() {
                return Err(V2RunnerError::InvalidSnapshotBootstrapParent);
            }
            None
        } else {
            let parent_height = usize::try_from(context.height.saturating_sub(1))?;
            let parent_height =
                NonZeroUsize::new(parent_height).ok_or(V2RunnerError::MissingParent)?;
            Some(
                kura.get_block(parent_height)
                    .ok_or(V2RunnerError::MissingParent)?,
            )
        };
        let (parent, logical_time) =
            match (context.snapshot_bootstrap.as_ref(), parent_body.as_deref()) {
                (Some(anchor), None) => (
                    CandidateParent::Snapshot(anchor),
                    snapshot_successor_logical_time(anchor, block_cadence)?,
                ),
                (None, Some(parent)) => {
                    let logical_time = parent
                        .header()
                        .creation_time()
                        .checked_add(block_cadence)
                        .ok_or(V2RunnerError::V2BlockTimeOverflow)?;
                    u64::try_from(logical_time.as_millis())
                        .map_err(|_| V2RunnerError::V2BlockTimeOverflow)?;
                    (CandidateParent::Block(parent), logical_time)
                }
                _ => return Err(V2RunnerError::InvalidSnapshotBootstrapParent),
            };
        let carrier_context_header =
            lane_work.merge_carrier_context_header(directive.tag().view())?;
        if carrier_context_header.creation_time() != logical_time {
            return Err(V2RunnerError::Candidate(
                "shared merge carrier timestamp differs from the frozen height cadence".to_owned(),
            ));
        }
        let (_, time_source) =
            iroha_primitives::time::TimeSource::new_mock(carrier_context_header.creation_time());
        let assembler = V2CandidateAssembler::new(candidate_limits, time_source.clone());
        let attachments = candidate_attachments(
            context,
            state,
            parent,
            directive.tag().view(),
            &carrier_context_header,
            npos_vrf,
        )?;
        let assembly = assembler.assemble(CandidateRequest {
            context,
            directive,
            local_validator,
            parent,
            state,
            queue,
            key_pair,
            output_guard,
            attachments,
            work_provider: &mut *lane_work,
        })?;
        let candidate = match assembly {
            CandidateAssemblyOutcome::Assembled(candidate) => candidate,
            CandidateAssemblyOutcome::NoProposalWork(report) => {
                let now = Instant::now();
                proposal_state.retire_unsubmitted_non_empty_retry(owner);
                if report.work_deferred > 0 {
                    proposal_state.defer_candidate_work(owner, now, candidate_work_wait_bound);
                } else {
                    // A clean idle height is not recovery work. Keep a bounded
                    // recheck so asynchronous queue/lane arrivals are observed
                    // promptly without manufacturing a cadence heartbeat.
                    proposal_state.candidate_work_wait = Some(CandidateWorkWait {
                        owner,
                        started_at: now,
                        next_retry: deadline_after(now, CANDIDATE_WORK_RECHECK),
                    });
                }
                iroha_logger::trace!(
                    height = owner.tag.height(),
                    view = owner.tag.view(),
                    ?report,
                    "deferred Sumeragi v2 proposal because no proposal work is ready"
                );
                return Ok(());
            }
        };
        let tag = candidate.tag();
        if tag != owner.tag {
            return Err(V2RunnerError::StaleTag);
        }
        let report = candidate.scan_report();
        if proposal_state.non_empty_retry != Some(owner)
            && report.selected == 0
            && report.work_deferred > 0
        {
            let now = Instant::now();
            proposal_state.defer_candidate_work(owner, now, candidate_work_wait_bound);
            return Ok(());
        }
        proposal_state.candidate_work_wait = None;
        if lane_work.bind_local_candidate(round_for_tag(context, tag)?, candidate.block().hash())
            == V2LaneIngressOutcome::Rejected
        {
            // Binding happens before body storage and reducer submission.
            // Drop the abandoned candidate first so its ordinary queue lease
            // is available to a later-height reproposal.
            drop(candidate);
            match proposal_state.handle_candidate_binding_rejection(owner) {
                LocalValidationDisposition::RetryNonEmpty => {
                    iroha_logger::warn!(
                        height = tag.height(),
                        view = tag.view(),
                        "discarded an unsubmitted candidate after lane-local ownership binding rejected; retrying with non-empty work only"
                    );
                    return Ok(());
                }
                LocalValidationDisposition::FatalNonEmpty | LocalValidationDisposition::Ignored => {
                    return Err(V2RunnerError::LaneCandidateBinding);
                }
            }
        }
        let (_block, canonical_wire, encoded_payload, events, report, selection_lease) =
            candidate.into_parts();
        let subject = encoded_payload.manifest().subject;
        proposal_state.pending_events = Some(PendingLocalEvents {
            owner,
            subject,
            events,
        });
        iroha_logger::debug!(?report, "assembled bounded Sumeragi v2 candidate");
        submit_encoded_body(
            owner,
            canonical_wire,
            encoded_payload,
            executor,
            services,
            proposal_state,
        )?;
        proposal_state.global_selection = Some(PendingGlobalSelection {
            owner,
            subject,
            _lease: selection_lease,
        });
        proposal_state.attempted = Some(owner);
    }

    Ok(())
}

fn canonical_height_one_proposal_wire(body: &SignedBlock) -> Result<Vec<u8>, V2RunnerError> {
    body.canonical_resultless_proposal()
        .encode_wire()
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))
}

fn submit_exact_body(
    context: &wire::HeightContext,
    directive: LocalProposalDirective,
    canonical_wire: Vec<u8>,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    proposal_state: &mut LocalProposalState,
) -> Result<(), V2RunnerError> {
    let payload = encode_exact_local_body(
        context,
        directive.tag(),
        directive.locked_subject(),
        &canonical_wire,
    )?;
    submit_encoded_body(
        LocalProposalOwner::from(directive),
        canonical_wire,
        payload,
        executor,
        services,
        proposal_state,
    )
}

fn encode_exact_local_body(
    context: &wire::HeightContext,
    tag: EventTag,
    locked_subject: Option<wire::BlockSubject>,
    canonical_wire: &[u8],
) -> Result<EncodedV2Payload, V2RunnerError> {
    let block = iroha_data_model::block::decode_framed_signed_block(canonical_wire)
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))?;
    if !block.is_resultless_proposal() {
        return Err(V2RunnerError::ResultBearingProposal);
    }
    let subject = wire::BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    if locked_subject.is_some_and(|locked| locked != subject) {
        return Err(V2RunnerError::LockedBodyMismatch);
    }
    let round = round_for_tag(context, tag)?;
    encode_payload(context, round, subject, canonical_wire)
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))
}

fn submit_encoded_body(
    owner: LocalProposalOwner,
    canonical_wire: Vec<u8>,
    payload: EncodedV2Payload,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    proposal_state: &mut LocalProposalState,
) -> Result<(), V2RunnerError> {
    let manifest = services
        .register_outbound_payload(owner.tag, payload)
        .map_err(V2RunnerError::Service)?;
    proposal_state.submitted = Some((owner, manifest.subject));
    executor.admit_local_proposal(owner.tag, manifest, canonical_wire, services)?;
    Ok(())
}

fn drive_block_sync(
    now: Instant,
    next_attempt: &mut Instant,
    retransmit_interval: Duration,
    request_hash: &mut Option<HashOf<wire::CommitCertificateRequest>>,
    discovery: &mut V2BlockSyncDiscovery,
    key_pair: &KeyPair,
    output_guard: &ConsensusOutputGuard,
    services: &ProductionV2Services,
) -> Result<(), V2RunnerError> {
    if now < *next_attempt {
        return Ok(());
    }

    let next = deadline_after(now, retransmit_interval);
    if let Some(hash) = request_hash.as_ref() {
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        let message = discovery
            .retransmit(*hash)
            .ok_or(V2RunnerError::BlockSyncRequestDisappeared)?;
        services
            .broadcast_to_voters_while_guarded(message, operation.permit())
            .map_err(V2RunnerError::Service)?;
        operation.complete();
    } else {
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        let message = discovery.begin(key_pair)?;
        let wire::ConsensusMessageV2Payload::CommitCertificateRequest(request) = &message.payload
        else {
            return Err(V2RunnerError::BlockSyncRequestDisappeared);
        };
        *request_hash = Some(HashOf::new(request));
        if let Err(error) = services.broadcast_to_voters_while_guarded(message, operation.permit())
        {
            drop(operation);
            return Err(V2RunnerError::Service(error));
        }
        operation.complete();
    }
    *next_attempt = next;
    Ok(())
}

fn broadcast_npos_vrf_messages(
    messages: impl IntoIterator<Item = wire::ConsensusMessageV2>,
    output_guard: &ConsensusOutputGuard,
    services: &ProductionV2Services,
) -> Result<(), V2RunnerError> {
    for message in messages {
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        services
            .broadcast_to_voters_while_guarded(message, operation.permit())
            .map_err(V2RunnerError::Service)?;
        operation.complete();
    }
    Ok(())
}

enum PreparedCertifiedServe {
    Admitted(CertifiedServeAdmission),
    Rejected(String),
    Service(String),
}

enum DecidedLaneRecoveryCurrentServe {
    Authenticated {
        authenticated_via: PeerId,
        request: AuthenticatedCertifiedBodyRequest,
    },
    Negative {
        request_hash: HashOf<wire::CertifiedBodyRequest>,
        outcome: CertifiedServeNegativeOutcome,
        reason: String,
    },
    Service(String),
}

enum DecidedLaneRecoveryIngressPreparation {
    LaneLocal,
    KuraReplicaAdvert,
    CurrentServe(DecidedLaneRecoveryCurrentServe),
    HistoricalServe,
    LeaderWireRetire,
}

enum DecidedLaneRecoveryCurrentDrain<Admission> {
    Admitted(Admission),
    Rejected(String),
}

enum DecidedLaneRecoveryDrainAuthorization<Admission> {
    LaneLocal,
    KuraReplicaAdvert,
    CurrentServe(DecidedLaneRecoveryCurrentDrain<Admission>),
    HistoricalServe,
    LeaderWireRetire,
}

enum DecidedLaneRecoveryDrainDecision<Admission> {
    Retain,
    Authorized(DecidedLaneRecoveryDrainAuthorization<Admission>),
    FailClosed(String),
}

trait DecidedLaneRecoveryDrainAuthorizer {
    type Admission;

    fn stage_negative(
        &mut self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
        outcome: CertifiedServeNegativeOutcome,
    ) -> Result<(), String>;

    fn prepare_exact(
        &mut self,
        authenticated_via: &PeerId,
        request: AuthenticatedCertifiedBodyRequest,
    ) -> Result<Self::Admission, CertifiedServePrepareError>;
}

impl DecidedLaneRecoveryDrainAuthorizer for ProductionV2Services {
    type Admission = CertifiedServeAdmission;

    fn stage_negative(
        &mut self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
        outcome: CertifiedServeNegativeOutcome,
    ) -> Result<(), String> {
        self.stage_certified_serve_rejection(request_hash, outcome)
    }

    fn prepare_exact(
        &mut self,
        authenticated_via: &PeerId,
        request: AuthenticatedCertifiedBodyRequest,
    ) -> Result<Self::Admission, CertifiedServePrepareError> {
        self.prepare_certified_request(authenticated_via, request)
    }
}

fn authorize_decided_lane_recovery_drain<A: DecidedLaneRecoveryDrainAuthorizer>(
    preparation: DecidedLaneRecoveryIngressPreparation,
    authorizer: &mut A,
) -> DecidedLaneRecoveryDrainDecision<A::Admission> {
    match preparation {
        DecidedLaneRecoveryIngressPreparation::LaneLocal => {
            DecidedLaneRecoveryDrainDecision::Authorized(
                DecidedLaneRecoveryDrainAuthorization::LaneLocal,
            )
        }
        DecidedLaneRecoveryIngressPreparation::KuraReplicaAdvert => {
            DecidedLaneRecoveryDrainDecision::Authorized(
                DecidedLaneRecoveryDrainAuthorization::KuraReplicaAdvert,
            )
        }
        DecidedLaneRecoveryIngressPreparation::HistoricalServe => {
            DecidedLaneRecoveryDrainDecision::Authorized(
                DecidedLaneRecoveryDrainAuthorization::HistoricalServe,
            )
        }
        DecidedLaneRecoveryIngressPreparation::LeaderWireRetire => {
            DecidedLaneRecoveryDrainDecision::Authorized(
                DecidedLaneRecoveryDrainAuthorization::LeaderWireRetire,
            )
        }
        DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Negative {
                request_hash,
                outcome,
                reason,
            },
        ) => match authorizer.stage_negative(request_hash, outcome) {
            Ok(()) => DecidedLaneRecoveryDrainDecision::Authorized(
                DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                    DecidedLaneRecoveryCurrentDrain::Rejected(reason),
                ),
            ),
            Err(error) => DecidedLaneRecoveryDrainDecision::FailClosed(error),
        },
        DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Service(reason),
        ) => DecidedLaneRecoveryDrainDecision::FailClosed(reason),
        DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Authenticated {
                authenticated_via,
                request,
            },
        ) => match authorizer.prepare_exact(&authenticated_via, request) {
            Ok(admission) => DecidedLaneRecoveryDrainDecision::Authorized(
                DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                    DecidedLaneRecoveryCurrentDrain::Admitted(admission),
                ),
            ),
            Err(CertifiedServePrepareError::Backpressure) => {
                DecidedLaneRecoveryDrainDecision::Retain
            }
            Err(CertifiedServePrepareError::Rejected(reason)) => {
                DecidedLaneRecoveryDrainDecision::Authorized(
                    DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                        DecidedLaneRecoveryCurrentDrain::Rejected(reason),
                    ),
                )
            }
            Err(CertifiedServePrepareError::Service(reason)) => {
                DecidedLaneRecoveryDrainDecision::FailClosed(reason)
            }
        },
    }
}

fn prepare_decided_lane_recovery_ingress(
    inbound: &InboundBlockMessage,
    active_height: wire::Height,
    decided_subject: wire::BlockSubject,
    authenticate: impl FnOnce(
        wire::CertifiedBodyRequest,
        &PeerId,
    ) -> Result<AuthenticatedCertifiedBodyRequest, String>,
) -> DecidedLaneRecoveryIngressPreparation {
    if matches!(inbound.message(), BlockMessage::KuraReplicaAdvert(_)) {
        return DecidedLaneRecoveryIngressPreparation::KuraReplicaAdvert;
    }
    if inbound.message().is_lane_local() {
        return DecidedLaneRecoveryIngressPreparation::LaneLocal;
    }
    let BlockMessage::V2(message) = inbound.message() else {
        return DecidedLaneRecoveryIngressPreparation::LeaderWireRetire;
    };
    let wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) = &message.payload else {
        return DecidedLaneRecoveryIngressPreparation::LeaderWireRetire;
    };
    if message.validate_version().is_err() {
        return DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Service(
                "terminal recovery Serve ingress crossed version validation".to_owned(),
            ),
        );
    }
    if request.round.height < active_height {
        return DecidedLaneRecoveryIngressPreparation::HistoricalServe;
    }
    if request.round.height > active_height {
        return DecidedLaneRecoveryIngressPreparation::LeaderWireRetire;
    }
    let Some(sender) = inbound.sender() else {
        return DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Service(
                "terminal recovery Serve ingress lost its authenticated sender".to_owned(),
            ),
        );
    };
    let Some(authenticated_via) = inbound.via() else {
        return DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Service(
                "terminal recovery Serve ingress lost its authenticated source".to_owned(),
            ),
        );
    };
    let Some(reply_routes) = inbound.reply_routes() else {
        return DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Service(
                "terminal recovery Serve ingress lost its reply capability".to_owned(),
            ),
        );
    };
    let Some(ingress_ownership) = inbound.ingress_ownership() else {
        return DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Service(
                "terminal recovery Serve ingress lost its ownership evidence".to_owned(),
            ),
        );
    };
    if reply_routes.semantic_target() != sender
        || !ingress_ownership.validate_exact()
        || !ingress_ownership.matches_message(inbound.message())
        || !ingress_ownership.matches_semantic_origin(Some(sender))
        || !ingress_ownership.matches_reply_routes(Some(reply_routes))
    {
        return DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Service(
                "terminal recovery Serve ingress changed its transport ownership".to_owned(),
            ),
        );
    }
    let authenticated = match authenticate(request.clone(), sender) {
        Ok(authenticated) => authenticated,
        Err(reason) => {
            return DecidedLaneRecoveryIngressPreparation::CurrentServe(
                DecidedLaneRecoveryCurrentServe::Negative {
                    request_hash: HashOf::new(request),
                    outcome: CertifiedServeNegativeOutcome::InvalidCertificate,
                    reason,
                },
            );
        }
    };
    if request.subject != decided_subject {
        return DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Negative {
                request_hash: authenticated.request_hash(),
                outcome: CertifiedServeNegativeOutcome::SupersededByDurableDecision(
                    decided_subject,
                ),
                reason: "terminal recovery Serve request was superseded by durable Decision"
                    .to_owned(),
            },
        );
    }
    DecidedLaneRecoveryIngressPreparation::CurrentServe(
        DecidedLaneRecoveryCurrentServe::Authenticated {
            authenticated_via: authenticated_via.clone(),
            request: authenticated,
        },
    )
}

#[derive(Debug)]
enum KuraReplicaAdvertAdmissionError {
    InvalidAdvert(String),
    Fatal(crate::kura::Error),
}

fn classify_kura_replica_advert_admission_error(
    error: crate::kura::Error,
) -> KuraReplicaAdvertAdmissionError {
    match error {
        crate::kura::Error::InvalidKuraReplicaAdvert(reason) => {
            KuraReplicaAdvertAdmissionError::InvalidAdvert(reason)
        }
        error => KuraReplicaAdvertAdmissionError::Fatal(error),
    }
}

/// Consume one fixed-small authenticated Kura replica advert without exposing
/// it to either consensus reducer.
///
/// Fair admission already checks the signature and direct transport binding.
/// This terminal seam repeats the complete local ownership proof so mutation
/// of the queued carrier fails closed. Kura then revalidates the exact durable
/// body, finality artifact, CommitQC signer, and deterministic keeper set; a
/// remotely invalid claim is simply retired.
fn admit_kura_replica_advert_ingress(
    receiver: &FairV2Ingress,
    kura: &Kura,
    mut inbound: InboundBlockMessage,
) -> Result<(), V2RunnerError> {
    let advertised_keeper = match inbound.message() {
        BlockMessage::KuraReplicaAdvert(advert) => advert.keeper.clone(),
        _ => {
            return Err(V2RunnerError::Service(
                "Kura replica advert terminal received another message class".to_owned(),
            ));
        }
    };
    let authenticated_via = inbound.via().cloned();
    let mut ingress_ownership = inbound.take_ingress_ownership().ok_or_else(|| {
        V2RunnerError::Service(
            "Kura replica advert lost its fair-ingress ownership carrier".to_owned(),
        )
    })?;
    if !ingress_ownership.validate_exact()
        || !ingress_ownership.matches_message(inbound.message())
        || !ingress_ownership.matches_semantic_origin(inbound.sender())
        || !ingress_ownership.matches_reply_routes(inbound.reply_routes())
    {
        return Err(V2RunnerError::Service(
            "Kura replica advert carried altered fair-ingress ownership".to_owned(),
        ));
    }
    receiver
        .bind_leader_wire_runtime_ownership(&mut ingress_ownership)
        .map_err(V2RunnerError::Service)?;
    let (message, sender, reply_routes) = inbound.into_message_sender_and_reply_routes();
    let BlockMessage::KuraReplicaAdvert(advert) = message else {
        return Err(V2RunnerError::Service(
            "Kura replica advert changed message class after ownership validation".to_owned(),
        ));
    };
    if sender.as_ref() != Some(&advertised_keeper)
        || authenticated_via.as_ref() != Some(&advertised_keeper)
        || advert.keeper != advertised_keeper
        || !ingress_ownership.matches_reply_routes(reply_routes.as_ref())
    {
        return Err(V2RunnerError::Service(
            "Kura replica advert changed its direct authenticated keeper route".to_owned(),
        ));
    }
    match kura.admit_kura_replica_advert(&advert) {
        Ok(()) => {
            iroha_logger::debug!(
                height = advert.height,
                keeper = %advert.keeper,
                "admitted authenticated Kura replica advert"
            );
        }
        Err(error) => match classify_kura_replica_advert_admission_error(error) {
            KuraReplicaAdvertAdmissionError::InvalidAdvert(reason) => {
                iroha_logger::debug!(
                    %reason,
                    height = advert.height,
                    keeper = %advert.keeper,
                    "retired invalid Kura replica advert"
                );
            }
            KuraReplicaAdvertAdmissionError::Fatal(error) => {
                return Err(V2RunnerError::Service(format!(
                    "Kura replica advert admission encountered local durable-state failure: {error}"
                )));
            }
        },
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum V2IngressDrainMode {
    /// Normal completion/runtime/ingress round-robin.
    Ordinary,
    /// Only a TC/CommitQC which can supersede a hung signing fence.
    CertifiedFenceEscape,
    /// Only one member of the finite current-view TimeoutVote producer episode.
    /// This remains available after a retained response spends its separate
    /// certificate credit.
    TimeoutVoteEpisode,
}

fn drain_v2_ingress(
    receiver: &FairV2Ingress,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
    output_guard: &ConsensusOutputGuard,
    kura: &Kura,
    local_key: &KeyPair,
    block_sync_server: &mut V2BlockSyncServer,
    block_sync: &mut V2BlockSyncDiscovery,
    block_sync_request: &mut Option<HashOf<wire::CommitCertificateRequest>>,
    npos_vrf: &mut V2NposVrfLifecycle,
    mode: V2IngressDrainMode,
    limit: usize,
) -> Result<(), V2RunnerError> {
    if mode == V2IngressDrainMode::Ordinary && executor.has_retained_certified_body_response() {
        // The dedicated outer episode owns all progress until this exact
        // transport completion either crosses capacity or reaches a permanent
        // terminal. Do not give even the Runtime half-turn of a new batch to a
        // later owner.
        return Ok(());
    }
    let mut outer_turns =
        outer_ingress_turns(limit, executor.context().id(), executor.context().height);
    while let Some(current_turn) = outer_turns.next_current() {
        let turn = current_turn.turn();
        if mode != V2IngressDrainMode::Ordinary && turn != OuterIngressTurn::Ingress {
            continue;
        }
        if turn == OuterIngressTurn::Completion {
            if services
                .certified_serve_barrier_request_hash()
                .map_err(V2RunnerError::Service)?
                .is_some()
            {
                // A provisional or prepared exact target owns this turn. The
                // outer runner services it before any queued completion.
                continue;
            }
            // I/O completion is a separate producer from the serialized
            // reducer. Service it before every ingress occurrence so a
            // completed durable store cannot remain hidden for the duration
            // of a large authenticated ingress batch.
            services.drain_completions(executor)?;
            continue;
        }
        if turn == OuterIngressTurn::Runtime {
            if services
                .certified_serve_barrier()
                .map_err(V2RunnerError::Service)?
                .is_some()
            {
                // A provisional or prepared exact target owns this turn. The
                // outer runner services it before any queued runtime producer.
                continue;
            }
            // A whole authenticated ingress batch can be expensive. Give the
            // serialized runtime one service turn after completions and before
            // every outer occurrence so trusted timers and reducer work cannot
            // remain hidden behind that batch.
            let was_terminal = executor
                .local_proposal_directive()?
                .decided_subject()
                .is_some();
            advance_executor(receiver, executor, services, 1)?;
            let is_terminal = executor
                .local_proposal_directive()?
                .decided_subject()
                .is_some();
            if !was_terminal && is_terminal {
                // Publish the new terminal carrier to lane work before any
                // further ingress occurrence can be admitted. In particular,
                // do not use a pre-batch snapshot to enqueue another global
                // reducer event after this runtime turn installed Decision.
                return Ok(());
            }
            continue;
        }
        let terminal_subject = executor.local_proposal_directive()?.decided_subject();
        let terminal_decision = terminal_subject.is_some();
        let mut prepared_serve = None;
        let barrier_bypass = match mode {
            V2IngressDrainMode::TimeoutVoteEpisode => {
                FairV2IngressBarrierBypass::TimeoutVoteEpisode
            }
            V2IngressDrainMode::Ordinary | V2IngressDrainMode::CertifiedFenceEscape => {
                FairV2IngressBarrierBypass::None
            }
        };
        let Some((mut inbound, dequeue_disposition)) = receiver
            .try_recv_if_checked_retiring_obsolete_with_barrier_bypass(barrier_bypass, |inbound| {
                if mode != V2IngressDrainMode::Ordinary {
                    let BlockMessage::V2(message) = inbound.message() else {
                        return false;
                    };
                    if message.validate_version().is_err() {
                        return false;
                    }
                    let selected_mode_matches = match mode {
                        V2IngressDrainMode::Ordinary => true,
                        V2IngressDrainMode::CertifiedFenceEscape => {
                            network_ingress_is_certified_fence_escape(&message.payload)
                        }
                        V2IngressDrainMode::TimeoutVoteEpisode => {
                            inbound.ingress_ownership().is_some_and(|ownership| {
                                executor.can_admit_timeout_vote_recovery_episode(message, ownership)
                            })
                        }
                    };
                    if !selected_mode_matches {
                        return false;
                    }
                }
                if !v2_ingress_head_can_drain(inbound, executor, terminal_subject) {
                    return false;
                }
                let BlockMessage::V2(message) = inbound.message() else {
                    return true;
                };
                if message.validate_version().is_err() {
                    prepared_serve = Some(PreparedCertifiedServe::Service(
                        "reserved certified-body ingress crossed version validation".to_owned(),
                    ));
                    return true;
                }
                let wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) =
                    &message.payload
                else {
                    return true;
                };
                if request.round.height != executor.context().height {
                    return true;
                }
                let superseded_by_decision = certified_body_request_is_superseded_after_decision(
                    request,
                    terminal_subject,
                    executor.context().height,
                );
                let Some(sender) = inbound.sender() else {
                    prepared_serve = Some(PreparedCertifiedServe::Service(
                        "reserved certified-body ingress lost its authenticated sender".to_owned(),
                    ));
                    return true;
                };
                let Some(authenticated_via) = inbound.via() else {
                    prepared_serve = Some(PreparedCertifiedServe::Service(
                        "reserved certified-body ingress lost its authenticated source".to_owned(),
                    ));
                    return true;
                };
                let Some(reply_routes) = inbound.reply_routes() else {
                    prepared_serve = Some(PreparedCertifiedServe::Service(
                        "reserved certified-body ingress lost its reply capability".to_owned(),
                    ));
                    return true;
                };
                let Some(ingress_ownership) = inbound.ingress_ownership() else {
                    prepared_serve = Some(PreparedCertifiedServe::Service(
                        "reserved certified-body ingress lost its ownership evidence".to_owned(),
                    ));
                    return true;
                };
                if reply_routes.semantic_target() != sender
                    || !ingress_ownership.validate_exact()
                    || !ingress_ownership.matches_message(inbound.message())
                    || !ingress_ownership.matches_semantic_origin(Some(sender))
                    || !ingress_ownership.matches_reply_routes(Some(reply_routes))
                {
                    prepared_serve = Some(PreparedCertifiedServe::Service(
                        "reserved certified-body ingress changed its transport ownership"
                            .to_owned(),
                    ));
                    return true;
                }
                let authenticated =
                    match executor.authenticate_certified_body_request(request.clone(), sender) {
                        Ok(authenticated) => authenticated,
                        Err(error) => {
                            prepared_serve = Some(
                                match services.stage_certified_serve_rejection(
                                    HashOf::new(request),
                                    CertifiedServeNegativeOutcome::InvalidCertificate,
                                ) {
                                    Ok(()) => PreparedCertifiedServe::Rejected(error.to_string()),
                                    Err(reason) => PreparedCertifiedServe::Service(reason),
                                },
                            );
                            return true;
                        }
                    };
                if superseded_by_decision {
                    let decided = terminal_subject.expect(
                        "Decision supersession requires the durable exact terminal subject",
                    );
                    prepared_serve = Some(
                        match services.stage_certified_serve_rejection(
                            authenticated.request_hash(),
                            CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided),
                        ) {
                            Ok(()) => PreparedCertifiedServe::Rejected(
                                "certified body request was superseded by durable Decision"
                                    .to_owned(),
                            ),
                            Err(reason) => PreparedCertifiedServe::Service(reason),
                        },
                    );
                    return true;
                }
                match services.prepare_certified_request(authenticated_via, authenticated) {
                    Ok(admission) => {
                        prepared_serve = Some(PreparedCertifiedServe::Admitted(admission));
                        true
                    }
                    Err(CertifiedServePrepareError::Backpressure) => {
                        // `prepare_certified_request` installs the off-queue debt
                        // before returning capacity backpressure. The fair
                        // selector's immutable physical cutoff keeps every later
                        // ingress occurrence behind this retained target.
                        false
                    }
                    Err(CertifiedServePrepareError::Rejected(reason)) => {
                        prepared_serve = Some(PreparedCertifiedServe::Rejected(reason));
                        true
                    }
                    Err(CertifiedServePrepareError::Service(reason)) => {
                        prepared_serve = Some(PreparedCertifiedServe::Service(reason));
                        true
                    }
                }
            })
            .map_err(V2RunnerError::Service)?
        else {
            break;
        };
        if matches!(inbound.message(), BlockMessage::KuraReplicaAdvert(_)) {
            admit_kura_replica_advert_ingress(receiver, kura, inbound)?;
            continue;
        }
        if inbound.message().is_lane_local() {
            let _ = lane_work
                .accept_lane_message_with_ingress_ownership(inbound, executor.current_tag().view());
            let _ = lane_work.service_next_historical_recovery()?;
            continue;
        }
        let mut ingress_ownership = inbound.take_ingress_ownership().ok_or_else(|| {
            V2RunnerError::Service(
                "global Sumeragi v2 ingress lost its fair ownership carrier".to_owned(),
            )
        })?;
        if !ingress_ownership.validate_exact()
            || !ingress_ownership.matches_message(inbound.message())
            || !ingress_ownership.matches_semantic_origin(inbound.sender())
        {
            return Err(V2RunnerError::Service(
                "global Sumeragi v2 ingress carried altered fair ownership".to_owned(),
            ));
        }
        receiver
            .bind_leader_wire_runtime_ownership(&mut ingress_ownership)
            .map_err(V2RunnerError::Service)?;
        if dequeue_disposition == FairV2IngressDequeueDisposition::RetireObsolete {
            let receipt = ingress_ownership
                .leader_wire_runtime_receipt()
                .ok_or_else(|| {
                    V2RunnerError::Service(
                        "obsolete leader-wire dequeue lost its runtime receipt".to_owned(),
                    )
                })?;
            let token = receipt.token();
            iroha_logger::debug!(
                message_kind = ?super::FairV2IngressMessageKind::classify(inbound.message()),
                semantic_origin = ?inbound.sender(),
                authenticated_via = ?inbound.via(),
                obsolete_view = token.view(),
                active_view = executor.current_tag().view(),
                "retired WAL-obsolete Sumeragi v2 leader-wire carrier"
            );
            receiver
                .mark_obsolete_leader_wire_volatile_terminal(receipt)
                .map_err(V2RunnerError::Service)?;
            continue;
        }
        let (message, sender, reply_routes) = inbound.into_message_sender_and_reply_routes();
        if !ingress_ownership.matches_reply_routes(reply_routes.as_ref()) {
            return Err(V2RunnerError::Service(
                "global Sumeragi v2 ingress changed its authenticated reply routes".to_owned(),
            ));
        }
        let BlockMessage::V2(message) = message else {
            iroha_logger::debug!("rejected legacy global message on v2-only consensus ingress");
            mark_leader_wire_volatile(receiver, &ingress_ownership)?;
            continue;
        };
        if let Err(error) = message.validate_version() {
            iroha_logger::debug!(%error, "rejected wrong-version Sumeragi v2 envelope");
            mark_leader_wire_volatile(receiver, &ingress_ownership)?;
            continue;
        }
        match message.payload {
            wire::ConsensusMessageV2Payload::VrfCommit(commit) => {
                drop(ingress_ownership);
                let outcome = npos_vrf.accept_commit(commit, sender.as_ref());
                if matches!(outcome, super::v2_npos::V2VrfIngressOutcome::Rejected(_)) {
                    iroha_logger::debug!(?outcome, "rejected NPoS VRF commitment");
                }
            }
            wire::ConsensusMessageV2Payload::VrfReveal(reveal) => {
                drop(ingress_ownership);
                let outcome = npos_vrf.accept_reveal(reveal, sender.as_ref());
                if matches!(outcome, super::v2_npos::V2VrfIngressOutcome::Rejected(_)) {
                    iroha_logger::debug!(?outcome, "rejected NPoS VRF reveal");
                }
            }
            wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                if !terminal_decision {
                    enqueue_control(
                        executor,
                        receiver,
                        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                            proposal,
                        )),
                        ingress_ownership,
                    )?;
                } else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                }
            }
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                if !terminal_decision {
                    enqueue_control(
                        executor,
                        receiver,
                        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote)),
                        ingress_ownership,
                    )?;
                } else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                }
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
                if !terminal_decision {
                    enqueue_control(
                        executor,
                        receiver,
                        wire::ConsensusMessageV2::new(
                            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
                        ),
                        ingress_ownership,
                    )?;
                } else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                }
            }
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
                if !terminal_decision {
                    enqueue_control(
                        executor,
                        receiver,
                        wire::ConsensusMessageV2::new(
                            wire::ConsensusMessageV2Payload::TimeoutVote(vote),
                        ),
                        ingress_ownership,
                    )?;
                } else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                }
            }
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
                if !terminal_decision {
                    enqueue_control(
                        executor,
                        receiver,
                        wire::ConsensusMessageV2::new(
                            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate),
                        ),
                        ingress_ownership,
                    )?;
                } else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                }
            }
            wire::ConsensusMessageV2Payload::PayloadManifest(manifest) => {
                if let Err(error) = manifest.validate(executor.context()) {
                    iroha_logger::debug!(%error, "rejected standalone Sumeragi v2 manifest");
                }
                mark_leader_wire_volatile(receiver, &ingress_ownership)?;
            }
            wire::ConsensusMessageV2Payload::PayloadChunk(chunk) => {
                let Some(sender) = sender else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                };
                if terminal_decision
                    && services
                        .fetch_work_for_manifest(chunk.manifest_hash)
                        .is_none()
                {
                    // Proposal reordering justifies buffering an orphan chunk
                    // only while another Proposal can still open its fetch.
                    // After Decision, unmatched chunks can never become
                    // relevant and must not crowd the decided body's bounded
                    // transport completion out of the orphan buffer.
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                }
                services
                    .route_payload_chunk(executor, sender, chunk, ingress_ownership)
                    .map_err(V2RunnerError::Service)?;
            }
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) => {
                let Some(sender) = sender else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                };
                let Some(reply_routes) = reply_routes else {
                    iroha_logger::debug!(
                        %sender,
                        "rejected certified body request without authenticated reply route"
                    );
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                };
                if reply_routes.semantic_target() != &sender {
                    iroha_logger::debug!(
                        %sender,
                        "rejected certified body request with mismatched reply target"
                    );
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                }
                if request.round.height < executor.context().height {
                    let response_peer = sender.clone();
                    let terminal_ownership = ingress_ownership.clone();
                    let served = serve_block_sync_while_guarded(
                        output_guard,
                        || {
                            block_sync_server
                                .serve_historical_body(kura, request, &sender, local_key)
                        },
                        |response, permit| {
                            services.post_durable_history_response_on_reply_routes_with_permit(
                                response_peer,
                                reply_routes,
                                ingress_ownership,
                                response,
                                permit,
                            )
                        },
                    );
                    match finalize_bound_block_sync_serve(
                        served,
                        || mark_leader_wire_volatile(receiver, &terminal_ownership),
                        |error| {
                            iroha_logger::debug!(%error, "rejected historical certified body request");
                        },
                    )? {
                        BoundBlockSyncServeOutcome::Posted
                        | BoundBlockSyncServeOutcome::VolatileRemoteRejection => {}
                        BoundBlockSyncServeOutcome::VolatileNoResponse => {
                            iroha_logger::debug!(
                                "retired historical certified body request without a local response"
                            );
                        }
                    }
                } else if request.round.height == executor.context().height {
                    if certified_body_request_is_superseded_after_decision(
                        &request,
                        terminal_subject,
                        executor.context().height,
                    ) {
                        // Current-height serving authority narrows to the
                        // exact Decision. A certified losing body remains
                        // useful only before that terminal choice.
                        match prepared_serve.take() {
                            Some(PreparedCertifiedServe::Rejected(reason)) => {
                                iroha_logger::debug!(
                                    %reason,
                                    "retired certified body request superseded by Decision"
                                );
                                mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                                continue;
                            }
                            Some(PreparedCertifiedServe::Service(reason)) => {
                                return Err(V2RunnerError::Service(reason));
                            }
                            Some(PreparedCertifiedServe::Admitted(_)) | None => {
                                return Err(V2RunnerError::Service(
                                    "Decision-superseded certified-body ingress crossed physical drain without its durable negative outcome"
                                        .to_owned(),
                                ));
                            }
                        }
                    }
                    match prepared_serve.take() {
                        Some(PreparedCertifiedServe::Admitted(admission)) => {
                            services
                                .serve_certified_request_on_routes(
                                    admission,
                                    reply_routes,
                                    ingress_ownership,
                                )
                                .map_err(V2RunnerError::Service)?;
                        }
                        Some(PreparedCertifiedServe::Rejected(reason)) => {
                            iroha_logger::debug!(%reason, "rejected certified body request");
                            mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                        }
                        Some(PreparedCertifiedServe::Service(reason)) => {
                            return Err(V2RunnerError::Service(reason));
                        }
                        None => {
                            return Err(V2RunnerError::Service(
                                "current-height certified-body ingress crossed fair removal without an atomic Serve admission"
                                    .to_owned(),
                            ));
                        }
                    }
                } else {
                    iroha_logger::debug!(
                        requested_height = request.round.height,
                        active_height = executor.context().height,
                        "rejected future-height certified body request"
                    );
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                }
            }
            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) => {
                let Some(sender) = sender else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                };
                let admission = executor.accept_certified_body_response_with_ingress_ownership(
                    response,
                    &sender,
                    &ingress_ownership,
                    services,
                );
                match admission {
                    Ok(_) => {}
                    Err(EffectTransportError::Backpressure) => {
                        // End the complete batch immediately. A second
                        // Runtime/Ingress pair could otherwise let later work
                        // overtake the newly retained exact carrier before the
                        // dedicated outer episode observes it.
                        return Ok(());
                    }
                    Err(EffectTransportError::FailClosed(reason)) => {
                        return Err(V2RunnerError::Service(reason));
                    }
                    Err(error) => {
                        iroha_logger::debug!(%error, "rejected certified body response");
                    }
                }
            }
            wire::ConsensusMessageV2Payload::CommitCertificateRequest(request) => {
                let Some(sender) = sender else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                };
                let Some(reply_routes) = reply_routes else {
                    iroha_logger::debug!(
                        %sender,
                        "rejected CommitQC request without authenticated reply route"
                    );
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                };
                if reply_routes.semantic_target() != &sender {
                    iroha_logger::debug!(
                        %sender,
                        "rejected CommitQC request with mismatched reply target"
                    );
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                }
                let response_peer = sender.clone();
                let terminal_ownership = ingress_ownership.clone();
                let served = serve_block_sync_while_guarded(
                    output_guard,
                    || block_sync_server.serve(kura, request, &sender, local_key),
                    |response, permit| {
                        services.post_durable_history_response_on_reply_routes_with_permit(
                            response_peer,
                            reply_routes,
                            ingress_ownership,
                            response,
                            permit,
                        )
                    },
                );
                match finalize_bound_block_sync_serve(
                    served,
                    || mark_leader_wire_volatile(receiver, &terminal_ownership),
                    |error| {
                        iroha_logger::debug!(%error, "rejected CommitQC discovery request");
                    },
                )? {
                    BoundBlockSyncServeOutcome::Posted
                    | BoundBlockSyncServeOutcome::VolatileRemoteRejection => {}
                    BoundBlockSyncServeOutcome::VolatileNoResponse => {
                        iroha_logger::debug!(
                            "retired CommitQC discovery request without a local response"
                        );
                    }
                }
            }
            wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) => {
                if terminal_decision {
                    // A discovery response unwraps into a CommitQC and is
                    // therefore reducer-producing, unlike body/chunk
                    // transport completions. Decision is terminal for global
                    // consensus input at this height.
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                }
                let Some(sender) = sender else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                };
                let discovered = match block_sync.authenticate_response(response, &sender) {
                    Ok(discovered) => discovered,
                    Err(error) => {
                        iroha_logger::debug!(%error, "rejected CommitQC discovery response");
                        mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                        continue;
                    }
                };
                let admission = block_sync.enqueue_and_complete(discovered, |message| {
                    executor.enqueue_discovered_commit_certificate(message, ingress_ownership)
                });
                if commit_certificate_admission_completed(admission)? {
                    *block_sync_request = None;
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecidedLaneRecoveryDrainCommitOutcome {
    LaneLocal,
    KuraReplicaAdvert,
    CurrentServe,
    HistoricalServe,
    LeaderWireVolatile,
}

trait DecidedLaneRecoveryDrainCommitter {
    type Admission;

    fn commit_lane_local(&mut self) -> Result<(), V2RunnerError>;

    fn commit_kura_replica_advert(&mut self) -> Result<(), V2RunnerError>;

    fn commit_current_serve(
        &mut self,
        current: DecidedLaneRecoveryCurrentDrain<Self::Admission>,
    ) -> Result<(), V2RunnerError>;

    fn bind_leader_wire(&mut self) -> Result<(), V2RunnerError>;

    fn commit_historical_serve(&mut self) -> Result<(), V2RunnerError>;

    fn commit_leader_wire_volatile(&mut self) -> Result<(), V2RunnerError>;
}

fn commit_decided_lane_recovery_drain<C: DecidedLaneRecoveryDrainCommitter>(
    authorization: DecidedLaneRecoveryDrainAuthorization<C::Admission>,
    committer: &mut C,
) -> Result<DecidedLaneRecoveryDrainCommitOutcome, V2RunnerError> {
    match authorization {
        DecidedLaneRecoveryDrainAuthorization::LaneLocal => {
            committer.commit_lane_local()?;
            Ok(DecidedLaneRecoveryDrainCommitOutcome::LaneLocal)
        }
        DecidedLaneRecoveryDrainAuthorization::KuraReplicaAdvert => {
            committer.commit_kura_replica_advert()?;
            Ok(DecidedLaneRecoveryDrainCommitOutcome::KuraReplicaAdvert)
        }
        DecidedLaneRecoveryDrainAuthorization::CurrentServe(current) => {
            committer.commit_current_serve(current)?;
            Ok(DecidedLaneRecoveryDrainCommitOutcome::CurrentServe)
        }
        DecidedLaneRecoveryDrainAuthorization::HistoricalServe => {
            committer.bind_leader_wire()?;
            committer.commit_historical_serve()?;
            Ok(DecidedLaneRecoveryDrainCommitOutcome::HistoricalServe)
        }
        DecidedLaneRecoveryDrainAuthorization::LeaderWireRetire => {
            committer.bind_leader_wire()?;
            committer.commit_leader_wire_volatile()?;
            Ok(DecidedLaneRecoveryDrainCommitOutcome::LeaderWireVolatile)
        }
    }
}

struct ProductionDecidedLaneRecoveryDrainCommitter<'a> {
    receiver: &'a FairV2Ingress,
    inbound: Option<InboundBlockMessage>,
    bound_leader_wire: Option<FairV2IngressOwnershipEvidence>,
    executor: &'a V2EffectExecutor,
    services: &'a mut ProductionV2Services,
    lane_work: &'a mut V2LaneWorkAdapter,
    active_view: wire::View,
    output_guard: &'a ConsensusOutputGuard,
    kura: &'a Kura,
    local_key: &'a KeyPair,
    block_sync_server: &'a mut V2BlockSyncServer,
}

impl ProductionDecidedLaneRecoveryDrainCommitter<'_> {
    fn take_inbound(&mut self) -> Result<InboundBlockMessage, V2RunnerError> {
        self.inbound.take().ok_or_else(|| {
            V2RunnerError::Service(
                "terminal recovery drain attempted to consume one ingress occurrence twice"
                    .to_owned(),
            )
        })
    }

    fn take_bound_leader_wire(&mut self) -> Result<FairV2IngressOwnershipEvidence, V2RunnerError> {
        self.bound_leader_wire.take().ok_or_else(|| {
            V2RunnerError::Service(
                "terminal recovery drain used leader-wire ownership before binding it".to_owned(),
            )
        })
    }
}

impl DecidedLaneRecoveryDrainCommitter for ProductionDecidedLaneRecoveryDrainCommitter<'_> {
    type Admission = CertifiedServeAdmission;

    fn commit_lane_local(&mut self) -> Result<(), V2RunnerError> {
        let inbound = self.take_inbound()?;
        let _ = self
            .lane_work
            .accept_lane_message_with_ingress_ownership(inbound, self.active_view);
        let _ = self.lane_work.service_next_historical_recovery()?;
        Ok(())
    }

    fn commit_kura_replica_advert(&mut self) -> Result<(), V2RunnerError> {
        let inbound = self.take_inbound()?;
        admit_kura_replica_advert_ingress(self.receiver, self.kura, inbound)
    }

    fn commit_current_serve(
        &mut self,
        current: DecidedLaneRecoveryCurrentDrain<Self::Admission>,
    ) -> Result<(), V2RunnerError> {
        let mut inbound = self.take_inbound()?;
        match current {
            DecidedLaneRecoveryCurrentDrain::Admitted(admission) => {
                let ingress_ownership = inbound.take_ingress_ownership().ok_or_else(|| {
                    V2RunnerError::Service(
                        "terminal recovery Serve admission lost its fair ownership".to_owned(),
                    )
                })?;
                let (_, _, reply_routes) = inbound.into_message_sender_and_reply_routes();
                self.services
                    .serve_certified_request_on_routes(
                        admission,
                        reply_routes.ok_or_else(|| {
                            V2RunnerError::Service(
                                "terminal recovery Serve admission lost its reply routes"
                                    .to_owned(),
                            )
                        })?,
                        ingress_ownership,
                    )
                    .map_err(V2RunnerError::Service)
            }
            DecidedLaneRecoveryCurrentDrain::Rejected(reason) => {
                iroha_logger::debug!(
                    %reason,
                    "retired terminal-recovery certified body request"
                );
                Ok(())
            }
        }
    }

    fn bind_leader_wire(&mut self) -> Result<(), V2RunnerError> {
        let inbound = self.inbound.as_mut().ok_or_else(|| {
            V2RunnerError::Service(
                "discarded terminal-recovery ingress was already consumed".to_owned(),
            )
        })?;
        let mut ingress_ownership = inbound.take_ingress_ownership().ok_or_else(|| {
            V2RunnerError::Service(
                "discarded terminal-recovery ingress lost its fair ownership carrier".to_owned(),
            )
        })?;
        if !ingress_ownership.validate_exact()
            || !ingress_ownership.matches_message(inbound.message())
            || !ingress_ownership.matches_semantic_origin(inbound.sender())
            || !ingress_ownership.matches_reply_routes(inbound.reply_routes())
        {
            return Err(V2RunnerError::Service(
                "discarded terminal-recovery ingress carried altered fair ownership".to_owned(),
            ));
        }
        self.receiver
            .bind_leader_wire_runtime_ownership(&mut ingress_ownership)
            .map_err(V2RunnerError::Service)?;
        self.bound_leader_wire = Some(ingress_ownership);
        Ok(())
    }

    fn commit_historical_serve(&mut self) -> Result<(), V2RunnerError> {
        let inbound = self.take_inbound()?;
        let ingress_ownership = self.take_bound_leader_wire()?;
        let (message, sender, reply_routes) = inbound.into_message_sender_and_reply_routes();
        let BlockMessage::V2(message) = message else {
            return Err(V2RunnerError::Service(
                "historical terminal-recovery route changed message class after authorization"
                    .to_owned(),
            ));
        };
        message
            .validate_version()
            .map_err(|error| V2RunnerError::Service(error.to_string()))?;
        let wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) = message.payload else {
            return Err(V2RunnerError::Service(
                "historical terminal-recovery route changed payload after authorization".to_owned(),
            ));
        };
        if request.round.height >= self.executor.context().height {
            return Err(V2RunnerError::Service(
                "historical terminal-recovery route crossed the active height".to_owned(),
            ));
        }
        let Some(sender) = sender else {
            mark_leader_wire_volatile(self.receiver, &ingress_ownership)?;
            return Ok(());
        };
        let Some(reply_routes) = reply_routes else {
            mark_leader_wire_volatile(self.receiver, &ingress_ownership)?;
            return Ok(());
        };
        if reply_routes.semantic_target() != &sender {
            mark_leader_wire_volatile(self.receiver, &ingress_ownership)?;
            return Ok(());
        }
        let response_peer = sender.clone();
        let terminal_ownership = ingress_ownership.clone();
        let output_guard = self.output_guard;
        let block_sync_server = &mut *self.block_sync_server;
        let kura = self.kura;
        let local_key = self.local_key;
        let services = &mut *self.services;
        let served = serve_block_sync_while_guarded(
            output_guard,
            || block_sync_server.serve_historical_body(kura, request, &sender, local_key),
            |response, permit| {
                services.post_durable_history_response_on_reply_routes_with_permit(
                    response_peer,
                    reply_routes,
                    ingress_ownership,
                    response,
                    permit,
                )
            },
        );
        match finalize_bound_block_sync_serve(
            served,
            || mark_leader_wire_volatile(self.receiver, &terminal_ownership),
            |error| {
                iroha_logger::debug!(
                    %error,
                    "rejected historical certified body request during terminal recovery"
                );
            },
        )? {
            BoundBlockSyncServeOutcome::Posted
            | BoundBlockSyncServeOutcome::VolatileRemoteRejection => {}
            BoundBlockSyncServeOutcome::VolatileNoResponse => {
                iroha_logger::debug!(
                    "retired terminal-recovery historical body request without a local response"
                );
            }
        }
        Ok(())
    }

    fn commit_leader_wire_volatile(&mut self) -> Result<(), V2RunnerError> {
        let _ = self.take_inbound()?;
        let ingress_ownership = self.take_bound_leader_wire()?;
        mark_leader_wire_volatile(self.receiver, &ingress_ownership)
    }
}

#[allow(clippy::too_many_arguments)]
fn drain_decided_lane_recovery_ingress(
    receiver: &FairV2Ingress,
    executor: &V2EffectExecutor,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
    active_view: wire::View,
    output_guard: &ConsensusOutputGuard,
    kura: &Kura,
    local_key: &KeyPair,
    block_sync_server: &mut V2BlockSyncServer,
) -> Result<(), V2RunnerError> {
    let decided_subject = executor
        .local_proposal_directive()?
        .decided_subject()
        .ok_or_else(|| {
            V2RunnerError::Service(
                "terminal lane recovery ingress lost its durable Decision subject".to_owned(),
            )
        })?;
    let mut authorization = None;
    let mut authorization_error = None;
    let inbound = receiver
        .try_recv_if_checked(|inbound| {
            if authorization_error.is_some() {
                return false;
            }
            let preparation = prepare_decided_lane_recovery_ingress(
                inbound,
                executor.context().height,
                decided_subject,
                |request, sender| {
                    executor
                        .authenticate_certified_body_request(request, sender)
                        .map_err(|error| error.to_string())
                },
            );
            match authorize_decided_lane_recovery_drain(preparation, services) {
                DecidedLaneRecoveryDrainDecision::Retain => false,
                DecidedLaneRecoveryDrainDecision::Authorized(candidate) => {
                    if authorization.replace(candidate).is_some() {
                        authorization_error = Some(
                            "terminal recovery selected more than one checked ingress occurrence"
                                .to_owned(),
                        );
                        false
                    } else {
                        true
                    }
                }
                DecidedLaneRecoveryDrainDecision::FailClosed(reason) => {
                    authorization_error = Some(reason);
                    false
                }
            }
        })
        .map_err(V2RunnerError::Service)?;
    if let Some(reason) = authorization_error {
        return Err(V2RunnerError::Service(reason));
    }
    let Some(inbound) = inbound else {
        return Ok(());
    };
    let authorization = authorization.ok_or_else(|| {
        V2RunnerError::Service(
            "terminal recovery checked dequeue lost its pre-drain authorization".to_owned(),
        )
    })?;
    let mut committer = ProductionDecidedLaneRecoveryDrainCommitter {
        receiver,
        inbound: Some(inbound),
        bound_leader_wire: None,
        executor,
        services,
        lane_work,
        active_view,
        output_guard,
        kura,
        local_key,
        block_sync_server,
    };
    let _ = commit_decided_lane_recovery_drain(authorization, &mut committer)?;
    // Non-Serve global traffic for this replayed terminal height is
    // intentionally dropped. The durable Decision and finality tuple are the
    // only global reducer authority. Current-height Serve traffic is instead
    // fully authenticated above and atomically terminalized before the carrier
    // can leave fair ingress. One occurrence per outer loop keeps pending
    // Apply/completion work dominant.
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OuterIngressTurn {
    Completion,
    Runtime,
    Ingress,
}

/// Closed outer-runner target named by one lifecycle rank observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum LifecycleRunnerRankTarget {
    /// The next effect/I/O completion service turn.
    Completion,
    /// The next serialized reducer-runtime service turn.
    Runtime,
    /// The next authenticated fair-ingress service turn.
    Ingress,
}

impl LifecycleRunnerRankTarget {
    #[cfg(test)]
    const fn turn(self) -> OuterIngressTurn {
        match self {
            Self::Completion => OuterIngressTurn::Completion,
            Self::Runtime => OuterIngressTurn::Runtime,
            Self::Ingress => OuterIngressTurn::Ingress,
        }
    }
}

impl From<OuterIngressTurn> for LifecycleRunnerRankTarget {
    fn from(turn: OuterIngressTurn) -> Self {
        match turn {
            OuterIngressTurn::Completion => Self::Completion,
            OuterIngressTurn::Runtime => Self::Runtime,
            OuterIngressTurn::Ingress => Self::Ingress,
        }
    }
}

/// Borrow-bound proof of the outer runner cursor's exact current turn.
///
/// Construction is private to [`OuterIngressTurns::next_current`]. While this
/// value exists, its mutable cursor borrow prevents another turn from being
/// observed or advanced. Dropping it advances exactly the represented turn,
/// so a retained same-context value can never be reused after the live cursor
/// moves.
#[derive(Debug)]
#[must_use = "the current runner turn must be serviced before the cursor advances"]
pub(crate) struct LifecycleCurrentRunnerTurn<'cursor> {
    cursor: &'cursor mut OuterIngressTurns,
    turn: OuterIngressTurn,
}

impl LifecycleCurrentRunnerTurn<'_> {
    /// Frozen height-context identity owned by the borrowed cursor.
    pub(crate) const fn context_id(&self) -> wire::HeightContextId {
        self.cursor.context_id
    }

    /// Frozen height owned by the borrowed cursor.
    pub(crate) const fn height(&self) -> wire::Height {
        self.cursor.height
    }

    /// Exact current outer-runner target.
    pub(crate) fn target(&self) -> LifecycleRunnerRankTarget {
        self.turn.into()
    }

    /// Current-turn reach debt. A borrow can represent only the turn presently
    /// at the cursor, so its debt is necessarily zero.
    pub(crate) const fn debt(&self) -> u64 {
        0
    }

    const fn turn(&self) -> OuterIngressTurn {
        self.turn
    }
}

impl Drop for LifecycleCurrentRunnerTurn<'_> {
    fn drop(&mut self) {
        self.cursor.advance_current(self.turn);
    }
}

/// Test-only runner reach-debt observation for one outer turn.
///
/// Production cannot mint or consume this free-standing shape; its planner
/// accepts only [`LifecycleCurrentRunnerTurn`].
#[derive(Debug, PartialEq, Eq)]
#[must_use = "the runner observation must be consumed by the composite planner snapshot"]
#[cfg(test)]
pub(crate) struct LifecycleRunnerRankSnapshot {
    context_id: wire::HeightContextId,
    height: wire::Height,
    target: LifecycleRunnerRankTarget,
    debt: u64,
    _linearity: LifecycleRunnerRankSnapshotLinearity,
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
struct LifecycleRunnerRankSnapshotLinearity;

#[cfg(test)]
impl Drop for LifecycleRunnerRankSnapshotLinearity {
    fn drop(&mut self) {}
}

#[cfg(test)]
impl LifecycleRunnerRankSnapshot {
    /// Frozen height-context identity owning this cursor observation.
    pub(crate) const fn context_id(&self) -> wire::HeightContextId {
        self.context_id
    }

    /// Frozen height owning this cursor observation.
    pub(crate) const fn height(&self) -> wire::Height {
        self.height
    }

    /// Closed outer turn whose reach was measured.
    pub(crate) const fn target(&self) -> LifecycleRunnerRankTarget {
        self.target
    }

    /// Number of cursor turns strictly before the target.
    pub(crate) const fn debt(&self) -> u64 {
        self.debt
    }
}

/// Move-only cursor for the exact outer Completion/Runtime/Ingress cycle.
///
/// Reifying the cursor preserves the existing iterator order while giving the
/// guarded lifecycle planner a real runner-reach debt instead of a
/// caller-supplied zero. It remains private and never mints SchedulerInputs by
/// itself.
// TODO: Call the owner transaction while this cursor is borrowed at the live
// Ingress turn, together with the consuming owner-to-worker body-store launch.
#[derive(Debug)]
struct OuterIngressTurns {
    context_id: wire::HeightContextId,
    height: wire::Height,
    cycles_remaining: usize,
    next_turn: OuterIngressTurn,
}

impl OuterIngressTurns {
    fn new(limit: usize, context_id: wire::HeightContextId, height: wire::Height) -> Self {
        Self {
            context_id,
            height,
            cycles_remaining: limit.max(1),
            next_turn: OuterIngressTurn::Completion,
        }
    }

    #[cfg(test)]
    fn reach_debt(&self, target: OuterIngressTurn) -> Option<u64> {
        if self.cycles_remaining == 0 {
            return None;
        }
        let next = outer_ingress_turn_index(self.next_turn);
        let target = outer_ingress_turn_index(target);
        if target >= next {
            return Some(u64::from(target - next));
        }
        (self.cycles_remaining > 1).then(|| u64::from(3 - next + target))
    }

    #[cfg(test)]
    fn lifecycle_rank_snapshot(
        &self,
        target: LifecycleRunnerRankTarget,
    ) -> Option<LifecycleRunnerRankSnapshot> {
        Some(LifecycleRunnerRankSnapshot {
            context_id: self.context_id,
            height: self.height,
            target,
            debt: self.reach_debt(target.turn())?,
            _linearity: LifecycleRunnerRankSnapshotLinearity,
        })
    }

    /// Borrow the exact current turn without advancing the cursor early.
    fn next_current(&mut self) -> Option<LifecycleCurrentRunnerTurn<'_>> {
        if self.cycles_remaining == 0 {
            return None;
        }
        Some(LifecycleCurrentRunnerTurn {
            turn: self.next_turn,
            cursor: self,
        })
    }

    fn advance_current(&mut self, turn: OuterIngressTurn) {
        assert_eq!(
            self.next_turn, turn,
            "borrow-bound outer runner turn must remain current until drop"
        );
        self.next_turn = match turn {
            OuterIngressTurn::Completion => OuterIngressTurn::Runtime,
            OuterIngressTurn::Runtime => OuterIngressTurn::Ingress,
            OuterIngressTurn::Ingress => {
                self.cycles_remaining -= 1;
                OuterIngressTurn::Completion
            }
        };
    }
}

/// Mint the exact Ingress reach observation after Completion and Runtime for
/// the production-owner cross-module transaction regression.
#[cfg(test)]
pub(in crate::sumeragi) fn lifecycle_ingress_rank_snapshot_for_test(
    context: &wire::HeightContext,
) -> LifecycleRunnerRankSnapshot {
    let mut turns = OuterIngressTurns::new(1, context.id(), context.height);
    {
        let turn = turns
            .next_current()
            .expect("the outer cursor starts at Completion");
        assert_eq!(turn.turn(), OuterIngressTurn::Completion);
    }
    {
        let turn = turns
            .next_current()
            .expect("the outer cursor continues at Runtime");
        assert_eq!(turn.turn(), OuterIngressTurn::Runtime);
    }
    let turn = turns
        .next_current()
        .expect("the current outer cursor owns its immediate Ingress turn");
    assert_eq!(turn.turn(), OuterIngressTurn::Ingress);
    LifecycleRunnerRankSnapshot {
        context_id: turn.context_id(),
        height: turn.height(),
        target: turn.target(),
        debt: turn.debt(),
        _linearity: LifecycleRunnerRankSnapshotLinearity,
    }
}

/// Mint the exact first Completion observation for a lifecycle worker fixture.
#[cfg(test)]
pub(in crate::sumeragi) fn lifecycle_completion_rank_snapshot_for_test(
    context: &wire::HeightContext,
) -> LifecycleRunnerRankSnapshot {
    let turns = OuterIngressTurns::new(1, context.id(), context.height);
    turns
        .lifecycle_rank_snapshot(LifecycleRunnerRankTarget::Completion)
        .expect("the outer cursor starts at its immediate Completion turn")
}

#[cfg(test)]
const fn outer_ingress_turn_index(turn: OuterIngressTurn) -> u8 {
    match turn {
        OuterIngressTurn::Completion => 0,
        OuterIngressTurn::Runtime => 1,
        OuterIngressTurn::Ingress => 2,
    }
}

fn outer_ingress_turns(
    limit: usize,
    context_id: wire::HeightContextId,
    height: wire::Height,
) -> OuterIngressTurns {
    OuterIngressTurns::new(limit, context_id, height)
}

fn is_remote_block_sync_rejection(error: &V2BlockSyncError) -> bool {
    matches!(
        error,
        V2BlockSyncError::Wire(_)
            | V2BlockSyncError::Transport(_)
            | V2BlockSyncError::ConflictingServerRequest { .. }
            | V2BlockSyncError::ConflictingHistoricalBodyRequest { .. }
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GuardedBlockSyncServeOutcome {
    Posted,
    NoResponse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoundBlockSyncServeOutcome {
    Posted,
    VolatileNoResponse,
    VolatileRemoteRejection,
}

fn serve_block_sync_while_guarded<Response>(
    output_guard: &ConsensusOutputGuard,
    serve: impl FnOnce() -> Result<Option<Response>, V2BlockSyncError>,
    post: impl FnOnce(Response, &ConsensusOutputPermit<'_>) -> Result<(), String>,
) -> Result<GuardedBlockSyncServeOutcome, V2BlockSyncError> {
    let operation = output_guard
        .begin_fail_stop_operation()
        .ok_or(V2BlockSyncError::RestartRequired)?;
    match serve() {
        Ok(Some(response)) => {
            if let Err(error) = post(response, operation.permit()) {
                drop(operation);
                return Err(V2BlockSyncError::ResponsePost(error));
            }
            operation.complete();
            Ok(GuardedBlockSyncServeOutcome::Posted)
        }
        Ok(None) => {
            operation.complete();
            Ok(GuardedBlockSyncServeOutcome::NoResponse)
        }
        Err(error) if is_remote_block_sync_rejection(&error) => {
            operation.complete();
            Err(error)
        }
        Err(error) => {
            drop(operation);
            Err(error)
        }
    }
}

fn finalize_bound_block_sync_serve(
    served: Result<GuardedBlockSyncServeOutcome, V2BlockSyncError>,
    retire_volatile: impl FnOnce() -> Result<(), V2RunnerError>,
    observe_remote_rejection: impl FnOnce(&V2BlockSyncError),
) -> Result<BoundBlockSyncServeOutcome, V2RunnerError> {
    match served {
        Ok(GuardedBlockSyncServeOutcome::Posted) => Ok(BoundBlockSyncServeOutcome::Posted),
        Ok(GuardedBlockSyncServeOutcome::NoResponse) => {
            retire_volatile()?;
            Ok(BoundBlockSyncServeOutcome::VolatileNoResponse)
        }
        Err(error) if is_remote_block_sync_rejection(&error) => {
            retire_volatile()?;
            observe_remote_rejection(&error);
            Ok(BoundBlockSyncServeOutcome::VolatileRemoteRejection)
        }
        Err(error) => Err(error.into()),
    }
}

fn enqueue_control(
    executor: &mut V2EffectExecutor,
    receiver: &FairV2Ingress,
    message: wire::ConsensusMessageV2,
    ingress_ownership: FairV2IngressOwnershipEvidence,
) -> Result<(), V2RunnerError> {
    let terminal_ownership = ingress_ownership.clone();
    complete_control_ingress_admission(
        receiver,
        &terminal_ownership,
        executor.enqueue_network_with_ingress_ownership(message, ingress_ownership),
    )
}

fn complete_control_ingress_admission(
    receiver: &FairV2Ingress,
    terminal_ownership: &FairV2IngressOwnershipEvidence,
    admission: Result<EventTag, NetworkIngressError>,
) -> Result<(), V2RunnerError> {
    match admission {
        Ok(_) => Ok(()),
        Err(NetworkIngressError::FailClosed) => {
            mark_leader_wire_volatile(receiver, terminal_ownership)?;
            Err(V2RunnerError::RuntimeFailClosed)
        }
        Err(NetworkIngressError::Authentication(error)) => {
            iroha_logger::debug!(%error, "rejected Sumeragi v2 control ingress");
            mark_leader_wire_volatile(receiver, terminal_ownership)?;
            Ok(())
        }
        Err(NetworkIngressError::Backpressure(error)) => {
            mark_leader_wire_volatile(receiver, terminal_ownership)?;
            Err(V2RunnerError::RuntimeAdmissionInvariant(error.to_string()))
        }
        Err(NetworkIngressError::TransportPayload) => {
            mark_leader_wire_volatile(receiver, terminal_ownership)?;
            Err(V2RunnerError::RuntimeAdmissionInvariant(
                "transport payload reached reducer-control admission".to_owned(),
            ))
        }
    }
}

fn mark_leader_wire_volatile(
    receiver: &FairV2Ingress,
    ownership: &FairV2IngressOwnershipEvidence,
) -> Result<(), V2RunnerError> {
    if let Some(receipt) = ownership.leader_wire_runtime_receipt() {
        receiver
            .mark_leader_wire_volatile_terminal(receipt)
            .map_err(V2RunnerError::Service)?;
    }
    Ok(())
}

fn commit_certificate_admission_completed(
    admission: Result<(), CommitCertificateAdmissionError<NetworkIngressError>>,
) -> Result<bool, V2RunnerError> {
    match admission {
        Ok(()) => Ok(true),
        Err(CommitCertificateAdmissionError::Enqueue(NetworkIngressError::FailClosed)) => {
            Err(V2RunnerError::RuntimeFailClosed)
        }
        Err(CommitCertificateAdmissionError::Enqueue(NetworkIngressError::Backpressure(error))) => {
            // The dequeue predicate couples the outer occurrence to this exact
            // Progress admission. Treat a defensive mismatch as retryable: the
            // discovery request remains outstanding and retransmission can
            // supply another occurrence after capacity changes.
            iroha_logger::debug!(%error, "deferred authenticated CommitQC response after runtime backpressure");
            Ok(false)
        }
        Err(CommitCertificateAdmissionError::Enqueue(error)) => {
            iroha_logger::debug!(%error, "deferred authenticated CommitQC response");
            Ok(false)
        }
        Err(CommitCertificateAdmissionError::MismatchedReducerAdmission) => {
            Err(V2RunnerError::RuntimeAdmissionInvariant(
                "authenticated CommitQC discovery received foreign reducer admission ownership"
                    .to_owned(),
            ))
        }
        Err(CommitCertificateAdmissionError::RequestDisappeared) => {
            Err(V2RunnerError::BlockSyncRequestDisappeared)
        }
        Err(CommitCertificateAdmissionError::RefinementRejected) => {
            Err(V2RunnerError::RuntimeAdmissionInvariant(
                "authenticated CommitQC discovery failed exact historical refinement".to_owned(),
            ))
        }
    }
}

fn advance_executor(
    receiver: &FairV2Ingress,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    limit: usize,
) -> Result<(), V2RunnerError> {
    for _ in 0..limit.max(1) {
        executor.set_ingress_physical_cut(receiver.next_physical_admission_ordinal())?;
        match executor.step(Instant::now(), services)? {
            EffectExecutorStep::Idle => break,
            EffectExecutorStep::Advanced { .. } => {
                // A PrepareQC can replace the protected lock without changing
                // the EventTag. Reconcile immediately after every serialized
                // transition so later ingress in the same outer batch cannot
                // reclaim service ownership for the superseded subject.
                let _ = reconcile_executor_locked_body(executor, services)?;
            }
        }
    }
    Ok(())
}

/// Execute at most one serialized transition from an older lifecycle before
/// an exact Serve target turn.
///
/// Lock reconciliation and every other local producer stay behind the target;
/// the ordinary runner path performs them after the barrier drains. This is
/// deliberately not a loop, even when the older causal episode remains live.
fn advance_executor_once_before_exact_serve(
    receiver: &FairV2Ingress,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
) -> Result<(), V2RunnerError> {
    executor.set_ingress_physical_cut(receiver.next_physical_admission_ordinal())?;
    let _ = executor.step(Instant::now(), services)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CertifiedServeBarrierLivenessAction {
    TimeoutVoteEpisode,
    TimeoutRecoveryPrefix,
    Pacemaker,
}

/// Service the complete timeout-recovery suffix of one selected Serve turn.
///
/// The one-shot predecessor claim is deliberately not an episode owner. Once
/// it is Complete, the still-selected Serve carrier continues to admit one
/// eligible direct-roster timeout vote, service an already-owned prefix
/// completion, and run one typed pacemaker transition in that exact order.
fn service_certified_serve_barrier_liveness_turn<E>(
    recovering_interrupted_tip: bool,
    older_runtime_episode_claimed: bool,
    mut service: impl FnMut(CertifiedServeBarrierLivenessAction) -> Result<(), E>,
) -> Result<(), E> {
    if !recovering_interrupted_tip {
        service(CertifiedServeBarrierLivenessAction::TimeoutVoteEpisode)?;
    }
    service(CertifiedServeBarrierLivenessAction::TimeoutRecoveryPrefix)?;
    service_certified_serve_barrier_pacemaker_turn(
        recovering_interrupted_tip,
        older_runtime_episode_claimed,
        || service(CertifiedServeBarrierLivenessAction::Pacemaker),
    )
}

/// Keep the pacemaker live for the full lifetime of a certified Serve barrier.
///
/// `older_runtime_episode_claimed` deliberately does not gate service. The
/// finite predecessor episode becomes Complete before a backpressured target
/// necessarily leaves fair ingress, while the absolute timeout and certified
/// progress roots remain independently live.
fn service_certified_serve_barrier_pacemaker_turn<E>(
    recovering_interrupted_tip: bool,
    _older_runtime_episode_claimed: bool,
    service: impl FnOnce() -> Result<(), E>,
) -> Result<(), E> {
    if recovering_interrupted_tip {
        return Ok(());
    }
    service()
}

/// Execute at most one typed timeout/Progress-root transition while an exact
/// transport episode retains ordinary ownership.
fn advance_pacemaker_once(
    receiver: &FairV2Ingress,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
) -> Result<(), V2RunnerError> {
    executor.set_ingress_physical_cut(receiver.next_physical_admission_ordinal())?;
    let _ = executor.step_pacemaker_once(Instant::now(), services)?;
    Ok(())
}

fn reconcile_executor_locked_body(
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
) -> Result<LocalProposalDirective, V2RunnerError> {
    let directive = executor.local_proposal_directive()?;
    if directive.decided_subject().is_none()
        && let Some(lock) = directive.locked_body()
    {
        executor.reconcile_locked_body_for_recovery(directive.tag(), lock, services)?;
    }
    Ok(directive)
}

/// Keep lane-work construction behind the completed interrupted-tip
/// application boundary.
///
/// The recovery worker reports completion only after canonical State,
/// post-apply metadata, and strict Native AMX evidence repair are durable.
/// Keeping the constructor in this continuation makes that ordering explicit
/// and independently testable.
fn construct_after_pending_tip_application_recovery<T>(
    recovering_interrupted_tip: bool,
    recovery_complete: bool,
    construct: impl FnOnce() -> Result<T, V2RunnerError>,
) -> Result<T, V2RunnerError> {
    if recovering_interrupted_tip && !recovery_complete {
        return Err(V2RunnerError::PendingTipRecoveryIncomplete);
    }
    construct()
}

fn pending_tip_recovery_deadline_error(
    output_guard: &ConsensusOutputGuard,
    timeout: Duration,
    attempts: u64,
    stage: Option<PendingKuraApplyRecoveryStage>,
) -> V2RunnerError {
    output_guard.activate_restart_required();
    super::status::mark_v2_restart_required();
    V2RunnerError::PendingTipRecoveryDeadlineExceeded {
        timeout,
        attempts,
        stage,
    }
}

fn advance_pending_tip_recovery_executor(
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    limit: usize,
) -> Result<usize, V2RunnerError> {
    let mut advanced = 0_usize;
    for _ in 0..limit.max(1) {
        match executor.step_pending_tip_recovery(Instant::now(), services)? {
            EffectExecutorStep::Idle => break,
            EffectExecutorStep::Advanced { effects } => {
                advanced = advanced.saturating_add(effects);
            }
        }
    }
    Ok(advanced)
}

fn local_validator_index(
    context: &wire::HeightContext,
    local_peer: &PeerId,
    role: NodeRole,
) -> Result<Option<wire::ValidatorIndex>, V2RunnerError> {
    let index = context
        .roster
        .iter()
        .position(|entry| &entry.validator == local_peer)
        .map(u32::try_from)
        .transpose()?;
    match (role, index) {
        (NodeRole::Observer, _) => Ok(None),
        (NodeRole::Validator, Some(index)) => Ok(Some(index)),
        // Roster changes are authenticated height transitions, not process
        // role changes. A configured validator absent from this height runs
        // the global protocol as an observer while retaining authority to
        // serve any independently frozen lane descriptor that still names
        // its key, including unfinished predecessor work.
        (NodeRole::Validator, None) => Ok(None),
    }
}

/// Claim the immutable process capability independently of this height's roster duty.
///
/// Configured validators must own one generation even while absent from the recovered global
/// roster: a later height may rotate them back in, and a retained frozen lane descriptor may
/// still name their key. Explicit observers never mutate this Kura lifecycle namespace.
fn claim_runner_lifecycle_process_generation(
    role: NodeRole,
    kura: &Kura,
    context: &wire::HeightContext,
    local_peer: &PeerId,
) -> Result<Option<AutonomousLifecycleProcessGenerationClaim>, V2RunnerError> {
    match role {
        NodeRole::Observer => Ok(None),
        NodeRole::Validator => kura
            .claim_autonomous_lifecycle_process_generation(context.network_id, local_peer)
            .map(Some)
            .map_err(|error| {
                V2RunnerError::Service(format!(
                    "failed to claim the durable autonomous lifecycle process generation: {error}"
                ))
            }),
    }
}

fn round_for_tag(
    context: &wire::HeightContext,
    tag: EventTag,
) -> Result<wire::ConsensusRound, V2RunnerError> {
    if tag.height() != context.height {
        return Err(V2RunnerError::StaleTag);
    }
    Ok(wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: tag.view(),
    })
}

fn runtime_queue_config(config: &SumeragiV2Config) -> Result<RuntimeQueueConfig, V2RunnerError> {
    Ok(RuntimeQueueConfig::new(
        usize::try_from(config.limits.runtime_command_capacity)?,
        usize::try_from(config.limits.runtime_progress_reserve)?,
        usize::try_from(config.limits.runtime_completion_reserve)?,
    ))
}

fn effect_queue_config(config: &SumeragiV2Config) -> Result<EffectQueueConfig, V2RunnerError> {
    let max_pending_work = usize::try_from(config.limits.effect_work_capacity)?;
    let completion_reserve = usize::try_from(config.limits.runtime_completion_reserve)?;
    if max_pending_work > completion_reserve {
        return Err(V2RunnerError::EffectWorkExceedsCompletionReserve {
            pending: max_pending_work,
            reserve: completion_reserve,
        });
    }
    Ok(EffectQueueConfig::new(
        max_pending_work,
        usize::try_from(config.limits.ready_body_capacity)?,
        config.limits.ready_body_bytes,
        usize::try_from(config.limits.certified_request_capacity)?,
    ))
}

fn lane_work_limits(
    config: &SumeragiV2Config,
    reply_source_capacity: usize,
    consensus_frame_byte_capacity: usize,
    block_sync_frame_byte_capacity: usize,
    historical_recovery_retry_floor: Duration,
    historical_recovery_retry_ceiling: Duration,
) -> Result<V2LaneWorkLimits, V2RunnerError> {
    let non_zero = |value: u64| {
        usize::try_from(value)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or(V2RunnerError::InvalidLimits)
    };
    let non_zero_u32 = |value: u64| {
        u32::try_from(value)
            .ok()
            .and_then(std::num::NonZeroU32::new)
            .ok_or(V2RunnerError::InvalidLimits)
    };
    let merge_leader_body_frame_headroom_bytes =
        non_zero(config.limits.merge_leader_body_frame_headroom_bytes)?;
    if merge_leader_body_frame_headroom_bytes.get() >= consensus_frame_byte_capacity {
        return Err(V2RunnerError::InvalidLimits);
    }
    let autonomous_producer_recheck_ms =
        NonZeroU64::new(config.limits.autonomous_producer_recheck_ms)
            .ok_or(V2RunnerError::InvalidLimits)?;
    let native_amx_signing_guard_limits = crate::native_amx::NativeAmxSigningGuardLimits::new(
        non_zero(config.limits.native_amx_signing_guard_record_capacity)?,
        non_zero(config.limits.native_amx_signing_guard_record_bytes)?,
        non_zero(config.limits.native_amx_signing_guard_anchor_bytes)?,
    )
    .map_err(|_| V2RunnerError::InvalidLimits)?;
    let merge_sidecar_request_timeout_ms =
        NonZeroU64::new(config.limits.merge_sidecar_request_timeout_ms)
            .ok_or(V2RunnerError::InvalidLimits)?;
    let merge_sidecar_limits = MergeSidecarLimits::new(
        non_zero(config.limits.merge_sidecar_inbound_session_capacity)?,
        non_zero(config.limits.merge_sidecar_inbound_sessions_per_peer)?,
        non_zero(config.limits.merge_sidecar_inbound_assembly_bytes)?,
        non_zero(config.limits.merge_sidecar_inbound_assembly_bytes_per_peer)?,
        non_zero(config.limits.merge_sidecar_deferred_block_capacity)?,
        NonZeroU64::new(config.limits.merge_sidecar_future_block_distance)
            .ok_or(V2RunnerError::InvalidLimits)?,
        Duration::from_millis(merge_sidecar_request_timeout_ms.get()),
        non_zero(config.limits.merge_sidecar_outbound_sessions_per_source)?,
        non_zero(config.limits.merge_sidecar_outbound_bytes_per_source)?,
        non_zero(config.limits.merge_sidecar_server_request_gates_per_source)?,
    )
    .map_err(|_| V2RunnerError::InvalidLimits)?;
    let merge_signing_guard_limits = MergeSigningGuardLimits::new(
        non_zero(config.limits.merge_signing_guard_record_capacity)?,
        non_zero(config.limits.merge_signing_guard_record_bytes)?,
        non_zero(config.limits.merge_signing_guard_total_bytes)?,
    )
    .map_err(|_| V2RunnerError::InvalidLimits)?;
    Ok(V2LaneWorkLimits::new(
        non_zero(config.limits.control_queue_capacity)?,
        non_zero(config.limits.max_transactions)?,
        non_zero(config.limits.effect_work_capacity)?,
        non_zero(config.limits.chunk_queue_capacity)?,
        non_zero(config.limits.certified_request_capacity)?,
        non_zero(config.limits.control_queue_capacity)?,
        NonZeroUsize::new(reply_source_capacity).ok_or(V2RunnerError::InvalidLimits)?,
        NonZeroUsize::new(consensus_frame_byte_capacity).ok_or(V2RunnerError::InvalidLimits)?,
        NonZeroUsize::new(block_sync_frame_byte_capacity).ok_or(V2RunnerError::InvalidLimits)?,
        non_zero(config.limits.authenticated_merge_qc_capacity)?,
        merge_leader_body_frame_headroom_bytes,
        non_zero(config.limits.autonomous_carrier_headroom_bytes)?,
        Duration::from_millis(autonomous_producer_recheck_ms.get()),
        historical_recovery_retry_floor,
        historical_recovery_retry_ceiling,
        non_zero_u32(config.limits.historical_recovery_stuck_attempts)?,
        non_zero_u32(config.limits.historical_recovery_retry_tier_attempts)?,
        non_zero_u32(config.limits.historical_recovery_max_retry_tier)?,
        non_zero(config.limits.sidecar_service_burst)?,
        merge_sidecar_limits,
        merge_signing_guard_limits,
        native_amx_signing_guard_limits,
    ))
}

fn candidate_limits(
    context: &wire::HeightContext,
    config: &SumeragiV2Config,
) -> Result<CandidateLimits, V2RunnerError> {
    let max_transactions = NonZeroUsize::new(usize::try_from(config.limits.max_transactions)?)
        .ok_or(V2RunnerError::InvalidLimits)?;
    let context_payload = usize::try_from(context.da_layout.max_payload_size_bytes)?;
    let configured_payload = usize::try_from(config.limits.max_payload_bytes)?;
    let max_payload = NonZeroUsize::new(context_payload.min(configured_payload))
        .ok_or(V2RunnerError::InvalidLimits)?;
    CandidateLimits::new(
        max_transactions,
        max_payload,
        NonZeroUsize::new(usize::try_from(config.limits.max_queue_scan)?)
            .ok_or(V2RunnerError::InvalidLimits)?,
    )
    .map_err(Into::into)
}

fn candidate_attachments(
    context: &wire::HeightContext,
    state: &State,
    parent: CandidateParent<'_>,
    view: wire::View,
    round_header: &BlockHeader,
    npos_vrf: &V2NposVrfLifecycle,
) -> Result<CandidateAttachments, V2RunnerError> {
    if round_header.height().get() != context.height
        || round_header.prev_block_hash() != Some(parent.hash())
        || round_header.view_change_index() != view
        || round_header.merkle_root().is_some()
        || round_header.result_merkle_root().is_some()
    {
        return Err(V2RunnerError::Candidate(
            "certified merge carrier probe differs from the frozen round".to_owned(),
        ));
    }
    let effects = if context.mode == wire::ConsensusMode::Npos {
        super::penalties::PenaltyApplier::from_parts(
            state,
            #[cfg(feature = "telemetry")]
            Some(state.metrics()),
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(context.height, npos_vrf.pending_records())
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))?
    } else {
        Default::default()
    };
    let npos_consensus_effects = (!effects.is_empty()).then_some(effects);
    super::v2_npos::validate_candidate_records(context, state, npos_consensus_effects.as_ref())
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))?;
    let merge_selection = certified_merge_selection_for_npos(npos_consensus_effects.is_some());
    if merge_selection == PendingCertifiedMergeSelection::ControlOnly {
        iroha_logger::debug!(
            height = context.height,
            view,
            "prioritizing deterministic NPoS effects before a certified execution carrier"
        );
    }
    let expected_merge_epoch = state
        .merge_ledger()
        .latest()
        .map_or(1, |latest| latest.epoch_id.saturating_add(1));
    let selected_merge_entry = state
        .select_pending_certified_merge_entry_for_round(
            round_header,
            expected_merge_epoch,
            merge_selection,
        )
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))?;
    let certified_merge_entry = selected_merge_entry
        .map(|(_, entry, _)| entry)
        .map(|entry| {
            super::v2_lane_work::authenticate_merge_entry_for_height_context(context, &entry)
                .map(|()| entry)
                .map_err(V2RunnerError::Candidate)
        })
        .transpose()?;
    let parent_creation_time = match parent {
        CandidateParent::Block(parent) => parent.header().creation_time(),
        CandidateParent::Snapshot(anchor) => {
            Duration::from_millis(anchor.snapshot_block_creation_time_ms)
        }
    };
    Ok(CandidateAttachments {
        time_trigger_clock_progress_required: state
            .time_trigger_clock_progress_required_fast(parent_creation_time),
        npos_consensus_effects,
        certified_merge_carrier_header: certified_merge_entry
            .as_ref()
            .and_then(|entry| entry.execution_batch.as_ref())
            .map(|batch| batch.application_block_header.clone()),
        certified_merge_entry,
        ..CandidateAttachments::default()
    })
}

const fn certified_merge_selection_for_npos(
    has_npos_effects: bool,
) -> PendingCertifiedMergeSelection {
    if has_npos_effects {
        PendingCertifiedMergeSelection::ControlOnly
    } else {
        PendingCertifiedMergeSelection::Any
    }
}

fn adapter_fingerprints(local_peer: &PeerId, config: &SumeragiV2Config) -> AdapterFingerprints {
    let node = Hash::new(local_peer.encode());
    let mut build_preimage = env!("CARGO_PKG_VERSION").as_bytes().to_vec();
    build_preimage.extend_from_slice(
        option_env!("GIT_COMMIT_HASH")
            .unwrap_or("unknown")
            .as_bytes(),
    );
    AdapterFingerprints {
        node,
        build: Hash::new(build_preimage),
        config: config.fingerprint(),
    }
}

fn apply_bounded_sidecar_admissions<T, Error>(
    limit: usize,
    mut next: impl FnMut() -> Result<Option<T>, Error>,
    mut apply: impl FnMut(T) -> Result<(), Error>,
) -> Result<usize, Error> {
    let mut applied = 0usize;
    for _ in 0..limit.max(1) {
        let Some(admission) = next()? else {
            break;
        };
        apply(admission)?;
        applied = applied.saturating_add(1);
    }
    Ok(applied)
}

fn apply_certified_merge_sidecar_chunk_admissions(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
    limit: usize,
) -> Result<(), V2RunnerError> {
    apply_bounded_sidecar_admissions(
        limit,
        || {
            let mut admissions = services
                .drain_certified_merge_sidecar_chunk_admissions(1)
                .map_err(V2RunnerError::Service)?;
            Ok(admissions.pop())
        },
        |admission| {
            lane_work
                .acknowledge_certified_merge_sidecar_chunk_admission(&admission, Instant::now())
                .map_err(V2RunnerError::LaneWork)
        },
    )?;
    Ok(())
}

fn retry_exact_output_and_apply_sidecar_admissions(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
    limit: usize,
) -> Result<bool, V2RunnerError> {
    apply_certified_merge_sidecar_closed_prefixes(lane_work, services)?;
    let pending = services
        .retry_pending_exact_output()
        .map_err(V2RunnerError::Service)?;
    apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;
    Ok(pending)
}

include!("v2_runner/finalized_output_rollover.rs");

fn apply_certified_merge_sidecar_closed_prefixes(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
) -> Result<(), V2RunnerError> {
    apply_certified_merge_sidecar_closed_prefixes_with(lane_work, |prefix| {
        services
            .close_certified_merge_sidecar_prefix(prefix)
            .map(|_| ())
    })
}

fn apply_certified_merge_sidecar_closed_prefixes_with(
    lane_work: &mut V2LaneWorkAdapter,
    mut apply: impl FnMut(&CertifiedMergeSidecarClosedPrefix) -> Result<(), String>,
) -> Result<(), V2RunnerError> {
    let mut prefixes = std::collections::VecDeque::from(lane_work.drain_closed_sidecar_prefixes());
    let had_prefixes = !prefixes.is_empty();
    while let Some(prefix) = prefixes.pop_front() {
        if let Err(error) = apply(&prefix) {
            lane_work.requeue_closed_sidecar_prefixes(std::iter::once(prefix).chain(prefixes));
            return Err(V2RunnerError::Service(error));
        }
    }
    if had_prefixes {
        lane_work.confirm_closed_sidecar_prefix_handoff();
    }
    Ok(())
}

/// Require the exact queue owner observed by the preceding guarded peek.
///
/// The runner holds exclusive access to the lane adapter, so an empty second
/// read cannot reflect a competing dequeue. It means the shared output guard
/// closed between the peek and drain permits; leave the original queued owner
/// untouched and surface the already-required process restart.
fn require_peeked_lane_work_effect(
    drained: Option<V2LaneWorkEffect>,
) -> Result<V2LaneWorkEffect, V2RunnerError> {
    drained.ok_or(V2RunnerError::RestartRequired)
}

include!("v2_runner/canonical_recovery_ingress.rs");

pub(in crate::sumeragi) fn dispatch_lane_work_effects(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
    limit: usize,
) -> Result<(), V2RunnerError> {
    apply_certified_merge_sidecar_closed_prefixes(lane_work, services)?;
    apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;
    let scan_limit = lane_work.effect_count();
    let mut dispatched = 0usize;
    for _ in 0..scan_limit {
        if dispatched >= limit.max(1) {
            break;
        }
        let Some(mut next_effect) = lane_work.next_effect() else {
            break;
        };
        if !retain_active_owned_reply_routes(&mut next_effect) {
            let _ = require_peeked_lane_work_effect(lane_work.drain_effects(1).pop())?;
            continue;
        }
        if !services
            .can_retain_lane_work_effect(&next_effect)
            .map_err(V2RunnerError::Service)?
        {
            let effect = require_peeked_lane_work_effect(lane_work.drain_effects(1).pop())?;
            drop(effect);
            if !lane_work.requeue_effect(next_effect) {
                return Err(V2RunnerError::Service(
                    "lane-work scheduler could not restore a reserved effect".to_owned(),
                ));
            }
            continue;
        }
        let effect = require_peeked_lane_work_effect(lane_work.drain_effects(1).pop())?;
        drop(effect);
        match dispatch_lane_work_effect(services, next_effect)? {
            LaneWorkEffectDispatch::Complete => {
                dispatched = dispatched.saturating_add(1);
            }
            LaneWorkEffectDispatch::SourceRetained(effect) => {
                if !lane_work.requeue_effect(effect) {
                    return Err(V2RunnerError::Service(
                        "lane-work scheduler could not retain a source-backpressured sidecar effect"
                            .to_owned(),
                    ));
                }
            }
        }
        apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;
    }
    Ok(())
}

include!("v2_runner/reply_route_retention.rs");

#[derive(Debug)]
enum LaneWorkEffectDispatch {
    Complete,
    SourceRetained(V2LaneWorkEffect),
}

fn dispatch_lane_work_effect(
    services: &ProductionV2Services,
    effect: V2LaneWorkEffect,
) -> Result<LaneWorkEffectDispatch, V2RunnerError> {
    match effect {
        V2LaneWorkEffect::PostLaneBlock { peer, message } => services
            .post_lane_block(peer, message)
            .map_err(V2RunnerError::Service)?,
        V2LaneWorkEffect::PostDurableLaneCertificate {
            peer,
            reply_routes,
            ingress_ownership,
            certificate,
        } => {
            let reply_routes = reply_routes.ok_or_else(|| {
                V2RunnerError::Service(
                    "durable lane-certificate response lost its authenticated reply routes"
                        .to_owned(),
                )
            })?;
            let ingress_ownership = ingress_ownership.ok_or_else(|| {
                V2RunnerError::Service(
                    "durable lane-certificate response lost its fair-ingress ownership".to_owned(),
                )
            })?;
            if !ingress_ownership.validate_exact()
                || !ingress_ownership.matches_reply_routes(Some(&reply_routes))
            {
                return Err(V2RunnerError::Service(
                    "durable lane-certificate response carried altered ingress ownership"
                        .to_owned(),
                ));
            }
            services
                .post_durable_lane_certificate_on_reply_routes(
                    peer,
                    reply_routes,
                    ingress_ownership,
                    certificate,
                )
                .map_err(V2RunnerError::Service)?;
        }
        V2LaneWorkEffect::PostNativeAmx {
            peer,
            reply_routes,
            message,
        } => {
            services.post_native_amx_with_reply_routes(peer, reply_routes, message);
        }
        V2LaneWorkEffect::PostLaneDrainVote { peer, vote } => {
            services.post_lane_drain_vote(peer, vote);
        }
        V2LaneWorkEffect::BroadcastMerge(signature) => {
            services.broadcast_merge_to_voters(signature);
        }
        V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer,
            reply_routes,
            message,
        } => {
            let route_shape_is_valid = match message.as_ref() {
                CertifiedMergeSidecarMessage::Request(_)
                | CertifiedMergeSidecarMessage::Close(_) => reply_routes.is_none(),
                CertifiedMergeSidecarMessage::CloseAck(_)
                | CertifiedMergeSidecarMessage::GenerationHint(_)
                | CertifiedMergeSidecarMessage::Chunk(_) => reply_routes.is_some(),
            };
            if !route_shape_is_valid {
                return Err(V2RunnerError::Service(
                    "certified merge-sidecar effect lost its exact reply-route ownership"
                        .to_owned(),
                ));
            }
            let ownership = services
                .post_certified_merge_sidecar_with_reply_routes(
                    peer.clone(),
                    reply_routes.clone(),
                    Arc::clone(&message),
                )
                .map_err(V2RunnerError::Service)?;
            if ownership == ExactFanoutOwnership::SourceRetained {
                return Ok(LaneWorkEffectDispatch::SourceRetained(
                    V2LaneWorkEffect::PostCertifiedMergeSidecar {
                        peer,
                        reply_routes,
                        message,
                    },
                ));
            }
        }
    }
    Ok(LaneWorkEffectDispatch::Complete)
}

include!("v2_runner/merge_sidecar_recovery.rs");

fn drain_lane_relay_ingress(
    lane_relay_rx: &std::sync::mpsc::Receiver<super::LaneRelayMessage>,
    lane_work: &mut V2LaneWorkAdapter,
    active_view: wire::View,
    limit: usize,
) -> std::result::Result<(), V2LaneWorkError> {
    let mut drained_any = false;
    for _ in 0..limit.max(1) {
        let mut drained = false;
        if let Ok(message) = lane_relay_rx.try_recv() {
            let _ = lane_work.accept_relay_message(message, active_view);
            drained = true;
            drained_any = true;
        }
        if !drained {
            break;
        }
    }
    if drained_any {
        let _ = lane_work.service_next_historical_recovery()?;
    }
    Ok(())
}

/// Fail-closed live-runner error.
#[derive(Debug, Error)]
pub(super) enum V2RunnerError {
    /// Active-height recovery failed.
    #[error(transparent)]
    Recovery(#[from] super::v2_recovery::V2RecoveryError),
    /// Runner/status activation ownership was inconsistent.
    #[error(transparent)]
    SuccessorActivation(#[from] super::status::V2SuccessorActivationError),
    /// Successor construction returned authority for another same-height predecessor.
    #[error(
        "Sumeragi v2 successor predecessor authority changed during construction: expected {expected:?}, actual {actual:?}"
    )]
    SuccessorPredecessorAuthorityMismatch {
        /// Exact predecessor identity which began the Running handoff.
        expected: DurableV2PredecessorIdentity,
        /// Exact predecessor identity returned by verified construction.
        actual: DurableV2PredecessorIdentity,
    },
    /// A typed successor lifecycle transition failed the shared pure refinement kernel.
    #[error("Sumeragi v2 successor lifecycle failed the production refinement kernel")]
    SuccessorRefinementRejected,
    /// Canonical CompleteTip lifecycle storage could not be retired exactly.
    #[error(transparent)]
    CompleteTipPredecessorStorage(#[from] CompleteTipPredecessorStorageErrorV1),
    /// CompleteTip retirement succeeded, but its retained canonical successor
    /// ledger or prepared status no longer matches the exact restart authority.
    #[error(
        "Sumeragi v2 CompleteTip predecessor {predecessor:?} no longer authenticates its canonical successor"
    )]
    CompleteTipSuccessorAuthorityInvalid {
        /// Exact retired durable predecessor whose successor was rejected.
        predecessor: DurableV2PredecessorIdentity,
    },
    /// The runner activation permit named another fair-ingress instance.
    #[error("launched lifecycle changed the runner-owned fair-ingress instance")]
    LifecycleActivationIngressMismatch,
    /// Reducer/WAL adapter failed.
    #[error(transparent)]
    Adapter(#[from] super::v2::AdapterError),
    /// Runtime configuration failed.
    #[error("invalid Sumeragi v2 runtime configuration: {0}")]
    RuntimeConfig(#[from] super::v2_runtime::RuntimeConfigError),
    /// Live pacemaker clocks were activated outside the one-shot startup boundary.
    #[error(transparent)]
    RuntimeClock(#[from] super::v2_runtime::RuntimeClockError),
    /// Canonical shared consensus configuration was invalid.
    #[error(transparent)]
    SharedConfig(#[from] iroha_config::parameters::actual::SumeragiV2ConfigError),
    /// Effect boundary failed closed.
    #[error(transparent)]
    Effect(#[from] super::v2_effects::EffectExecutorError),
    /// Candidate construction failed.
    #[error(transparent)]
    CandidateBuild(#[from] super::v2_candidate::CandidateError),
    /// Bounded lane-local/merge/Native-AMX adapter failed closed.
    #[error(transparent)]
    LaneWork(#[from] super::v2_lane_work::V2LaneWorkError),
    /// Authenticated NPoS VRF lifecycle failed closed.
    #[error(transparent)]
    NposVrf(#[from] super::v2_npos::V2NposError),
    /// Durable lane reservation ownership could not be reconciled exactly.
    #[error(transparent)]
    Reservation(#[from] V2ReservationLifecycleError),
    /// Integer conversion failed.
    #[error(transparent)]
    Integer(#[from] std::num::TryFromIntError),
    /// Sequential CommitQC/body synchronization failed closed.
    #[error(transparent)]
    BlockSync(#[from] V2BlockSyncError),
    /// Production service failed.
    #[error("Sumeragi v2 production service failed: {0}")]
    Service(String),
    /// Fresh genesis leader no longer has the signed genesis body.
    #[error("Sumeragi v2 height one is missing its signed genesis body")]
    MissingGenesisBody,
    /// Staged and pending-replay height-one capabilities were both present.
    #[error("Sumeragi v2 startup produced conflicting authenticated genesis Nexus/AMX contexts")]
    ConflictingGenesisNexusContext,
    /// Interrupted-tip application did not reach its strict durable repair boundary.
    #[error(
        "Sumeragi v2 interrupted-tip recovery did not complete post-apply metadata and Native AMX evidence repair before lane-work construction"
    )]
    PendingTipRecoveryIncomplete,
    /// Closed-ingress interrupted-tip recovery exhausted its cadence-derived deadline.
    #[error(
        "Sumeragi v2 interrupted-tip recovery exceeded {timeout:?} after {attempts} serialized attempts at stage {stage:?}; process restart is required"
    )]
    PendingTipRecoveryDeadlineExceeded {
        /// Cadence-derived maximum local recovery duration.
        timeout: Duration,
        /// Number of serialized recovery scheduler attempts completed.
        attempts: u64,
        /// Exact authenticated recovery stage retained at expiry.
        stage: Option<PendingKuraApplyRecoveryStage>,
    },
    /// Durable parent body is unavailable in Kura.
    #[error("Sumeragi v2 successor is missing its canonical parent block")]
    MissingParent,
    /// Snapshot bootstrap context is not the exact successor of an unavailable Kura parent.
    #[error("Sumeragi v2 snapshot bootstrap parent geometry is invalid or unexpectedly has a body")]
    InvalidSnapshotBootstrapParent,
    /// Snapshot successor cadence is zero or not representable as whole wire milliseconds.
    #[error("Sumeragi v2 snapshot bootstrap cadence must be positive whole milliseconds")]
    InvalidSnapshotBootstrapCadence,
    /// Locked subject differs from loaded durable bytes.
    #[error("loaded Sumeragi v2 locked body differs from the reducer lock")]
    LockedBodyMismatch,
    /// A local or recovered proposal carried execution results.
    #[error("Sumeragi v2 proposal body must be resultless")]
    ResultBearingProposal,
    /// A locally assembled body could not bind its lane-local work to the exact round.
    #[error("local Sumeragi v2 candidate could not bind its lane-local ownership artifacts")]
    LaneCandidateBinding,
    /// Candidate tag belongs to another height.
    #[error("stale Sumeragi v2 proposal tag")]
    StaleTag,
    /// Runtime has already failed closed.
    #[error("Sumeragi v2 runtime is fail-closed")]
    RuntimeFailClosed,
    /// Single-owner runtime capacity changed between fair dequeue and enqueue.
    #[error("Sumeragi v2 atomic runtime admission invariant failed: {0}")]
    RuntimeAdmissionInvariant(String),
    /// A process-lifetime fatal guard was activated by another consensus service.
    #[error("Sumeragi v2 consensus requires process restart")]
    RestartRequired,
    /// A configured limit is zero.
    #[error("Sumeragi v2 configured limits must be positive")]
    InvalidLimits,
    /// The fixed v2 ingress cannot reserve first-message and progress slots for the roster.
    #[error(
        "Sumeragi v2 body ingress capacity {configured} is smaller than the {required} first-message, progress, and untrusted slots required by the frozen roster"
    )]
    IngressCapacity {
        /// Configured fixed queue capacity.
        configured: usize,
        /// Required validator-lane plus untrusted-lane capacity.
        required: usize,
    },
    /// The fixed v2 ingress cannot isolate one wire-byte quota per active source lane.
    #[error(
        "Sumeragi v2 body ingress byte capacity {configured} is smaller than the {required} bytes required to isolate the frozen roster plus the untrusted lane"
    )]
    IngressByteCapacity {
        /// Configured aggregate canonical-wire byte capacity.
        configured: usize,
        /// Required per-source byte reservations for validators and untrusted traffic.
        required: usize,
    },
    /// Outstanding asynchronous work could overflow trusted completion admission.
    #[error(
        "Sumeragi v2 effect-work capacity {pending} exceeds runtime completion reserve {reserve}"
    )]
    EffectWorkExceedsCompletionReserve {
        /// Maximum outstanding asynchronous tasks.
        pending: usize,
        /// Runtime slots reserved for their trusted completions.
        reserve: usize,
    },
    /// The deterministic parent-plus-cadence timestamp exceeded wire range.
    #[error("Sumeragi v2 logical block timestamp exceeds u64 milliseconds")]
    V2BlockTimeOverflow,
    /// Deterministic local candidate operation failed.
    #[error("Sumeragi v2 candidate failed: {0}")]
    Candidate(String),
    /// A fresh, inbound, or recovered body carried no deterministic ledger work.
    #[error("Sumeragi v2 proposal carries no transaction, internal, or time-trigger work")]
    EmptyProposalWork,
    /// The one bounded non-empty recovery retry failed deterministic validation.
    #[error("Sumeragi v2 non-empty recovery retry failed validation: {0}")]
    LocalNonEmptyRetryRejected(String),
    /// The exact bounded discovery request vanished before reducer admission.
    #[error("Sumeragi v2 CommitQC discovery request disappeared before reducer admission")]
    BlockSyncRequestDisappeared,
}

#[cfg(test)]
#[path = "v2_runner_tests.rs"]
mod tests;
