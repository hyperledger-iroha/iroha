struct OrdinaryLaneDispatchFixture {
    lane_work: V2LaneWorkAdapter,
    executor: crate::sumeragi::v2_effects::V2EffectExecutor<
        crate::sumeragi::v2_runtime::SerializedV2Runtime,
    >,
    services: crate::sumeragi::v2_worker::ProductionV2Services,
    ingress: Arc<crate::sumeragi::FairV2Ingress>,
    actor: Option<iroha_p2p::network::NetworkActorAdmissionTestFixture<crate::NetworkMessage>>,
    proposal: LaneBlockProposalV1,
    prepare_qc: LaneBlockQcV1,
    expected_commit: LaneBlockVoteV1,
    second_message: BlockMessage,
    second_sender: PeerId,
    parent_hash: HashOf<BlockHeader>,
    initial_kura_count: usize,
    initial_state_height: usize,
    _directory: tempfile::TempDir,
}

impl OrdinaryLaneDispatchFixture {
    fn new(actor_capacity: usize) -> Self {
        use crate::sumeragi::{
            v2::{
                AdapterFingerprints, DeferredAdmissionOrdinalSource, SumeragiV2Adapter,
                VerifiedHeightContext,
            },
            v2_core::Generation,
            v2_runtime::{RuntimeLifecycleOrdinalSource, RuntimeQueueConfig, SerializedV2Runtime},
        };
        let (mut lane_work, keys) =
            fixture_at_height_inner(wire::ConsensusMode::Permissioned, 2, true);
        let (parent, parent_receipt) = lane_work
            .kura
            .v2_finality_artifact_with_receipt(1)
            .expect("read authenticated ordinary fixture parent")
            .expect("durable parent is present");
        let parent_hash = parent.block_hash;
        let initial_kura_count = lane_work.kura.blocks_count();
        let initial_state_height = lane_work.state.committed_height();
        assert_eq!(initial_kura_count, 1);
        assert_eq!(initial_state_height, 1);
        assert_eq!(
            lane_work.state.committed_block_hash_at_height(1),
            Some(parent_hash)
        );
        let (block, proposal) = planned_lane_candidate_block_at_view(&lane_work, &keys, 0);
        let _ = mark_global_body_locked_for_block(&mut lane_work, &block);
        assert_ne!(
            lane_work.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected
        );
        // Establish the candidate and local Prepare outside the regression.
        // The tested input is the valid remote Prepare QC which enables Commit.
        let initial = lane_work.drain_effects(usize::MAX);
        assert!(initial.iter().any(|effect| matches!(effect,
                V2LaneWorkEffect::PostLaneBlock { message: BlockMessage::LaneBlockVote(vote), .. }
                    if vote.body.phase == CertPhase::Prepare)));
        assert_eq!(lane_work.effect_count(), 0);
        assert!(!lane_work.output_guard.restart_required());
        let context = lane_work.context.clone();
        let local_validator = context
            .roster
            .iter()
            .position(|entry| entry.validator == lane_work.local_peer)
            .and_then(|index| u32::try_from(index).ok())
            .expect("local roster index");
        let proofs = keys
            .iter()
            .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("fixture PoP"))
            .collect();
        let verified = VerifiedHeightContext::successor(
            context.clone(),
            proofs,
            &parent,
            &parent_receipt,
            &parent.validator_set_pops,
        )
        .expect("verified exact ordinary successor context");
        let directory = tempfile::TempDir::new().expect("ordinary dispatch WAL directory");
        let (adapter, startup) = SumeragiV2Adapter::open(
            directory.path().join("ordinary-dispatch.wal"),
            verified,
            Some(local_validator),
            Generation::INITIAL,
            [0x63; 32],
            AdapterFingerprints {
                node: Hash::new(b"ordinary dispatch node"),
                build: Hash::new(b"ordinary dispatch build"),
                config: Hash::new(b"ordinary dispatch config"),
            },
            DeferredAdmissionOrdinalSource::new(0),
        )
        .expect("real safety-WAL adapter");
        assert!(startup.is_empty());
        let (runtime, startup) = SerializedV2Runtime::new_with_lifecycle_ordinals(
            adapter,
            startup,
            Instant::now(),
            Duration::from_secs(10),
            RuntimeQueueConfig::default(),
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        )
        .expect("real serialized runtime");
        assert!(startup.is_empty());
        let executor =
            crate::sumeragi::v2_effects::V2EffectExecutor::ordinary_dispatch_executor_for_test(
                runtime,
                context.clone(),
                lane_work.local_peer.clone(),
                Some(local_validator),
                Arc::clone(&lane_work.output_guard),
            );
        let mut services = crate::sumeragi::v2_worker::tests::ordinary_dispatch_services_for_test(
            Arc::clone(&lane_work.kura),
            context.clone(),
            &keys,
            local_validator,
            Arc::clone(&lane_work.state),
            Arc::clone(&lane_work.output_guard),
            executor.current_tag(),
        );
        let targets = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .filter(|peer| peer != &lane_work.local_peer)
            .collect();
        let (network, actor) = crate::IrohaNetwork::actor_admission_for_tests(
            lane_work.local_peer.clone(),
            targets,
            NonZeroUsize::new(actor_capacity).expect("positive actor capacity"),
        );
        crate::sumeragi::v2_worker::tests::install_network_for_test(&mut services, network);
        let ingress = Arc::new(
            crate::sumeragi::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
                crate::sumeragi::fair_v2_ingress_required_capacity(context.roster.len(), None)
                    .expect("ordinary fixture ingress source capacity"),
                512 * 1024 * 1024,
                64 * 1024 * 1024,
                crate::sumeragi::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
                8 * 1024 * 1024,
                8 * 1024 * 1024,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                None,
            ),
        );
        ingress
            .configure_roster_for_context(
                context.roster.iter().map(|entry| entry.validator.clone()),
                &context.network_id,
                context.da_layout,
            )
            .expect("exact roster and lane ingress ownership");
        ingress
            .open()
            .expect("open recovered ordinary fixture ingress");
        let sender_key = keys
            .iter()
            .find(|key| key.public_key() != lane_work.key_pair.public_key())
            .expect("remote authenticated validator");
        let sender = PeerId::new(sender_key.public_key().clone());
        let prepare_qc = lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare);
        let expected_commit = signed_lane_vote(&proposal, CertPhase::Commit, &lane_work.key_pair);
        let second_message = BlockMessage::LaneBlockVote(signed_lane_vote(
            &proposal,
            CertPhase::Prepare,
            sender_key,
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::LaneBlockQc(prepare_qc.clone()),
                sender.clone(),
            )),
            Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
        ));
        Self {
            lane_work,
            executor,
            services,
            ingress,
            actor: Some(actor),
            proposal,
            prepare_qc,
            expected_commit,
            second_message,
            second_sender: sender,
            parent_hash,
            initial_kura_count,
            initial_state_height,
            _directory: directory,
        }
    }

    fn prepare_first_and_queue_second(
        &self,
    ) -> (
        crate::sumeragi::v2_runner::ordinary_ingress_consumer::PreparedDequeuedV2IngressV1,
        u64,
    ) {
        use crate::sumeragi::v2_runner::ordinary_ingress_consumer::PreparedDequeuedV2IngressV1;
        let inbound = self
            .ingress
            .try_recv_if_checked(|_| true)
            .expect("checked physical dequeue")
            .expect("first QC");
        assert!(
            matches!(inbound.message(), BlockMessage::LaneBlockQc(qc) if qc == &self.prepare_qc)
        );
        let prepared = PreparedDequeuedV2IngressV1::new(
            Arc::clone(&self.ingress),
            inbound,
            crate::sumeragi::FairV2IngressDequeueDisposition::Admit,
            None,
            None,
            Arc::clone(&self.lane_work.output_guard),
        );
        let first = prepared.physical_ordinal_for_test();
        assert!(matches!(
            self.ingress
                .try_push(InboundBlockMessage::from_authenticated_peer(
                    self.second_message.clone(),
                    self.second_sender.clone(),
                )),
            Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
        ));
        let second = self.ingress.state.lock().last_admission_ordinal;
        assert!(second > first);
        (prepared, second)
    }

    fn queued_ordinals(&self) -> Vec<u64> {
        self.ingress
            .state
            .lock()
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter().map(|entry| entry.admission_ordinal))
            .collect()
    }

    fn consume(
        &mut self,
        prepared: crate::sumeragi::v2_runner::ordinary_ingress_consumer::PreparedDequeuedV2IngressV1,
        limit: usize,
    ) -> Result<(), crate::sumeragi::v2_runner::V2RunnerError> {
        use crate::sumeragi::v2_runner::ordinary_ingress_consumer::{
            ProductionPreparedOrdinaryIngressConsumptionV1, consume_prepared_dequeued_v2_ingress,
        };
        let context = self.lane_work.context.clone();
        let kura = Arc::clone(&self.lane_work.kura);
        let key = self.lane_work.key_pair.clone();
        let mut server =
            crate::sumeragi::v2_block_sync::V2BlockSyncServer::new(context.network_id, 16)
                .expect("block sync server");
        let mut discovery = crate::sumeragi::v2_block_sync::V2BlockSyncDiscovery::new(
            context.clone(),
            self.lane_work.local_peer.clone(),
            16,
        )
        .expect("block sync discovery");
        let mut request = None;
        let mut beacon = crate::sumeragi::v2_beacon::V2GlobalBeaconLifecycle::open(
            &context,
            self.lane_work.state.as_ref(),
            None,
            None,
        )
        .expect("inactive global beacon");
        let outcome = consume_prepared_dequeued_v2_ingress(
            prepared,
            self.ingress.as_ref(),
            &mut self.executor,
            &mut self.services,
            &mut self.lane_work,
            kura.as_ref(),
            &key,
            &mut server,
            &mut discovery,
            &mut request,
            &mut beacon,
            limit,
        )?;
        assert_eq!(
            outcome,
            ProductionPreparedOrdinaryIngressConsumptionV1::Continue
        );
        Ok(())
    }

    fn drain_actor(&mut self, seen: &mut Vec<(PeerId, BlockMessage)>) -> usize {
        let expected_commit = self.expected_commit.clone();
        let prepare_qc = self.prepare_qc.clone();
        self.actor
            .as_mut()
            .expect("retained actor receiver")
            .drain_posts(|post| {
                let crate::NetworkMessage::SumeragiBlock(envelope) = &post.data else {
                    panic!("only lane outputs belong to this actor fixture")
                };
                let message = envelope.as_message();
                assert!(
                    matches!(message, BlockMessage::LaneBlockVote(vote) if vote == &expected_commit)
                        || matches!(message, BlockMessage::LaneBlockQc(qc) if qc == &prepare_qc),
                    "no changed body, extra voting phase or unrelated output"
                );
                assert!(
                    !seen.iter().any(|(peer, previous)| peer == &post.peer_id
                        && std::mem::discriminant(previous) == std::mem::discriminant(message)),
                    "the same exact output must not be admitted twice"
                );
                seen.push((post.peer_id.clone(), message.clone()));
            })
    }

    fn finish_outputs(&mut self, seen: &mut Vec<(PeerId, BlockMessage)>) {
        for _ in 0..16 {
            self.drain_actor(seen);
            let _ = self
                .services
                .retry_pending_exact_output()
                .expect("bounded exact output retry");
            self.drain_actor(seen);
            if self.lane_work.effect_count() == 0
                && !self
                    .services
                    .has_pending_exact_output()
                    .expect("pending exact ownership")
            {
                break;
            }
        }
        assert_eq!(self.lane_work.effect_count(), 0);
        assert!(
            !self
                .services
                .has_pending_exact_output()
                .expect("exact output drained")
        );
        let actual: BTreeSet<_> = seen.iter().filter_map(|(peer, message)|
                matches!(message, BlockMessage::LaneBlockVote(vote) if vote == &self.expected_commit)
                    .then_some(peer.clone())).collect();
        let expected: BTreeSet<_> = self
            .proposal
            .descriptor
            .validator_set
            .iter()
            .filter(|peer| *peer != &self.lane_work.local_peer)
            .cloned()
            .collect();
        assert_eq!(
            actual, expected,
            "one exact local Commit reaches every remote actor target"
        );
        assert!(!self.lane_work.output_guard.restart_required());
        self.assert_candidate_not_applied();
    }

    fn assert_candidate_not_applied(&self) {
        assert_eq!(self.lane_work.kura.blocks_count(), self.initial_kura_count);
        assert_eq!(
            self.lane_work.state.committed_height(),
            self.initial_state_height
        );
        assert_eq!(
            self.lane_work.state.committed_block_hash_at_height(1),
            Some(self.parent_hash),
            "transport servicing must preserve the exact durable parent"
        );
        assert_eq!(
            self.lane_work
                .state
                .committed_block_hash_at_height(self.lane_work.context.height),
            None,
            "transport servicing must not economically apply the candidate"
        );
    }
}

#[test]
fn ordinary_lane_consumer_admits_bounded_commit_before_next_physical_ingress() {
    let mut fixture = OrdinaryLaneDispatchFixture::new(16);
    let (prepared, second) = fixture.prepare_first_and_queue_second();
    fixture
        .consume(prepared, 1)
        .expect("first exact ordinary consumer");
    assert_eq!(fixture.queued_ordinals(), vec![second]);
    let mut seen = Vec::new();
    assert_eq!(
        fixture.drain_actor(&mut seen),
        1,
        "one real actor post must precede the next dequeue; effect creation alone is insufficient"
    );
    assert!(
        matches!(&seen[0].1, BlockMessage::LaneBlockVote(vote) if vote == &fixture.expected_commit)
    );
    assert!(
        fixture.lane_work.effect_count() >= 2,
        "the output bound retains the remaining fanout"
    );
    let inbound = fixture
        .ingress
        .try_recv_if_checked(|_| true)
        .expect("second checked dequeue")
        .expect("second retained occurrence");
    assert!(matches!((inbound.message(), &fixture.second_message),
            (BlockMessage::LaneBlockVote(actual), BlockMessage::LaneBlockVote(expected)) if actual == expected));
    let prepared =
        crate::sumeragi::v2_runner::ordinary_ingress_consumer::PreparedDequeuedV2IngressV1::new(
            Arc::clone(&fixture.ingress),
            inbound,
            crate::sumeragi::FairV2IngressDequeueDisposition::Admit,
            None,
            None,
            Arc::clone(&fixture.lane_work.output_guard),
        );
    assert_eq!(prepared.physical_ordinal_for_test(), second);
    fixture
        .consume(prepared, 16)
        .expect("second exact ordinary consumer");
    assert!(fixture.queued_ordinals().is_empty());
    fixture.finish_outputs(&mut seen);
}

#[test]
fn ordinary_lane_consumer_retains_exact_commit_under_real_actor_backpressure() {
    let mut fixture = OrdinaryLaneDispatchFixture::new(1);
    let (prepared, second) = fixture.prepare_first_and_queue_second();
    fixture
        .consume(prepared, 16)
        .expect("network pressure is retained, not discarded or fatal");
    assert_eq!(fixture.queued_ordinals(), vec![second]);
    assert_eq!(
        fixture.lane_work.effect_count(),
        0,
        "the bounded fanout must transfer into the worker before retrying it"
    );
    assert!(
        fixture
            .services
            .has_pending_exact_output()
            .expect("worker owns blocked output")
    );
    assert!(!fixture.lane_work.output_guard.restart_required());
    let mut seen = Vec::new();
    assert_eq!(
        fixture.drain_actor(&mut seen),
        1,
        "capacity-one actor is actually saturated"
    );
    fixture.finish_outputs(&mut seen);
    assert_eq!(
        fixture.queued_ordinals(),
        vec![second],
        "output retry cannot consume later ingress"
    );
}

#[test]
fn ordinary_lane_consumer_actor_closure_fails_stop_before_next_ingress() {
    let mut fixture = OrdinaryLaneDispatchFixture::new(1);
    let (prepared, second) = fixture.prepare_first_and_queue_second();
    drop(
        fixture
            .actor
            .take()
            .expect("close the real retained actor receiver"),
    );
    let error = fixture
        .consume(prepared, 16)
        .expect_err("actor loss must escape the consumer");
    assert!(
        matches!(error, crate::sumeragi::v2_runner::V2RunnerError::Service(ref message)
            if message.contains("network actor closed")),
        "{error:?}"
    );
    assert!(fixture.lane_work.output_guard.restart_required());
    assert!(fixture.lane_work.output_guard.acquire().is_none());
    assert_eq!(fixture.queued_ordinals(), vec![second]);
    assert!(
        fixture.lane_work.effect_count() != 0,
        "unacknowledged source effect is retained"
    );
    assert!(
        fixture
            .services
            .has_pending_exact_output()
            .expect("returned actor message retained")
    );
    fixture.assert_candidate_not_applied();
}
