// Actual Native process, fsynced local Commit and finite actor/ingress service.
// Asynchronous body/WAL completion is serviced over bounded turns; a ready
// output must get an actor attempt before the next physical Native dequeue.

use crate::sumeragi::{
    v2::VerifiedHeightContext as DispatchVerifiedContext,
    v2_lane_transport::NativeTransportProgress as DispatchProgress,
    v2_runner::native_process::NativeRunnerProcess as DispatchProcess,
};
use iroha_data_model::block::lane_consensus::{
    LANE_MESSAGE_VERSION_V1 as DISPATCH_LANE_VERSION, LaneMessageEnvelopeV1 as DispatchEnvelope,
    LaneMessageV1 as DispatchMessage, LanePhaseV1 as DispatchPhase, LaneQcV1 as DispatchQc,
    LaneRoundV1 as DispatchRound, LaneSignatureShareV1 as DispatchShare,
    LaneVoteStatementV1 as DispatchStatement, LaneVoteV1 as DispatchVote,
};

type DispatchFrame = Arc<crate::sumeragi::message::BlockMessageWire>;

struct OrdinaryLaneDispatchFixture {
    state: Arc<State>,
    guard: Arc<crate::sumeragi::output_guard::ConsensusOutputGuard>,
    native: Option<DispatchProcess>,
    executor: crate::sumeragi::v2_effects::V2EffectExecutor<
        crate::sumeragi::v2_runtime::SerializedV2Runtime,
    >,
    services: crate::sumeragi::v2_worker::ProductionV2Services,
    global: DispatchVerifiedContext,
    ingress: Arc<crate::sumeragi::FairV2Ingress>,
    network: crate::IrohaNetwork,
    actor: Option<iroha_p2p::network::NetworkActorAdmissionTestFixture<crate::NetworkMessage>>,
    key: KeyPair,
    lane: crate::state::VerifiedLaneContext,
    prepare_qc: DispatchQc,
    expected_commit: DispatchVote,
    next_votes: Vec<(PeerId, DispatchEnvelope)>,
    seen: Vec<(PeerId, DispatchFrame)>,
    now: Instant,
    parent_hash: HashOf<BlockHeader>,
    initial_height: usize,
    initial_kura_count: usize,
    initial_snapshot: Hash,
    _directory: tempfile::TempDir,
}

impl OrdinaryLaneDispatchFixture {
    fn new(actor_capacity: usize) -> Self {
        use crate::sumeragi::{
            v2::{AdapterFingerprints, DeferredAdmissionOrdinalSource, SumeragiV2Adapter},
            v2_core::Generation,
            v2_runtime::{RuntimeLifecycleOrdinalSource, RuntimeQueueConfig, SerializedV2Runtime},
        };
        let (state, native_keys) = crate::state::native_dispatch_state_fixture();
        let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
        let lane = observed.contexts()[0].clone();
        let initial_height = state.committed_height();
        let kura = state.kura_handle();
        let initial_kura_count = kura.blocks_count();
        let (parent, receipt) = kura
            .v2_finality_artifact_with_receipt(u64::try_from(initial_height).unwrap())
            .unwrap()
            .unwrap();
        let parent_hash = parent.block_hash;
        assert_eq!(initial_kura_count, initial_height);
        assert_eq!(state.latest_block_hash_fast(), Some(parent_hash));
        let context = crate::sumeragi::v2_context::build_successor_height_context(
            &parent,
            parent.height_context.nexus_amx_context_hash,
            None,
        )
        .unwrap();
        let global = DispatchVerifiedContext::successor(
            context.clone(),
            parent.validator_set_pops.clone(),
            &parent,
            &receipt,
            &parent.validator_set_pops,
        )
        .unwrap();
        let local = lane
            .reducer_context()
            .roster()
            .iter()
            .position(|entry| entry.id() == lane.reducer_context().leader(0))
            .unwrap();
        let key_for = |signer: usize| {
            native_keys
                .iter()
                .find(|key| key.public_key() == lane.frozen().committee[signer].public_key())
                .unwrap()
                .clone()
        };
        let key = key_for(local);
        let local_peer = PeerId::new(key.public_key().clone());
        let crate::state::FirstLaneAdmittedInputReadV1::Ready(source) =
            state.first_lane_admitted_input(&observed, &lane).unwrap()
        else {
            panic!("real authenticated first carrier");
        };
        let crate::state::LaneInputBodyPreparationV1::Ready(body) = state
            .prepare_lane_input_body(&observed, &lane, &source)
            .unwrap()
        else {
            panic!("actual complete route input");
        };
        let manifest = *crate::sumeragi::v2_lane_payload::encode_lane_input(&lane, &body, 0)
            .unwrap()
            .manifest();
        assert_eq!(manifest.value.origin_producer as usize, local);
        let statement = DispatchStatement {
            round: DispatchRound {
                instance_id: manifest.value.instance_id,
                lane_height: lane.frozen().next_lane_height,
                voting_view: 0,
            },
            phase: DispatchPhase::Prepare,
            value: manifest.value,
        };
        let share = |statement: DispatchStatement, signer: usize| DispatchShare {
            signer: u32::try_from(signer).unwrap(),
            signature: iroha_crypto::Signature::try_new(
                key_for(signer).private_key(),
                &statement.signature_preimage().unwrap(),
            )
            .unwrap()
            .payload()
            .to_vec(),
        };
        let prepare_qc = DispatchQc {
            statement,
            shares: (0..3).map(|i| share(statement, i)).collect(),
        };
        let commit = DispatchStatement {
            phase: DispatchPhase::Commit,
            ..statement
        };
        let expected_commit = DispatchVote {
            statement: commit,
            share: share(commit, local),
        };
        let next_votes = (0..4)
            .filter(|i| *i != local)
            .take(2)
            .map(|signer| {
                (
                    lane.frozen().committee[signer].clone(),
                    DispatchEnvelope {
                        version: DISPATCH_LANE_VERSION,
                        message: DispatchMessage::Vote(DispatchVote {
                            statement,
                            share: share(statement, signer),
                        }),
                    },
                )
            })
            .collect();
        let guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        let directory = tempfile::TempDir::new().unwrap();
        // This node is a Native committee member and a global observer. The
        // ordinary global executor is real but receives no global voting input.
        let (adapter, startup) = SumeragiV2Adapter::open(
            directory.path().join("ordinary-native.wal"),
            global.clone(),
            None,
            Generation::INITIAL,
            [0x63; 32],
            AdapterFingerprints {
                node: Hash::new(b"native dispatch node"),
                build: Hash::new(b"native dispatch build"),
                config: Hash::new(b"native dispatch config"),
            },
            DeferredAdmissionOrdinalSource::new(0),
        )
        .unwrap();
        assert!(startup.is_empty());
        let now = Instant::now();
        let (runtime, startup) = SerializedV2Runtime::new_with_lifecycle_ordinals(
            adapter,
            startup,
            now,
            Duration::from_secs(600),
            RuntimeQueueConfig::default(),
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        )
        .unwrap();
        assert!(startup.is_empty());
        let executor =
            crate::sumeragi::v2_effects::V2EffectExecutor::ordinary_dispatch_executor_for_test(
                runtime,
                context.clone(),
                local_peer.clone(),
                None,
                Arc::clone(&guard),
            );
        let mut global_keys = (0xD3_u8..=0xD6)
            .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
            .collect::<Vec<_>>();
        global_keys.sort_by(|a, b| a.public_key().cmp(b.public_key()));
        assert!(
            global_keys
                .iter()
                .zip(&context.roster)
                .all(|(key, member)| key.public_key() == member.validator.public_key())
        );
        let mut services = crate::sumeragi::v2_worker::tests::ordinary_dispatch_services_for_test(
            Arc::clone(&kura),
            context.clone(),
            &global_keys,
            0,
            Arc::clone(&state),
            Arc::clone(&guard),
            executor.current_tag(),
        );
        let (network, actor) = crate::IrohaNetwork::actor_admission_for_tests(
            local_peer.clone(),
            lane.frozen()
                .committee
                .iter()
                .filter(|peer| *peer != &local_peer)
                .cloned()
                .collect(),
            NonZeroUsize::new(actor_capacity).unwrap(),
        );
        crate::sumeragi::v2_worker::tests::install_network_for_test(&mut services, network.clone());
        let capacity = crate::sumeragi::fair_v2_ingress_required_capacity(4, None).unwrap();
        let ingress = Arc::new(
            crate::sumeragi::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
                capacity,
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
                lane.frozen().committee.clone(),
                &context.network_id,
                context.da_layout,
            )
            .unwrap();
        ingress.open().unwrap();
        let mut config = iroha_config::parameters::actual::Sumeragi::default()
            .v2_config(Duration::from_secs(1), context.mode)
            .unwrap();
        config.limits.max_transactions = 1; // One actual physical lane owner in this bounded test.
        config.limits.control_queue_capacity = 1;
        let native = DispatchProcess::new(
            Arc::clone(&state),
            Arc::clone(&guard),
            local_peer,
            key.clone(),
            true,
            &config,
            8 * 1024 * 1024,
            Duration::from_secs(600),
            Duration::from_secs(60),
        )
        .unwrap();
        let initial_snapshot = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
        let mut fixture = Self {
            state,
            guard,
            native: Some(native),
            executor,
            services,
            global,
            ingress,
            network,
            actor: Some(actor),
            key,
            lane,
            prepare_qc,
            expected_commit,
            next_votes,
            seen: Vec::new(),
            now,
            parent_hash,
            initial_height,
            initial_kura_count,
            initial_snapshot,
            _directory: directory,
        };
        fixture.establish_ready_commit();
        fixture
    }

    fn full_poll(&mut self) -> Result<(), crate::sumeragi::v2_runner::V2RunnerError> {
        self.native
            .as_mut()
            .unwrap()
            .poll(&self.global, &self.network, self.now, &self.ingress)
    }

    fn service_ready(
        &mut self,
    ) -> Result<DispatchProgress, crate::sumeragi::v2_runner::V2RunnerError> {
        let observed = self
            .state
            .verified_lane_consensus_contexts()
            .unwrap()
            .unwrap();
        self.native
            .as_mut()
            .unwrap()
            .service_ready_output(&observed, &self.global, &self.network)
    }

    fn queued_ordinals(&self) -> Vec<u64> {
        let mut rows = self
            .ingress
            .state
            .lock()
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter().map(|entry| entry.admission_ordinal))
            .collect::<Vec<_>>();
        rows.sort_unstable();
        rows
    }

    fn enqueue(&self, sender: PeerId, envelope: DispatchEnvelope) -> u64 {
        assert!(matches!(
            self.ingress
                .try_push(InboundBlockMessage::from_authenticated_peer(
                    BlockMessage::NativeLane(envelope),
                    sender,
                )),
            Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
        ));
        self.ingress.state.lock().last_admission_ordinal
    }

    fn consume_prepare(&mut self) {
        use crate::sumeragi::v2_runner::ordinary_ingress_consumer::{
            PreparedDequeuedV2IngressV1, ProductionPreparedOrdinaryIngressConsumptionV1,
            consume_prepared_dequeued_v2_ingress,
        };
        let sender = self.next_votes[0].0.clone();
        self.enqueue(
            sender,
            DispatchEnvelope {
                version: DISPATCH_LANE_VERSION,
                message: DispatchMessage::QuorumCertificate(self.prepare_qc.clone()),
            },
        );
        let inbound = self.ingress.try_recv_if_checked(|_| true).unwrap().unwrap();
        assert!(
            matches!(inbound.message(), BlockMessage::NativeLane(DispatchEnvelope {
            message: DispatchMessage::QuorumCertificate(qc), .. }) if qc == &self.prepare_qc)
        );
        let ownership = inbound.ingress_ownership().expect("original fair carrier");
        assert!(ownership.validate_exact(), "dequeue preserves exact ownership");
        assert!(
            ownership.matches_message(inbound.message()),
            "dequeue preserves the exact canonical Native QC bytes"
        );
        assert!(
            ownership.matches_semantic_origin(inbound.sender()),
            "dequeue preserves the authenticated Native QC sender"
        );
        assert!(
            ownership.matches_reply_routes(inbound.reply_routes()),
            "dequeue preserves original authenticated reply routes"
        );
        let prepared = PreparedDequeuedV2IngressV1::new(
            Arc::clone(&self.ingress),
            inbound,
            crate::sumeragi::FairV2IngressDequeueDisposition::Admit,
            None,
            None,
            Arc::clone(&self.guard),
        );
        let context = self.global.context();
        let mut server =
            crate::sumeragi::v2_block_sync::V2BlockSyncServer::new(context.network_id, 16).unwrap();
        let mut discovery = crate::sumeragi::v2_block_sync::V2BlockSyncDiscovery::new(
            context.clone(),
            PeerId::new(self.key.public_key().clone()),
            16,
        )
        .unwrap();
        let mut request = None;
        let mut beacon = crate::sumeragi::v2_beacon::V2GlobalBeaconLifecycle::open(
            context,
            &self.state,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            consume_prepared_dequeued_v2_ingress(
                prepared,
                &self.ingress,
                &mut self.executor,
                &mut self.services,
                self.native.as_mut().unwrap(),
                self.state.kura(),
                &self.key,
                &mut server,
                &mut discovery,
                &mut request,
                &mut beacon
            )
            .unwrap(),
            ProductionPreparedOrdinaryIngressConsumptionV1::Continue
        );
    }

    fn establish_ready_commit(&mut self) {
        let until = Instant::now() + Duration::from_secs(30);
        let mut saw_prepare = false;
        let mut saw_proposal = false;
        while !saw_prepare {
            self.full_poll().unwrap();
            self.actor.as_mut().unwrap().drain_posts(|post| {
                let crate::NetworkMessage::SumeragiBlock(frame) = &post.data else {
                    panic!("Native output");
                };
                let BlockMessage::NativeLane(envelope) = frame.as_message() else {
                    panic!("Native control");
                };
                match &envelope.message {
                    DispatchMessage::Proposal(_) => saw_proposal = true,
                    DispatchMessage::Vote(vote) => {
                        assert_eq!(vote.statement, self.prepare_qc.statement);
                        saw_prepare = true;
                    }
                    other => panic!("unexpected initial Native output: {other:?}"),
                }
            });
            assert!(
                Instant::now() < until,
                "real body/WAL workers must produce local Prepare"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            saw_proposal,
            "the actual local body owner authored its native proposal"
        );
        // Drain the bounded initial fanouts before the Prepare QC is introduced.
        while !self.outputs().is_empty() {
            self.full_poll().unwrap();
            self.actor.as_mut().unwrap().drain_posts(|_| {});
            assert!(Instant::now() < until);
        }
        self.consume_prepare();
        loop {
            self.full_poll().unwrap();
            self.drain_actor();
            let pending = self.outputs();
            if !self.seen.is_empty()
                && !pending.is_empty()
                && pending
                    .iter()
                    .all(|(frame, _)| self.is_expected_commit(frame))
            {
                break;
            }
            assert!(
                Instant::now() < until,
                "full process polls must produce and deliver genuine Commit"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            self.native
                .as_mut()
                .unwrap()
                .driver_mut()
                .process()
                .occupancy()
                .instances,
            1
        );
        assert_eq!(
            self.seen.len(),
            1,
            "one genuine Commit reached the real actor during full process polling"
        );
        assert_eq!(
            self.outputs()
                .iter()
                .map(|(_, peers)| peers.len())
                .sum::<usize>(),
            2,
            "the exact remaining two Commit destinations belong to transport"
        );
        assert!(!self.guard.restart_required());
        self.assert_candidate_not_applied();
    }

    fn outputs(&self) -> Vec<(DispatchFrame, Vec<PeerId>)> {
        self.native
            .as_ref()
            .unwrap()
            .retained_transport_outputs_for_test()
    }

    fn is_expected_commit(&self, frame: &DispatchFrame) -> bool {
        matches!(frame.as_message(), BlockMessage::NativeLane(DispatchEnvelope {
            message: DispatchMessage::Vote(vote), .. }) if vote == &self.expected_commit)
    }

    fn drain_actor(&mut self) -> usize {
        let expected = &self.expected_commit;
        let prepare = &self.prepare_qc;
        let seen = &mut self.seen;
        self.actor.as_mut().unwrap().drain_posts(|post| {
            let crate::NetworkMessage::SumeragiBlock(frame) = &post.data else {
                panic!("Native output");
            };
            assert_eq!(post.priority, iroha_p2p::Priority::High);
            match frame.as_message() {
                BlockMessage::NativeLane(DispatchEnvelope {
                    message: DispatchMessage::Vote(vote),
                    ..
                }) if vote == expected => {
                    assert!(
                        !seen.iter().any(|(peer, _)| peer == &post.peer_id),
                        "no duplicate Commit destination"
                    );
                    if let Some((_, original)) = seen.first() {
                        assert!(Arc::ptr_eq(original, frame));
                    }
                    seen.push((post.peer_id.clone(), Arc::clone(frame)));
                }
                BlockMessage::NativeLane(DispatchEnvelope {
                    message: DispatchMessage::QuorumCertificate(qc),
                    ..
                }) if qc == prepare => {}
                other => panic!("changed phase, bytes or unrelated output: {other:?}"),
            }
        })
    }

    fn finish_outputs(&mut self) {
        for _ in 0..16 {
            self.drain_actor();
            if self.outputs().is_empty() {
                break;
            }
            self.service_ready().unwrap();
        }
        self.drain_actor();
        assert!(self.outputs().is_empty(), "finite exact fanout drained");
        let actual = self
            .seen
            .iter()
            .map(|(peer, _)| peer.clone())
            .collect::<BTreeSet<_>>();
        let expected = self
            .lane
            .frozen()
            .committee
            .iter()
            .filter(|peer| peer.public_key() != self.key.public_key())
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual, expected,
            "one genuine local Commit reaches every remote actor target"
        );
        assert!(!self.guard.restart_required());
        self.assert_candidate_not_applied();
    }

    fn assert_candidate_not_applied(&mut self) {
        assert_eq!(self.state.kura().blocks_count(), self.initial_kura_count);
        assert_eq!(self.state.committed_height(), self.initial_height);
        assert_eq!(self.state.latest_block_hash_fast(), Some(self.parent_hash));
        assert_eq!(
            self.state
                .committed_block_hash_at_height(self.global.context().height),
            None
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&self.state).unwrap(),
            self.initial_snapshot,
            "transport cannot economically apply the authenticated input"
        );
        let owner = self
            .native
            .as_mut()
            .unwrap()
            .driver_mut()
            .process()
            .instance(self.lane.instance_id())
            .unwrap();
        assert!(
            owner
                .native_records()
                .iter()
                .any(|record| matches!(&record.record,
            crate::sumeragi::v2_lane_wire::LaneWalRecordV1::LockAndCommit { statement, .. }
                if statement == &self.expected_commit.statement)),
            "the real fsynced Commit intent remains owned"
        );
        assert!(
            owner.native_decision().unwrap().is_none(),
            "no Commit quorum or Apply was manufactured"
        );
    }
}

impl Drop for OrdinaryLaneDispatchFixture {
    fn drop(&mut self) {
        if let Some(native) = self.native.take() {
            native.shutdown().join().unwrap();
        }
    }
}

#[test]
fn ordinary_lane_consumer_admits_bounded_commit_before_next_physical_ingress() {
    let mut fixture = OrdinaryLaneDispatchFixture::new(16);
    let (sender, message) = fixture.next_votes[0].clone();
    let second = fixture.enqueue(sender, message);
    let (sender, message) = fixture.next_votes[1].clone();
    let third = fixture.enqueue(sender, message);
    assert!(third > second);
    assert!(matches!(
        fixture.service_ready().unwrap(),
        DispatchProgress::Admitted { .. }
    ));
    assert_eq!(fixture.queued_ordinals(), vec![second, third]);
    assert_eq!(
        fixture.drain_actor(),
        1,
        "one actual post, not just a queued effect, precedes the next dequeue"
    );
    fixture.full_poll().unwrap();
    assert_eq!(
        fixture.queued_ordinals(),
        vec![third],
        "one full turn selects at most one fresh Native occurrence"
    );
    fixture.finish_outputs();
    fixture.full_poll().unwrap();
    assert!(fixture.queued_ordinals().is_empty());
    fixture.assert_candidate_not_applied();
}

#[test]
fn ordinary_lane_consumer_retains_exact_commit_under_real_actor_backpressure() {
    let mut fixture = OrdinaryLaneDispatchFixture::new(1);
    let (sender, message) = fixture.next_votes[0].clone();
    let second = fixture.enqueue(sender, message);
    assert!(matches!(
        fixture.service_ready().unwrap(),
        DispatchProgress::Admitted { .. }
    ));
    let retained = fixture.outputs();
    assert!(
        !retained.is_empty(),
        "capacity-one actor cannot absorb the remaining fanout"
    );
    for _ in 0..3 {
        assert!(matches!(
            fixture.service_ready().unwrap(),
            DispatchProgress::Backpressured { rank: 1, .. }
        ));
        let actual = fixture.outputs();
        assert_eq!(actual.len(), retained.len());
        for ((frame, peers), (original, original_peers)) in actual.iter().zip(&retained) {
            assert!(
                Arc::ptr_eq(frame, original),
                "retry retains the exact original frame allocation"
            );
            assert_eq!(
                peers.iter().collect::<BTreeSet<_>>(),
                original_peers.iter().collect()
            );
        }
        assert_eq!(fixture.queued_ordinals(), vec![second]);
        assert!(!fixture.guard.restart_required());
    }
    // A full turn still admits fresh Native work under actor pressure; it does
    // not wait for network capacity or the asynchronous body/WAL workers.
    fixture.full_poll().unwrap();
    assert!(fixture.queued_ordinals().is_empty());
    assert_eq!(
        fixture.drain_actor(),
        1,
        "the real capacity-one actor was saturated"
    );
    fixture.finish_outputs();
}

#[test]
fn ordinary_lane_consumer_actor_closure_fails_stop_before_next_ingress() {
    let mut fixture = OrdinaryLaneDispatchFixture::new(1);
    let (sender, message) = fixture.next_votes[0].clone();
    let second = fixture.enqueue(sender, message);
    let before = fixture.outputs();
    assert!(!before.is_empty());
    drop(fixture.actor.take().unwrap());
    let error = fixture
        .full_poll()
        .expect_err("real actor closure must fail before fresh physical selection");
    assert!(
        matches!(error, crate::sumeragi::v2_runner::V2RunnerError::Service(ref message)
        if message.contains("native output actor closed")),
        "{error:?}"
    );
    assert!(fixture.guard.restart_required());
    assert!(fixture.guard.acquire().is_none());
    assert_eq!(fixture.queued_ordinals(), vec![second]);
    let after = fixture.outputs();
    assert_eq!(after.len(), before.len());
    for ((frame, peers), (original, original_peers)) in after.iter().zip(&before) {
        assert!(Arc::ptr_eq(frame, original));
        assert_eq!(
            peers.iter().collect::<BTreeSet<_>>(),
            original_peers.iter().collect::<BTreeSet<_>>(),
            "actor refusal retains the original source and every unfinished destination"
        );
    }
    fixture.assert_candidate_not_applied();
}
