// These retained test names now exercise the production Native ingress tail,
// physical process, and actor transport. Native signing is asynchronous: the
// fixture waits for actual WAL/body completions before servicing a bounded send.
// No retired lane signer or global-height service participates in this path.
struct OrdinaryLaneDispatchFixture {
    state: Arc<State>,
    observed: crate::state::VerifiedLaneContexts,
    driver: Option<crate::sumeragi::v2_lane_driver::NativeLaneDriver>,
    transport: crate::sumeragi::v2_lane_transport::NativeLaneTransport,
    guard: Arc<crate::sumeragi::output_guard::ConsensusOutputGuard>,
    ingress: Arc<crate::sumeragi::FairV2Ingress>,
    network: crate::IrohaNetwork,
    actor: Option<iroha_p2p::network::NetworkActorAdmissionTestFixture<crate::NetworkMessage>>,
    expected_commit: iroha_data_model::block::lane_consensus::LaneMessageEnvelopeV1,
    prepare_qc: iroha_data_model::block::lane_consensus::LaneMessageEnvelopeV1,
    second_message: BlockMessage,
    second_sender: PeerId,
    targets: BTreeSet<PeerId>,
    parent_hash: HashOf<BlockHeader>,
    initial_kura_count: usize,
    initial_state_height: usize,
    now: Instant,
}

impl OrdinaryLaneDispatchFixture {
    fn new(actor_capacity: usize) -> Self {
        use crate::sumeragi::{
            output_guard::ConsensusOutputGuard,
            v2_lane_driver::{NativeLaneDriver, NativeLaneDriverLimits},
            v2_lane_instance::LaneProcessLimits,
            v2_lane_transport::NativeLaneTransport,
        };
        use iroha_data_model::block::lane_consensus::{
            LANE_MESSAGE_VERSION_V1, LaneMessageEnvelopeV1, LaneMessageV1, LanePhaseV1, LaneQcV1,
            LaneSignatureShareV1, LaneVoteStatementV1, LaneVoteV1,
        };
        let (state, keys) = State::native_dispatch_source_fixture_for_test();
        let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
        let lane = &observed.contexts()[0];
        let id = lane.instance_id();
        let local = lane
            .reducer_context()
            .roster()
            .iter()
            .position(|entry| entry.id() == lane.reducer_context().leader(0))
            .expect("actual Native initial author");
        let key_for = |index: usize| {
            keys.iter()
                .find(|key| key.public_key() == lane.frozen().committee[index].public_key())
                .expect("frozen committee key")
        };
        let local_peer = lane.frozen().committee[local].clone();
        let guard = ConsensusOutputGuard::isolated();
        let mut driver = NativeLaneDriver::new(
            Arc::clone(&state),
            Arc::clone(&guard),
            key_for(local).clone(),
            NativeLaneDriverLimits {
                voting_enabled: true,
                process: LaneProcessLimits {
                    instances: NonZeroUsize::MIN,
                    workers_per_class: NonZeroUsize::MIN,
                    queued_per_class: NonZeroUsize::MIN,
                    completed: NonZeroUsize::MIN,
                    effect_limit: 3 * crate::sumeragi::v2_core::MAX_EFFECTS_PER_STEP,
                    base_timeout: Duration::from_secs(10),
                    retransmit: Duration::from_secs(1),
                },
                ingress: NonZeroUsize::MIN,
                outbound: NonZeroUsize::new(8).unwrap(),
                maximum_message_bytes: NonZeroUsize::new(8 * 1024 * 1024).unwrap(),
            },
        )
        .expect("original Native physical process");
        let now = Instant::now();
        let until = now + Duration::from_secs(20);
        let mut proposal = None;
        let mut prepare = None;
        loop {
            driver
                .poll(&observed, now)
                .expect("actual opening/body/WAL progress");
            while let Some(packet) = driver.take_outbound().unwrap() {
                assert_eq!(
                    packet.canonical_bytes,
                    norito::encode_canonical(&packet.envelope).unwrap()
                );
                match packet.envelope.message {
                    LaneMessageV1::Proposal(value) => {
                        assert!(
                            proposal.replace(value).is_none(),
                            "one initial native proposal"
                        );
                    }
                    LaneMessageV1::Vote(value) if value.statement.phase == LanePhaseV1::Prepare => {
                        assert!(
                            prepare.replace(value).is_none(),
                            "one durable initial Prepare"
                        );
                    }
                    other => panic!("unexpected initialization output: {other:?}"),
                }
            }
            if proposal.is_some()
                && prepare.is_some()
                && driver
                    .process()
                    .instance(id)
                    .unwrap()
                    .held_effects()
                    .next()
                    .is_none()
            {
                break;
            }
            assert!(
                Instant::now() < until,
                "real Native initial proposal and Prepare deadline"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let proposal = proposal.unwrap();
        let prepare = prepare.unwrap();
        assert_eq!(prepare.statement.value, proposal.body.manifest.value);
        assert_eq!(prepare.share.signer, local as u32);
        let sign = |index: usize, statement: &LaneVoteStatementV1| LaneSignatureShareV1 {
            signer: index as u32,
            signature: Signature::try_new(
                key_for(index).private_key(),
                &statement.signature_preimage().unwrap(),
            )
            .unwrap()
            .payload()
            .to_vec(),
        };
        let remote = (0..keys.len()).find(|index| *index != local).unwrap();
        let prepare_qc = LaneMessageEnvelopeV1 {
            version: LANE_MESSAGE_VERSION_V1,
            message: LaneMessageV1::QuorumCertificate(LaneQcV1 {
                statement: prepare.statement,
                shares: (0..3)
                    .map(|index| sign(index, &prepare.statement))
                    .collect(),
            }),
        };
        let commit_statement = LaneVoteStatementV1 {
            phase: LanePhaseV1::Commit,
            ..prepare.statement
        };
        let expected_commit = LaneMessageEnvelopeV1 {
            version: LANE_MESSAGE_VERSION_V1,
            message: LaneMessageV1::Vote(LaneVoteV1 {
                statement: commit_statement,
                share: sign(local, &commit_statement),
            }),
        };
        let second_message = BlockMessage::NativeLane(LaneMessageEnvelopeV1 {
            version: LANE_MESSAGE_VERSION_V1,
            message: LaneMessageV1::Vote(LaneVoteV1 {
                statement: prepare.statement,
                share: sign(remote, &prepare.statement),
            }),
        });
        let second_sender = lane.frozen().committee[remote].clone();
        let targets = lane
            .frozen()
            .committee
            .iter()
            .filter(|peer| **peer != local_peer)
            .cloned()
            .collect::<BTreeSet<_>>();
        let (network, actor) = crate::IrohaNetwork::actor_admission_for_tests(
            local_peer.clone(),
            targets.iter().cloned().collect(),
            NonZeroUsize::new(actor_capacity).unwrap(),
        );
        let source_byte_capacity = 8 * 1024 * 1024;
        let roster_len = lane.frozen().committee.len();
        let ingress = Arc::new(crate::sumeragi::FairV2Ingress::new(
            crate::sumeragi::fair_v2_ingress_required_capacity(roster_len, None)
                .expect("bounded fixture roster"),
            crate::sumeragi::fair_v2_ingress_required_byte_capacity(
                roster_len,
                None,
                source_byte_capacity,
            )
            .expect("bounded fixture bytes"),
            source_byte_capacity,
            0,
            0,
        ));
        ingress
            .configure_roster(lane.frozen().committee.iter().cloned())
            .unwrap();
        ingress.open().unwrap();
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::NativeLane(prepare_qc.clone()),
                second_sender.clone(),
            )),
            Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
        ));
        let initial_kura_count = state.kura().blocks_count();
        let initial_state_height = state.committed_height();
        let parent_hash = state
            .committed_block_hash_at_height(initial_state_height as u64)
            .unwrap();
        let transport = NativeLaneTransport::new(
            Arc::clone(&state),
            Arc::clone(&guard),
            local_peer,
            NonZeroUsize::MIN,
        );
        Self {
            state,
            observed,
            driver: Some(driver),
            transport,
            guard,
            ingress,
            network,
            actor: Some(actor),
            expected_commit,
            prepare_qc,
            second_message,
            second_sender,
            targets,
            parent_hash,
            initial_kura_count,
            initial_state_height,
            now,
        }
    }

    fn prepare_first_and_queue_second(
        &self,
    ) -> (
        crate::sumeragi::v2_runner::ordinary_ingress_consumer::PreparedDequeuedV2IngressV1,
        u64,
    ) {
        let inbound = self.ingress.try_recv_if_checked(|_| true).unwrap().unwrap();
        assert!(
            matches!(inbound.message(), BlockMessage::NativeLane(exact) if exact == &self.prepare_qc)
        );
        let prepared = self.prepare(inbound);
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

    fn prepare(
        &self,
        inbound: InboundBlockMessage,
    ) -> crate::sumeragi::v2_runner::ordinary_ingress_consumer::PreparedDequeuedV2IngressV1 {
        crate::sumeragi::v2_runner::ordinary_ingress_consumer::PreparedDequeuedV2IngressV1::new(
            Arc::clone(&self.ingress),
            inbound,
            crate::sumeragi::FairV2IngressDequeueDisposition::Admit,
            None,
            None,
            Arc::clone(&self.guard),
        )
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
    ) {
        let retained =
            crate::sumeragi::v2_runner::ordinary_ingress_consumer::consume_prepared_native_ingress(
                prepared,
                self.ingress.as_ref(),
                self.driver.as_mut().unwrap(),
            )
            .expect("current exact Native ingress tail");
        assert!(
            retained.is_none(),
            "empty driver transfers this physical occurrence exactly once"
        );
    }

    fn await_commit(&mut self) {
        use crate::sumeragi::v2_lane_transport::NativeTransportAdmission;
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            let driver = self.driver.as_mut().unwrap();
            driver.poll(&self.observed, self.now).unwrap();
            if let Some(packet) = driver.take_outbound().unwrap() {
                assert_eq!(
                    packet.envelope, self.expected_commit,
                    "the actual durable Commit keeps the exact native body and signer"
                );
                assert_eq!(
                    packet.canonical_bytes,
                    norito::encode_canonical(&packet.envelope).unwrap()
                );
                assert!(matches!(
                    self.transport.retain(&self.observed, packet),
                    NativeTransportAdmission::Retained
                ));
                break;
            }
            assert!(
                Instant::now() < until,
                "real Commit WAL/signing completion deadline"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!self.guard.restart_required());
        self.assert_candidate_not_applied();
    }

    fn poll_transport(
        &mut self,
    ) -> Result<crate::sumeragi::v2_lane_transport::NativeTransportProgress, String> {
        self.transport.poll(&self.observed, None, &self.network)
    }

    fn drain_actor(
        &mut self,
        seen: &mut BTreeSet<PeerId>,
        original_frame: &mut Option<Arc<crate::sumeragi::message::BlockMessageWire>>,
    ) -> usize {
        let expected = &self.expected_commit;
        self.actor.as_mut().expect("retained actor receiver").drain_posts(|post| {
            let crate::NetworkMessage::SumeragiBlock(frame) = &post.data else {
                panic!("only actual Native outputs belong to this actor")
            };
            assert!(matches!(frame.as_message(), BlockMessage::NativeLane(exact) if exact == expected));
            assert!(seen.insert(post.peer_id.clone()), "no duplicate actor admission for a destination");
            if let Some(original) = original_frame {
                assert!(Arc::ptr_eq(original, frame), "pressure retains the original exact wire allocation");
            } else {
                *original_frame = Some(Arc::clone(frame));
            }
        })
    }

    fn assert_transport_retains_commit(&mut self) {
        use crate::sumeragi::{
            v2_lane_instance::LaneOutbound, v2_lane_transport::NativeTransportAdmission,
        };
        // Capacity is exactly one. A second valid occurrence must be returned
        // intact while any destination of the original fanout remains owned.
        let packet = LaneOutbound {
            canonical_bytes: norito::encode_canonical(&self.expected_commit).unwrap(),
            envelope: self.expected_commit.clone(),
            destinations: self.observed.contexts()[0].frozen().committee.clone(),
        };
        let NativeTransportAdmission::Retry(returned) =
            self.transport.retain(&self.observed, packet)
        else {
            panic!("the original unfinished Commit fanout lost its bounded custody")
        };
        assert_eq!(returned.envelope, self.expected_commit);
        assert_eq!(
            returned.canonical_bytes,
            norito::encode_canonical(&self.expected_commit).unwrap()
        );
    }

    fn finish_outputs(
        &mut self,
        seen: &mut BTreeSet<PeerId>,
        frame: &mut Option<Arc<crate::sumeragi::message::BlockMessageWire>>,
    ) {
        use crate::sumeragi::v2_lane_transport::NativeTransportProgress;
        for _ in 0..16 {
            self.drain_actor(seen, frame);
            if self.poll_transport().unwrap() == NativeTransportProgress::Idle {
                break;
            }
        }
        self.drain_actor(seen, frame);
        assert_eq!(
            self.poll_transport().unwrap(),
            NativeTransportProgress::Idle
        );
        assert_eq!(
            *seen, self.targets,
            "one exact local Commit reaches each remote actor target"
        );
        assert!(!self.guard.restart_required());
        self.assert_candidate_not_applied();
    }

    fn assert_candidate_not_applied(&self) {
        assert_eq!(self.state.kura().blocks_count(), self.initial_kura_count);
        assert_eq!(self.state.committed_height(), self.initial_state_height);
        assert_eq!(
            self.state
                .committed_block_hash_at_height(self.initial_state_height as u64),
            Some(self.parent_hash)
        );
        assert_eq!(
            self.state
                .committed_block_hash_at_height(self.initial_state_height as u64 + 1),
            None,
            "physical Native control/actor progress cannot economically apply a candidate"
        );
    }
}

impl Drop for OrdinaryLaneDispatchFixture {
    fn drop(&mut self) {
        if let Some(driver) = self.driver.take() {
            driver
                .shutdown()
                .join()
                .expect("join actual Native physical workers");
        }
    }
}

#[test]
fn ordinary_lane_consumer_admits_bounded_commit_before_next_physical_ingress() {
    use crate::sumeragi::v2_lane_transport::NativeTransportProgress;
    let mut fixture = OrdinaryLaneDispatchFixture::new(16);
    let (prepared, second) = fixture.prepare_first_and_queue_second();
    fixture.consume(prepared);
    fixture.await_commit();
    assert_eq!(fixture.queued_ordinals(), vec![second]);
    assert!(matches!(
        fixture.poll_transport().unwrap(),
        NativeTransportProgress::Admitted { .. }
    ));
    let mut seen = BTreeSet::new();
    let mut frame = None;
    assert_eq!(
        fixture.drain_actor(&mut seen, &mut frame),
        1,
        "a bounded transport turn admits one real actor post, not just an effect"
    );
    fixture.assert_transport_retains_commit();
    let inbound = fixture
        .ingress
        .try_recv_if_checked(|_| true)
        .unwrap()
        .unwrap();
    assert!(matches!((inbound.message(), &fixture.second_message),
        (BlockMessage::NativeLane(actual), BlockMessage::NativeLane(expected)) if actual == expected));
    let prepared = fixture.prepare(inbound);
    assert_eq!(prepared.physical_ordinal_for_test(), second);
    fixture.consume(prepared);
    assert!(fixture.queued_ordinals().is_empty());
    fixture.finish_outputs(&mut seen, &mut frame);
}

#[test]
fn ordinary_lane_consumer_retains_exact_commit_under_real_actor_backpressure() {
    use crate::sumeragi::v2_lane_transport::NativeTransportProgress;
    let mut fixture = OrdinaryLaneDispatchFixture::new(1);
    let (prepared, second) = fixture.prepare_first_and_queue_second();
    fixture.consume(prepared);
    fixture.await_commit();
    assert!(matches!(
        fixture.poll_transport().unwrap(),
        NativeTransportProgress::Admitted { .. }
    ));
    // Real capacity-one actor pressure assigns FIFO tickets; repeated attempts
    // retain rank one instead of creating a new wait occurrence.
    for _ in 0..3 {
        assert!(matches!(
            fixture.poll_transport().unwrap(),
            NativeTransportProgress::Backpressured { rank: 1, .. }
        ));
    }
    assert_eq!(fixture.queued_ordinals(), vec![second]);
    fixture.assert_transport_retains_commit();
    assert!(!fixture.guard.restart_required());
    let mut seen = BTreeSet::new();
    let mut frame = None;
    assert_eq!(
        fixture.drain_actor(&mut seen, &mut frame),
        1,
        "the actual actor is saturated"
    );
    fixture.finish_outputs(&mut seen, &mut frame);
    assert_eq!(
        fixture.queued_ordinals(),
        vec![second],
        "output retry cannot consume later physical ingress"
    );
}

#[test]
fn ordinary_lane_consumer_actor_closure_fails_stop_before_next_ingress() {
    let mut fixture = OrdinaryLaneDispatchFixture::new(1);
    let (prepared, second) = fixture.prepare_first_and_queue_second();
    fixture.consume(prepared);
    fixture.await_commit();
    drop(
        fixture
            .actor
            .take()
            .expect("close the actual actor receiver"),
    );
    let error = fixture
        .poll_transport()
        .expect_err("actor loss must escape the Native transport");
    assert!(error.contains("closed"), "{error}");
    assert!(fixture.guard.restart_required());
    assert!(fixture.guard.acquire().is_none());
    assert_eq!(fixture.queued_ordinals(), vec![second]);
    fixture.assert_transport_retains_commit();
    assert!(fixture.poll_transport().unwrap_err().contains("restart"));
    fixture.assert_candidate_not_applied();
}
