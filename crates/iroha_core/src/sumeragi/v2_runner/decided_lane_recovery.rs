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
