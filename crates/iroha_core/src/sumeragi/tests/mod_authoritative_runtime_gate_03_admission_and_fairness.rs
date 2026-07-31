    #[test]
    fn full_ingress_does_not_persist_a_carrierless_leader_wire_barrier() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
        let validator = PeerId::new(KeyPair::random().public_key().clone());
        let layout = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::Plain,
            chunk_size_bytes: 1,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 1,
            max_chunk_count: 1,
        };
        let proposal_message = v2_maximum_structural_proposal_wire(layout, 1);
        let BlockMessage::V2(proposal_envelope) = &proposal_message else {
            unreachable!("proposal fixture is a v2 envelope");
        };
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &proposal_envelope.payload else {
            unreachable!("proposal fixture carries Proposal");
        };
        let _directory = bind_test_leader_wire_gate(&ingress, &validator, proposal.round, 1);

        let mut occurrence = 0_u64;
        loop {
            let request = InboundBlockMessage::new(
                v2_commit_certificate_request(occurrence, &validator),
                Some(validator.clone()),
            );
            match ingress.try_push(request) {
                Ok(super::FairV2IngressPushDisposition::Enqueued) => {
                    occurrence = occurrence
                        .checked_add(1)
                        .expect("bounded ingress fills before u64 exhaustion");
                }
                Err(super::FairV2IngressPushError::Full(_)) => break,
                _ => panic!("unexpected filler admission result"),
            }
        }
        assert_ne!(occurrence, 0, "the test must materialize a physical prefix");

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                proposal_message.clone(),
                Some(validator.clone()),
            )),
            Err(super::FairV2IngressPushError::Full(_))
        ));
        assert!(
            ingress.state.lock().leader_wire_lifecycles.is_empty(),
            "ordinary backpressure must not leave a durable off-queue barrier"
        );

        while ingress.try_recv_if(|_| true).is_some() {}
        assert!(
            matches!(
                ingress.try_push(InboundBlockMessage::new(
                    v2_commit_certificate_request(occurrence, &validator),
                    Some(validator.clone()),
                )),
                Ok(super::FairV2IngressPushDisposition::Enqueued)
            ),
            "unrelated traffic must remain admissible after the rejected packet disappears"
        );
        assert!(ingress.try_recv_if(|_| true).is_some());
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(proposal_message, Some(validator),)),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert_eq!(
            ingress.state.lock().leader_wire_lifecycles.len(),
            1,
            "the exact lifecycle begins only with its physically owned carrier"
        );
    }

    #[test]
    fn delayed_proposal_keeps_first_chunk_lossless_without_a_global_orphan_barrier() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
        ingress.close();

        let validator = PeerId::new(KeyPair::random().public_key().clone());
        ingress
            .configure_roster([validator.clone()])
            .expect("one-validator fair-ingress geometry");
        ingress.require_leader_wire_lifecycle_gate();
        ingress.state.lock().leader_wire_max_chunk_count = 1;

        let layout = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::Plain,
            chunk_size_bytes: 1,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 1,
            max_chunk_count: 1,
        };
        let proposal_message = v2_maximum_structural_proposal_wire(layout, 1);
        let BlockMessage::V2(proposal_envelope) = &proposal_message else {
            unreachable!("proposal fixture is a v2 envelope");
        };
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &proposal_envelope.payload else {
            unreachable!("proposal fixture carries Proposal");
        };
        let manifest_hash = HashOf::new(&proposal.manifest);
        let round = proposal.round;

        let directory = TempDir::new().expect("temporary leader-wire directory");
        let wal_path = directory.path().join("safety.wal");
        let owner = [0xA5; 32];
        let roster = [validator.clone()].into_iter().collect();
        let capacity =
            super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(1, 1)
                .expect("finite leader-wire geometry");
        let recovery_authority =
            super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
                round.context_id,
                round.height,
                owner,
                0,
                false,
            );
        let (gate, restore) = super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
            &wal_path,
            round.context_id,
            round.height,
            owner,
            roster,
            capacity,
            1,
            recovery_authority,
            &[],
            &[],
        )
        .expect("open exact leader-wire gate");
        ingress
            .bind_leader_wire_lifecycle_gate(
                gate,
                restore,
                super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(0),
                round.context_id,
                round.height,
            )
            .expect("bind exact leader-wire gate");
        ingress.open().expect("open bound fair ingress");

        let chunk_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
                manifest_hash,
                index: 0,
                bytes: vec![0x5A],
                sender: 0,
                signature: vec![0xC3],
            }),
        ));
        assert!(
            matches!(
                ingress.try_push(InboundBlockMessage::new(
                    chunk_message.clone(),
                    Some(validator.clone()),
                )),
                Ok(super::FairV2IngressPushDisposition::Enqueued)
            ),
            "a chunk reordered before Proposal must reach the bounded worker orphan lifecycle"
        );
        assert!(
            matches!(
                ingress.try_push(InboundBlockMessage::new(
                    chunk_message.clone(),
                    Some(validator.clone()),
                )),
                Ok(super::FairV2IngressPushDisposition::Coalesced)
            ),
            "an exact physical retransmission must retain one ingress owner"
        );
        assert!(
            ingress.state.lock().leader_wire_lifecycles.is_empty(),
            "an unbound chunk must not mint a Byzantine-pinnable global scheduler owner"
        );

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                proposal_message,
                Some(validator.clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(
            matches!(
                ingress.try_push(InboundBlockMessage::new(chunk_message, Some(validator),)),
                Ok(super::FairV2IngressPushDisposition::Coalesced)
            ),
            "global exact-wire coalescing must run before the now-bindable chunk can mint a rank"
        );
        assert!(
            ingress
                .state
                .lock()
                .leader_wire_lifecycles
                .values()
                .all(|record| {
                    record.token.identity.phase != super::FairV2IngressLeaderWirePhase::Chunk
                }),
            "a Proposal must not retrofit a durable lifecycle onto the already queued proofless chunk"
        );
        let chunk = ingress
            .try_recv_if(|_| true)
            .expect("the frozen physical predecessor chunk drains first");
        assert_eq!(payload_chunk_index(&chunk), Some(0));
        assert!(
            chunk
                .ingress_ownership()
                .is_some_and(|ownership| ownership.leader_wire_token().is_none()),
            "the proofless orphan episode retains fair ownership without exact rank ownership"
        );
        let proposal = ingress
            .try_recv_if(|_| true)
            .expect("the exact Proposal drains after its frozen predecessor");
        assert!(matches!(
            proposal.message(),
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Proposal(_),
                ..
            })
        ));
        assert!(
            proposal
                .ingress_ownership()
                .is_some_and(|ownership| ownership.leader_wire_token().is_some()),
            "manifest-bound Proposal begins the durable exact lifecycle"
        );
    }

    #[test]
    fn ingress_stays_closed_until_replay_owner_acknowledges_ready() {
        let (handle, receiver, _relay_receiver) = test_sumeragi_handle(1);
        handle.ingress_ready.store(false, Ordering::Release);

        assert!(!handle.incoming_block_message(v2_message()));
        assert!(receiver.try_recv().is_none());

        handle.ingress_ready.store(true, Ordering::Release);
        assert!(handle.incoming_block_message(v2_message()));
        assert!(receiver.try_recv().is_some());
    }

    #[test]
    fn retired_global_v1_messages_never_enter_live_queues() {
        let (handle, receiver, _relay_receiver) = test_sumeragi_handle(1);
        handle.ingress_ready.store(true, Ordering::Release);

        assert!(!handle.incoming_block_message(BlockMessage::invalid_wire_sentinel()));
        assert!(receiver.try_recv().is_none());
    }

    #[test]
    fn first_release_vrf_frames_are_decode_only_and_never_enter_live_queues() {
        let (handle, receiver, _relay_receiver) = test_sumeragi_handle(1);
        handle.ingress_ready.store(true, Ordering::Release);
        let commit = BlockMessage::VrfCommit(super::consensus::VrfCommit {
            epoch: 4,
            commitment: [0xA5; 32],
            signer: 0,
            bls_sig: vec![0x5A],
        });
        let reveal = BlockMessage::VrfReveal(super::consensus::VrfReveal {
            epoch: 4,
            reveal: [0xA6; 32],
            signer: 0,
            bls_sig: vec![0x5B],
        });

        assert!(!handle.incoming_block_message(commit));
        assert!(!handle.incoming_block_message(reveal));
        assert!(receiver.try_recv().is_none());
    }

    #[test]
    fn authenticated_lane_drain_votes_enter_the_bounded_live_relay_queue() {
        let (handle, _receiver, relay_receiver) = test_sumeragi_handle(1);
        handle.ingress_ready.store(true, Ordering::Release);
        let keypair = KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::BlsNormal)
            .expect("generate BLS-normal lane-drain signer");
        let signer = PeerId::new(keypair.public_key().clone());
        let validator_set = vec![signer.clone()];
        let vote = crate::lane_consensus::LaneDrainVoteV1::new_signed(
            LaneDrainCertificateBodyV1 {
                version: 1,
                intent: LaneDrainIntentV1 {
                    version: 1,
                    chain_id_digest: Hash::new(b"live-drain-ingress-chain"),
                    lane_id: LaneId::new(7),
                    dataspace_id: DataSpaceId::new(9),
                    lane_incarnation: Hash::new(b"live-drain-ingress-incarnation"),
                    close_global_height: 3,
                    initial_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                        LaneId::new(7),
                        DataSpaceId::new(9),
                        Hash::new(b"live-drain-ingress-incarnation"),
                        0,
                        None,
                    ),
                    validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash: HashOf::new(&validator_set),
                    validator_set,
                    validator_count: 1,
                    min_quorum: 1,
                },
                final_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                    LaneId::new(7),
                    DataSpaceId::new(9),
                    Hash::new(b"live-drain-ingress-incarnation"),
                    0,
                    None,
                ),
            },
            signer.clone(),
            keypair.private_key(),
        )
        .expect("sign valid lane-drain vote");

        assert!(handle.try_incoming_lane_drain_vote(signer.clone(), vote.clone()));
        let LaneRelayMessage::DrainVote {
            sender,
            vote: queued_vote,
        } = relay_receiver
            .try_recv()
            .expect("valid drain vote reaches the bounded relay queue")
        else {
            panic!("valid drain vote changed relay message kind");
        };
        assert_eq!(sender, signer);
        assert_eq!(queued_vote, vote);

        let mismatched_sender = PeerId::new(KeyPair::random().public_key().clone());
        assert!(!handle.try_incoming_lane_drain_vote(mismatched_sender, vote.clone()));
        assert!(relay_receiver.try_recv().is_err());

        let mut tampered = vote;
        tampered.bls_signature[0] ^= 0x01;
        assert!(!handle.try_incoming_lane_drain_vote(signer, tampered));
        assert!(relay_receiver.try_recv().is_err());
    }

    #[test]
    fn v2_ingress_is_bounded_and_never_blocks_a_network_caller() {
        let (handle, receiver, _relay_receiver) = test_sumeragi_handle(1);
        handle.ingress_ready.store(true, Ordering::Release);

        assert!(handle.incoming_block_message(v2_message()));
        assert!(
            !handle.incoming_block_message(v2_auxiliary_prepare(1)),
            "a distinct message at saturated capacity must reject promptly and rely on retransmission"
        );
        let _ = receiver.try_recv().expect("drain the bounded v2 queue");
        assert!(handle.incoming_block_message(v2_message()));
    }

    #[test]
    fn saturated_v2_ingress_returns_the_exact_owned_message_for_retry() {
        let (handle, receiver, _relay_receiver) =
            test_sumeragi_handle_with_source_geometry(3, Some(1));
        let sender = validator_peers(1).pop().expect("sender fixture");

        assert!(matches!(
            handle.try_incoming_block_message_from_owned(sender.clone(), v2_message()),
            super::SumeragiIngressDisposition::Accepted
        ));
        let retry =
            handle.try_incoming_block_message_from_owned(sender.clone(), v2_auxiliary_prepare(1));
        let super::SumeragiIngressDisposition::Retry(inbound) = retry else {
            panic!("saturated ingress must return caller ownership");
        };
        assert_eq!(inbound.sender(), Some(&sender));
        assert_eq!(vote_height(&inbound), Some(2));

        let _ = receiver
            .try_recv()
            .expect("release bounded ingress capacity");
        assert!(matches!(
            handle.try_incoming_block_message_owned(inbound),
            super::SumeragiIngressDisposition::Accepted
        ));
    }

    #[test]
    fn direct_and_synthetic_envelopes_keep_identity_roles_consistent() {
        let sender = validator_peers(1).pop().expect("sender fixture");
        let direct = InboundBlockMessage::new(v2_message(), Some(sender.clone()));
        assert_eq!(direct.sender(), Some(&sender));
        assert_eq!(direct.via(), Some(&sender));

        let synthetic = InboundBlockMessage::new(v2_message(), None);
        assert!(synthetic.sender().is_none());
        assert!(synthetic.via().is_none());
    }

    #[test]
    fn atomic_lane_certificate_uses_the_shared_progress_owner() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(1);
        let certificate = lane_block_certificate(71);
        let expected = certificate.encode();

        assert_eq!(
            FairV2IngressClass::classify(&InboundBlockMessage::new(certificate.clone(), None,)),
            FairV2IngressClass::Progress
        );
        assert!(matches!(
            handle.try_incoming_block_message_owned(InboundBlockMessage::new(certificate, None)),
            super::SumeragiIngressDisposition::Accepted
        ));
        let retained = ingress
            .try_recv()
            .expect("shared fair ingress retains the lane certificate");
        assert_eq!(retained.message().encode(), expected);
    }

    #[test]
    fn oversized_atomic_lane_certificate_is_returned_exactly() {
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(1);
        let mut certificate = lane_block_certificate(72);
        let BlockMessage::LaneBlockCertificate(envelope) = &mut certificate else {
            unreachable!("fixture is an atomic lane certificate")
        };
        envelope.commit_qc.bls_aggregate_signature =
            vec![0xA5; super::MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES];
        let expected = certificate.encode();
        assert!(expected.len() > super::MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES);

        let disposition =
            handle.try_incoming_block_message_owned(InboundBlockMessage::new(certificate, None));
        let super::SumeragiIngressDisposition::Rejected(retained) = disposition else {
            panic!("oversized lane certificate must be rejected with exact ownership")
        };
        assert_eq!(retained.message().encode(), expected);
        assert!(ingress.try_recv().is_none());
    }

    #[test]
    fn saturated_lane_ingress_returns_the_exact_owned_message_for_retry() {
        let (handle, _receiver, relay_receiver) = test_sumeragi_handle(1);
        let first = MergeCommitteeSignature {
            version: iroha_data_model::merge::MERGE_COMMITTEE_SIGNATURE_VERSION_V2,
            epoch_id: 7,
            view: 1,
            signer: 0,
            message_digest: Hash::new(b"first retained lane item"),
            bls_sig: vec![0xA5],
            leader_candidate_body: None,
        };
        let second = MergeCommitteeSignature {
            version: iroha_data_model::merge::MERGE_COMMITTEE_SIGNATURE_VERSION_V2,
            epoch_id: 7,
            view: 2,
            signer: 0,
            message_digest: Hash::new(b"second retained lane item"),
            bls_sig: vec![0x5A],
            leader_candidate_body: None,
        };

        assert!(matches!(
            handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::MergeSignature(first)),
            super::SumeragiIngressDisposition::Accepted
        ));
        let retry =
            handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::MergeSignature(second));
        let super::SumeragiIngressDisposition::Retry(message) = retry else {
            panic!("saturated lane ingress must return caller ownership");
        };
        let super::LaneRelayMessage::MergeSignature(retained) = &message else {
            panic!("retry must preserve the exact lane message variant");
        };
        assert_eq!(retained.view, 2);
        assert_eq!(retained.bls_sig, vec![0x5A]);

        let _ = relay_receiver
            .try_recv()
            .expect("release bounded lane ingress capacity");
        assert!(matches!(
            handle.try_incoming_lane_relay_owned(message),
            super::SumeragiIngressDisposition::Accepted
        ));
    }

    #[test]
    fn sidecar_allocations_require_roster_requester_before_lane_queue_admission() {
        use std::num::NonZeroU64;

        use crate::merge_sidecar::{
            CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarCloseV1,
            CertifiedMergeSidecarMessage, CertifiedMergeSidecarRequestV1,
            CertifiedMergeSidecarSemanticSequenceV1, CertifiedMergeSidecarServiceGenerationV1,
            CertifiedMergeSidecarStreamEpochV1,
        };

        let ingress_capacity = super::fair_v2_ingress_required_capacity(1, None)
            .expect("one-validator ingress geometry is representable");
        assert_eq!(ingress_capacity, 6);
        let (handle, ingress, relay_receiver) = test_sumeragi_handle(ingress_capacity);
        let mut peers = validator_peers(3);
        let roster_requester = peers.remove(0);
        let outsider = peers.remove(0);
        let hub = peers.remove(0);
        ingress.close();
        ingress
            .configure_roster([roster_requester.clone()])
            .expect("one frozen sidecar requester fits the ingress geometry");
        ingress.open().expect("open the frozen sidecar roster");

        let request_for = |requester: &PeerId| {
            let mut request = CertifiedMergeSidecarRequestV1 {
                version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
                service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
                stream_epoch: CertifiedMergeSidecarStreamEpochV1(NonZeroU64::MIN),
                semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1(NonZeroU64::MIN),
                closed_through: 0,
                request_id: Hash::prehashed([0; Hash::LENGTH]),
                entry_hash: HashOf::<MergeLedgerEntry>::from_untyped_unchecked(Hash::new(
                    b"early sidecar roster gate",
                )),
                encoded_len: 1,
                epoch_id: 1,
                reference_digest: Hash::new(b"early sidecar roster reference"),
                requester: requester.clone(),
                responder: roster_requester.clone(),
            };
            request.request_id = request.canonical_request_id();
            request
        };
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub.clone(), 4);

        let outsider_request = request_for(&outsider);
        let outsider_route = routes.mint_via(outsider.clone(), hub.clone());
        let rejected =
            handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::CertifiedMergeSidecar {
                sender: outsider.clone(),
                reply_route: Some(outsider_route),
                message: CertifiedMergeSidecarMessage::Request(outsider_request.clone()),
            });
        assert!(matches!(
            rejected,
            super::SumeragiIngressDisposition::Rejected(
                super::LaneRelayMessage::CertifiedMergeSidecar {
                    sender,
                    message: CertifiedMergeSidecarMessage::Request(request),
                    ..
                }
            ) if sender == outsider && request == outsider_request
        ));
        assert!(
            matches!(
                relay_receiver.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ),
            "an outsider Request must allocate no lane-relay slot"
        );

        let mut outsider_close = CertifiedMergeSidecarCloseV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
            stream_epoch: CertifiedMergeSidecarStreamEpochV1(NonZeroU64::MIN),
            closed_through: 1,
            close_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: outsider.clone(),
            responder: roster_requester.clone(),
        };
        outsider_close.close_id = outsider_close.canonical_close_id();
        let outsider_close_route = routes.mint_via(outsider.clone(), hub.clone());
        assert!(matches!(
            handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::CertifiedMergeSidecar {
                sender: outsider.clone(),
                reply_route: Some(outsider_close_route),
                message: CertifiedMergeSidecarMessage::Close(outsider_close),
            },),
            super::SumeragiIngressDisposition::Rejected(_)
        ));
        assert!(
            matches!(
                relay_receiver.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ),
            "an outsider standalone Close must allocate no lane-relay slot"
        );

        let mismatched_request = request_for(&outsider);
        let roster_route = routes.mint_via(roster_requester.clone(), hub.clone());
        assert!(matches!(
            handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::CertifiedMergeSidecar {
                sender: roster_requester.clone(),
                reply_route: Some(roster_route),
                message: CertifiedMergeSidecarMessage::Request(mismatched_request),
            },),
            super::SumeragiIngressDisposition::Rejected(_)
        ));
        assert!(
            matches!(
                relay_receiver.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ),
            "a roster transport identity cannot allocate for another semantic requester"
        );

        let roster_request = request_for(&roster_requester);
        let roster_route = routes.mint_via(roster_requester.clone(), hub.clone());
        assert!(matches!(
            handle.try_incoming_lane_relay_owned(super::LaneRelayMessage::CertifiedMergeSidecar {
                sender: roster_requester.clone(),
                reply_route: Some(roster_route),
                message: CertifiedMergeSidecarMessage::Request(roster_request.clone()),
            },),
            super::SumeragiIngressDisposition::Accepted
        ));
        assert!(matches!(
            relay_receiver
                .try_recv()
                .expect("a roster requester may use an authenticated non-roster relay"),
            super::LaneRelayMessage::CertifiedMergeSidecar {
                sender,
                reply_route: Some(route),
                message: CertifiedMergeSidecarMessage::Request(request),
            } if sender == roster_requester
                && request == roster_request
                && route.is_authenticated_via(&hub)
        ));
    }

    #[test]
    fn restart_required_ingress_rejects_before_queue_mutation() {
        let (handle, receiver, _relay_receiver) = test_sumeragi_handle(1);
        handle.output_guard.activate_restart_required();

        assert!(handle.restart_required());
        assert!(!handle.incoming_block_message(v2_message()));
        assert!(
            receiver.try_recv().is_none(),
            "restart-required admission must not mutate the bounded ingress queue"
        );
    }

    fn validator_peers(count: u8) -> Vec<PeerId> {
        (0..count)
            .map(|seed| {
                PeerId::new(
                    KeyPair::try_from_seed(
                        vec![seed.saturating_add(1); 32],
                        iroha_crypto::Algorithm::Ed25519,
                    )
                    .expect("derive deterministic ingress peer")
                    .public_key()
                    .clone(),
                )
            })
            .collect()
    }

    #[test]
    fn byzantine_v2_source_cannot_consume_honest_ingress_reservations_or_service_turns() {
        // The exact N=4, H=2 corridor needs 22 slots. Add one deliberate
        // ordinary-pressure slot so this test can retain two attacker items
        // while still proving that a third cannot consume any protected slot.
        let (handle, ingress, _relay_receiver) =
            test_sumeragi_handle_with_source_geometry(23, Some(2));
        let validators = validator_peers(4);
        let attacker = validators[0].clone();
        let outsider = validator_peers(5).pop().expect("outsider fixture");
        ingress.close();
        ingress
            .configure_roster(validators.clone())
            .expect("four validators, their progress and TimeoutVote slots, and anonymous fit");
        ingress.open().expect("open configured roster");

        for index in 0..2 {
            assert!(
                handle.try_incoming_block_message_from(
                    attacker.clone(),
                    v2_auxiliary_prepare(index),
                )
            );
        }
        assert!(
            !handle.try_incoming_block_message_from(attacker.clone(), v2_auxiliary_prepare(2),),
            "attacker cannot consume ordinary, progress, or TimeoutVote slots reserved for empty validator lanes"
        );
        for honest in validators.iter().skip(1) {
            assert!(handle.try_incoming_block_message_from(honest.clone(), v2_message()));
        }
        assert!(handle.try_incoming_block_message_from(outsider.clone(), v2_message()));
        assert_eq!(ingress.len(), 6);

        let first_cycle = (0..5)
            .map(|_| {
                ingress
                    .try_recv()
                    .expect("one ready source per fair service turn")
                    .into_message_and_sender()
                    .1
            })
            .collect::<Vec<_>>();
        assert_eq!(
            first_cycle,
            vec![
                Some(attacker),
                Some(validators[1].clone()),
                Some(validators[2].clone()),
                Some(validators[3].clone()),
                Some(outsider),
            ]
        );
        assert_eq!(ingress.len(), 1, "only the attacker's second item remains");
    }

    #[test]
    fn relayed_origin_churn_uses_one_via_lane_and_preserves_protocol_origin() {
        const RELAYED_ORIGINS: usize = 32;
        let (handle, ingress, _relay_receiver) = test_sumeragi_handle(19);
        let validators = validator_peers(4);
        let via = validators[0].clone();
        let lane_origin = validators[1].clone();
        let origins = validator_peers(64)
            .into_iter()
            .skip(validators.len())
            .take(RELAYED_ORIGINS)
            .collect::<Vec<_>>();
        ingress.close();
        ingress
            .configure_roster(validators.clone())
            .expect("four validator owners and the anonymous owner fit");
        ingress.open().expect("open configured roster");

        let mut accepted = 0_usize;
        for (index, origin) in origins.iter().enumerate() {
            let inbound = InboundBlockMessage::from_transport(
                v2_auxiliary_prepare(u64::try_from(index).expect("fixture index fits u64")),
                origin.clone(),
                via.clone(),
            );
            match handle.try_incoming_block_message_owned(inbound) {
                super::SumeragiIngressDisposition::Accepted => accepted += 1,
                super::SumeragiIngressDisposition::Retry(retained) => {
                    assert_eq!(retained.sender(), Some(origin));
                    assert_eq!(retained.via(), Some(&via));
                }
                disposition => panic!("unexpected relayed-origin disposition: {disposition:?}"),
            }
        }
        assert_eq!(
            accepted, 2,
            "semantic-origin churn must remain inside one validator lane instead of multiplying its reserved slots"
        );
        {
            let state = ingress.state.lock();
            let nonempty = state
                .lanes
                .iter()
                .filter(|(_, lane)| !lane.entries.is_empty())
                .map(|(source, _)| source.clone())
                .collect::<Vec<_>>();
            assert_eq!(
                nonempty,
                vec![super::FairV2IngressSource::Validator(via.clone())]
            );
            assert_eq!(
                state.ready,
                std::collections::VecDeque::from([nonempty[0].clone()])
            );
        }

        assert!(
            handle.try_incoming_block_message_from(validators[2].clone(), v2_message()),
            "one relayed via cannot consume a responsive validator's reserved owner"
        );
        let first = ingress
            .try_recv()
            .expect("oldest relayed origin owns the via's first fair turn");
        assert_eq!(first.sender(), Some(&origins[0]));
        assert_eq!(first.via(), Some(&via));
        let responsive = ingress
            .try_recv()
            .expect("responsive validator follows after one via turn");
        assert_eq!(responsive.sender(), Some(&validators[2]));
        let second = ingress
            .try_recv()
            .expect("the via retains its second admitted origin");
        assert_eq!(second.sender(), Some(&origins[1]));
        assert!(ingress.try_recv().is_none());

        assert!(matches!(
            handle.try_incoming_block_message_owned(InboundBlockMessage::from_transport(
                lane_block_certificate(73),
                lane_origin.clone(),
                via.clone(),
            )),
            super::SumeragiIngressDisposition::Accepted
        ));
        let inbound = ingress
            .try_recv()
            .expect("relayed lane certificate reaches serialized validation");
        assert_eq!(inbound.sender(), Some(&lane_origin));
        assert_eq!(inbound.via(), Some(&via));
        let (message, sender) = inbound.into_message_and_sender();
        assert_eq!(sender, Some(lane_origin));
        assert!(matches!(message, BlockMessage::LaneBlockCertificate(_)));
    }

