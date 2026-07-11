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

use iroha_config::parameters::actual::{
    ConsensusMode as ConfigConsensusMode, NodeRole, SumeragiNpos, SumeragiV2Config,
};
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    Encode as _,
    account::AccountId,
    block::{SignedBlock, consensus_v2 as wire},
    events::{EventBox, pipeline::PipelineEventBox},
    peer::PeerId,
};
use iroha_sumeragi_core::{EventTag, Generation};
use thiserror::Error;

use super::{
    GenesisWithPubKey, InboundBlockMessage, SumeragiWorker,
    message::BlockMessage,
    v2::{AdapterFingerprints, LocalProposalDirective, SumeragiV2Adapter},
    v2_block_sync::{
        CommitCertificateAdmissionError, V2BlockSyncDiscovery, V2BlockSyncError, V2BlockSyncServer,
    },
    v2_body_store::BlockSignaturePolicy,
    v2_candidate::{
        CandidateAttachments, CandidateDescriptor, CandidateLimits, CandidateRequest,
        CandidateWorkProvider, CandidateWorkUnavailable, PreparedCandidateWork,
        V2CandidateAssembler,
    },
    v2_chunks::{EncodedV2Payload, encode_payload},
    v2_effects::{
        EffectExecutorStep, EffectQueueConfig, EffectTransportError, PostFinalityCleanupOutcome,
        PostFinalityCleanupTarget, V2EffectExecutor,
    },
    v2_lane_work::{
        MergeSidecarDeferralDisposition, V2LaneIngressOutcome, V2LaneWorkAdapter, V2LaneWorkEffect,
        V2LaneWorkLimits,
    },
    v2_recovery::{build_verified_successor, recover_active_height},
    v2_runtime::{NetworkIngressError, RuntimeQueueConfig, SerializedV2Runtime},
    v2_worker::ProductionV2Services,
};
use crate::{block::BlockBuilder, kura::Kura, queue::Queue, state::State};

const IDLE_POLL: Duration = Duration::from_millis(10);
const CANDIDATE_WORK_RECHECK: Duration = Duration::from_millis(100);

/// Run the v2-only worker until shutdown or a fail-closed error.
pub(super) fn run(worker: SumeragiWorker) {
    let _status_clear = V2StatusClearGuard::new();
    let ingress_ready = Arc::clone(&worker.ingress_ready);
    let _ingress_clear = V2IngressClearGuard::new(Arc::clone(&ingress_ready));
    if let Err(error) = run_inner(worker) {
        iroha_logger::error!(%error, "authoritative Sumeragi v2 runner stopped fail-closed");
    }
    ingress_ready.store(false, Ordering::Release);
}

struct V2IngressClearGuard(Arc<AtomicBool>);

impl V2IngressClearGuard {
    fn new(ingress_ready: Arc<AtomicBool>) -> Self {
        ingress_ready.store(false, Ordering::Release);
        Self(ingress_ready)
    }
}

impl Drop for V2IngressClearGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
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

fn close_ingress_for_rollover(ingress_ready: &AtomicBool) {
    ingress_ready.store(false, Ordering::Release);
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
        rbc_chunk_rx,
        consensus_rx,
        lane_relay_rx,
        background_rx,
        wake_rx,
        shutdown_signal,
        // The following fields belong to retained block-sync/lane adapters or
        // the legacy actor and are deliberately not consulted by global v2.
        consensus_frame_cap: _,
        consensus_payload_frame_cap: _,
        peers_gossiper: _,
        block_count: _,
        block_sync_gossip_limit: _,
        #[cfg(feature = "telemetry")]
            telemetry: _,
        epoch_roster_provider: _,
        rbc_store: _,
        background_post_tx: _,
        da_spool_dir: _,
        vote_dedup: _,
        block_payload_dedup: _,
        frontier_block_sync_hint: _,
        ingress_ready,
        wake_tx: _,
        rbc_status_handle: _,
    } = worker;

    let GenesisWithPubKey {
        genesis,
        public_key: genesis_public_key,
        v2_bootstrap,
    } = genesis_network;
    let genesis_body = genesis.map(|block| block.0);
    let recovered = recover_active_height(
        kura.as_ref(),
        state.as_ref(),
        v2_bootstrap,
        genesis_public_key.clone(),
    )?;
    let mut pending_kura_apply = recovered.pending_kura_apply();
    let (mut verified_context, context_store, mut signature_policy) = recovered.into_parts();
    let local_peer = common_config.peer.id().clone();
    let genesis_account = AccountId::new(genesis_public_key);
    let mut first_height_genesis = genesis_body;
    let mut block_sync_server = None;

    loop {
        if shutdown_signal.is_sent() {
            return Ok(());
        }
        let context = verified_context.context().clone();
        let block_cadence = state.sumeragi_effective_block_time();
        let shared_config = config.v2_config(block_cadence, context.mode)?;
        let fingerprints = adapter_fingerprints(&local_peer, &shared_config);
        let control_queue_capacity = usize::try_from(shared_config.limits.control_queue_capacity)?;
        let body_queue_capacity = usize::try_from(shared_config.limits.body_queue_capacity)?;
        let chunk_queue_capacity = usize::try_from(shared_config.limits.chunk_queue_capacity)?;
        let certified_request_capacity =
            usize::try_from(shared_config.limits.certified_request_capacity)?;
        let round_timeout = Duration::from_millis(shared_config.round_timeout_ms);
        let retransmit_interval = Duration::from_millis(shared_config.retransmit_interval_ms);
        if block_sync_server.is_none() {
            block_sync_server = Some(V2BlockSyncServer::new(
                context.chain_id.clone(),
                certified_request_capacity,
            )?);
        }
        let local_validator = local_validator_index(&context, &local_peer, config.role)?;
        let consensus_key_hash: [u8; 32] =
            Hash::new(common_config.key_pair.public_key().encode()).into();
        let storage_root = kura.sumeragi_v2_storage_root();
        let wal_path = storage_root
            .join("wal")
            .join(format!("{:020}.wal", context.height));
        let (adapter, startup_effects) = SumeragiV2Adapter::open(
            wal_path,
            verified_context,
            local_validator,
            Generation::new(context.height),
            consensus_key_hash,
            fingerprints,
        )?;
        let runtime_queue = runtime_queue_config(&shared_config)?;
        let (runtime, startup_effects) = SerializedV2Runtime::new(
            adapter,
            startup_effects,
            Instant::now(),
            round_timeout,
            runtime_queue,
        )?;
        let effect_queue = effect_queue_config(&shared_config)?;
        let (mut executor, body_store) = V2EffectExecutor::open(
            runtime,
            storage_root.join("bodies"),
            context.clone(),
            local_peer.clone(),
            local_validator,
            signature_policy,
            effect_queue,
        )?;
        let recovered_applied_height = pending_kura_apply.filter(|pending| {
            usize::try_from(pending.height()).is_ok_and(|height| state.committed_height() == height)
        });
        if let Some(pending) = pending_kura_apply.take() {
            executor.verify_pending_kura_apply_replay(pending)?;
        }
        let mut services = ProductionV2Services::start(
            context.clone(),
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
            control_queue_capacity,
            chunk_queue_capacity,
        )
        .map_err(V2RunnerError::Service)?;
        let mut lane_work = V2LaneWorkAdapter::new(
            context.clone(),
            local_peer.clone(),
            common_config.key_pair.clone(),
            local_validator.is_some(),
            Arc::clone(&state),
            Arc::clone(&kura),
            lane_work_limits(&shared_config)?,
            recovered_applied_height,
        )?;
        executor.consume_effects(startup_effects, &mut services)?;
        let startup_directive = executor.local_proposal_directive()?;
        lane_work.retain_merge_sidecars_for_global_view(
            startup_directive.tag().view(),
            startup_directive.locked_subject(),
            startup_directive.decided_subject(),
        )?;
        dispatch_lane_work_effects(&mut lane_work, &services, control_queue_capacity)?;
        ingress_ready.store(true, Ordering::Release);

        let candidate_limits = candidate_limits(&context, &shared_config)?;
        let height_started_at = Instant::now();
        let mut block_sync = V2BlockSyncDiscovery::new(
            context.clone(),
            local_peer.clone(),
            certified_request_capacity,
        )?;
        let mut block_sync_request = None;
        let mut next_block_sync_attempt = height_started_at
            .checked_add(round_timeout)
            .ok_or(V2RunnerError::InvalidLimits)?;
        let mut next_lane_retransmit = height_started_at
            .checked_add(retransmit_interval)
            .ok_or(V2RunnerError::InvalidLimits)?;
        let mut attempted_tag = None;
        let mut local_subject = None;
        let mut heartbeat_only_tag = None;
        let mut candidate_work_wait: Option<(EventTag, Instant, Instant)> = None;
        let mut pending_local_events: Option<(
            EventTag,
            wire::BlockSubject,
            Vec<PipelineEventBox>,
        )> = None;

        let finality = loop {
            if shutdown_signal.is_sent() {
                return Ok(());
            }

            services.drain_completions(&mut executor)?;
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
                &services,
            )?;
            drain_v2_ingress(
                &block_rx,
                &mut executor,
                &mut services,
                &mut lane_work,
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
            drain_retired_v1_queues(
                &rbc_chunk_rx,
                &consensus_rx,
                &background_rx,
                control_queue_capacity,
            );

            let now = Instant::now();
            if now >= next_lane_retransmit {
                lane_work.schedule_retransmission();
                next_lane_retransmit = now
                    .checked_add(retransmit_interval)
                    .ok_or(V2RunnerError::InvalidLimits)?;
            }
            dispatch_lane_work_effects(&mut lane_work, &services, control_queue_capacity)?;

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

            if executor.ready_to_finish() {
                close_ingress_for_rollover(&ingress_ready);
                lane_work.persist_anchored_sessions()?;
                lane_work.prune_finalized_merge_sidecars()?;
                dispatch_lane_work_effects(&mut lane_work, &services, control_queue_capacity)?;
                let (runtime, receipt, artifact) = executor.into_finalized_parts()?;
                let finalized = runtime.into_driver().finish_height(&receipt, &artifact)?;
                let mut cleanup = PostFinalityCleanupOutcome::default();
                if let Some(warning) = finalized.wal_retirement_warning() {
                    cleanup.record(PostFinalityCleanupTarget::SafetyWal, warning);
                }
                cleanup.append(services.finish_height(receipt.clone()));
                for warning in cleanup.warnings() {
                    iroha_logger::warn!(
                        cleanup_target = warning.target().as_str(),
                        reason = warning.reason(),
                        "Sumeragi v2 finalized with retained local cleanup state"
                    );
                }
                break (receipt, artifact);
            }

            schedule_local_proposal(
                candidate_limits,
                &context,
                local_validator,
                &common_config.key_pair,
                state.as_ref(),
                queue.as_ref(),
                kura.as_ref(),
                &config.npos,
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
        verified_context =
            build_verified_successor(state.as_ref(), &context_store, &artifact, &receipt)?;
        signature_policy = BlockSignaturePolicy::RotatingLeader;
        first_height_genesis = None;
    }
}

#[allow(clippy::too_many_arguments)]
fn schedule_local_proposal(
    candidate_limits: CandidateLimits,
    context: &wire::HeightContext,
    local_validator: Option<wire::ValidatorIndex>,
    key_pair: &KeyPair,
    state: &State,
    queue: &Queue,
    kura: &Kura,
    npos_config: &SumeragiNpos,
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
            continue;
        }
        let canonical_wire = loaded.into_canonical_wire();
        let block = iroha_data_model::block::decode_framed_signed_block(&canonical_wire)
            .map_err(|error| V2RunnerError::Candidate(error.to_string()))?;
        if lane_work.bind_locked_global_body(&block) == V2LaneIngressOutcome::Rejected {
            return Err(V2RunnerError::LaneCandidateBinding);
        }
        if current.leader() != local_validator {
            continue;
        }
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
        submit_exact_body(
            context,
            directive,
            body.encode_wire()
                .map_err(|error| V2RunnerError::Candidate(error.to_string()))?,
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
        let parent_height = usize::try_from(context.height.saturating_sub(1))?;
        let parent_height = NonZeroUsize::new(parent_height).ok_or(V2RunnerError::MissingParent)?;
        let parent = kura
            .get_block(parent_height)
            .ok_or(V2RunnerError::MissingParent)?;
        let logical_time = parent
            .header()
            .creation_time()
            .checked_add(block_cadence)
            .ok_or(V2RunnerError::V2BlockTimeOverflow)?;
        u64::try_from(logical_time.as_millis()).map_err(|_| V2RunnerError::V2BlockTimeOverflow)?;
        let (_, time_source) = iroha_primitives::time::TimeSource::new_mock(logical_time);
        let assembler = V2CandidateAssembler::new(candidate_limits, time_source.clone());
        let attachments = candidate_attachments(
            context,
            state,
            npos_config,
            parent.as_ref(),
            directive.tag().view(),
            time_source,
        )?;
        let candidate = if heartbeat_only_tag == Some(directive.tag()) {
            assembler.assemble(CandidateRequest {
                context,
                directive,
                local_validator,
                parent: parent.as_ref(),
                state,
                queue,
                key_pair,
                attachments,
                work_provider: HeartbeatOnlyWorkProvider,
            })?
        } else {
            assembler.assemble(CandidateRequest {
                context,
                directive,
                local_validator,
                parent: parent.as_ref(),
                state,
                queue,
                key_pair,
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
                *candidate_work_wait = Some((
                    tag,
                    started_at,
                    now.checked_add(CANDIDATE_WORK_RECHECK)
                        .ok_or(V2RunnerError::InvalidLimits)?,
                ));
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
    services: &ProductionV2Services,
) -> Result<(), V2RunnerError> {
    if now < *next_attempt {
        return Ok(());
    }

    let message = if let Some(hash) = request_hash.as_ref() {
        discovery
            .retransmit(*hash)
            .ok_or(V2RunnerError::BlockSyncRequestDisappeared)?
    } else {
        let message = discovery.begin(key_pair)?;
        let wire::ConsensusMessageV2Payload::CommitCertificateRequest(request) = &message.payload
        else {
            return Err(V2RunnerError::BlockSyncRequestDisappeared);
        };
        *request_hash = Some(HashOf::new(request));
        message
    };
    services.broadcast_to_voters(message);
    *next_attempt = now
        .checked_add(retransmit_interval)
        .ok_or(V2RunnerError::InvalidLimits)?;
    Ok(())
}

fn drain_v2_ingress(
    receiver: &std::sync::mpsc::Receiver<InboundBlockMessage>,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
    kura: &Kura,
    context_store: &super::v2_context_store::V2ContextStore,
    local_key: &KeyPair,
    block_sync_server: &mut V2BlockSyncServer,
    block_sync: &mut V2BlockSyncDiscovery,
    block_sync_request: &mut Option<HashOf<wire::CommitCertificateRequest>>,
    limit: usize,
) -> Result<(), V2RunnerError> {
    for _ in 0..limit.max(1) {
        let Ok(inbound) = receiver.try_recv() else {
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
                    match block_sync_server.serve_historical_body(
                        kura,
                        context_store,
                        request,
                        &sender,
                        local_key,
                    ) {
                        Ok(Some(response)) => services.post_to_peer(sender, response),
                        Ok(None) => {}
                        Err(error) if is_remote_block_sync_rejection(&error) => {
                            iroha_logger::debug!(%error, "rejected historical certified body request");
                        }
                        Err(error) => return Err(error.into()),
                    }
                } else if request.round.height == executor.context().height {
                    match executor.authenticate_certified_body_request(request, &sender) {
                        Ok(request) => {
                            if let Err(error) = services.serve_certified_request(request) {
                                iroha_logger::debug!(%error, "deferred certified body request");
                            }
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
                match block_sync_server.serve(kura, request, &sender, local_key) {
                    Ok(Some(response)) => services.post_to_peer(sender, response),
                    Ok(None) => {}
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

fn is_remote_block_sync_rejection(error: &V2BlockSyncError) -> bool {
    matches!(
        error,
        V2BlockSyncError::Wire(_)
            | V2BlockSyncError::Transport(_)
            | V2BlockSyncError::ConflictingServerRequest { .. }
            | V2BlockSyncError::ConflictingHistoricalBodyRequest { .. }
    )
}

fn enqueue_control(
    executor: &mut V2EffectExecutor,
    message: wire::ConsensusMessageV2,
) -> Result<(), V2RunnerError> {
    match executor.enqueue_network(message) {
        Ok(_) => Ok(()),
        Err(NetworkIngressError::FailClosed) => Err(V2RunnerError::RuntimeFailClosed),
        Err(error) => {
            iroha_logger::debug!(%error, "rejected Sumeragi v2 control ingress");
            Ok(())
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
    Ok(EffectQueueConfig::new(
        usize::try_from(config.limits.effect_work_capacity)?,
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
    npos_config: &SumeragiNpos,
    parent: &SignedBlock,
    view: wire::View,
    time_source: iroha_primitives::time::TimeSource,
) -> Result<CandidateAttachments, V2RunnerError> {
    let round_header = BlockBuilder::new_with_time_source(Vec::new(), time_source)
        .chain(view, Some(parent))
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

    let mode = match context.mode {
        wire::ConsensusMode::Permissioned => ConfigConsensusMode::Permissioned,
        wire::ConsensusMode::Npos => ConfigConsensusMode::Npos,
    };
    let applier = super::penalties::PenaltyApplier::from_parts(
        state,
        npos_config,
        mode,
        #[cfg(feature = "telemetry")]
        Some(state.metrics()),
        #[cfg(not(feature = "telemetry"))]
        None,
    );
    let effects = applier
        .derive_npos_consensus_effects(context.height, std::iter::empty())
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))?;
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

fn drain_retired_v1_queues(
    rbc_chunk_rx: &std::sync::mpsc::Receiver<InboundBlockMessage>,
    consensus_rx: &std::sync::mpsc::Receiver<super::ControlFlow>,
    background_rx: &std::sync::mpsc::Receiver<super::BackgroundRequest>,
    limit: usize,
) {
    for _ in 0..limit.max(1) {
        let mut drained = false;
        drained |= rbc_chunk_rx.try_recv().is_ok();
        drained |= consensus_rx.try_recv().is_ok();
        drained |= background_rx.try_recv().is_ok();
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
    /// Durable parent body is unavailable in Kura.
    #[error("Sumeragi v2 successor is missing its canonical parent block")]
    MissingParent,
    /// Locked subject differs from loaded durable bytes.
    #[error("loaded Sumeragi v2 locked body differs from the reducer lock")]
    LockedBodyMismatch,
    /// A locally assembled body could not bind its lane-local work to the exact round.
    #[error("local Sumeragi v2 candidate could not bind its lane-local ownership artifacts")]
    LaneCandidateBinding,
    /// Candidate tag belongs to another height.
    #[error("stale Sumeragi v2 proposal tag")]
    StaleTag,
    /// Runtime has already failed closed.
    #[error("Sumeragi v2 runtime is fail-closed")]
    RuntimeFailClosed,
    /// A configured limit is zero.
    #[error("Sumeragi v2 configured limits must be positive")]
    InvalidLimits,
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
    use iroha_config::parameters::actual::{NodeRole, SumeragiV2KeyPolicy, SumeragiV2Limits};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{ChainId, peer::PeerId};

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
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
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
            format_version: 1,
            protocol_version: wire::PROTOCOL_VERSION,
            mode: wire::ConsensusMode::Permissioned,
            block_cadence_ms: 1_000,
            round_timeout_ms: 10_000,
            retransmit_interval_ms: 2_000,
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
                effect_work_capacity: 32,
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
            npos: None,
        };
        assert!(runtime_queue_config(&config).is_ok());
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
    fn finalized_rollover_closes_ingress_before_successor_replay() {
        let ready = AtomicBool::new(true);
        close_ingress_for_rollover(&ready);
        assert!(!ready.load(Ordering::Acquire));
    }

    #[test]
    fn ingress_guard_fails_closed_during_unwind() {
        let ready = Arc::new(AtomicBool::new(true));
        let unwind = std::panic::catch_unwind({
            let ready = Arc::clone(&ready);
            move || {
                let _guard = V2IngressClearGuard::new(Arc::clone(&ready));
                ready.store(true, Ordering::Release);
                panic!("model runner panic");
            }
        });
        assert!(unwind.is_err());
        assert!(!ready.load(Ordering::Acquire));
    }
}
