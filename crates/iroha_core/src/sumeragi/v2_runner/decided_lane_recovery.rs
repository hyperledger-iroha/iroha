enum DecidedLaneRecoveryIngressPreparation {
    LaneLocal,
    KuraReplicaAdvert,
    CurrentServeRetain,
    HistoricalServe,
    LeaderWireRetire,
}

enum DecidedLaneRecoveryDrainAuthorization {
    LaneLocal,
    KuraReplicaAdvert,
    HistoricalServe,
    LeaderWireRetire,
}

enum DecidedLaneRecoveryDrainDecision {
    Retain,
    Authorized(DecidedLaneRecoveryDrainAuthorization),
}

fn authorize_decided_lane_recovery_drain(
    preparation: DecidedLaneRecoveryIngressPreparation,
) -> DecidedLaneRecoveryDrainDecision {
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
        DecidedLaneRecoveryIngressPreparation::CurrentServeRetain => {
            DecidedLaneRecoveryDrainDecision::Retain
        }
    }
}

fn prepare_decided_lane_recovery_ingress(
    inbound: &InboundBlockMessage,
    active_height: wire::Height,
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
    if request.round.height < active_height {
        return DecidedLaneRecoveryIngressPreparation::HistoricalServe;
    }
    if request.round.height == active_height {
        return DecidedLaneRecoveryIngressPreparation::CurrentServeRetain;
    }
    DecidedLaneRecoveryIngressPreparation::LeaderWireRetire
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
    receiver: &Arc<FairV2Ingress>,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
    output_guard: &Arc<ConsensusOutputGuard>,
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
            // I/O completion is a separate producer from the serialized
            // reducer. Service it before every ingress occurrence so a
            // completed durable store cannot remain hidden for the duration
            // of a large authenticated ingress batch.
            services.drain_completions(executor)?;
            continue;
        }
        if turn == OuterIngressTurn::Runtime {
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
        let prepared_serve = None;
        let barrier_bypass = match mode {
            V2IngressDrainMode::TimeoutVoteEpisode => {
                FairV2IngressBarrierBypass::TimeoutVoteEpisode
            }
            V2IngressDrainMode::Ordinary | V2IngressDrainMode::CertifiedFenceEscape => {
                FairV2IngressBarrierBypass::None
            }
        };
        let Some((inbound, dequeue_disposition)) = receiver
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
                let current_serve = matches!(
                    &message.payload,
                    wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request)
                        if request.round.height == executor.context().height
                );
                if current_serve {
                    // Current-height Serve is owned exclusively by the
                    // lifecycle selector/registry scheduler. Terminal lane
                    // recovery must leave its physical carrier in fair ingress.
                    return false;
                }
                true
            })
            .map_err(V2RunnerError::Service)?
        else {
            break;
        };
        let prepared = PreparedDequeuedV2IngressV1::new(
            Arc::clone(receiver),
            inbound,
            dequeue_disposition,
            prepared_serve,
            terminal_subject,
            Arc::clone(output_guard),
        );
        match consume_prepared_dequeued_v2_ingress(
            prepared,
            receiver,
            executor,
            services,
            lane_work,
            kura,
            local_key,
            block_sync_server,
            block_sync,
            block_sync_request,
            npos_vrf,
        )? {
            ProductionPreparedOrdinaryIngressConsumptionV1::Continue => {}
            ProductionPreparedOrdinaryIngressConsumptionV1::StopBatch => return Ok(()),
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecidedLaneRecoveryDrainCommitOutcome {
    LaneLocal,
    KuraReplicaAdvert,
    HistoricalServe,
    LeaderWireVolatile,
}

trait DecidedLaneRecoveryDrainCommitter {
    fn commit_lane_local(&mut self) -> Result<(), V2RunnerError>;

    fn commit_kura_replica_advert(&mut self) -> Result<(), V2RunnerError>;

    fn bind_leader_wire(&mut self) -> Result<(), V2RunnerError>;

    fn commit_historical_serve(&mut self) -> Result<(), V2RunnerError>;

    fn commit_leader_wire_volatile(&mut self) -> Result<(), V2RunnerError>;
}

fn commit_decided_lane_recovery_drain<C: DecidedLaneRecoveryDrainCommitter>(
    authorization: DecidedLaneRecoveryDrainAuthorization,
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
    let _decided_subject = executor
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
            let preparation =
                prepare_decided_lane_recovery_ingress(inbound, executor.context().height);
            match authorize_decided_lane_recovery_drain(preparation) {
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
    // only global reducer authority. Current-height Serve traffic remains in
    // fair ingress for the single lifecycle selector/coordinator path. One
    // occurrence per outer loop keeps pending Apply/completion work dominant.
    Ok(())
}
