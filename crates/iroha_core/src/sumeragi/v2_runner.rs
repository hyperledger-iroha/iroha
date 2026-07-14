//! Serialized production height runner for the authoritative Sumeragi v2 reducer.
//!
//! This module owns exactly one reducer/effect executor at a time. It opens the
//! immutable context and safety WAL before processing network traffic, routes
//! authenticated control and body messages, schedules bounded proposal work,
//! and performs an explicit Kura-authorized rollover after application.

use std::{
    collections::BTreeSet,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use super::v2_core::{EventTag, Generation};
#[cfg(test)]
use iroha_config::parameters::actual::SUMERAGI_V2_CONFIG_FORMAT_VERSION;
use iroha_config::parameters::actual::{NodeRole, SumeragiV2Config, sumeragi_v2_timing_ms};
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    Encode as _,
    account::AccountId,
    block::{SignedBlock, consensus_v2 as wire},
    events::{EventBox, pipeline::PipelineEventBox},
    peer::PeerId,
};
use thiserror::Error;

use super::{
    FairV2Ingress, GenesisWithPubKey, InboundBlockMessage, SumeragiWorker,
    message::BlockMessage,
    output_guard::{ConsensusOutputGuard, ConsensusOutputPermit},
    v2::{
        AdapterEffect, AdapterFingerprints, LocalProposalDirective, SignRequest, SumeragiV2Adapter,
    },
    v2_apply::{V2ReservationLifecycleError, reconcile_lane_reservation_ownership},
    v2_block_sync::{
        CommitCertificateAdmissionError, V2BlockSyncDiscovery, V2BlockSyncError, V2BlockSyncServer,
    },
    v2_body_store::BlockSignaturePolicy,
    v2_candidate::{
        CandidateAttachments, CandidateDescriptor, CandidateLimits, CandidateParent,
        CandidateRequest, CandidateWorkProvider, CandidateWorkUnavailable, PreparedCandidateWork,
        V2CandidateAssembler,
    },
    v2_chunks::{EncodedV2Payload, encode_payload},
    v2_effects::{
        EffectExecutorStep, EffectQueueConfig, EffectTransportError, PostFinalityCleanupOutcome,
        PostFinalityCleanupTarget, V2EffectExecutor,
    },
    v2_lane_work::{
        AuthenticatedGenesisNexusAmxContext, MergeSidecarDeferralDisposition, V2LaneIngressOutcome,
        V2LaneWorkAdapter, V2LaneWorkEffect, V2LaneWorkLimits,
    },
    v2_recovery::{build_verified_successor, recover_active_height},
    v2_runtime::{NetworkIngressError, RuntimeQueueConfig, SerializedV2Runtime},
    v2_worker::{ProductionV2Services, V2CleanupSupervisor},
};
use crate::{block::BlockBuilder, kura::Kura, queue::Queue, state::State};

const IDLE_POLL: Duration = Duration::from_millis(10);
const CANDIDATE_WORK_RECHECK: Duration = Duration::from_millis(100);

/// Run the v2-only worker until shutdown or a fail-closed error.
pub(super) fn run(worker: SumeragiWorker) {
    let _status_clear = V2StatusClearGuard::new();
    let ingress_ready = Arc::clone(&worker.ingress_ready);
    let block_ingress = Arc::clone(&worker.block_rx);
    let output_guard = Arc::clone(&worker.output_guard);
    let _ingress_clear = V2IngressClearGuard::new(Arc::clone(&ingress_ready), block_ingress);
    // Declared after ingress cleanup so reverse-order unwinding closes the
    // process output gate before readiness state is released.
    let mut failure_guard = V2RunnerFailureGuard::new(Arc::clone(&output_guard));
    match run_inner(worker) {
        Ok(()) => failure_guard.disarm(),
        Err(error) => {
            output_guard.activate_restart_required();
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

struct V2StatusClearGuard;

impl V2StatusClearGuard {
    fn new() -> Self {
        super::status::clear_v2_status();
        Self
    }
}

impl Drop for V2StatusClearGuard {
    fn drop(&mut self) {
        super::status::clear_v2_status();
    }
}

fn close_ingress_for_rollover(ingress_ready: &AtomicBool, block_ingress: &FairV2Ingress) {
    ingress_ready.store(false, Ordering::Release);
    block_ingress.close();
}

fn validate_deadline_duration(duration: Duration) -> Result<(), V2RunnerError> {
    Instant::now()
        .checked_add(duration)
        .ok_or(V2RunnerError::InvalidLimits)?;
    Ok(())
}

fn deadline_after(now: Instant, duration: Duration) -> Instant {
    now.checked_add(duration)
        .expect("consensus deadline duration was prevalidated before height startup")
}

fn snapshot_successor_logical_time(
    anchor: &wire::SnapshotBootstrapAnchor,
    block_cadence: Duration,
) -> Result<Duration, V2RunnerError> {
    let cadence_ms =
        u64::try_from(block_cadence.as_millis()).map_err(|_| V2RunnerError::V2BlockTimeOverflow)?;
    if cadence_ms == 0 || Duration::from_millis(cadence_ms) != block_cadence {
        return Err(V2RunnerError::InvalidSnapshotBootstrapCadence);
    }
    let successor_ms = anchor
        .snapshot_block_creation_time_ms
        .checked_add(cadence_ms)
        .ok_or(V2RunnerError::V2BlockTimeOverflow)?;
    Ok(Duration::from_millis(successor_ms))
}

#[allow(clippy::too_many_lines)]
fn run_inner(worker: SumeragiWorker) -> Result<(), V2RunnerError> {
    let SumeragiWorker {
        config,
        common_config,
        events_sender,
        state,
        queue,
        kura,
        network,
        genesis_network,
        block_rx,
        vote_rx,
        block_payload_rx,
        lane_relay_rx,
        wake_rx,
        shutdown_signal,
        ingress_ready,
        output_guard,
    } = worker;

    let GenesisWithPubKey {
        genesis,
        public_key: genesis_public_key,
        v2_bootstrap,
    } = genesis_network;
    let genesis_body = genesis.map(|block| block.0);
    let recovery = output_guard
        .begin_fail_stop_operation()
        .ok_or(V2RunnerError::RestartRequired)?;
    let recovered = recover_active_height(
        kura.as_ref(),
        state.as_ref(),
        v2_bootstrap,
        genesis_public_key.clone(),
    )?;
    recovery.complete();
    let mut pending_kura_apply = recovered.pending_kura_apply();
    let (
        mut verified_context,
        context_store,
        mut signature_policy,
        mut staged_genesis_nexus_amx_context,
    ) = recovered.into_parts();
    let reservation_recovery = output_guard
        .begin_fail_stop_operation()
        .ok_or(V2RunnerError::RestartRequired)?;
    let recovered_reservations = queue.live_lane_reservations().len();
    let (finalized_committed_reservations, released_orphans) =
        reconcile_lane_reservation_ownership(
            state.as_ref(),
            queue.as_ref(),
            kura.as_ref(),
            &verified_context.context().chain_id,
        )?;
    reservation_recovery.complete();
    if recovered_reservations > 0 || finalized_committed_reservations > 0 || released_orphans > 0 {
        iroha_logger::info!(
            recovered = recovered_reservations,
            finalized_committed = finalized_committed_reservations,
            released_orphans,
            "reconciled durable lane reservations against committed v2 merge history"
        );
    }
    let local_peer = common_config.peer.id().clone();
    let genesis_account = AccountId::new(genesis_public_key);
    let mut first_height_genesis = genesis_body;
    let mut block_sync_server = None;
    // The first-release cadence is selected by the signed startup state and
    // remains immutable for the lifetime of this consensus process. Reading
    // mutable world parameters again at each height would let an unrelated
    // parameter update change the handshake/config fingerprint mid-chain.
    let block_cadence = state.sumeragi_block_cadence();
    let block_cadence_ms = u64::try_from(block_cadence.as_millis())?;
    let (round_timeout_ms, retransmit_interval_ms) = sumeragi_v2_timing_ms(block_cadence_ms)?;
    let round_timeout = Duration::from_millis(round_timeout_ms);
    let retransmit_interval = Duration::from_millis(retransmit_interval_ms);
    validate_deadline_duration(round_timeout)?;
    validate_deadline_duration(retransmit_interval)?;
    let post_finality_cleanup_timeout = round_timeout;
    let mut cleanup_supervisor = V2CleanupSupervisor::default();

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
            .configure_roster(
                context
                    .roster
                    .iter()
                    .map(|validator| validator.validator.clone()),
            )
            .map_err(|error| V2RunnerError::IngressCapacity {
                configured: error.configured(),
                required: error.required(),
            })?;
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
        let lane_work_limits = lane_work_limits(&shared_config)?;
        let candidate_limits = candidate_limits(&context, &shared_config)?;
        let local_validator = local_validator_index(&context, &local_peer, config.role)?;
        let new_block_sync_server = block_sync_server
            .is_none()
            .then(|| V2BlockSyncServer::new(context.chain_id.clone(), certified_request_capacity))
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
        let adapter_construction = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        let (adapter, startup_effects) = SumeragiV2Adapter::open(
            wal_path,
            verified_context,
            local_validator,
            Generation::new(context.height),
            consensus_key_hash,
            fingerprints,
        )?;
        adapter_construction.complete();
        let runtime_construction = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        let (runtime, startup_effects) = SerializedV2Runtime::new(
            adapter,
            startup_effects,
            Instant::now(),
            round_timeout,
            runtime_queue,
        )?;
        runtime_construction.complete();
        let (mut executor, body_store) = V2EffectExecutor::open(
            runtime,
            storage_root.join("bodies"),
            context.clone(),
            local_peer.clone(),
            local_validator,
            signature_policy,
            Arc::clone(&output_guard),
            effect_queue,
        )?;
        // A replayed ProposalIntent already owns this reducer incarnation.  Its
        // asynchronous signature completion must restore and broadcast the
        // exact durable payload before any fresh candidate work is admitted.
        let replayed_proposal_tag = replayed_proposal_sign_tag(&startup_effects);
        let recovering_interrupted_tip = pending_kura_apply.is_some();
        let recovered_applied_height = pending_kura_apply.filter(|pending| {
            usize::try_from(pending.height()).is_ok_and(|height| state.committed_height() == height)
        });
        let mut authenticated_genesis_nexus_amx_context =
            staged_genesis_nexus_amx_context.map(AuthenticatedGenesisNexusAmxContext::Staged);
        if let Some(pending) = pending_kura_apply.take() {
            let pending_replay_verification = output_guard
                .begin_fail_stop_operation()
                .ok_or(V2RunnerError::RestartRequired)?;
            let replayed_genesis_nexus_amx_context =
                executor.verify_pending_kura_apply_replay(pending)?;
            pending_replay_verification.complete();
            if recovered_applied_height.is_none()
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
        let mut services = ProductionV2Services::start(
            context.clone(),
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
            block_cadence,
            genesis_account.clone(),
            events_sender.clone(),
            effect_work_capacity,
            certified_request_capacity,
            chunk_queue_capacity,
            Arc::clone(&output_guard),
        )
        .map_err(V2RunnerError::Service)?;
        let mut lane_work = V2LaneWorkAdapter::new_with_output_guard(
            context.clone(),
            local_peer.clone(),
            common_config.key_pair.clone(),
            local_validator.is_some(),
            Arc::clone(&state),
            Arc::clone(&kura),
            lane_work_limits,
            authenticated_genesis_nexus_amx_context,
            recovered_applied_height,
            Arc::clone(&output_guard),
        )?;
        if recovering_interrupted_tip {
            executor.consume_pending_tip_recovery_effects(startup_effects, &mut services)?;
        } else {
            executor.consume_effects(startup_effects, &mut services)?;
            let startup_directive = executor.local_proposal_directive()?;
            // Adapter construction is deliberately merge-silent. Only the exact
            // reducer/WAL recovery directive may unlock candidate signing for its
            // recovered view; a lock or Decision keeps it disabled.
            lane_work.retain_merge_sidecars_for_global_view(
                startup_directive.tag().view(),
                startup_directive.locked_subject(),
                startup_directive.decided_subject(),
            )?;
            dispatch_lane_work_effects(&mut lane_work, &services, control_queue_capacity)?;
        }
        // Startup recovery and durable constructor work must not consume the
        // live height cadence. Interrupted-tip replay remains permanently
        // unarmed because the already-decided runtime is consumed as soon as
        // its local Apply finishes; the fresh successor is armed normally.
        // These additions are infallible after the early representability
        // probes above.
        let height_started_at = Instant::now();
        if !recovering_interrupted_tip {
            executor.arm_live_clocks(height_started_at)?;
        }
        let mut next_block_sync_attempt = deadline_after(height_started_at, round_timeout);
        let mut next_lane_retransmit = deadline_after(height_started_at, retransmit_interval);
        if recovering_interrupted_tip {
            // The replayed Decision may already have crossed Kura or WSV, but it is not a
            // completed height until V2ApplyService has idempotently published the checkpoint,
            // manifest, and finality artifact. Keep all network ingress closed while the normal
            // completion loop drains that exact startup Apply; rollover opens ingress only for
            // the authenticated successor context.
            close_ingress_for_rollover(&ingress_ready, &block_rx);
        } else {
            let Some(ingress_permit) = output_guard.acquire() else {
                return Err(V2RunnerError::RestartRequired);
            };
            block_rx
                .open()
                .map_err(|error| V2RunnerError::IngressCapacity {
                    configured: error.configured(),
                    required: error.required(),
                })?;
            ingress_ready.store(true, Ordering::Release);
            drop(ingress_permit);
        }

        let mut block_sync_request = None;
        let mut attempted_tag = replayed_proposal_tag;
        let mut local_subject = None;
        let mut heartbeat_only_tag = None;
        let mut candidate_work_wait: Option<(EventTag, Instant, Instant)> = None;
        let mut pending_local_events: Option<(
            EventTag,
            wire::BlockSubject,
            Vec<PipelineEventBox>,
        )> = None;

        let finality = loop {
            cleanup_supervisor.reap_finished();
            if output_guard.restart_required() {
                return Err(V2RunnerError::RestartRequired);
            }
            if shutdown_signal.is_sent() {
                services.allow_clean_shutdown();
                return Ok(());
            }

            services.drain_completions(&mut executor)?;
            if !recovering_interrupted_tip {
                drive_merge_sidecar_recovery(&mut executor, &mut services, &mut lane_work)?;
                services
                    .replay_buffered_chunks(&mut executor)
                    .map_err(V2RunnerError::Service)?;
                while let Some(rejection) = services.take_validation_rejection() {
                    if pending_local_events
                        .as_ref()
                        .is_some_and(|(tag, subject, _)| {
                            *tag == executor.current_tag() && *subject == rejection.subject()
                        })
                    {
                        pending_local_events = None;
                    }
                    if local_subject == Some(rejection.subject())
                        && rejection.round().view == executor.current_tag().view()
                    {
                        if heartbeat_only_tag == Some(executor.current_tag()) {
                            return Err(V2RunnerError::LocalHeartbeatRejected(
                                rejection.reason().to_owned(),
                            ));
                        }
                        iroha_logger::warn!(
                            reason = rejection.reason(),
                            "local Sumeragi v2 candidate rejected; retrying an empty heartbeat"
                        );
                        attempted_tag = None;
                        heartbeat_only_tag = Some(executor.current_tag());
                        local_subject = None;
                    }
                }

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
                drain_v2_ingress(
                    &block_rx,
                    &mut executor,
                    &mut services,
                    &mut lane_work,
                    output_guard.as_ref(),
                    kura.as_ref(),
                    &context_store,
                    &common_config.key_pair,
                    block_sync_server
                        .as_mut()
                        .expect("block-sync server initialized before ingress"),
                    &mut block_sync,
                    &mut block_sync_request,
                    body_queue_capacity,
                )?;
                drain_lane_work_ingress(
                    &vote_rx,
                    &block_payload_rx,
                    &lane_relay_rx,
                    &mut lane_work,
                    executor.current_tag().view(),
                    control_queue_capacity,
                );
                drive_merge_sidecar_recovery(&mut executor, &mut services, &mut lane_work)?;
                let now = Instant::now();
                if now >= next_lane_retransmit {
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
                advance_executor(&mut executor, &mut services, control_queue_capacity)?;
                let directive = executor.local_proposal_directive()?;
                lane_work.retain_merge_sidecars_for_global_view(
                    directive.tag().view(),
                    directive.locked_subject(),
                    directive.decided_subject(),
                )?;
                if let Some(locked) = directive.locked_subject() {
                    let newly_locked = lane_work.mark_global_body_locked(locked.block_hash);
                    if newly_locked && local_validator.is_some() {
                        services
                            .request_locked_candidate(executor.current_tag(), locked)
                            .map_err(V2RunnerError::Service)?;
                    }
                }
                while let Some(prepared) = services.take_prepared_candidate() {
                    let matches = pending_local_events
                        .as_ref()
                        .is_some_and(|(tag, subject, _)| {
                            *tag == prepared.tag()
                                && *subject == prepared.subject()
                                && executor.current_tag() == prepared.tag()
                        });
                    if matches && let Some((_, _, events)) = pending_local_events.take() {
                        let Some(_permit) = output_guard.acquire() else {
                            return Err(V2RunnerError::RestartRequired);
                        };
                        for event in events {
                            let _ = events_sender.send(EventBox::Pipeline(event));
                        }
                    }
                }
                if pending_local_events
                    .as_ref()
                    .is_some_and(|(tag, _, _)| *tag != executor.current_tag())
                {
                    pending_local_events = None;
                }
                services
                    .replay_buffered_chunks(&mut executor)
                    .map_err(V2RunnerError::Service)?;
            }

            if executor.ready_to_finish() {
                close_ingress_for_rollover(&ingress_ready, &block_rx);
                lane_work.persist_anchored_sessions()?;
                lane_work.prune_finalized_merge_sidecars()?;
                if !recovering_interrupted_tip {
                    dispatch_lane_work_effects(&mut lane_work, &services, control_queue_capacity)?;
                }
                let (runtime, receipt, artifact) = executor.into_finalized_parts()?;
                let wal_retirement = output_guard
                    .begin_fail_stop_operation()
                    .ok_or(V2RunnerError::RestartRequired)?;
                let finalized = runtime.into_driver().finish_height(&receipt, &artifact)?;
                wal_retirement.complete();
                let mut cleanup = PostFinalityCleanupOutcome::default();
                if let Some(warning) = finalized.wal_retirement_warning() {
                    cleanup.record(PostFinalityCleanupTarget::SafetyWal, warning);
                }
                cleanup.append(services.finish_height(
                    receipt.clone(),
                    post_finality_cleanup_timeout,
                    &mut cleanup_supervisor,
                ));
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
                break (receipt, artifact);
            }

            if recovering_interrupted_tip {
                // The exact body is already durable and no peer can contribute to this recovery
                // boundary. Wait only for the local I/O worker to return Apply durability; the
                // recovery-specific executor rejects every network-producing reducer effect.
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
                queue.as_ref(),
                kura.as_ref(),
                first_height_genesis.as_ref(),
                height_started_at,
                block_cadence,
                &mut attempted_tag,
                &mut local_subject,
                heartbeat_only_tag,
                &mut pending_local_events,
                &mut executor,
                &mut services,
                &mut lane_work,
                &mut candidate_work_wait,
                retransmit_interval,
            )?;
            dispatch_lane_work_effects(&mut lane_work, &services, control_queue_capacity)?;

            let _ = wake_rx.recv_timeout(IDLE_POLL);
        };

        let (receipt, artifact) = finality;
        let successor_construction = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        verified_context =
            build_verified_successor(state.as_ref(), &context_store, &artifact, &receipt)?;
        successor_construction.complete();
        signature_policy = BlockSignaturePolicy::RotatingLeader;
        first_height_genesis = None;
        staged_genesis_nexus_amx_context = None;
    }
}

fn replayed_proposal_sign_tag(effects: &[AdapterEffect]) -> Option<EventTag> {
    effects.iter().find_map(|effect| match effect {
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(_),
        } => Some(*tag),
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

#[allow(clippy::too_many_arguments)]
fn schedule_local_proposal(
    candidate_limits: CandidateLimits,
    context: &wire::HeightContext,
    local_validator: Option<wire::ValidatorIndex>,
    key_pair: &KeyPair,
    output_guard: &ConsensusOutputGuard,
    state: &State,
    queue: &Queue,
    kura: &Kura,
    genesis_body: Option<&SignedBlock>,
    height_started_at: Instant,
    block_cadence: Duration,
    attempted_tag: &mut Option<EventTag>,
    local_subject: &mut Option<wire::BlockSubject>,
    heartbeat_only_tag: Option<EventTag>,
    pending_local_events: &mut Option<(EventTag, wire::BlockSubject, Vec<PipelineEventBox>)>,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
    candidate_work_wait: &mut Option<(EventTag, Instant, Instant)>,
    candidate_work_wait_bound: Duration,
) -> Result<(), V2RunnerError> {
    let Some(local_validator) = local_validator else {
        return Ok(());
    };
    let directive = executor.local_proposal_directive()?;
    if candidate_work_wait
        .as_ref()
        .is_some_and(|(tag, _, _)| *tag != directive.tag())
    {
        *candidate_work_wait = None;
    }
    while let Some(loaded) = services.take_loaded_candidate() {
        let current = executor.local_proposal_directive()?;
        if loaded.tag() != current.tag() || current.locked_subject() != Some(loaded.subject()) {
            iroha_logger::debug!(
                loaded_height = loaded.tag().height(),
                loaded_view = loaded.tag().view(),
                current_height = current.tag().height(),
                current_view = current.tag().view(),
                loaded_subject = ?loaded.subject(),
                current_locked_subject = ?current.locked_subject(),
                "discarded stale locked-body load before Sumeragi v2 re-proposal"
            );
            continue;
        }
        let loaded_subject = loaded.subject();
        let canonical_wire = loaded.into_canonical_wire();
        let block = iroha_data_model::block::decode_framed_signed_block(&canonical_wire)
            .map_err(|error| V2RunnerError::Candidate(error.to_string()))?;
        if !block.is_resultless_proposal() {
            return Err(V2RunnerError::ResultBearingProposal);
        }
        if lane_work.bind_locked_global_body(&block) == V2LaneIngressOutcome::Rejected {
            return Err(V2RunnerError::LaneCandidateBinding);
        }
        if current.leader() != local_validator {
            executor.retain_locked_body_for_reproposal(
                current.tag(),
                loaded_subject,
                canonical_wire,
                services,
            )?;
            iroha_logger::debug!(
                height = current.tag().height(),
                view = current.tag().view(),
                leader = current.leader(),
                local_validator,
                "staged locked body for current-view follower revalidation"
            );
            continue;
        }
        iroha_logger::debug!(
            height = current.tag().height(),
            view = current.tag().view(),
            leader = current.leader(),
            subject = ?current.locked_subject(),
            "submitting exact locked body for Sumeragi v2 re-proposal"
        );
        submit_exact_body(
            context,
            current,
            canonical_wire,
            executor,
            services,
            local_subject,
        )?;
        *attempted_tag = Some(current.tag());
        return Ok(());
    }
    if directive.leader() != local_validator
        || directive.decided_subject().is_some()
        || *attempted_tag == Some(directive.tag())
        || (directive.tag().view() == 0
            && height_started_at.elapsed() < block_cadence
            && context.height > 1)
    {
        return Ok(());
    }

    if let Some(locked) = directive.locked_subject() {
        services
            .request_locked_candidate(directive.tag(), locked)
            .map_err(V2RunnerError::Service)?;
        *attempted_tag = Some(directive.tag());
    } else if context.height == 1 {
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
            local_subject,
        )?;
        *attempted_tag = Some(directive.tag());
    } else {
        if candidate_work_wait
            .as_ref()
            .is_some_and(|(tag, _, next_retry)| {
                *tag == directive.tag() && Instant::now() < *next_retry
            })
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
        let (_, time_source) = iroha_primitives::time::TimeSource::new_mock(logical_time);
        let assembler = V2CandidateAssembler::new(candidate_limits, time_source.clone());
        let attachments =
            candidate_attachments(context, state, parent, directive.tag().view(), time_source)?;
        let candidate = if heartbeat_only_tag == Some(directive.tag()) {
            assembler.assemble(CandidateRequest {
                context,
                directive,
                local_validator,
                parent,
                state,
                queue,
                key_pair,
                output_guard,
                attachments,
                work_provider: HeartbeatOnlyWorkProvider,
            })?
        } else {
            assembler.assemble(CandidateRequest {
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
            })?
        };
        let tag = candidate.tag();
        let report = candidate.scan_report();
        if heartbeat_only_tag != Some(tag) && report.selected == 0 && report.work_deferred > 0 {
            let now = Instant::now();
            let started_at = candidate_work_wait
                .as_ref()
                .filter(|(waiting_tag, _, _)| *waiting_tag == tag)
                .map_or(now, |(_, started_at, _)| *started_at);
            if now.saturating_duration_since(started_at) < candidate_work_wait_bound {
                *candidate_work_wait =
                    Some((tag, started_at, deadline_after(now, CANDIDATE_WORK_RECHECK)));
                return Ok(());
            }
        }
        *candidate_work_wait = None;
        if lane_work.bind_local_candidate(round_for_tag(context, tag)?, candidate.block().hash())
            == V2LaneIngressOutcome::Rejected
        {
            return Err(V2RunnerError::LaneCandidateBinding);
        }
        let (_block, canonical_wire, encoded_payload, events, report) = candidate.into_parts();
        let subject = encoded_payload.manifest().subject;
        *pending_local_events = Some((tag, subject, events));
        iroha_logger::debug!(?report, "assembled bounded Sumeragi v2 candidate");
        submit_encoded_body(
            tag,
            canonical_wire,
            encoded_payload,
            executor,
            services,
            local_subject,
        )?;
        *attempted_tag = Some(tag);
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
    local_subject: &mut Option<wire::BlockSubject>,
) -> Result<(), V2RunnerError> {
    let block = iroha_data_model::block::decode_framed_signed_block(&canonical_wire)
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))?;
    if !block.is_resultless_proposal() {
        return Err(V2RunnerError::ResultBearingProposal);
    }
    let subject = wire::BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    if directive
        .locked_subject()
        .is_some_and(|locked| locked != subject)
    {
        return Err(V2RunnerError::LockedBodyMismatch);
    }
    let round = round_for_tag(context, directive.tag())?;
    let payload = encode_payload(context, round, subject, &canonical_wire)
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))?;
    submit_encoded_body(
        directive.tag(),
        canonical_wire,
        payload,
        executor,
        services,
        local_subject,
    )
}

fn submit_encoded_body(
    tag: EventTag,
    canonical_wire: Vec<u8>,
    payload: EncodedV2Payload,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    local_subject: &mut Option<wire::BlockSubject>,
) -> Result<(), V2RunnerError> {
    let manifest = services
        .register_outbound_payload(payload)
        .map_err(V2RunnerError::Service)?;
    *local_subject = Some(manifest.subject);
    executor.admit_local_proposal(tag, manifest, canonical_wire, services)?;
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

fn drain_v2_ingress(
    receiver: &FairV2Ingress,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
    output_guard: &ConsensusOutputGuard,
    kura: &Kura,
    context_store: &super::v2_context_store::V2ContextStore,
    local_key: &KeyPair,
    block_sync_server: &mut V2BlockSyncServer,
    block_sync: &mut V2BlockSyncDiscovery,
    block_sync_request: &mut Option<HashOf<wire::CommitCertificateRequest>>,
    limit: usize,
) -> Result<(), V2RunnerError> {
    for _ in 0..limit.max(1) {
        let Some(inbound) =
            receiver.try_recv_if(|inbound| v2_ingress_head_can_drain(inbound, executor, services))
        else {
            break;
        };
        let (message, sender) = inbound.into_message_and_sender();
        if matches!(
            message,
            BlockMessage::LaneBlockProposal(_)
                | BlockMessage::LaneBlockVote(_)
                | BlockMessage::LaneBlockQc(_)
        ) {
            let _ = lane_work.accept_lane_message(
                InboundBlockMessage::new(message, sender),
                executor.current_tag().view(),
            );
            continue;
        }
        let BlockMessage::V2(message) = message else {
            iroha_logger::debug!("rejected legacy global message on v2-only consensus ingress");
            continue;
        };
        if let Err(error) = message.validate_version() {
            iroha_logger::debug!(%error, "rejected wrong-version Sumeragi v2 envelope");
            continue;
        }
        match message.payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                enqueue_control(
                    executor,
                    wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                        proposal,
                    )),
                )?;
            }
            wire::ConsensusMessageV2Payload::Vote(vote) => enqueue_control(
                executor,
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote)),
            )?,
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => enqueue_control(
                executor,
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                    certificate,
                )),
            )?,
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => enqueue_control(
                executor,
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutVote(vote)),
            )?,
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => enqueue_control(
                executor,
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
                    certificate,
                )),
            )?,
            wire::ConsensusMessageV2Payload::PayloadManifest(manifest) => {
                if let Err(error) = manifest.validate(executor.context()) {
                    iroha_logger::debug!(%error, "rejected standalone Sumeragi v2 manifest");
                }
            }
            wire::ConsensusMessageV2Payload::PayloadChunk(chunk) => {
                let Some(sender) = sender else {
                    continue;
                };
                services
                    .route_payload_chunk(executor, sender, chunk)
                    .map_err(V2RunnerError::Service)?;
            }
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) => {
                let Some(sender) = sender else {
                    continue;
                };
                if request.round.height < executor.context().height {
                    let response_peer = sender.clone();
                    match serve_block_sync_while_guarded(
                        output_guard,
                        || {
                            block_sync_server.serve_historical_body(
                                kura,
                                context_store,
                                request,
                                &sender,
                                local_key,
                            )
                        },
                        |response, permit| {
                            services.post_to_peer_with_permit(response_peer, response, permit)
                        },
                    ) {
                        Ok(()) => {}
                        Err(error) if is_remote_block_sync_rejection(&error) => {
                            iroha_logger::debug!(%error, "rejected historical certified body request");
                        }
                        Err(error) => return Err(error.into()),
                    }
                } else if request.round.height == executor.context().height {
                    match executor.authenticate_certified_body_request(request, &sender) {
                        Ok(request) => {
                            services
                                .serve_certified_request(request)
                                .map_err(V2RunnerError::Service)?;
                        }
                        Err(error) => {
                            iroha_logger::debug!(%error, "rejected certified body request");
                        }
                    }
                } else {
                    iroha_logger::debug!(
                        requested_height = request.round.height,
                        active_height = executor.context().height,
                        "rejected future-height certified body request"
                    );
                }
            }
            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) => {
                let Some(sender) = sender else {
                    continue;
                };
                match executor.accept_certified_body_response(response, &sender, services) {
                    Ok(_) => {}
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
                    continue;
                };
                let response_peer = sender.clone();
                match serve_block_sync_while_guarded(
                    output_guard,
                    || block_sync_server.serve(kura, request, &sender, local_key),
                    |response, permit| {
                        services.post_to_peer_with_permit(response_peer, response, permit)
                    },
                ) {
                    Ok(()) => {}
                    Err(error) if is_remote_block_sync_rejection(&error) => {
                        iroha_logger::debug!(%error, "rejected CommitQC discovery request");
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) => {
                let Some(sender) = sender else {
                    continue;
                };
                let discovered = match block_sync.authenticate_response(response, &sender) {
                    Ok(discovered) => discovered,
                    Err(error) => {
                        iroha_logger::debug!(%error, "rejected CommitQC discovery response");
                        continue;
                    }
                };
                match block_sync.enqueue_and_complete(discovered, |message| {
                    executor.enqueue_network(message).map(|_| ())
                }) {
                    Ok(()) => *block_sync_request = None,
                    Err(CommitCertificateAdmissionError::Enqueue(
                        NetworkIngressError::FailClosed,
                    )) => return Err(V2RunnerError::RuntimeFailClosed),
                    Err(CommitCertificateAdmissionError::Enqueue(
                        NetworkIngressError::Backpressure(error),
                    )) => {
                        return Err(V2RunnerError::RuntimeAdmissionInvariant(error.to_string()));
                    }
                    Err(CommitCertificateAdmissionError::Enqueue(error)) => {
                        iroha_logger::debug!(%error, "deferred authenticated CommitQC response");
                    }
                    Err(CommitCertificateAdmissionError::RequestDisappeared) => {
                        return Err(V2RunnerError::BlockSyncRequestDisappeared);
                    }
                }
            }
        }
    }
    Ok(())
}

fn v2_ingress_head_can_drain(
    inbound: &InboundBlockMessage,
    executor: &V2EffectExecutor,
    services: &ProductionV2Services,
) -> bool {
    let BlockMessage::V2(message) = inbound.message() else {
        return true;
    };
    if message.validate_version().is_err() {
        return true;
    }
    if !executor.can_admit_network_payload(&message.payload) {
        return false;
    }
    match &message.payload {
        wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request)
            if inbound.sender().is_some() && request.round.height == executor.context().height =>
        {
            services.can_serve_certified_request()
        }
        _ => true,
    }
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

fn serve_block_sync_while_guarded<Response>(
    output_guard: &ConsensusOutputGuard,
    serve: impl FnOnce() -> Result<Option<Response>, V2BlockSyncError>,
    post: impl FnOnce(Response, &ConsensusOutputPermit<'_>) -> Result<(), String>,
) -> Result<(), V2BlockSyncError> {
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
            Ok(())
        }
        Ok(None) => {
            operation.complete();
            Ok(())
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

fn enqueue_control(
    executor: &mut V2EffectExecutor,
    message: wire::ConsensusMessageV2,
) -> Result<(), V2RunnerError> {
    match executor.enqueue_network(message) {
        Ok(_) => Ok(()),
        Err(NetworkIngressError::FailClosed) => Err(V2RunnerError::RuntimeFailClosed),
        Err(NetworkIngressError::Authentication(error)) => {
            iroha_logger::debug!(%error, "rejected Sumeragi v2 control ingress");
            Ok(())
        }
        Err(NetworkIngressError::Backpressure(error)) => {
            Err(V2RunnerError::RuntimeAdmissionInvariant(error.to_string()))
        }
        Err(NetworkIngressError::TransportPayload) => {
            Err(V2RunnerError::RuntimeAdmissionInvariant(
                "transport payload reached reducer-control admission".to_owned(),
            ))
        }
    }
}

fn advance_executor(
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    limit: usize,
) -> Result<(), V2RunnerError> {
    for _ in 0..limit.max(1) {
        match executor.step(Instant::now(), services)? {
            EffectExecutorStep::Idle => break,
            EffectExecutorStep::Advanced { .. } => {}
        }
    }
    Ok(())
}

fn advance_pending_tip_recovery_executor(
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    limit: usize,
) -> Result<(), V2RunnerError> {
    for _ in 0..limit.max(1) {
        match executor.step_pending_tip_recovery(Instant::now(), services)? {
            EffectExecutorStep::Idle => break,
            EffectExecutorStep::Advanced { .. } => {}
        }
    }
    Ok(())
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
        (NodeRole::Validator, None) => Err(V2RunnerError::ValidatorAbsent),
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

fn lane_work_limits(config: &SumeragiV2Config) -> Result<V2LaneWorkLimits, V2RunnerError> {
    let non_zero = |value: u64| {
        usize::try_from(value)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or(V2RunnerError::InvalidLimits)
    };
    Ok(V2LaneWorkLimits::new(
        non_zero(config.limits.control_queue_capacity)?,
        non_zero(config.limits.max_transactions)?,
        non_zero(config.limits.effect_work_capacity)?,
        non_zero(config.limits.chunk_queue_capacity)?,
        non_zero(config.limits.certified_request_capacity)?,
        non_zero(config.limits.control_queue_capacity)?,
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
    time_source: iroha_primitives::time::TimeSource,
) -> Result<CandidateAttachments, V2RunnerError> {
    let pending = BlockBuilder::new_with_time_source(Vec::new(), time_source);
    let round_header = match parent {
        CandidateParent::Block(parent) => pending.chain(view, Some(parent)),
        CandidateParent::Snapshot(anchor) => {
            pending.chain_with_parent_hash(view, anchor.snapshot_height, anchor.snapshot_block_hash)
        }
    }
    .carrier_context_header();
    if round_header.height().get() != context.height
        || round_header.prev_block_hash() != Some(parent.hash())
        || round_header.view_change_index() != view
    {
        return Err(V2RunnerError::Candidate(
            "certified merge carrier probe differs from the frozen round".to_owned(),
        ));
    }
    let expected_merge_epoch = state
        .merge_ledger()
        .latest()
        .map_or(1, |latest| latest.epoch_id.saturating_add(1));
    let certified_merge_entry = state
        .select_pending_certified_merge_entry_for_round(&round_header, expected_merge_epoch)
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))?
        .map(|(_, entry, _)| entry);

    let effects = if context.mode == wire::ConsensusMode::Npos {
        super::penalties::PenaltyApplier::from_parts(
            state,
            #[cfg(feature = "telemetry")]
            Some(state.metrics()),
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(context.height, std::iter::empty())
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))?
    } else {
        Default::default()
    };
    Ok(CandidateAttachments {
        npos_consensus_effects: (!effects.is_empty()).then_some(effects),
        certified_merge_entry,
        ..CandidateAttachments::default()
    })
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

#[derive(Clone, Copy, Debug)]
struct HeartbeatOnlyWorkProvider;

impl CandidateWorkProvider for HeartbeatOnlyWorkProvider {
    fn prepare(
        &mut self,
        _context: &wire::HeightContext,
        _view: wire::View,
        candidates: &[CandidateDescriptor<'_>],
    ) -> Result<PreparedCandidateWork, CandidateWorkUnavailable> {
        Err(CandidateWorkUnavailable::new(
            (0..candidates.len()).collect::<BTreeSet<_>>(),
            "local fallback requested an empty heartbeat",
        ))
    }
}

fn dispatch_lane_work_effects(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
    limit: usize,
) -> Result<(), V2RunnerError> {
    for effect in lane_work.drain_effects(limit.max(1)) {
        match effect {
            V2LaneWorkEffect::PostLaneBlock { peer, message } => services
                .post_lane_block(peer, message)
                .map_err(V2RunnerError::Service)?,
            V2LaneWorkEffect::PostNativeAmx { peer, message } => {
                services.post_native_amx(peer, message);
            }
            V2LaneWorkEffect::BroadcastMerge(signature) => {
                services.broadcast_merge_to_voters(signature);
            }
            V2LaneWorkEffect::PostCertifiedMergeSidecar { peer, message } => {
                services.post_certified_merge_sidecar(peer, message);
            }
        }
    }
    Ok(())
}

fn drive_merge_sidecar_recovery(
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
) -> Result<(), V2RunnerError> {
    lane_work.retain_deferred_merge_sidecars(&executor.deferred_merge_sidecar_blocks())?;
    while let Some(deferred) = services.take_merge_sidecar_deferral() {
        let entry_hash = deferred.reference().entry_hash;
        if !executor.retains_deferred_merge_sidecar(
            deferred.work_id(),
            deferred.round(),
            deferred.subject(),
            entry_hash,
        ) {
            continue;
        }
        let disposition = if executor.deferred_merge_sidecar_is_decided(deferred.work_id()) {
            lane_work.defer_missing_decided_merge_sidecar(
                deferred.round(),
                deferred.subject(),
                deferred.reference().clone(),
            )?
        } else {
            lane_work.defer_missing_merge_sidecar(
                deferred.round(),
                deferred.subject(),
                deferred.reference().clone(),
            )?
        };
        match disposition {
            MergeSidecarDeferralDisposition::Fetching
            | MergeSidecarDeferralDisposition::Available => {}
            MergeSidecarDeferralDisposition::RetryLater => {
                services
                    .requeue_merge_sidecar_deferral(deferred)
                    .map_err(V2RunnerError::Service)?;
                break;
            }
            MergeSidecarDeferralDisposition::Rejected(reason) => {
                let _ = executor.reject_deferred_merge_sidecar_work(
                    deferred.work_id(),
                    reason,
                    services,
                )?;
            }
        }
    }
    while let Some(entry_hash) = lane_work.take_completed_merge_sidecar() {
        let _ = executor.retry_deferred_merge_sidecar(entry_hash, services)?;
    }
    while let Some(rejected) = lane_work.take_rejected_merge_sidecar() {
        let _ = executor.reject_deferred_merge_sidecar(
            rejected.entry_hash(),
            rejected.reason(),
            services,
        )?;
    }
    lane_work.retain_deferred_merge_sidecars(&executor.deferred_merge_sidecar_blocks())?;
    Ok(())
}

fn drain_lane_work_ingress(
    vote_rx: &std::sync::mpsc::Receiver<InboundBlockMessage>,
    block_payload_rx: &std::sync::mpsc::Receiver<InboundBlockMessage>,
    lane_relay_rx: &std::sync::mpsc::Receiver<super::LaneRelayMessage>,
    lane_work: &mut V2LaneWorkAdapter,
    active_view: wire::View,
    limit: usize,
) {
    for _ in 0..limit.max(1) {
        let mut drained = false;
        if let Ok(message) = vote_rx.try_recv() {
            let _ = lane_work.accept_lane_message(message, active_view);
            drained = true;
        }
        if let Ok(message) = block_payload_rx.try_recv() {
            let _ = lane_work.accept_lane_message(message, active_view);
            drained = true;
        }
        if let Ok(message) = lane_relay_rx.try_recv() {
            let _ = lane_work.accept_relay_message(message, active_view);
            drained = true;
        }
        if !drained {
            break;
        }
    }
}

/// Fail-closed live-runner error.
#[derive(Debug, Error)]
pub(super) enum V2RunnerError {
    /// Active-height recovery failed.
    #[error(transparent)]
    Recovery(#[from] super::v2_recovery::V2RecoveryError),
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
    /// Local validator role is absent from the frozen voting roster.
    #[error("Sumeragi v2 node is configured as validator but absent from the frozen roster")]
    ValidatorAbsent,
    /// Fresh genesis leader no longer has the signed genesis body.
    #[error("Sumeragi v2 height one is missing its signed genesis body")]
    MissingGenesisBody,
    /// Staged and pending-replay height-one capabilities were both present.
    #[error("Sumeragi v2 startup produced conflicting authenticated genesis Nexus/AMX contexts")]
    ConflictingGenesisNexusContext,
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
    /// The fixed v2 ingress cannot reserve one slot per active source lane.
    #[error(
        "Sumeragi v2 body ingress capacity {configured} is smaller than the {required} slots required by the frozen roster plus the untrusted lane"
    )]
    IngressCapacity {
        /// Configured fixed queue capacity.
        configured: usize,
        /// Required validator-lane plus untrusted-lane capacity.
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
    /// Even an empty fallback failed deterministic validation.
    #[error("Sumeragi v2 empty heartbeat failed validation: {0}")]
    LocalHeartbeatRejected(String),
    /// The exact bounded discovery request vanished before reducer admission.
    #[error("Sumeragi v2 CommitQC discovery request disappeared before reducer admission")]
    BlockSyncRequestDisappeared,
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use iroha_config::parameters::actual::{NodeRole, SumeragiV2KeyPolicy, SumeragiV2Limits};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        block::decode_framed_signed_block,
        isi::Log,
        peer::PeerId,
        transaction::{TransactionBuilder, signed::TransactionResultInner},
        trigger::DataTriggerSequence,
    };
    use iroha_logger::Level;

    use super::*;

    fn context() -> (wire::HeightContext, Vec<KeyPair>) {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        (
            wire::HeightContext {
                chain_id: ChainId::from("v2-runner-test"),
                protocol_version: wire::PROTOCOL_VERSION,
                height: 1,
                epoch: 0,
                epoch_end_height: u64::MAX,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"runner-test-nexus-amx"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::Plain,
                    chunk_size_bytes: 1024,
                    data_shards: 0,
                    parity_shards: 0,
                    max_payload_size_bytes: 4096,
                    max_chunk_count: 4,
                },
                leader_seed: [0x42; 32],
            },
            keys,
        )
    }

    #[test]
    fn snapshot_successor_time_is_exact_bounded_and_restart_deterministic() {
        let anchor = wire::SnapshotBootstrapAnchor {
            snapshot_height: 99,
            snapshot_block_hash: HashOf::from_untyped_unchecked(Hash::new(b"snapshot tip")),
            snapshot_block_creation_time_ms: 50_000,
            snapshot_state_hash: Hash::new(b"snapshot state"),
        };
        let cadence = Duration::from_millis(750);
        let first = snapshot_successor_logical_time(&anchor, cadence)
            .expect("representable snapshot successor time");
        let restarted = snapshot_successor_logical_time(&anchor, cadence)
            .expect("restart derives the same successor time");
        assert_eq!(first, Duration::from_millis(50_750));
        assert_eq!(restarted, first);

        assert!(matches!(
            snapshot_successor_logical_time(&anchor, Duration::ZERO),
            Err(V2RunnerError::InvalidSnapshotBootstrapCadence)
        ));
        assert!(matches!(
            snapshot_successor_logical_time(&anchor, Duration::from_nanos(999_999)),
            Err(V2RunnerError::InvalidSnapshotBootstrapCadence)
        ));
        let overflowing = wire::SnapshotBootstrapAnchor {
            snapshot_block_creation_time_ms: u64::MAX,
            ..anchor
        };
        assert!(matches!(
            snapshot_successor_logical_time(&overflowing, Duration::from_millis(1)),
            Err(V2RunnerError::V2BlockTimeOverflow)
        ));
    }

    #[test]
    fn explicit_observer_never_votes_even_when_present_in_roster() {
        let (context, keys) = context();
        let peer = PeerId::new(keys[0].public_key().clone());
        assert_eq!(
            local_validator_index(&context, &peer, NodeRole::Observer).expect("observer"),
            None
        );
        assert!(
            local_validator_index(
                &context,
                &PeerId::new(
                    KeyPair::try_from_seed(vec![0x55; 32], Algorithm::BlsNormal)
                        .expect("deterministic non-member key")
                        .public_key()
                        .clone()
                ),
                NodeRole::Validator
            )
            .is_err()
        );
    }

    #[test]
    fn runtime_queue_reserves_progress_and_completions() {
        let config = SumeragiV2Config {
            format_version: SUMERAGI_V2_CONFIG_FORMAT_VERSION,
            protocol_version: wire::PROTOCOL_VERSION,
            mode: wire::ConsensusMode::Permissioned,
            block_cadence_ms: 1_000,
            limits: SumeragiV2Limits {
                max_transactions: 512,
                max_payload_bytes: 16 * 1024 * 1024,
                max_queue_scan: 2_048,
                control_queue_capacity: 128,
                runtime_command_capacity: 8,
                runtime_progress_reserve: 2,
                runtime_completion_reserve: 2,
                body_queue_capacity: 16,
                chunk_queue_capacity: 64,
                effect_work_capacity: 2,
                ready_body_capacity: 8,
                ready_body_bytes: 32 * 1024 * 1024,
                certified_request_capacity: 8,
            },
            key_policy: SumeragiV2KeyPolicy {
                activation_lead_blocks: 1,
                overlap_grace_blocks: 1,
                expiry_grace_blocks: 1,
                require_hsm: false,
                allowed_algorithms: vec![Algorithm::BlsNormal],
                allowed_hsm_providers: Vec::new(),
            },
        };
        assert!(runtime_queue_config(&config).is_ok());
        assert!(effect_queue_config(&config).is_ok());

        let mut invalid = config;
        invalid.limits.effect_work_capacity = 3;
        assert!(matches!(
            effect_queue_config(&invalid),
            Err(V2RunnerError::EffectWorkExceedsCompletionReserve {
                pending: 3,
                reserve: 2,
            })
        ));
    }

    #[test]
    fn tag_roundtrip_rejects_another_height() {
        let (context, _) = context();
        let tag = EventTag::new(1, 3, Generation::new(7));
        assert_eq!(round_for_tag(&context, tag).expect("round").view, 3);
        assert!(matches!(
            round_for_tag(&context, EventTag::new(2, 0, Generation::new(7))),
            Err(V2RunnerError::StaleTag)
        ));
    }

    #[test]
    fn height_one_proposal_projects_staged_genesis_to_resultless_wire() {
        let key_pair = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519)
            .expect("deterministic genesis key");
        let transaction = TransactionBuilder::new(
            ChainId::from("height-one-resultless-projection"),
            AccountId::new(key_pair.public_key().clone()),
        )
        .with_instructions([Log::new(Level::INFO, "staged genesis execution".to_owned())])
        .sign(key_pair.private_key());
        let entrypoint = transaction.hash_as_entrypoint();
        let mut staged =
            SignedBlock::genesis(vec![transaction], key_pair.private_key(), None, None);
        staged
            .set_transaction_results(
                Vec::new(),
                &[entrypoint],
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("attach deterministic staged genesis results");
        assert!(staged.has_results());
        assert!(!staged.is_resultless_proposal());
        assert!(staged.header().result_merkle_root().is_some());

        let staged_header_hash = staged.header().hash();
        let staged_hash = staged.hash();
        let staged_signatures = staged.signatures().cloned().collect::<Vec<_>>();
        let staged_result_root = staged.header().result_merkle_root();
        let staged_execution_wire = staged.encode_wire().expect("encode staged execution image");
        let wire = canonical_height_one_proposal_wire(&staged)
            .expect("encode canonical height-one proposal");
        let proposal = decode_framed_signed_block(&wire).expect("decode height-one proposal");

        assert!(proposal.is_resultless_proposal());
        assert!(!proposal.has_results());
        assert!(proposal.header().result_merkle_root().is_none());
        assert_eq!(proposal.header().hash(), staged_header_hash);
        assert_eq!(proposal.hash(), staged_hash);
        assert_eq!(
            proposal.signatures().cloned().collect::<Vec<_>>(),
            staged_signatures
        );
        assert_eq!(
            staged.header().result_merkle_root(),
            staged_result_root,
            "proposal projection must not mutate the staged result root"
        );
        assert_eq!(
            staged
                .encode_wire()
                .expect("re-encode staged execution image"),
            staged_execution_wire,
            "proposal projection must not mutate the staged execution image"
        );
        assert_eq!(
            Hash::new(&wire),
            staged
                .canonical_proposal_wire_hash()
                .expect("hash canonical staged-genesis proposal"),
        );
    }

    #[test]
    fn replayed_proposal_sign_reserves_its_reducer_incarnation() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 3, Generation::new(9));
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: tag.view(),
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"replayed proposal block")),
            payload_hash: Hash::new(b"replayed proposal payload"),
        };
        let manifest =
            wire::PayloadManifest::derive(&context, round, subject, 5, &[b"chunk".to_vec()])
                .expect("fixture manifest");
        let proposal = wire::Proposal {
            round,
            proposer: context.leader(round.view),
            subject,
            manifest,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: Vec::new(),
        };
        let effects = [
            AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
            )),
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Proposal(proposal),
            },
        ];

        assert_eq!(replayed_proposal_sign_tag(&effects), Some(tag));
        assert_eq!(replayed_proposal_sign_tag(&effects[..1]), None);
        assert_eq!(replayed_proposal_sign_tag(&[]), None);
    }

    #[test]
    fn finalized_rollover_closes_ingress_before_successor_replay() {
        let ready = AtomicBool::new(true);
        let ingress = FairV2Ingress::new(1);
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure untrusted test lane");
        ingress.open().expect("open test ingress");
        close_ingress_for_rollover(&ready, &ingress);
        assert!(!ready.load(Ordering::Acquire));
        assert!(
            ingress
                .try_push(InboundBlockMessage::new(
                    BlockMessage::invalid_wire_sentinel(),
                    None,
                ))
                .is_err()
        );
    }

    #[test]
    fn ingress_guard_fails_closed_during_unwind() {
        let ready = Arc::new(AtomicBool::new(true));
        let ingress = Arc::new(FairV2Ingress::new(1));
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure untrusted test lane");
        ingress.open().expect("open test ingress");
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe({
            let ready = Arc::clone(&ready);
            let ingress = Arc::clone(&ingress);
            move || {
                let _guard = V2IngressClearGuard::new(Arc::clone(&ready), Arc::clone(&ingress));
                ingress.open().expect("reopen inside guarded runner");
                ready.store(true, Ordering::Release);
                panic!("model runner panic");
            }
        }));
        assert!(unwind.is_err());
        assert!(!ready.load(Ordering::Acquire));
        assert!(
            ingress
                .try_push(InboundBlockMessage::new(
                    BlockMessage::invalid_wire_sentinel(),
                    None,
                ))
                .is_err()
        );
    }

    #[test]
    fn runner_failure_guard_latches_restart_required_during_unwind() {
        let output_guard = ConsensusOutputGuard::isolated();
        let admitted_output = output_guard.acquire().expect("admit earlier output");
        let unwind = std::panic::catch_unwind({
            let output_guard = Arc::clone(&output_guard);
            move || {
                let _failure_guard = V2RunnerFailureGuard::new(output_guard);
                panic!("model runner panic before production services start");
            }
        });

        assert!(unwind.is_err(), "runner panic must continue unwinding");
        assert!(output_guard.restart_required());
        assert!(output_guard.acquire().is_none());
        drop(admitted_output);
        assert!(output_guard.acquire().is_none());
    }

    #[test]
    fn clean_runner_completion_leaves_output_guard_open() {
        let output_guard = ConsensusOutputGuard::isolated();
        let mut failure_guard = V2RunnerFailureGuard::new(Arc::clone(&output_guard));
        failure_guard.disarm();
        drop(failure_guard);

        assert!(!output_guard.restart_required());
        assert!(output_guard.acquire().is_some());
    }

    #[test]
    fn prelatched_historical_serve_invokes_no_signer_cache_or_network() {
        let output_guard = ConsensusOutputGuard::isolated();
        output_guard.activate_restart_required();
        let signer_calls = Cell::new(0_u8);
        let cache_writes = Cell::new(0_u8);
        let network_posts = Cell::new(0_u8);

        let result = serve_block_sync_while_guarded(
            output_guard.as_ref(),
            || {
                signer_calls.set(signer_calls.get().saturating_add(1));
                cache_writes.set(cache_writes.get().saturating_add(1));
                Ok(Some(()))
            },
            |(), _permit| {
                network_posts.set(network_posts.get().saturating_add(1));
                Ok(())
            },
        );

        assert!(matches!(result, Err(V2BlockSyncError::RestartRequired)));
        assert_eq!(signer_calls.get(), 0);
        assert_eq!(cache_writes.get(), 0);
        assert_eq!(network_posts.get(), 0);
    }
}
