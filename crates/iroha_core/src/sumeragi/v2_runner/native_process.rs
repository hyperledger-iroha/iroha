//! One Native lane process retained across global-height activation and rollover.

use super::native_source::{NativeSourceRequest, NativeSourceTarget};
use super::ordinary_ingress_consumer::PreparedDequeuedV2IngressV1;
use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::sumeragi::{
    v2::VerifiedHeightContext,
    v2_lane_driver::{NativeLaneDecisionHandoff, NativeLaneDriver, NativeLaneDriverLimits},
    v2_lane_instance::{LaneCurrentGate, LaneOutbound, LanePhysicalShutdown, LaneProcessLimits},
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
    // Scheduling only: a turn lacking current State uses the existing IDLE_POLL
    // wake instead of exposing expired clocks it could not service.
    awaiting_current_observation: bool,
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
            awaiting_current_observation: true,
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
        self.note_current_observation(observed.as_ref());
        if let Some(observed) = observed.as_ref() {
            self.service_ready_output(observed, global, network)?;
        }
        self.service_native_ingress(receiver)?;
        let source_gate = NativeSourceRequest::retire_closed_instance(
            &mut self.source,
            self.driver.process(),
            observed.as_ref(),
        );
        self.awaiting_current_observation |= source_gate == LaneCurrentGate::ObservationChanged;
        if source_gate != LaneCurrentGate::ObservationChanged {
            if let Some(source) = self.source.as_mut() {
                source.poll(network, &self.guard, now, self.retransmit)?;
            }
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
            .prepare_one_retirement()
            .map_err(V2RunnerError::Service)?;
        self.driver
            .poll(&observed, now)
            .map_err(V2RunnerError::Service)?;
        self.note_current_observation(Some(&observed));
        self.awaiting_current_observation |= source_gate == LaneCurrentGate::ObservationChanged;
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
        // Authentication is needed only for an actual instance source need.
        // The common no-source turn must not repeat Kura/State observation work.
        let needs_current =
            self.source
                .as_ref()
                .is_some_and(NativeSourceRequest::targets_instance)
                || self.driver.process().instance_ids().any(|id| {
                    self.driver.process().is_productive(id)
                        && self.driver.process().instance(id).is_some_and(|instance| {
                            instance.source_recovery_requirement().is_some()
                        })
                });
        let observed = if needs_current {
            self.state
                .verified_lane_consensus_contexts()
                .map_err(V2RunnerError::Service)?
        } else {
            None
        };
        if NativeSourceRequest::retire_closed_instance(
            &mut self.source,
            self.driver.process(),
            observed.as_ref(),
        ) == LaneCurrentGate::ObservationChanged
        {
            self.awaiting_current_observation = true;
            return Ok(());
        }
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
            Some((source, NativeSourceTarget::Validation(Box::new(subject))))
        } else {
            self.driver.process().instance_ids().find_map(|id| {
                let target = self
                    .driver
                    .process()
                    .source_recovery_target(id, observed.as_ref()?)?;
                self.driver
                    .process()
                    .instance(id)
                    .and_then(|instance| instance.source_recovery_requirement())
                    .map(|source| {
                        (
                            Arc::new(source.clone()),
                            NativeSourceTarget::Instance(target),
                        )
                    })
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

    fn note_current_observation(&mut self, observed: Option<&crate::state::VerifiedLaneContexts>) {
        self.awaiting_current_observation =
            observed.is_none_or(|observed| !observed.is_current(&self.state));
    }

    pub(in crate::sumeragi) fn next_deadline(&self) -> Option<Instant> {
        if self.awaiting_current_observation {
            return None;
        }
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

    #[cfg(all(test, feature = "bls"))]
    pub(super) fn assert_observation_deadline_for_test(
        state: Arc<State>,
        source: NativeSourceRequest,
        key: &KeyPair,
        now: Instant,
    ) {
        let mut process = Self::new(
            Arc::clone(&state),
            ConsensusOutputGuard::isolated(),
            PeerId::new(key.public_key().clone()),
            key.clone(),
            true,
            &crate::sumeragi::v2::SumeragiV2Adapter::native_source_lifecycle_config_for_test(),
            32 * 1024 * 1024,
            Duration::from_secs(10),
            Duration::from_secs(1),
        )
        .expect("actual Native process and bounded physical pools");
        process.source = Some(source);
        let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
        process.note_current_observation(Some(&observed));
        assert_eq!(
            process.next_deadline(),
            Some(now),
            "the genuine request is already due"
        );
        for _ in 0..3 {
            process.note_current_observation(None);
            assert_eq!(
                process.next_deadline(),
                None,
                "the runner must use its bounded IDLE_POLL fallback, not a zero-duration expired-source loop"
            );
            assert!(
                process.source.is_some(),
                "observation delay cannot consume the original request"
            );
        }
        process.note_current_observation(Some(&observed));
        assert_eq!(
            process.next_deadline(),
            Some(now),
            "the same retry remains due when service resumes"
        );
        process.shutdown().join().unwrap();
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

#[cfg(all(test, feature = "bls", unix, not(target_os = "espidf")))]
mod pending_ingress_rollover_tests {
    use super::*;
    use crate::sumeragi::{
        FairV2IngressPushDisposition, FairV2IngressSource,
        serviced_candidate_store::{LeaderWireLifecycleStoreGate, LeaderWireRecoveryAuthority},
        v2_runtime::RuntimeLifecycleOrdinalSource,
    };
    use iroha_crypto::Signature;
    use iroha_data_model::block::lane_consensus::{
        LANE_MESSAGE_VERSION_V1, LaneMessageEnvelopeV1, LaneMessageV1, LaneRoundV1,
        LaneSignatureShareV1, LaneTimeoutBodyV1, LaneTimeoutVoteV1,
    };

    fn bind_global_gate(
        ingress: &Arc<FairV2Ingress>,
        roster: &[PeerId],
        height: u64,
        marker: u8,
    ) -> tempfile::TempDir {
        ingress.close();
        ingress.configure_roster(roster.iter().cloned()).unwrap();
        if !ingress.state.lock().requires_leader_wire_lifecycle_gate {
            ingress.require_leader_wire_lifecycle_gate();
        }
        ingress.state.lock().leader_wire_max_chunk_count = 2;
        let directory = tempfile::TempDir::new().unwrap();
        let context =
            wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new([marker; 32])));
        let owner = [marker; 32];
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), 2).unwrap();
        let authority =
            LeaderWireRecoveryAuthority::from_replayed_adapter(context, height, owner, 0, false);
        let (gate, restore) = LeaderWireLifecycleStoreGate::open(
            &directory.path().join("safety.wal"),
            context,
            height,
            owner,
            roster.iter().cloned().collect(),
            capacity,
            2,
            authority,
            &[],
            &[],
        )
        .unwrap();
        ingress
            .bind_leader_wire_lifecycle_gate(
                gate,
                restore,
                RuntimeLifecycleOrdinalSource::after_high_watermark(0),
                context,
                height,
            )
            .unwrap();
        ingress.open().unwrap();
        directory
    }

    #[test]
    fn native_prepared_capacity_retry_survives_global_cut_and_rebind_then_advances() {
        let now = Instant::now();
        let (state, keys) = crate::state::State::native_dispatch_source_fixture_for_test();
        let state_before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
        let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
        let lane = &observed.contexts()[0];
        let id = lane.instance_id();
        let committee = lane.frozen().committee.clone();
        let key_for = |signer: usize| {
            keys.iter()
                .find(|key| key.public_key() == committee[signer].public_key())
                .unwrap()
        };
        let controls = std::array::from_fn::<_, 3, _>(|signer| {
            let body = LaneTimeoutBodyV1 {
                round: LaneRoundV1 {
                    instance_id: Hash::from(id.0),
                    lane_height: lane.frozen().next_lane_height,
                    voting_view: 0,
                },
                highest_prepare: None,
            };
            let signature = Signature::try_new(
                key_for(signer).private_key(),
                &body.signature_preimage().unwrap(),
            )
            .unwrap()
            .payload()
            .to_vec();
            LaneMessageEnvelopeV1 {
                version: LANE_MESSAGE_VERSION_V1,
                message: LaneMessageV1::TimeoutVote(LaneTimeoutVoteV1 {
                    body,
                    share: LaneSignatureShareV1 {
                        signer: signer as u32,
                        signature,
                    },
                }),
            }
        });
        let guard = ConsensusOutputGuard::isolated();
        let mut config =
            super::super::super::v2::SumeragiV2Adapter::native_source_lifecycle_config_for_test();
        config.limits.control_queue_capacity = 1;
        let mut process = NativeRunnerProcess::new(
            Arc::clone(&state),
            Arc::clone(&guard),
            committee[3].clone(),
            key_for(3).clone(),
            true,
            &config,
            32 * 1024 * 1024,
            Duration::from_secs(10),
            Duration::from_secs(1),
        )
        .unwrap();
        let ingress = Arc::new(FairV2Ingress::new(
            40,
            64 * 1_048_576,
            8 * 1_048_576,
            crate::sumeragi::TIMEOUT_VOTE_RESERVE_BYTES,
            0,
        ));
        let _predecessor = bind_global_gate(&ingress, &committee, 41, 0xA6);
        for (signer, envelope) in controls.iter().enumerate() {
            ingress
                .try_push(InboundBlockMessage::from_authenticated_peer(
                    BlockMessage::NativeLane(envelope.clone()),
                    committee[signer].clone(),
                ))
                .unwrap();
        }
        process.service_native_ingress(&ingress).unwrap();
        assert!(
            !process.has_pending_ingress(),
            "first control fills the real driver slot"
        );
        process.service_native_ingress(&ingress).unwrap();
        let original = process
            .pending_ingress
            .as_ref()
            .unwrap()
            .native_timeout_owner_snapshot_for_test();
        assert_eq!(ingress.len(), 1);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::NativeLane(controls[2].clone()),
                committee[2].clone(),
            )),
            Ok(FairV2IngressPushDisposition::Coalesced)
        ));
        let third_source = FairV2IngressSource::Native(committee[2].clone());
        let queued = || {
            let physical = ingress.state.lock();
            let entry = &physical.lanes[&third_source].entries[0];
            let BlockMessage::NativeLane(envelope) = entry.inbound.message() else {
                panic!("queued Native control");
            };
            let LaneMessageV1::TimeoutVote(vote) = &envelope.message else {
                panic!("queued timeout vote");
            };
            (
                Arc::as_ptr(&entry.inbound),
                entry.encoded_bytes.as_ptr(),
                entry.admission_ordinal,
                entry.ownership_snapshot.process_local_projection_hash(),
                physical.bytes,
                vote.share.signature.as_ptr(),
            )
        };
        let original_queued = queued();
        for _ in 0..3 {
            process.service_native_ingress(&ingress).unwrap();
            let prepared = process.pending_ingress.take().unwrap();
            process.consume_native_ingress(prepared, &ingress).unwrap();
            assert_eq!(
                process
                    .pending_ingress
                    .as_ref()
                    .unwrap()
                    .native_timeout_owner_snapshot_for_test(),
                original
            );
            assert_eq!(queued(), original_queued);
        }
        // A later global carrier cannot make finalization depend on draining
        // process-lived Native custody. The real driver is still full, the
        // second Native control is retained, and the third remains queued.
        let mut global_vote = wire::TimeoutVote {
            round: wire::ConsensusRound {
                context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                    [0xA6; 32],
                ))),
                height: 41,
                view: 0,
            },
            highest_prepare_qc: None,
            signer: 0,
            signature: Vec::new(),
        };
        global_vote.signature =
            Signature::try_new(key_for(0).private_key(), &global_vote.signature_preimage())
                .unwrap()
                .payload()
                .to_vec();
        ingress
            .try_push(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::V2(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::TimeoutVote(global_vote),
                )),
                committee[0].clone(),
            ))
            .unwrap();
        {
            let physical = ingress.state.lock();
            let record = physical.leader_wire_lifecycles.values().next().unwrap();
            assert_eq!(record.ingress_predecessors.get(&third_source), Some(&1));
        }
        ingress.close();
        assert_eq!(
            ingress.ensure_closed_global_drained_cut().unwrap_err(),
            "finalized global ingress cut retained global ownership",
            "a closed queue with a real global owner is not an empty global cut"
        );
        let (mut global, authorization) = super::super::select_decided_lane_recovery_ingress(
            &ingress,
            41,
            super::super::DecidedLaneRecoveryIngressDrainMode::FinalizedClosedPrefix,
        )
        .unwrap()
        .expect("both finalized callers must retire global work behind retained Native ingress");
        assert!(matches!(
            authorization,
            super::super::DecidedLaneRecoveryDrainAuthorization::LeaderWireRetire
        ));
        let mut global_ownership = global.take_ingress_ownership().unwrap();
        assert!(global_ownership.validate_exact());
        ingress
            .bind_leader_wire_runtime_ownership(&mut global_ownership)
            .unwrap();
        ingress
            .mark_leader_wire_volatile_terminal(
                global_ownership.leader_wire_runtime_receipt().unwrap(),
            )
            .unwrap();
        assert!(
            super::super::select_decided_lane_recovery_ingress(
                &ingress,
                41,
                super::super::DecidedLaneRecoveryIngressDrainMode::FinalizedClosedPrefix,
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(queued(), original_queued);
        assert_eq!(
            process
                .pending_ingress
                .as_ref()
                .unwrap()
                .native_timeout_owner_snapshot_for_test(),
            original,
            "global retirement cannot consume or replace the backpressured Native owner"
        );
        ingress.ensure_closed_global_drained_cut().unwrap();
        assert!(ingress.ensure_closed_drained_cut().is_err());
        let old_gate = ingress
            .state
            .lock()
            .leader_wire_lifecycle_gate
            .as_ref()
            .unwrap()
            .clone();
        ingress
            .retire_leader_wire_lifecycle_gate(&old_gate)
            .unwrap();
        let successor_roster = (0..4)
            .map(|_| PeerId::new(KeyPair::random().public_key().clone()))
            .collect::<Vec<_>>();
        let _successor = bind_global_gate(&ingress, &successor_roster, 42, 0xA7);
        assert_eq!(
            queued(),
            original_queued,
            "global retirement and roster change conserve Native queue custody"
        );
        assert_eq!(
            process
                .pending_ingress
                .as_ref()
                .unwrap()
                .native_timeout_owner_snapshot_for_test(),
            original
        );
        assert!(!guard.restart_required());
        // Drive the same production completion/control and exact retry phases as
        // poll, with fixed Native time so no fourth locally timed vote can stand
        // in for any of the three physically admitted controls.
        let until = Instant::now() + Duration::from_secs(30);
        while process.has_pending_ingress() {
            process.driver.prepare_one_retirement().unwrap();
            process.driver.poll(&observed, now).unwrap();
            let prepared = process.pending_ingress.take().unwrap();
            assert_eq!(prepared.native_timeout_owner_snapshot_for_test(), original);
            process.consume_native_ingress(prepared, &ingress).unwrap();
            assert_eq!(queued(), original_queued);
            assert!(
                Instant::now() < until,
                "actual driver capacity must recover"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        process.service_native_ingress(&ingress).unwrap();
        assert_eq!(ingress.len(), 0);
        assert!(
            process.has_pending_ingress(),
            "second control still owns the sole driver slot"
        );
        let third = process
            .pending_ingress
            .as_ref()
            .unwrap()
            .native_timeout_owner_snapshot_for_test();
        assert_eq!(
            third.0, original_queued.2,
            "the queued occurrence crosses the real dequeue once"
        );
        assert_eq!(
            third.2, original_queued.5,
            "the original queued signature allocation moves once"
        );
        while process.has_pending_ingress() {
            process.driver.prepare_one_retirement().unwrap();
            process.driver.poll(&observed, now).unwrap();
            let prepared = process.pending_ingress.take().unwrap();
            assert_eq!(prepared.native_timeout_owner_snapshot_for_test(), third);
            process.consume_native_ingress(prepared, &ingress).unwrap();
            assert!(
                Instant::now() < until,
                "third original control must cross recovered capacity"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        while process.driver.process().instance(id).unwrap().tag().view() != 1 {
            process.driver.prepare_one_retirement().unwrap();
            process.driver.poll(&observed, now).unwrap();
            assert!(
                Instant::now() < until,
                "all three real timeout votes must advance the shared reducer"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        ingress.close();
        ingress.ensure_closed_global_drained_cut().unwrap();
        ingress.ensure_closed_drained_cut().unwrap();
        assert!(!guard.restart_required());
        assert!(
            process.publication.is_none(),
            "ingress progress cannot fabricate publication authority"
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            state_before
        );
        process.shutdown().join().unwrap();
    }
}
