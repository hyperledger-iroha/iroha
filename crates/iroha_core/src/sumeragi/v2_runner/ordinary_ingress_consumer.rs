//! Exact post-dequeue consumption for one ordinary Sumeragi v2 ingress row.
//!
//! Selection remains owned by either the outer fair-ingress loop or the
//! lifecycle turn driver. Both paths move the already-dequeued carrier into
//! [`PreparedDequeuedV2IngressV1`] and consume it through the single tail in
//! this module, so control, lane, Serve, block-sync, payload, and NPoS routing
//! cannot drift during the runner cutover.

use super::*;

/// Prepared current-height Certified-Serve state retained beside one dequeue.
#[allow(variant_size_differences)]
pub(in crate::sumeragi) enum ProductionPreparedCertifiedServeV1 {
    /// Exact Serve admission prepared before physical removal.
    Admitted(CertifiedServeAdmission),
    /// Durable deterministic rejection prepared before physical removal.
    Rejected(String),
    /// Local service failure which must close output when consumed.
    Service(String),
}

/// Closed batch-control result of consuming one already-dequeued row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "the outer ingress cursor must observe the exact tail result"]
pub(in crate::sumeragi) enum ProductionPreparedOrdinaryIngressConsumptionV1 {
    /// The row reached its exact terminal and the outer cursor may continue.
    Continue,
    /// A retained transport owner requires the current outer batch to stop.
    StopBatch,
}

/// Test-only closed settlement of one prepared current-height Serve handoff.
#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionPreparedCertifiedServeTestSettlementV1 {
    /// The selected request durably retired as a deterministic negative.
    Rejected(String),
    /// The prepared admission was explicitly aborted before worker commit.
    AdmittedAborted,
}

/// Opaque ownership of one already-dequeued ordinary ingress row.
///
/// The physical carrier, dequeue disposition, frozen terminal subject, and
/// stateful Serve preparation remain inseparable. Dropping an unconsumed value
/// closes canonical output before its retained fields are released.
#[must_use = "the already-dequeued ingress owner must enter the exact runner tail"]
pub(in crate::sumeragi) struct PreparedDequeuedV2IngressV1 {
    ingress: Arc<FairV2Ingress>,
    inbound: Option<InboundBlockMessage>,
    disposition: FairV2IngressDequeueDisposition,
    prepared_serve: Option<ProductionPreparedCertifiedServeV1>,
    terminal_subject: Option<wire::BlockSubject>,
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}

impl PreparedDequeuedV2IngressV1 {
    /// Bind one exact physical dequeue to the state used during selection.
    pub(in crate::sumeragi) fn new(
        receiver: Arc<FairV2Ingress>,
        inbound: InboundBlockMessage,
        disposition: FairV2IngressDequeueDisposition,
        prepared_serve: Option<ProductionPreparedCertifiedServeV1>,
        terminal_subject: Option<wire::BlockSubject>,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            ingress: receiver,
            inbound: Some(inbound),
            disposition,
            prepared_serve,
            terminal_subject,
            output_guard,
            armed: true,
        }
    }

    fn matches_output_guard(&self, output_guard: &Arc<ConsensusOutputGuard>) -> bool {
        Arc::ptr_eq(&self.output_guard, output_guard)
    }

    fn matches_ingress(&self, receiver: &FairV2Ingress) -> bool {
        std::ptr::eq(Arc::as_ptr(&self.ingress), receiver)
    }

    /// Close the retained output owner before an outer wrapper releases fields.
    pub(in crate::sumeragi) fn close_output_for_restart(&self) {
        self.output_guard.close_admission_for_restart();
    }

    #[cfg(test)]
    pub(in crate::sumeragi) fn physical_ordinal_for_test(&self) -> u64 {
        self.inbound
            .as_ref()
            .and_then(InboundBlockMessage::ingress_ownership)
            .and_then(FairV2IngressOwnershipEvidence::physical_admission_ordinal)
            .expect("prepared ordinary turn retains its queue-minted ordinal")
    }

    #[cfg(test)]
    pub(in crate::sumeragi) fn has_prepared_serve_for_test(&self) -> bool {
        self.prepared_serve.is_some()
    }

    fn complete(&mut self) {
        self.armed = false;
    }
}

impl Drop for PreparedDequeuedV2IngressV1 {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}

/// Non-permit fail-stop scope for the shared post-dequeue tail.
///
/// The tail invokes service code which may itself close output synchronously,
/// so this scope deliberately retains no admission read permit.
struct PreparedDequeuedV2IngressFailStopScopeV1 {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}

impl PreparedDequeuedV2IngressFailStopScopeV1 {
    fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }

    fn complete(mut self) {
        self.armed = false;
    }
}

impl Drop for PreparedDequeuedV2IngressFailStopScopeV1 {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}

/// Settle a prepared Serve without runner output in focused lifecycle tests.
#[cfg(test)]
pub(in crate::sumeragi) fn settle_prepared_certified_serve_for_test(
    mut prepared: PreparedDequeuedV2IngressV1,
    services: &mut ProductionV2Services,
) -> Result<ProductionPreparedCertifiedServeTestSettlementV1, String> {
    let services_output_guard = services.lifecycle_output_guard();
    if !prepared.matches_output_guard(&services_output_guard) {
        services_output_guard.close_admission_for_restart();
        return Err("prepared Serve token belongs to another lifecycle output guard".to_owned());
    }
    let operation = services_output_guard
        .begin_fail_stop_operation()
        .ok_or_else(|| "prepared Serve token belongs to closed lifecycle output".to_owned())?;
    if prepared.inbound.is_none() {
        return Err("prepared Serve token lost its exact inbound carrier".to_owned());
    }
    let serve = prepared
        .prepared_serve
        .take()
        .ok_or_else(|| "ordinary token retained no prepared Serve result".to_owned())?;
    let settlement = match serve {
        ProductionPreparedCertifiedServeV1::Admitted(admission) => {
            services.abort_certified_serve_for_test(admission)?;
            ProductionPreparedCertifiedServeTestSettlementV1::AdmittedAborted
        }
        ProductionPreparedCertifiedServeV1::Rejected(reason) => {
            ProductionPreparedCertifiedServeTestSettlementV1::Rejected(reason)
        }
        ProductionPreparedCertifiedServeV1::Service(reason) => {
            return Err(format!(
                "prepared Serve entered a fatal service outcome: {reason}"
            ));
        }
    };
    drop(prepared.inbound.take());
    prepared.complete();
    operation.complete();
    Ok(settlement)
}

/// Consume one exact already-dequeued row through the established runner tail.
///
/// This is the sole post-selection implementation used by both legacy drain
/// and the activated lifecycle handoff. Every failure leaves both the local
/// non-permit scope and the move-only handoff armed for restart.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(in crate::sumeragi) fn consume_prepared_dequeued_v2_ingress(
    mut prepared: PreparedDequeuedV2IngressV1,
    receiver: &FairV2Ingress,
    executor: &mut V2EffectExecutor<SerializedV2Runtime>,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
    kura: &Kura,
    local_key: &KeyPair,
    block_sync_server: &mut V2BlockSyncServer,
    block_sync: &mut V2BlockSyncDiscovery,
    block_sync_request: &mut Option<HashOf<wire::CommitCertificateRequest>>,
    npos_vrf: &mut V2NposVrfLifecycle,
) -> Result<ProductionPreparedOrdinaryIngressConsumptionV1, V2RunnerError> {
    let services_output_guard = services.lifecycle_output_guard();
    if !prepared.matches_output_guard(&services_output_guard) {
        services_output_guard.close_admission_for_restart();
        return Err(V2RunnerError::Service(
            "ordinary ingress handoff changed its lifecycle output owner".to_owned(),
        ));
    }
    if !prepared.matches_ingress(receiver) {
        services_output_guard.close_admission_for_restart();
        return Err(V2RunnerError::Service(
            "ordinary ingress handoff changed its exact fair-ingress owner".to_owned(),
        ));
    }
    let initial_admission = services_output_guard
        .acquire()
        .ok_or(V2RunnerError::RestartRequired)?;
    drop(initial_admission);

    let mut inbound = prepared.inbound.take().ok_or_else(|| {
        V2RunnerError::Service("ordinary ingress handoff lost its exact carrier".to_owned())
    })?;
    let dequeue_disposition = prepared.disposition;
    let mut prepared_serve = prepared.prepared_serve.take();
    let terminal_subject = prepared.terminal_subject;
    let terminal_decision = terminal_subject.is_some();
    let fail_stop =
        PreparedDequeuedV2IngressFailStopScopeV1::new(Arc::clone(&services_output_guard));

    macro_rules! finish {
        ($outcome:expr) => {{
            let final_admission = services_output_guard
                .acquire()
                .ok_or(V2RunnerError::RestartRequired)?;
            fail_stop.complete();
            prepared.complete();
            drop(final_admission);
            return Ok($outcome);
        }};
    }

    if matches!(inbound.message(), BlockMessage::KuraReplicaAdvert(_)) {
        admit_kura_replica_advert_ingress(receiver, kura, inbound)?;
        finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
    }
    if inbound.message().is_lane_local() {
        let _ = lane_work
            .accept_lane_message_with_ingress_ownership(inbound, executor.current_tag().view());
        let _ = lane_work.service_next_historical_recovery()?;
        finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
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
            message_kind = ?super::super::FairV2IngressMessageKind::classify(inbound.message()),
            semantic_origin = ?inbound.sender(),
            authenticated_via = ?inbound.via(),
            obsolete_view = token.view(),
            active_view = executor.current_tag().view(),
            "retired WAL-obsolete Sumeragi v2 leader-wire carrier"
        );
        receiver
            .mark_obsolete_leader_wire_volatile_terminal(receipt)
            .map_err(V2RunnerError::Service)?;
        finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
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
        finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
    };
    if let Err(error) = message.validate_version() {
        iroha_logger::debug!(%error, "rejected wrong-version Sumeragi v2 envelope");
        mark_leader_wire_volatile(receiver, &ingress_ownership)?;
        finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
    }
    match message.payload {
        wire::ConsensusMessageV2Payload::VrfCommit(commit) => {
            drop(ingress_ownership);
            let outcome = npos_vrf.accept_commit(commit, sender.as_ref());
            if matches!(
                outcome,
                super::super::v2_npos::V2VrfIngressOutcome::Rejected(_)
            ) {
                iroha_logger::debug!(?outcome, "rejected NPoS VRF commitment");
            }
        }
        wire::ConsensusMessageV2Payload::VrfReveal(reveal) => {
            drop(ingress_ownership);
            let outcome = npos_vrf.accept_reveal(reveal, sender.as_ref());
            if matches!(
                outcome,
                super::super::v2_npos::V2VrfIngressOutcome::Rejected(_)
            ) {
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
                    wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutVote(
                        vote,
                    )),
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
                finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
            };
            if terminal_decision
                && services
                    .fetch_work_for_manifest(chunk.manifest_hash)
                    .is_none()
            {
                // Proposal reordering justifies buffering an orphan chunk only
                // while another Proposal can still open its fetch. After
                // Decision, unmatched chunks cannot crowd the decided body.
                mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
            }
            services
                .route_payload_chunk(executor, sender, chunk, ingress_ownership)
                .map_err(V2RunnerError::Service)?;
        }
        wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) => {
            let Some(sender) = sender else {
                mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
            };
            let Some(reply_routes) = reply_routes else {
                iroha_logger::debug!(
                    %sender,
                    "rejected certified body request without authenticated reply route"
                );
                mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
            };
            if reply_routes.semantic_target() != &sender {
                iroha_logger::debug!(
                    %sender,
                    "rejected certified body request with mismatched reply target"
                );
                mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
            }
            if request.round.height < executor.context().height {
                let response_peer = sender.clone();
                let terminal_ownership = ingress_ownership.clone();
                let served = serve_block_sync_while_guarded(
                    services_output_guard.as_ref(),
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
                    match prepared_serve.take() {
                        Some(ProductionPreparedCertifiedServeV1::Rejected(reason)) => {
                            iroha_logger::debug!(
                                %reason,
                                "retired certified body request superseded by Decision"
                            );
                            mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                            finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
                        }
                        Some(ProductionPreparedCertifiedServeV1::Service(reason)) => {
                            return Err(V2RunnerError::Service(reason));
                        }
                        Some(ProductionPreparedCertifiedServeV1::Admitted(_)) | None => {
                            return Err(V2RunnerError::Service(
                                "Decision-superseded certified-body ingress crossed physical drain without its durable negative outcome"
                                    .to_owned(),
                            ));
                        }
                    }
                }
                match prepared_serve.take() {
                    Some(ProductionPreparedCertifiedServeV1::Admitted(admission)) => {
                        services
                            .serve_certified_request_on_routes(
                                admission,
                                reply_routes,
                                ingress_ownership,
                            )
                            .map_err(V2RunnerError::Service)?;
                    }
                    Some(ProductionPreparedCertifiedServeV1::Rejected(reason)) => {
                        iroha_logger::debug!(%reason, "rejected certified body request");
                        mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    }
                    Some(ProductionPreparedCertifiedServeV1::Service(reason)) => {
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
                finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
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
                    finish!(ProductionPreparedOrdinaryIngressConsumptionV1::StopBatch);
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
                finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
            };
            let Some(reply_routes) = reply_routes else {
                iroha_logger::debug!(
                    %sender,
                    "rejected CommitQC request without authenticated reply route"
                );
                mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
            };
            if reply_routes.semantic_target() != &sender {
                iroha_logger::debug!(
                    %sender,
                    "rejected CommitQC request with mismatched reply target"
                );
                mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
            }
            let response_peer = sender.clone();
            let terminal_ownership = ingress_ownership.clone();
            let served = serve_block_sync_while_guarded(
                services_output_guard.as_ref(),
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
                mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
            }
            let Some(sender) = sender else {
                mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
            };
            let discovered = match block_sync.authenticate_response(response, &sender) {
                Ok(discovered) => discovered,
                Err(error) => {
                    iroha_logger::debug!(%error, "rejected CommitQC discovery response");
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
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
    finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
}
