//! One Native lane process retained across global-height activation and rollover.

use super::native_source::{NativeSourceRequest, NativeSourceTarget};
use super::ordinary_ingress_consumer::PreparedDequeuedV2IngressV1;
use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::sumeragi::{
    v2::VerifiedHeightContext,
    v2_lane_driver::{NativeLaneDecisionHandoff, NativeLaneDriver, NativeLaneDriverLimits},
    v2_lane_instance::{LaneOutbound, LanePhysicalShutdown, LaneProcessLimits},
    v2_lane_transport::{
        NativeDecisionTransportAdmission, NativeLaneTransport, NativeTransportAdmission,
        NativeTransportProgress,
    },
};
use iroha_data_model::block::lane_consensus::LaneDecisionV1;

/// Physical Native custody has process lifetime, independently of global consensus.
/// A pending outbox packet is moved exactly once and retained across actor pressure.
/// No old lane adapter, merge signer or ordinary economic producer is constructed here.
pub(in crate::sumeragi) struct NativeRunnerProcess {
    pub(super) state: Arc<State>,
    pub(super) guard: Arc<ConsensusOutputGuard>,
    local_peer: PeerId,
    pub(super) key: KeyPair,
    retransmit: Duration,
    source: Option<NativeSourceRequest>,
    pub(super) candidate_job: Option<super::native_candidate::NativeCandidateJob>,
    pub(super) candidate_result: Option<super::native_candidate::NativeCandidateResult>,
    pub(super) candidate_source:
        Option<super::super::v2_lane_driver::NativeLaneCandidatePreparation>,
    pub(super) recovered_sources:
        BTreeMap<Hash, Arc<crate::state::VerifiedFirstLaneAdmittedInputV1>>,
    driver: NativeLaneDriver,
    transport: NativeLaneTransport,
    outbound: Option<LaneOutbound>,
    decision: Option<LaneDecisionV1>,
    relayed: BTreeSet<wire::HeightContextId>,
    relay_context: Option<wire::HeightContextId>,
    pending_ingress: Option<PreparedDequeuedV2IngressV1>,
    publication: Option<NativePublication>,
}

struct NativePublication {
    published: super::super::v2_apply::PublishedNativeCarrier,
    settled: bool,
}

impl NativeRunnerProcess {
    pub(in crate::sumeragi) fn new(
        state: Arc<State>,
        guard: Arc<ConsensusOutputGuard>,
        local_peer: iroha_model_base::peer::PeerId,
        key: KeyPair,
        voting_enabled: bool,
        config: &SumeragiV2Config,
        maximum_frame_bytes: usize,
        round_timeout: Duration,
        retransmit: Duration,
    ) -> Result<Self, V2RunnerError> {
        let nonzero = |value| NonZeroUsize::new(value).ok_or(V2RunnerError::InvalidLimits);
        let ingress = nonzero(usize::try_from(config.limits.control_queue_capacity)?)?;
        let outbound = nonzero(usize::try_from(config.limits.effect_work_capacity)?)?;
        let instances = nonzero(usize::try_from(config.limits.max_transactions)?)?;

        let limits = NativeLaneDriverLimits {
            voting_enabled,
            process: LaneProcessLimits {
                instances,
                // One independent worker per physical class prevents slow body I/O
                // from monopolizing opening and WAL progress. Queue counts are not thread counts.
                workers_per_class: NonZeroUsize::MIN,
                queued_per_class: nonzero(usize::try_from(
                    config.limits.runtime_command_capacity,
                )?)?,
                completed: outbound,
                effect_limit: outbound
                    .get()
                    .max(3 * super::super::v2_core::MAX_EFFECTS_PER_STEP),
                base_timeout: round_timeout,
                retransmit,
            },
            ingress,
            outbound,
            maximum_message_bytes: nonzero(maximum_frame_bytes)?,
        };
        Ok(Self {
            driver: NativeLaneDriver::new(
                Arc::clone(&state),
                Arc::clone(&guard),
                key.clone(),
                limits,
            )
            .map_err(V2RunnerError::Service)?,
            transport: NativeLaneTransport::new(
                Arc::clone(&state),
                Arc::clone(&guard),
                local_peer.clone(),
                outbound,
            ),
            guard,
            local_peer,
            key,
            retransmit,
            source: None,
            candidate_job: None,
            candidate_result: None,
            candidate_source: None,
            recovered_sources: BTreeMap::new(),
            state,
            outbound: None,
            decision: None,
            relayed: BTreeSet::new(),
            relay_context: None,
            pending_ingress: None,
            publication: None,
        })
    }

    /// Service independent Native clocks, physical completions and one exact send.
    /// The authenticated global context is only a Decision relay destination.
    pub(in crate::sumeragi) fn poll(
        &mut self,
        global: &VerifiedHeightContext,
        network: &crate::IrohaNetwork,
        now: Instant,
        receiver: &Arc<FairV2Ingress>,
    ) -> Result<(), V2RunnerError> {
        self.settle_pending_publication()?;
        self.poll_candidate()?;
        // Already-issued output gets one bounded actor attempt before another
        // physical Native dequeue. Pressure retains custody and still permits
        // the independent ingress, clock and worker phases below.
        let observed = self
            .state
            .verified_lane_consensus_contexts()
            .map_err(V2RunnerError::Service)?;
        if let Some(observed) = observed.as_ref() {
            self.service_ready_output(observed, global, network)?;
        }
        self.service_native_ingress(receiver)?;
        if let Some(source) = self.source.as_mut() {
            source.poll(network, &self.guard, now, self.retransmit)?;
        }
        if let Some(prepared) = self.pending_ingress.take() {
            self.consume_native_ingress(prepared, receiver)?;
        }
        let Some(observed) = observed else {
            return Ok(());
        };
        self.recovered_sources.retain(|binding, _| {
            observed
                .contexts()
                .iter()
                .any(|lane| lane.frozen().admitted_binding_hash == *binding)
        });
        self.driver
            .poll(&observed, now)
            .map_err(V2RunnerError::Service)?;
        if self.relay_context != Some(global.context().id()) {
            self.relayed.clear();
            self.relay_context = Some(global.context().id());
        }
        self.relayed.retain(|id| {
            observed
                .contexts()
                .iter()
                .any(|lane| lane.instance_id() == *id)
        });
        if self.decision.is_none() {
            self.decision = self
                .driver
                .next_unrelayed_decision(&observed, &self.relayed)
                .map_err(V2RunnerError::Service)?;
        }
        if let Some(decision) = self.decision.take() {
            let id =
                wire::HeightContextId(HashOf::from_untyped_unchecked(decision.value().instance_id));
            match self.transport.retain_decision(&observed, global, decision) {
                NativeDecisionTransportAdmission::Retained => {
                    self.relayed.insert(id);
                }
                NativeDecisionTransportAdmission::Retry(decision) => self.decision = Some(decision),
                NativeDecisionTransportAdmission::Rejected { decision, reason } => {
                    if observed
                        .contexts()
                        .iter()
                        .any(|lane| lane.instance_id() == id)
                    {
                        self.decision = Some(decision);
                        return Err(V2RunnerError::Service(reason));
                    }
                }
            }
        }
        for id in self.driver.process().instance_ids().collect::<Vec<_>>() {
            if let Some(effect) = self.driver.take_diagnostic(id) {
                iroha_logger::debug!(?id, ?effect, "Native lane reducer diagnostic");
            }
        }
        Ok(())
    }

    /// Attempt one already-issued exact output before selecting fresh ingress.
    /// No signing, worker wait or economic Apply is performed in this phase.
    pub(in crate::sumeragi) fn service_ready_output(
        &mut self,
        observed: &crate::state::VerifiedLaneContexts,
        global: &VerifiedHeightContext,
        network: &crate::IrohaNetwork,
    ) -> Result<NativeTransportProgress, V2RunnerError> {
        if self.outbound.is_none() {
            self.outbound = self
                .driver
                .take_outbound()
                .map_err(V2RunnerError::Service)?;
        }
        if let Some(packet) = self.outbound.take() {
            match self.transport.retain(observed, packet) {
                NativeTransportAdmission::Retained => {}
                NativeTransportAdmission::Retry(packet) => self.outbound = Some(packet),
                NativeTransportAdmission::Rejected { packet, reason } => {
                    let id =
                        super::super::v2_lane_driver::message_instance(&packet.envelope.message);
                    if observed
                        .contexts()
                        .iter()
                        .any(|lane| Hash::from(lane.instance_id().0) == id)
                    {
                        self.outbound = Some(packet);
                        return Err(V2RunnerError::Service(reason));
                    }
                    // A complete authenticated current-set observation retires only
                    // this obsolete transport packet. The instance Apply stays owned.
                }
            }
        }
        self.transport
            .poll(observed, Some(global), network)
            .map_err(V2RunnerError::Service)
    }

    /// Inspect retained transport custody without completing or replacing an output.
    #[cfg(test)]
    pub(in crate::sumeragi) fn retained_transport_outputs_for_test(
        &self,
    ) -> Vec<(Arc<super::super::message::BlockMessageWire>, Vec<PeerId>)> {
        self.transport.retained_outputs_for_test()
    }

    /// Native and exact historical responses continue while global Validate waits.
    pub(in crate::sumeragi) fn service_native_ingress(
        &mut self,
        receiver: &Arc<FairV2Ingress>,
    ) -> Result<(), V2RunnerError> {
        let Some(inbound) = receiver
            .try_recv_native_process_checked(self)
            .map_err(V2RunnerError::Service)?
        else {
            return Ok(());
        };
        let is_native = inbound.message().is_native_lane();
        let prepared = PreparedDequeuedV2IngressV1::new(
            Arc::clone(receiver),
            inbound,
            FairV2IngressDequeueDisposition::Admit,
            None,
            None,
            Arc::clone(&self.guard),
        );
        if is_native {
            self.consume_native_ingress(prepared, receiver)
        } else {
            ordinary_ingress_consumer::consume_prepared_native_source_response(
                prepared, receiver, self,
            )
        }
    }

    pub(in crate::sumeragi) fn admits_source_response_hash(
        &self,
        hash: HashOf<wire::CertifiedBodyRequest>,
    ) -> bool {
        self.source
            .as_ref()
            .is_some_and(|source| source.admits_hash(hash))
    }

    pub(in crate::sumeragi) fn admits_source_response(&self, message: &BlockMessage) -> bool {
        self.source
            .as_ref()
            .is_some_and(|source| source.admits(message))
    }

    pub(in crate::sumeragi) fn accept_source_response(
        &mut self,
        response: wire::CertifiedBodyResponse,
        sender: &PeerId,
    ) -> Result<(), V2RunnerError> {
        self.source
            .as_mut()
            .ok_or_else(|| {
                V2RunnerError::Service("Native source response has no retained request".into())
            })?
            .accept(response, sender)
    }

    /// Service one exact source at a time; all other waits stay in their original owners.
    pub(in crate::sumeragi) fn service_sources(
        &mut self,
        services: &mut ProductionV2Services,
        now: Instant,
    ) -> Result<(), V2RunnerError> {
        if let Some(mut source) = self.source.take() {
            match source.settle(&mut self.driver, services, &mut self.recovered_sources) {
                Ok(true) => {}
                Ok(false) => self.source = Some(source),
                Err(error) => {
                    self.source = Some(source);
                    return Err(error);
                }
            }
        }
        if self.source.is_some() {
            return Ok(());
        }
        let need = if let Some((subject, _, source)) = services.native_source_requirement() {
            Some((source, NativeSourceTarget::Validation(subject)))
        } else {
            self.driver.process().instance_ids().find_map(|id| {
                self.driver
                    .process()
                    .instance(id)
                    .and_then(|instance| instance.source_recovery_requirement())
                    .map(|source| (Arc::new(source.clone()), NativeSourceTarget::Instance(id)))
            })
        }
        .or_else(|| {
            self.candidate_source_requirement()
                .map(|source| (source, NativeSourceTarget::Candidate))
        });
        if let Some((source, target)) = need {
            self.source = Some(NativeSourceRequest::new(
                source,
                target,
                &self.local_peer,
                &self.key,
                services.current_archive_targets(),
                now,
            )?);
        }
        Ok(())
    }

    /// Retain exactly one original physical dequeue through driver pressure.
    /// The fair selector must leave subsequent Native occurrences in its queue
    /// while this slot is occupied; unrelated global ingress remains eligible.
    pub(in crate::sumeragi) fn consume_native_ingress(
        &mut self,
        prepared: PreparedDequeuedV2IngressV1,
        receiver: &FairV2Ingress,
    ) -> Result<(), V2RunnerError> {
        if self.pending_ingress.is_some() {
            prepared.close_output_for_restart();
            return Err(V2RunnerError::Service(
                "second Native dequeue bypassed retained ingress".into(),
            ));
        }
        self.pending_ingress = ordinary_ingress_consumer::consume_prepared_native_ingress(
            prepared,
            receiver,
            &mut self.driver,
        )?;
        Ok(())
    }

    pub(in crate::sumeragi) fn has_pending_ingress(&self) -> bool {
        self.pending_ingress.is_some()
    }

    /// Transfer the sole actual publication from the height service without a clone.
    pub(in crate::sumeragi) fn take_service_publication(
        &mut self,
        services: &mut ProductionV2Services,
    ) {
        if self.publication.is_none() {
            self.publication =
                services
                    .take_native_publication()
                    .map(|published| NativePublication {
                        published,
                        settled: false,
                    });
        }
    }

    fn settle_pending_publication(&mut self) -> Result<(), V2RunnerError> {
        if let Some(mut publication) = self.publication.take() {
            let result = if publication.settled {
                Ok(true)
            } else {
                self.settle_published(&publication.published)
            };
            if let Ok(settled) = result.as_ref() {
                publication.settled = *settled;
            }
            // The real publication also authorizes the final global output seal.
            // Physical retirement alone cannot release it before that handoff.
            self.publication = Some(publication);
            result?;
        }
        Ok(())
    }

    /// Join actual publication, physical Apply settlement and exact finality.
    pub(in crate::sumeragi) fn preflight_publication(
        &mut self,
        services: &mut ProductionV2Services,
        receipt: &KuraV2CommitReceipt,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> Result<bool, V2RunnerError> {
        self.take_service_publication(services);
        self.settle_pending_publication()?;
        let Some(publication) = self.publication.as_ref() else {
            return Ok(false);
        };
        Self::authenticate_publication(&publication.published, receipt, artifact)?;
        Ok(publication.settled)
    }

    fn authenticate_publication(
        published: &super::super::v2_apply::PublishedNativeCarrier,
        receipt: &KuraV2CommitReceipt,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> Result<(), V2RunnerError> {
        let actual = published.receipt();
        if published.artifact() != artifact
            || actual.height() != receipt.height()
            || actual.block_hash() != receipt.block_hash()
            || actual.context_id() != receipt.context_id()
            || actual.subject() != receipt.subject()
            || actual.certificate() != receipt.certificate()
            || actual.artifact_hash() != receipt.artifact_hash()
        {
            return Err(V2RunnerError::Service(
                "Native publication does not authorize this exact finality output".into(),
            ));
        }
        Ok(())
    }

    /// Borrow the real carrier only after all original local Apply owners settle.
    pub(in crate::sumeragi) fn finalized_output_authority(
        &self,
        receipt: &KuraV2CommitReceipt,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> Result<super::NativeFinalizedOutputAuthority<'_>, V2RunnerError> {
        let publication = self
            .publication
            .as_ref()
            .filter(|publication| publication.settled)
            .ok_or_else(|| {
                V2RunnerError::Service(
                    "Native publication still owns unfinished physical Apply".into(),
                )
            })?;
        Self::authenticate_publication(&publication.published, receipt, artifact)?;
        Ok(super::NativeFinalizedOutputAuthority {
            published: &publication.published,
        })
    }

    /// Release the exact completion only after the global output handoff is sealed.
    pub(in crate::sumeragi) fn complete_output_handoff(
        &mut self,
        receipt: &KuraV2CommitReceipt,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> Result<(), V2RunnerError> {
        self.finalized_output_authority(receipt, artifact)?;
        self.publication.take();
        Ok(())
    }

    pub(in crate::sumeragi) fn driver_mut(&mut self) -> &mut NativeLaneDriver {
        &mut self.driver
    }

    pub(in crate::sumeragi) fn next_deadline(&self) -> Option<Instant> {
        self.driver
            .next_deadline()
            .into_iter()
            .chain(
                self.source
                    .as_ref()
                    .and_then(NativeSourceRequest::next_deadline),
            )
            .min()
    }

    /// Capture actual Decisions while all live reducer/Apply custody stays local.
    pub(in crate::sumeragi) fn capture_decisions(
        &self,
    ) -> Result<Option<NativeLaneDecisionHandoff>, V2RunnerError> {
        let Some(observed) = self
            .state
            .verified_lane_consensus_contexts()
            .map_err(V2RunnerError::Service)?
        else {
            return Ok(None);
        };
        self.driver
            .capture_decisions(&observed)
            .map_err(V2RunnerError::Service)
    }

    /// Borrow genuine physical publication while its completion owns the carrier.
    /// A false result leaves that exact completion pending; it is never reexecuted.
    pub(in crate::sumeragi) fn settle_published(
        &mut self,
        carrier: &super::super::v2_apply::PublishedNativeCarrier,
    ) -> Result<bool, V2RunnerError> {
        let Some(published) = carrier.native_apply() else {
            return Ok(true);
        };
        self.driver
            .settle_published_carrier(&published)
            .map_err(V2RunnerError::Service)
    }

    /// Move the actual physical join owner to a blocking shutdown consumer.
    pub(in crate::sumeragi) fn shutdown(self) -> NativeProcessShutdown {
        self.guard.close_admission_for_restart();
        NativeProcessShutdown {
            native: self.driver.shutdown(),
            candidate: self.candidate_job,
        }
    }
}

/// Physical shutdown ownership moves out only after both global loops have ended.
pub(in crate::sumeragi) struct NativeProcessShutdown {
    native: LanePhysicalShutdown,
    candidate: Option<super::native_candidate::NativeCandidateJob>,
}
impl NativeProcessShutdown {
    /// Join all actual workers from the terminal shutdown path, never a control turn.
    pub(in crate::sumeragi) fn join(self) -> Result<(), V2RunnerError> {
        let candidate = self.candidate.map(|candidate| candidate.join()).transpose();
        let native = self
            .native
            .join()
            .map_err(|error| V2RunnerError::Service(error.to_string()));
        candidate?;
        native
    }
}
