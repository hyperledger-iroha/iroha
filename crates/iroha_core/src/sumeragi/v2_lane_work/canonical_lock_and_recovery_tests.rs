    #[test]
    fn decided_mixed_carrier_accepts_canonical_successor_while_local_sidecars_lag() {
        let (mut parent, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let autonomous_lane_id = LaneId::new(1);
        let autonomous_dataspace_id = DataSpaceId::new(7);
        enable_multilane_nexus(
            &mut parent,
            &keys,
            autonomous_lane_id,
            autonomous_dataspace_id,
        );
        let (parent_block, parent_proposal) = globally_anchored_lane_block_fixture(&parent, &keys);
        parent
            .kura
            .store_block(parent_block.clone())
            .expect("persist exact raw lane predecessor");
        let parent_finality = verified_finality_artifact_for_block(&parent, &keys, &parent_block);
        let _ = parent
            .kura
            .store_v2_finality_artifact(&parent_finality)
            .expect("persist raw predecessor finality authority");
        let committed_parent = ValidBlock::committed_from_replay_signed_block(parent_block.clone());
        commit_test_block_to_state(parent.state.as_ref(), &committed_parent, &parent.context);
        assert_eq!(
            parent
                .state
                .unapplied_lane_block_artifact_heights_snapshot_cached()
                .get(&(
                    parent_proposal.descriptor.lane_id,
                    parent_proposal.descriptor.dataspace_id,
                )),
            Some(&parent_proposal.descriptor.lane_block_height),
            "the regression requires a canonical predecessor whose independent lane sidecars are still pending"
        );
        assert!(
            !parent
                .kura
                .lane_block_application_receipt_available(&parent_proposal),
            "the raw canonical predecessor must not already have an application receipt"
        );
        let successor_context = successor_context_for_parent(&parent, &parent_block);
        let local_peer = parent.local_peer.clone();
        let local_key = parent.key_pair.clone();
        let state = Arc::clone(&parent.state);
        let kura = Arc::clone(&parent.kura);
        let limits = parent.limits;
        drop(parent);
        let mut successor = V2LaneWorkAdapter::new(
            successor_context,
            local_peer,
            local_key,
            true,
            Arc::clone(&state),
            Arc::clone(&kura),
            limits,
            None,
        )
        .expect("open successor while predecessor sidecars remain pending");
        let (autonomous_source_block, mut autonomous_proposal) =
            planned_lane_candidate_block_for_route_at_view(
                &successor,
                &keys,
                0,
                autonomous_lane_id,
                autonomous_dataspace_id,
            );
        autonomous_proposal.payload_block_hint = None;
        let autonomous_entrypoint = autonomous_source_block
            .external_entrypoints_cloned()
            .next()
            .expect("autonomous mixed-carrier entrypoint");
        let autonomous_accepted = crate::tx::AcceptedTransaction::new_unchecked_entrypoint(
            std::borrow::Cow::Owned(autonomous_entrypoint.clone()),
        );
        let autonomous_routing_plan = RoutingPlan::single(RoutingDecision::new(
            autonomous_lane_id,
            autonomous_dataspace_id,
        ));
        let mut autonomous_reservation = crate::queue::LaneQueueReservationKeyV2 {
            version: crate::queue::LaneQueueReservationKeyV2::VERSION,
            signed_transaction_hash: autonomous_accepted.hash(),
            entrypoint_hash: autonomous_entrypoint.hash(),
            queue_plan_admission_binding_hash: Hash::new(
                b"mixed-raw-successor-queue-plan-admission-binding",
            ),
            routing_plan_digest: autonomous_routing_plan.digest(),
            coordinator_leg: autonomous_routing_plan.coordinator_leg(),
            lane_id: autonomous_lane_id,
            dataspace_id: autonomous_dataspace_id,
            lane_incarnation: autonomous_proposal.descriptor.lane_incarnation,
            proposal_height: autonomous_proposal.descriptor.proposal_height,
            lane_block_height: autonomous_proposal.descriptor.lane_block_height,
            lane_block_view: autonomous_proposal.descriptor.lane_block_view,
            reservation_owner_hash: Hash::new(b"mixed-raw-successor-reservation-owner"),
            proposal_identity_hash: autonomous_proposal.proposal_hash,
        };
        let autonomous_producer = successor
            .expected_autonomous_lane_author(&autonomous_proposal)
            .expect("deterministic mixed-carrier autonomous producer")
            .clone();
        bind_canonical_autonomous_reservation_identity(
            &successor,
            &autonomous_proposal,
            &autonomous_producer,
            &mut autonomous_reservation,
        );
        let autonomous_producer_key = keys
            .iter()
            .find(|key| key.public_key() == autonomous_producer.public_key())
            .expect("mixed-carrier fixture contains its autonomous producer");
        let autonomous_payload = LaneExecutablePayloadV1::new_signed_with_reservations(
            successor.native_network_id(),
            successor.context.epoch,
            autonomous_proposal.clone(),
            vec![autonomous_entrypoint],
            vec![autonomous_reservation],
            vec![autonomous_routing_plan],
            vec![None],
            autonomous_producer.clone(),
            autonomous_producer_key.private_key(),
        )
        .expect("construct exact autonomous mixed-carrier payload");
        successor
            .kura
            .persist_lane_executable_payload(
                &autonomous_payload,
                successor.native_network_id(),
                successor.context.epoch,
            )
            .expect("persist autonomous mixed-carrier payload");
        assert_eq!(
            successor.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneExecutablePayload(autonomous_payload.clone()),
                    Some(autonomous_producer),
                ),
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
        let autonomous_envelope = autonomous_lane_payload_envelope(
            &autonomous_payload,
            successor.native_network_id(),
            successor.context.epoch,
        )
        .expect("encode exact autonomous mixed-carrier envelope");
        let mut malformed_autonomous_envelope = autonomous_envelope.clone();
        malformed_autonomous_envelope.proposal_hash =
            Hash::new(b"malformed mixed-carrier autonomous proposal");
        assert!(
            decode_autonomous_lane_payload_envelope(
                &malformed_autonomous_envelope,
                successor.native_network_id(),
                successor.context.epoch,
            )
            .is_err(),
            "the unchanged autonomous envelope validator must reject a malformed mixed-carrier member"
        );
        let transaction_key = KeyPair::try_from_seed(vec![0xD4; 32], Algorithm::Ed25519)
            .expect("deterministic successor transaction key");
        let transaction = TransactionBuilder::new(
            successor.context.network_id,
            AccountId::new(transaction_key.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .sign(transaction_key.private_key());
        let entrypoint_hash = transaction.hash_as_entrypoint();
        let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        let global_view = 0;
        let leader_index = usize::try_from(successor.context.leader(global_view))
            .expect("successor leader index fits usize");
        let global_leader = &successor.context.roster[leader_index].validator;
        let strict = prepare_v2_lane_payload_plan(
            state.as_ref(),
            kura.as_ref(),
            &successor.context,
            global_view,
            global_leader,
            std::slice::from_ref(&route),
            std::slice::from_ref(&Hash::from(entrypoint_hash)),
        )
        .expect("strict producer planning remains deterministic");
        assert_eq!(
            strict.unavailable_indices,
            BTreeSet::from([0]),
            "fresh local production must remain blocked on the missing predecessor sidecars"
        );
        let recovered = prepare_v2_lane_payload_validation_plan(
            state.as_ref(),
            kura.as_ref(),
            &successor.context,
            global_view,
            global_leader,
            std::slice::from_ref(&route),
            std::slice::from_ref(&Hash::from(entrypoint_hash)),
        )
        .expect("derive received ownership from the exact canonical predecessor");
        assert!(recovered.unavailable_indices.is_empty());
        assert_eq!(recovered.ownerships.len(), 1);
        assert_eq!(
            recovered.ownerships[0].previous_lane_block_height,
            parent_proposal.descriptor.lane_block_height
        );
        assert_eq!(
            recovered.ownerships[0].previous_lane_block_descriptor_hash,
            Some(parent_proposal.descriptor.descriptor_hash)
        );
        let header = BlockHeader::new(
            NonZeroU64::new(successor.context.height).expect("non-zero successor height"),
            Some(parent_block.hash()),
            None,
            None,
            successor.context.height,
            global_view,
        );
        let mut builder = BlockBuilder::new(header);
        builder.push_transaction(transaction);
        builder.set_execution_context(Some(
            BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                entrypoint_hash,
                route.lane_id,
                route.dataspace_id,
            )])
            .with_lane_payload_ownerships(recovered.ownerships)
            .with_autonomous_lane_payloads(vec![autonomous_envelope]),
        ));
        let successor_block = builder
            .build_with_signature(
                u64::try_from(leader_index).expect("successor leader index fits u64"),
                keys[leader_index].private_key(),
            )
            .canonical_resultless_proposal();
        let successor_ownership = successor_block
            .execution_context()
            .expect("successor execution context")
            .lane_payload_ownerships[0]
            .clone();
        let successor_proposal =
            proposal_from_ownership(&successor_ownership, successor_block.hash())
                .expect("reconstruct exact raw successor proposal");
        let mut wrong_raw_successor = successor_proposal.clone();
        wrong_raw_successor
            .descriptor
            .previous_lane_block_descriptor_hash =
            Some(Hash::new(b"wrong canonical raw predecessor descriptor"));
        assert!(
            !canonical_raw_lane_predecessor_matches_proposal(
                successor.state.as_ref(),
                successor.kura.as_ref(),
                &wrong_raw_successor,
            ),
            "raw fallback must reject a mismatched predecessor descriptor"
        );
        let (locked_round, locked_subject) = global_lock_for_block(&successor, &successor_block);
        assert_eq!(
            successor.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_ne!(
            successor.bind_locked_global_body(&successor_block),
            V2LaneIngressOutcome::Rejected,
            "an exact PrepareQC-locked successor must not kill a validator merely because its independent predecessor sidecars are catching up"
        );
        assert!(!successor.proposal_can_progress(&successor_proposal));
        assert!(
            successor
                .lane_sessions
                .local_vote_rebroadcast_artifacts_for(&successor.local_peer)
                .iter()
                .all(|(proposal, _)| proposal != &successor_proposal),
            "raw predecessor authentication must not authorize a local successor vote"
        );
        let _ = successor.drain_effects(usize::MAX);
        successor
            .kura
            .store_block(successor_block.clone())
            .expect("persist exact raw successor carrier");
        let successor_finality =
            verified_finality_artifact_for_block(&successor, &keys, &successor_block);
        let successor_receipt = KuraV2CommitReceipt::for_test(&successor_finality);
        let committed_successor =
            ValidBlock::committed_from_replay_signed_block(successor_block.clone());
        commit_test_block_to_state(
            successor.state.as_ref(),
            &committed_successor,
            &successor.context,
        );
        assert!(
            !canonical_v2_lane_payload_matches_kura(
                successor.state.as_ref(),
                successor.kura.as_ref(),
                &successor.context,
                &successor_block,
            ),
            "the strict canonical matcher must retain applied-predecessor semantics"
        );
        assert!(canonical_v2_lane_payload_matches_kura_inner(
            successor.state.as_ref(),
            successor.kura.as_ref(),
            &successor.context,
            &successor_block,
            true,
        ));
        successor
            .retain_merge_sidecars_for_global_view(
                locked_round.view,
                Some(locked_subject),
                Some(locked_subject),
            )
            .expect("install exact raw-successor Decision");
        assert_ne!(
            successor
                .recover_decided_canonical_lane_body(&successor_receipt, &successor_finality,)
                .expect("recover decided carrier over exact raw predecessor"),
            V2LaneIngressOutcome::Rejected
        );
        assert!(
            successor
                .lane_sessions
                .proposals_without_commit_qc()
                .contains(&parent_proposal),
            "decided recovery hydrates the oldest exact raw predecessor"
        );
        assert!(
            successor
                .lane_sessions
                .proposals_without_commit_qc()
                .contains(&successor_proposal),
            "decided recovery retains the exact raw successor"
        );
        let successor_session = CommittedLaneBlockSession {
            proposal: successor_proposal.clone(),
            prepare_qc: lane_qc_for_phase(&successor_proposal, &keys[..3], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&successor_proposal, &keys[..3], CertPhase::Commit),
        };
        successor
            .pending_committed_lanes
            .push_back(successor_session.clone());
        assert_eq!(
            successor
                .persist_anchored_sessions()
                .expect("defer successor certificate behind raw predecessor"),
            0
        );
        assert!(
            successor
                .kura
                .read_certified_lane_block_artifact(
                    successor_proposal.descriptor.lane_id,
                    successor_proposal.descriptor.lane_block_height,
                )
                .is_none(),
            "the successor certificate must not become durable before its predecessor receipt"
        );
        assert!(
            !successor
                .kura
                .lane_block_application_receipt_available(&successor_proposal)
        );
        successor
            .schedule_retransmission()
            .expect("solicit raw predecessor certificate without consensus progress");
        let effects = successor.drain_effects(usize::MAX);
        assert!(effects.iter().any(|effect| {
            matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockProposal(proposal),
                    ..
                } if proposal == &parent_proposal
            )
        }));
        assert!(effects.iter().all(|effect| match effect {
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockVote(vote),
                ..
            } => vote.body.proposal_hash != successor_proposal.proposal_hash,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockQc(qc),
                ..
            } => qc.body.proposal_hash != successor_proposal.proposal_hash,
            V2LaneWorkEffect::PostDurableLaneCertificate { certificate, .. } => {
                certificate.proposal.proposal_hash != successor_proposal.proposal_hash
            }
            _ => true,
        }));
        let parent_certificate = LaneBlockCertificateV1 {
            proposal: parent_proposal.clone(),
            prepare_qc: lane_qc_for_phase(&parent_proposal, &keys[..3], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&parent_proposal, &keys[..3], CertPhase::Commit),
        };
        assert_eq!(
            successor.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockCertificate(Box::new(parent_certificate)),
                    Some(PeerId::new(keys[1].public_key().clone())),
                ),
                locked_round.view,
            ),
            V2LaneIngressOutcome::Inserted
        );
        assert!(matches!(
            successor
                .service_next_historical_recovery()
                .expect("persist exact predecessor certificate and receipt"),
            HistoricalRecoveryServiceOutcome::Complete(_)
        ));
        assert!(
            successor
                .kura
                .lane_block_application_receipt_available(&parent_proposal)
        );
        assert!(successor.proposal_can_progress(&successor_proposal));
        assert!(
            successor
                .lane_sessions
                .local_vote_rebroadcast_artifacts_for(&successor.local_peer)
                .iter()
                .any(|(proposal, _)| proposal == &successor_proposal),
            "predecessor receipt completion wakes the retained successor session"
        );
        let resumed_effects = successor.drain_effects(usize::MAX);
        assert!(
            resumed_effects.iter().any(|effect| {
                matches!(
                    effect,
                    V2LaneWorkEffect::PostLaneBlock {
                        message: BlockMessage::LaneBlockVote(vote),
                        ..
                    } if vote.body == successor_proposal.vote_body(CertPhase::Prepare)
                )
            }),
            "the unblocked exact H2 Prepare vote must cross the decided-carrier fanout gate"
        );
        assert_eq!(
            successor
                .persist_anchored_sessions()
                .expect("persist unblocked successor certificate and receipt"),
            1
        );
        assert!(
            successor
                .kura
                .lane_block_application_receipt_available(&successor_proposal)
        );
    }
    #[test]
    fn cold_restart_hydrates_two_link_raw_lane_chain_without_receipts() {
        let (first, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (first_block, first_proposal) = globally_anchored_lane_block_fixture(&first, &keys);
        first
            .kura
            .store_block(first_block.clone())
            .expect("persist first raw lane artifact");
        let committed_first = ValidBlock::committed_from_replay_signed_block(first_block.clone());
        commit_test_block_to_state(first.state.as_ref(), &committed_first, &first.context);
        assert!(
            first
                .kura
                .read_lane_block_application_receipt_without_sidecar_repair(
                    first_proposal.descriptor.lane_id,
                    first_proposal.descriptor.lane_block_height,
                )
                .is_none(),
            "the first raw artifact must not gain an application receipt"
        );
        let second_context = successor_context_for_parent(&first, &first_block);
        let local_peer = first.local_peer.clone();
        let local_key = first.key_pair.clone();
        let state = Arc::clone(&first.state);
        let kura = Arc::clone(&first.kura);
        let mut limits = first.limits;
        limits.session_capacity = NonZeroUsize::new(2).expect("two-link hydration bound");
        drop(first);
        let second = V2LaneWorkAdapter::new(
            second_context,
            local_peer.clone(),
            local_key.clone(),
            true,
            Arc::clone(&state),
            Arc::clone(&kura),
            limits,
            None,
        )
        .expect("open second height over an unreceipted raw predecessor");
        let transaction_key = KeyPair::try_from_seed(vec![0xD5; 32], Algorithm::Ed25519)
            .expect("deterministic second-link transaction key");
        let transaction = TransactionBuilder::new(
            second.context.network_id,
            AccountId::new(transaction_key.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .sign(transaction_key.private_key());
        let entrypoint_hash = transaction.hash_as_entrypoint();
        let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        let global_view = 0;
        let leader_index = usize::try_from(second.context.leader(global_view))
            .expect("second-link leader index fits usize");
        let global_leader = &second.context.roster[leader_index].validator;
        let strict = prepare_v2_lane_payload_plan(
            state.as_ref(),
            kura.as_ref(),
            &second.context,
            global_view,
            global_leader,
            std::slice::from_ref(&route),
            std::slice::from_ref(&Hash::from(entrypoint_hash)),
        )
        .expect("strict second-link producer planning remains deterministic");
        assert_eq!(strict.unavailable_indices, BTreeSet::from([0]));
        let recovered = prepare_v2_lane_payload_validation_plan(
            state.as_ref(),
            kura.as_ref(),
            &second.context,
            global_view,
            global_leader,
            std::slice::from_ref(&route),
            std::slice::from_ref(&Hash::from(entrypoint_hash)),
        )
        .expect("recover second-link ownership from the exact raw predecessor");
        assert!(recovered.unavailable_indices.is_empty());
        assert_eq!(recovered.ownerships.len(), 1);
        assert_eq!(
            recovered.ownerships[0].previous_lane_block_height,
            first_proposal.descriptor.lane_block_height
        );
        assert_eq!(
            recovered.ownerships[0].previous_lane_block_descriptor_hash,
            Some(first_proposal.descriptor.descriptor_hash)
        );
        let header = BlockHeader::new(
            NonZeroU64::new(second.context.height).expect("non-zero second-link height"),
            Some(first_block.hash()),
            None,
            None,
            second.context.height,
            global_view,
        );
        let mut builder = BlockBuilder::new(header);
        builder.push_transaction(transaction);
        builder.set_execution_context(Some(
            BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                entrypoint_hash,
                route.lane_id,
                route.dataspace_id,
            )])
            .with_lane_payload_ownerships(recovered.ownerships),
        ));
        let second_block = builder
            .build_with_signature(
                u64::try_from(leader_index).expect("second-link leader index fits u64"),
                keys[leader_index].private_key(),
            )
            .canonical_resultless_proposal();
        let second_ownership = second_block
            .execution_context()
            .expect("second-link block carries execution context")
            .lane_payload_ownerships[0]
            .clone();
        let second_proposal = proposal_from_ownership(&second_ownership, second_block.hash())
            .expect("reconstruct exact second raw proposal");
        second
            .kura
            .store_block(second_block.clone())
            .expect("persist second raw lane artifact");
        let committed_second = ValidBlock::committed_from_replay_signed_block(second_block.clone());
        commit_test_block_to_state(second.state.as_ref(), &committed_second, &second.context);
        assert!(canonical_v2_lane_payload_matches_kura_inner(
            second.state.as_ref(),
            second.kura.as_ref(),
            &second.context,
            &second_block,
            true,
        ));
        assert!(canonical_raw_lane_predecessor_matches_proposal(
            second.state.as_ref(),
            second.kura.as_ref(),
            &second_proposal,
        ));
        assert_eq!(
            second
                .state
                .unapplied_lane_block_artifact_heights_snapshot_cached()
                .get(&(route.lane_id, route.dataspace_id)),
            Some(&second_proposal.descriptor.lane_block_height)
        );
        for proposal in [&first_proposal, &second_proposal] {
            assert!(
                second
                    .kura
                    .read_lane_block_application_receipt_without_sidecar_repair(
                        proposal.descriptor.lane_id,
                        proposal.descriptor.lane_block_height,
                    )
                    .is_none(),
                "neither raw link may gain an application receipt before restart"
            );
        }
        let third_context = successor_context_for_parent(&second, &second_block);
        drop(second);
        let third = V2LaneWorkAdapter::new(
            third_context,
            local_peer,
            local_key,
            true,
            state,
            kura,
            limits,
            None,
        )
        .expect("cold-open the third height over a two-link raw chain");
        assert!(
            !third.output_guard.restart_required() && third.output_guard.acquire().is_some(),
            "bounded raw-chain hydration must leave authoritative admission healthy"
        );
        assert_eq!(
            third.lane_sessions.proposals_without_commit_qc(),
            vec![first_proposal.clone(), second_proposal.clone()],
            "cold hydration must reconstruct the exact raw chain oldest first"
        );
        for proposal in [&first_proposal, &second_proposal] {
            assert!(third.canonical_anchor_for_proposal(proposal).is_some());
            assert!(third.historical_raw_proposal_can_solicit_certificate(proposal));
            assert!(!third.proposal_can_progress(proposal));
            assert!(
                third
                    .kura
                    .read_lane_block_application_receipt_without_sidecar_repair(
                        proposal.descriptor.lane_id,
                        proposal.descriptor.lane_block_height,
                    )
                    .is_none()
            );
        }
        assert!(
            third
                .lane_sessions
                .local_vote_rebroadcast_artifacts_for(&third.local_peer)
                .iter()
                .all(|(proposal, _)| {
                    proposal != &first_proposal && proposal != &second_proposal
                }),
            "cold hydration must not mint votes for either unreceipted raw link"
        );
    }
    #[test]
    fn canonical_kura_recovery_accepts_global_view_one_with_fresh_lane_view() {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let global_view = 1;
        let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, global_view);
        assert_eq!(block.header().view_change_index(), global_view);
        assert_eq!(proposal.descriptor.lane_block_view, 0);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist planner-produced canonical recovery body");
        assert!(canonical_v2_lane_payload_matches_kura(
            adapter.state.as_ref(),
            adapter.kura.as_ref(),
            &adapter.context,
            &block,
        ));
        assert!(
            adapter.canonical_anchor_for_proposal(&proposal).is_some(),
            "the exact ownership/header global view must authenticate the lane-local proposal"
        );
    }
    #[test]
    fn canonical_kura_recovery_rejects_nonzero_planner_origin_lane_view() {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (planned, _) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let mut ownership = planned
            .execution_context()
            .expect("planned block carries its execution context")
            .lane_payload_ownerships[0]
            .clone();
        ownership.lane_block_view = 1;
        let replay = ownership
            .compute_replay_hashes()
            .expect("nonzero lane-view ownership replay material recomputes");
        ownership.subject_hash = replay.subject_hash;
        ownership.payload_ownership_hash = replay.payload_ownership_hash;
        ownership.rbc_instance_hash = replay.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
        ownership
            .validate_replay_material()
            .expect("nonzero lane-view fixture must not rely on stale replay hashes");
        let leader_index = usize::try_from(adapter.context.leader(0)).expect("leader index");
        let block = test_block(1, None, Some(ownership), &keys[leader_index]);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist adversarial nonzero lane-view body");
        assert!(
            !canonical_v2_lane_payload_matches_kura(
                adapter.state.as_ref(),
                adapter.kura.as_ref(),
                &adapter.context,
                &block,
            ),
            "canonical recovery must enforce the planner-origin lane-view invariant"
        );
    }
    #[test]
    fn lane_work_stays_quiescent_until_the_exact_global_prepare_lock() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let later_view =
            u64::try_from(adapter.context.roster.len()).expect("fixture roster length fits u64");
        assert_eq!(
            adapter.context.leader(0),
            adapter.context.leader(later_view)
        );
        let (block_zero, proposal_at_view_zero) =
            planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let round_zero = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: adapter.context.height,
            view: 0,
        };
        adapter
            .planned_lane_proposals
            .insert(round_zero, vec![proposal_at_view_zero.clone()]);
        assert_eq!(
            adapter.bind_local_candidate(round_zero, block_zero.hash()),
            V2LaneIngressOutcome::Inserted
        );
        adapter
            .schedule_retransmission()
            .expect("schedule pre-lock retransmission");
        assert!(
            adapter.drain_effects(usize::MAX).is_empty(),
            "local Prepare intent must not leak lane proposals or votes before PrepareQC"
        );
        assert!(adapter.lane_sessions.commit_vote_lock_slots().is_empty());
        let (later_block, proposal_at_later_view) =
            planned_lane_candidate_block_at_view(&adapter, &keys, later_view);
        assert_ne!(
            proposal_at_view_zero.proposal_hash,
            proposal_at_later_view.proposal_hash
        );
        assert_eq!(proposal_at_later_view.descriptor.lane_block_view, 0);
        assert_eq!(
            proposal_at_later_view
                .payload_block_hint
                .expect("replanned proposal carries its global block hint")
                .proposal_view,
            later_view,
            "a full global-leader rotation must not advance the fresh lane-local view"
        );
        let later_round = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: adapter.context.height,
            view: later_view,
        };
        adapter
            .planned_lane_proposals
            .insert(later_round, vec![proposal_at_later_view.clone()]);
        assert_eq!(
            adapter.bind_local_candidate(later_round, later_block.hash()),
            V2LaneIngressOutcome::Inserted,
            "a later global view must remain free to replan before any PrepareQC lock"
        );
        assert_eq!(
            adapter.bind_locked_global_body(&block_zero),
            V2LaneIngressOutcome::Rejected,
            "a validated body alone is insufficient without the reducer lock"
        );
        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &later_block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_eq!(
            adapter.bind_locked_global_body(&block_zero),
            V2LaneIngressOutcome::Rejected,
            "a stale body must not satisfy the exact locked subject"
        );
        adapter
            .schedule_retransmission()
            .expect("schedule locked-body retransmission");
        assert!(
            adapter.drain_effects(usize::MAX).is_empty(),
            "the lock without its exact durable body must not release lane work"
        );
        assert_ne!(
            adapter.bind_locked_global_body(&later_block),
            V2LaneIngressOutcome::Rejected
        );
        let effects = adapter.drain_effects(usize::MAX);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockProposal(proposal),
                ..
            } if proposal.proposal_hash == proposal_at_later_view.proposal_hash
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockVote(vote),
                ..
            } if vote.body.proposal_hash == proposal_at_later_view.proposal_hash
        )));
        assert!(!effects.iter().any(|effect| matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockProposal(proposal),
                ..
            } if proposal.proposal_hash == proposal_at_view_zero.proposal_hash
        )));
    }
    #[test]
    fn global_body_lock_replacement_requires_higher_prepare_round_and_exact_subject() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (block, losing_proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        let (_, subject_a) = global_lock_for_block(&adapter, &block);
        let block_hash = subject_a.block_hash;
        let subject_a = wire::BlockSubject {
            payload_hash: Hash::new(b"global lock payload A"),
            ..subject_a
        };
        let subject_b = wire::BlockSubject {
            payload_hash: Hash::new(b"global lock payload B"),
            ..subject_a
        };
        let context_id = adapter.context.id();
        let height = adapter.context.height;
        let round = |view| wire::ConsensusRound {
            context_id,
            height,
            view,
        };
        assert_eq!(
            adapter
                .lane_sessions
                .insert_proposal(losing_proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            adapter.mark_global_body_locked(round(0), subject_a),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_eq!(
            adapter.mark_global_body_locked(round(0), subject_a),
            Ok(GlobalBodyLockOutcome::Duplicate)
        );
        assert!(matches!(
            adapter.mark_global_body_locked(round(0), subject_b),
            Err(V2LaneWorkError::ConflictingGlobalBodyLock)
        ));
        assert_eq!(
            adapter.globally_locked_body,
            Some(GlobalBodyLock {
                round: round(0),
                subject: subject_a,
            })
        );
        adapter
            .pending_local_lane_proposals
            .insert(block_hash, Vec::new());
        adapter.locally_bound_lane_proposals.insert(
            Hash::new(b"losing local lane proposal"),
            LaneBlockProposalPayloadHintV1 {
                proposal_height: adapter.context.height,
                proposal_view: 0,
                proposal_block_hash: block_hash,
            },
        );
        assert_eq!(
            adapter.mark_global_body_locked(round(1), subject_b),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_eq!(
            adapter.globally_locked_body,
            Some(GlobalBodyLock {
                round: round(1),
                subject: subject_b,
            }),
            "same block hash with different payload is a distinct higher lock"
        );
        assert!(adapter.pending_local_lane_proposals.is_empty());
        assert!(adapter.locally_bound_lane_proposals.is_empty());
        assert!(
            !adapter.lane_sessions.contains_proposal(&losing_proposal),
            "uncommitted lane sessions for the superseded carrier must release capacity"
        );
        assert!(matches!(
            adapter.mark_global_body_locked(round(0), subject_a),
            Err(V2LaneWorkError::ConflictingGlobalBodyLock)
        ));
        assert_eq!(
            adapter.globally_locked_body.map(|lock| lock.subject),
            Some(subject_b),
            "a lower lock cannot restore the retired exact subject"
        );
    }
    #[test]
    fn superseded_commit_protected_lane_session_cannot_retransmit() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_ne!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected
        );
        assert_eq!(
            adapter.lane_sessions.insert_qc_with_pops(
                lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
                &lane_signer_pops(&keys),
            ),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        let commit_vote = signed_lane_vote(&proposal, CertPhase::Commit, &keys[0]);
        assert_eq!(
            adapter.lane_sessions.insert_vote(commit_vote, None),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        let replacement_round = wire::ConsensusRound {
            view: locked_round.view + 1,
            ..locked_round
        };
        let replacement_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"replacement global carrier block hash",
            )),
            payload_hash: Hash::new(b"replacement global carrier payload"),
            ..locked_subject
        };
        assert_eq!(
            adapter.mark_global_body_locked(replacement_round, replacement_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert!(
            adapter.lane_sessions.contains_proposal(&proposal),
            "Commit evidence remains cached as safety state"
        );
        adapter
            .schedule_retransmission()
            .expect("schedule after replacing the exact global lock");
        let effects = adapter.drain_effects(usize::MAX);
        assert!(
            !effects.iter().any(|effect| matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockProposal(candidate),
                    ..
                } if candidate.proposal_hash == proposal.proposal_hash
            ) || matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockVote(vote),
                    ..
                } if vote.body.proposal_hash == proposal.proposal_hash
            ) || matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockQc(qc),
                    ..
                } if qc.body.proposal_hash == proposal.proposal_hash
            )),
            "safety-retained state for the losing carrier must not remain live traffic"
        );
    }
    #[test]
    fn decision_cleanup_fairly_reconstructs_completed_commit_qc_fanout() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_ne!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected
        );
        let _ = adapter.drain_effects(usize::MAX);
        adapter.limits.effect_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
        assert_eq!(
            adapter.insert_lane_qc(
                lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
                locked_round.view,
            ),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(
            adapter.insert_lane_qc(
                lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
                locked_round.view,
            ),
            V2LaneIngressOutcome::Inserted
        );
        adapter.drive_lane_sessions();
        assert!(adapter.has_pending_committed_output_handoff());
        adapter
            .retain_merge_sidecars_for_global_view(
                locked_round.view,
                Some(locked_subject),
                Some(locked_subject),
            )
            .expect("install decided carrier state");
        let expected = proposal
            .descriptor
            .validator_set
            .iter()
            .filter(|peer| *peer != &adapter.local_peer)
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut observed = BTreeSet::new();
        for _ in 0..=expected.len() {
            adapter
                .schedule_retransmission()
                .expect("reconstruct the next final CommitQC destination");
            for effect in adapter.drain_effects(1) {
                match effect {
                    V2LaneWorkEffect::PostLaneBlock {
                        peer,
                        message: BlockMessage::LaneBlockQc(qc),
                    } => {
                        assert_eq!(qc.body.phase, CertPhase::Commit);
                        assert_eq!(qc.body.proposal_hash, proposal.proposal_hash);
                        assert!(
                            observed.insert(peer),
                            "destination must transfer exactly once"
                        );
                    }
                    other => panic!("decision cleanup retained non-final lane output: {other:?}"),
                }
            }
            if !adapter.has_pending_committed_output_handoff() {
                break;
            }
        }
        assert_eq!(observed, expected);
        assert!(!adapter.has_pending_committed_output_handoff());
    }
    #[test]
    fn durable_lane_certificate_is_one_atomic_kura_backed_response() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (_, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        let session = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
        };
        let pops = adapter.pops_for_lane_session(&session);
        adapter
            .kura
            .persist_committed_lane_block_session(&session, &pops)
            .expect("persist the authoritative recovery source");
        adapter.effects.clear();
        adapter.effect_keys.clear();
        adapter.lane_sessions = LaneBlockSessionCache::new(adapter.limits.session_capacity.get());
        let requester = session
            .commit_qc
            .validator_set
            .iter()
            .find(|peer| *peer != &adapter.local_peer)
            .cloned()
            .expect("fixture has a remote committee member");
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    Some(requester.clone()),
                ),
                0,
            ),
            V2LaneIngressOutcome::Rejected,
            "a durable response must never fall back to topology without an exact request route"
        );
        assert!(adapter.drain_effects(usize::MAX).is_empty());
        let relay = PeerId::new(
            KeyPair::try_from_seed(vec![0xD6; 32], Algorithm::BlsNormal)
                .expect("relay key")
                .public_key()
                .clone(),
        );
        assert_ne!(relay, requester);
        let mut routes = NetworkReplyRouteTestFixture::new(relay.clone());
        let cancelled_route = routes.mint(requester.clone());
        assert!(routes.retire(&cancelled_route));
        assert!(matches!(
            InboundBlockMessage::try_from_transport_with_reply_route(
                BlockMessage::LaneBlockProposal(proposal.clone()),
                requester.clone(),
                relay.clone(),
                cancelled_route,
            ),
            Err(NetworkReplyRouteError::Inactive)
        ));
        assert!(adapter.drain_effects(usize::MAX).is_empty());
        let reply_route = routes.mint(requester.clone());
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::try_from_transport_with_reply_route(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    requester.clone(),
                    relay,
                    reply_route.clone(),
                )
                .expect("active durable request route"),
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
        let effects = adapter.drain_effects(usize::MAX);
        assert_eq!(
            effects.len(),
            1,
            "Prepare and Commit must cross one owner boundary"
        );
        assert!(matches!(
            &effects[0],
            V2LaneWorkEffect::PostDurableLaneCertificate {
                peer,
                reply_routes: Some(emitted_routes),
                certificate,
                ..
            } if peer == &requester
                && emitted_routes.len() == 1
                && emitted_routes
                    .iter()
                    .any(|emitted_route| emitted_route.same_delivery(&reply_route))
                && certificate.proposal == session.proposal
                && certificate.prepare_qc == session.prepare_qc
                && certificate.commit_qc == session.commit_qc
        ));
    }
    #[test]
    fn durable_lane_certificate_coalescing_preserves_alternate_ingress_owners() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (_, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        let session = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
        };
        let pops = adapter.pops_for_lane_session(&session);
        adapter
            .kura
            .persist_committed_lane_block_session(&session, &pops)
            .expect("persist the authoritative recovery source");
        adapter.effects.clear();
        adapter.effect_keys.clear();
        adapter.lane_sessions = LaneBlockSessionCache::new(adapter.limits.session_capacity.get());
        let requester = session
            .commit_qc
            .validator_set
            .iter()
            .find(|peer| *peer != &adapter.local_peer)
            .cloned()
            .expect("fixture has a remote committee member");
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture =
            NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
        let route_a = route_fixture.mint_via(requester.clone(), hub_a.clone());
        let route_b = route_fixture.mint_via(requester.clone(), hub_b.clone());
        let admitted = |via: PeerId, route: NetworkReplyRoute| {
            fair_v2_ingress_admit_for_test(
                InboundBlockMessage::try_from_transport_with_reply_route(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    requester.clone(),
                    via,
                    route,
                )
                .expect("durable request route is exact"),
            )
        };
        assert_eq!(
            adapter.accept_lane_message_with_ingress_ownership(admitted(hub_a, route_a.clone()), 0),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(
            adapter.accept_lane_message_with_ingress_ownership(admitted(hub_b, route_b.clone()), 0),
            V2LaneIngressOutcome::Inserted
        );
        let effect = adapter
            .drain_effects(1)
            .pop()
            .expect("one coalesced durable response");
        let V2LaneWorkEffect::PostDurableLaneCertificate {
            reply_routes: Some(reply_routes),
            ingress_ownership: Some(ownership),
            certificate,
            ..
        } = effect
        else {
            panic!("durable response retains routes and fair ownership")
        };
        assert_eq!(
            certificate,
            LaneBlockCertificateV1 {
                proposal: session.proposal.clone(),
                prepare_qc: session.prepare_qc.clone(),
                commit_qc: session.commit_qc.clone(),
            }
        );
        assert_eq!(reply_routes.len(), 2);
        assert!(
            reply_routes
                .iter()
                .any(|route| route.same_delivery(&route_a))
        );
        assert!(
            reply_routes
                .iter()
                .any(|route| route.same_delivery(&route_b))
        );
        assert!(ownership.validate_exact());
        assert_eq!(ownership.admission_count, 2);
        assert!(ownership.matches_reply_routes(Some(&reply_routes)));
        let hub_c = PeerId::new(KeyPair::random().public_key().clone());
        let hub_d = PeerId::new(KeyPair::random().public_key().clone());
        let route_c = route_fixture.mint_via(requester.clone(), hub_c.clone());
        let route_d = route_fixture.mint_via(requester.clone(), hub_d.clone());
        let race_effect = |via: PeerId, route: NetworkReplyRoute| {
            let mut inbound = admitted(via, route);
            let ingress_ownership = inbound
                .take_ingress_ownership()
                .expect("fair admission attaches exact ownership");
            let (_, peer, reply_routes) = inbound.into_message_sender_and_reply_routes();
            V2LaneWorkEffect::PostDurableLaneCertificate {
                peer: peer.expect("transport request retains its semantic origin"),
                reply_routes,
                ingress_ownership: Some(ingress_ownership),
                certificate: certificate.clone(),
            }
        };
        let mut queued = race_effect(hub_c, route_c.clone());
        let candidate = race_effect(hub_d, route_d.clone());
        assert!(merge_lane_work_effect_reply_routes_after_route_merge(
            &mut queued,
            &candidate,
            || assert!(route_fixture.retire(&route_c))
        ));
        let V2LaneWorkEffect::PostDurableLaneCertificate {
            reply_routes: Some(mut race_routes),
            ingress_ownership: Some(mut race_ownership),
            ..
        } = queued
        else {
            panic!("coalesced race result retains route and ownership carriers")
        };
        assert_eq!(
            race_routes.len(),
            2,
            "source C retired after the authoritative merge snapshot"
        );
        assert!(
            race_routes
                .iter()
                .any(|route| route.same_delivery(&route_c))
        );
        assert!(
            race_routes
                .iter()
                .any(|route| route.same_delivery(&route_d)),
            "source C retirement cannot consume independent source D"
        );
        assert!(race_ownership.validate_exact());
        assert!(race_ownership.matches_reply_routes(Some(&race_routes)));
        let (retained, prune_receipt) = race_routes.retain_active_with_receipt();
        assert_eq!(
            retained, 1,
            "the next bounded snapshot observes only source C's retirement"
        );
        race_routes = race_ownership
            .project_retained_reply_routes(prune_receipt)
            .expect("the prune receipt owns the exact source-D output");
        assert!(race_ownership.validate_exact());
        assert!(race_ownership.matches_reply_routes(Some(&race_routes)));
        assert!(
            race_ownership
                .current_reply_routes()
                .is_some_and(|routes| routes.len() == 1
                    && routes.iter().any(|route| route.same_delivery(&route_d)))
        );
    }
    #[test]
    fn durable_lane_certificate_serves_rotated_validator_after_pressure() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let requester = adapter
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .find(|peer| peer != &adapter.local_peer)
            .expect("fixture has a remote current validator");
        let historical_keys = keys
            .iter()
            .filter(|key| key.public_key() != requester.public_key())
            .cloned()
            .collect::<Vec<_>>();
        let lane_incarnation = adapter
            .state
            .lane_incarnation_at_height(LaneId::SINGLE, adapter.context.height)
            .expect("default lane incarnation");
        let proposal = proposal_for_route(
            &adapter,
            &historical_keys,
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            lane_incarnation,
            adapter.context.height,
            1,
        );
        let proposal = store_canonical_anchor(&adapter, &proposal, &historical_keys[0]);
        let session = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &historical_keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &historical_keys, CertPhase::Commit),
        };
        assert!(!session.commit_qc.validator_set.contains(&requester));
        let pops = adapter.pops_for_lane_session(&session);
        adapter
            .kura
            .persist_committed_lane_block_session(&session, &pops)
            .expect("persist historical-committee certificate");
        adapter.effects.clear();
        adapter.effect_keys.clear();
        adapter.limits.effect_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
        assert!(adapter.push_effect(V2LaneWorkEffect::PostLaneBlock {
            peer: requester.clone(),
            message: BlockMessage::LaneBlockProposal(proposal.clone()),
        }));
        let mut routes = NetworkReplyRouteTestFixture::new(requester.clone());
        let requester_route = routes.mint(requester.clone());
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::try_from_transport_with_reply_route(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    requester.clone(),
                    requester.clone(),
                    requester_route.clone(),
                )
                .expect("active historical request route"),
                0,
            ),
            V2LaneIngressOutcome::Duplicate,
            "a full response slot must leave reconstruction at the requester's exact proposal"
        );
        let _ = adapter.drain_effects(1);
        let unauthorized = PeerId::new(
            KeyPair::try_from_seed(vec![0xE7; 32], Algorithm::BlsNormal)
                .expect("outsider key")
                .public_key()
                .clone(),
        );
        let mut unauthorized_routes = NetworkReplyRouteTestFixture::new(unauthorized.clone());
        let unauthorized_route = unauthorized_routes.mint(unauthorized.clone());
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::try_from_transport_with_reply_route(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    unauthorized.clone(),
                    unauthorized.clone(),
                    unauthorized_route,
                )
                .expect("active unauthorized route reaches consensus validation"),
                0,
            ),
            V2LaneIngressOutcome::Rejected,
            "an authenticated transport identity outside both canonical rosters is unauthorized"
        );
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::try_from_transport_with_reply_route(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    requester.clone(),
                    requester.clone(),
                    requester_route.clone(),
                )
                .expect("active retried request route"),
                0,
            ),
            V2LaneIngressOutcome::Inserted,
            "the durable request must reconstruct the atomic response after capacity opens"
        );
        assert!(matches!(
            adapter.drain_effects(1).as_slice(),
            [V2LaneWorkEffect::PostDurableLaneCertificate {
                peer,
                reply_routes: Some(emitted_routes),
                certificate,
                ..
            }] if peer == &requester
                && emitted_routes.len() == 1
                && emitted_routes
                    .iter()
                    .any(|emitted_route| emitted_route.same_delivery(&requester_route))
                && certificate.proposal == session.proposal
        ));
    }
    #[test]
    fn decided_lane_accepts_atomic_certificate_recovery_and_rejects_mismatch() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_ne!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected
        );
        let _ = adapter.drain_effects(usize::MAX);
        adapter
            .retain_merge_sidecars_for_global_view(
                locked_round.view,
                Some(locked_subject),
                Some(locked_subject),
            )
            .expect("install the exact decided carrier");
        let certificate = LaneBlockCertificateV1 {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
        };
        let mut mismatched = certificate.clone();
        mismatched.prepare_qc.body.phase = CertPhase::Commit;
        let before = adapter.lane_sessions.len();
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockCertificate(Box::new(mismatched)),
                    Some(keys[0].public_key().clone().into()),
                ),
                locked_round.view,
            ),
            V2LaneIngressOutcome::Rejected
        );
        assert_eq!(adapter.lane_sessions.len(), before);
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockCertificate(Box::new(certificate)),
                    Some(keys[0].public_key().clone().into()),
                ),
                locked_round.view,
            ),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(adapter.pending_committed_lanes.len(), 1);
        assert!(adapter.has_pending_committed_output_handoff());
    }
    #[test]
    fn historical_certificate_survives_successor_lock_decision_persistence_and_restart() {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (parent_block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(parent_block.clone())
            .expect("persist the globally committed lane carrier");
        let committed_parent = ValidBlock::committed_from_replay_signed_block(parent_block.clone());
        commit_test_block_to_state(adapter.state.as_ref(), &committed_parent, &adapter.context);
        let certificate = LaneBlockCertificateV1 {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
        };
        let successor_context = successor_context_for_parent(&adapter, &parent_block);
        let local_peer = adapter.local_peer.clone();
        let local_key = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);
        let mut successor = V2LaneWorkAdapter::new(
            successor_context.clone(),
            local_peer.clone(),
            local_key.clone(),
            true,
            Arc::clone(&state),
            Arc::clone(&kura),
            limits,
            None,
        )
        .expect("open the true successor-height adapter");
        assert!(
            successor
                .lane_sessions
                .proposals_without_commit_qc()
                .iter()
                .any(|pending| pending == &proposal),
            "the successor must hydrate the exact older proposal as its request source"
        );
        let _ = successor.drain_effects(usize::MAX);
        assert_eq!(
            successor.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockCertificate(Box::new(certificate)),
                    Some(PeerId::new(keys[0].public_key().clone())),
                ),
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(successor.historical_recovery_sessions.len(), 1);
        assert!(
            successor.pending_committed_lanes.is_empty(),
            "historical evidence must not enter current-carrier persistence ownership"
        );
        assert!(
            successor.committed_lane_outputs.is_empty(),
            "historical recovery must not create a fresh CommitQC fanout"
        );
        let successor_block = test_block(
            successor.context.height,
            Some(parent_block.hash()),
            None,
            &keys[0],
        );
        let (locked_round, locked_subject) = global_lock_for_block(&successor, &successor_block);
        assert_eq!(
            successor.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        successor
            .retain_merge_sidecars_for_global_view(
                locked_round.view,
                Some(locked_subject),
                Some(locked_subject),
            )
            .expect("install a distinct successor Decision");
        assert_eq!(
            successor.historical_recovery_sessions.len(),
            1,
            "successor lock and Decision filtering must preserve the historical owner"
        );
        assert!(matches!(
            successor
                .service_next_historical_recovery()
                .expect("persist historical certificate and application witness"),
            HistoricalRecoveryServiceOutcome::Complete(_)
        ));
        assert!(!successor.has_pending_historical_recovery());
        assert_eq!(
            kura.read_certified_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .map(|artifact| artifact.proposal),
            Some(proposal.clone())
        );
        assert!(kura.lane_block_application_receipt_available(&proposal));
        assert!(
            state
                .unapplied_lane_block_artifact_heights_snapshot_cached()
                .is_empty(),
            "the recovered application witness must unblock the lane frontier"
        );
        drop(successor);
        let reopened = V2LaneWorkAdapter::new(
            successor_context,
            local_peer,
            local_key,
            true,
            Arc::clone(&state),
            Arc::clone(&kura),
            limits,
            None,
        )
        .expect("restart after historical recovery remains self-sufficient");
        assert!(!reopened.has_pending_historical_recovery());
        assert!(reopened.historical_recovery_requests.is_empty());
        assert!(reopened.historical_recovery_request_owners.is_empty());
        assert!(
            state
                .unapplied_lane_block_artifact_heights_snapshot_cached()
                .is_empty()
        );
    }
    #[test]
    fn historical_losing_certificate_cannot_conflict_with_durable_canonical_slot() {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (parent_block, winning_proposal) =
            globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(parent_block.clone())
            .expect("persist the canonical winning carrier");
        let committed_parent = ValidBlock::committed_from_replay_signed_block(parent_block.clone());
        commit_test_block_to_state(adapter.state.as_ref(), &committed_parent, &adapter.context);
        let durable_session = CommittedLaneBlockSession {
            proposal: winning_proposal.clone(),
            prepare_qc: lane_qc_for_phase(&winning_proposal, &keys[..3], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&winning_proposal, &keys[..3], CertPhase::Commit),
        };
        adapter
            .kura
            .persist_committed_lane_block_session(&durable_session, &lane_signer_pops(&keys[..3]))
            .expect("persist the canonical winning lane certificate");
        let mut losing_ownership = ownership_from_proposal(&winning_proposal);
        losing_ownership.accepted_transaction_hashes =
            vec![Hash::new(b"late certified losing lane proposal")];
        let losing_replay = losing_ownership
            .compute_replay_hashes()
            .expect("derive internally consistent losing ownership");
        losing_ownership.subject_hash = losing_replay.subject_hash;
        losing_ownership.payload_ownership_hash = losing_replay.payload_ownership_hash;
        losing_ownership.rbc_instance_hash = losing_replay.rbc_instance_hash;
        losing_ownership.lane_block_descriptor_hash =
            Some(losing_replay.lane_block_descriptor_hash);
        let losing_proposal = proposal_from_ownership(&losing_ownership, parent_block.hash())
            .expect("construct the losing proposal with the winning carrier hint");
        assert_ne!(losing_proposal, winning_proposal);
        assert!(
            !adapter.historical_block_anchors_proposal(&parent_block, &losing_proposal),
            "copying a canonical block hint must not make a losing proposal canonical"
        );
        let losing_certificate = LaneBlockCertificateV1 {
            proposal: losing_proposal.clone(),
            prepare_qc: lane_qc_for_phase(&losing_proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&losing_proposal, &keys, CertPhase::Commit),
        };
        let successor_context = successor_context_for_parent(&adapter, &parent_block);
        let local_peer = adapter.local_peer.clone();
        let local_key = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);
        let mut successor = V2LaneWorkAdapter::new(
            successor_context,
            local_peer,
            local_key,
            true,
            state,
            kura,
            limits,
            None,
        )
        .expect("open the true successor-height adapter");
        assert_eq!(
            successor.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockCertificate(Box::new(losing_certificate.clone())),
                    Some(PeerId::new(keys[0].public_key().clone())),
                ),
                0,
            ),
            V2LaneIngressOutcome::Rejected,
            "historical ingress must require exact canonical carrier membership"
        );
        assert!(successor.historical_recovery_sessions.is_empty());
        successor
            .historical_recovery_sessions
            .push_back(CommittedLaneBlockSession {
                proposal: losing_certificate.proposal,
                prepare_qc: losing_certificate.prepare_qc,
                commit_qc: losing_certificate.commit_qc,
            });
        assert!(matches!(
            successor
                .service_next_historical_recovery()
                .expect("the losing proof is retired without touching the durable winner"),
            HistoricalRecoveryServiceOutcome::Complete(_)
        ));
        assert!(!successor.has_pending_historical_recovery());
        assert!(!successor.output_guard.restart_required());
        let durable = successor
            .kura
            .read_certified_lane_block_artifact(
                winning_proposal.descriptor.lane_id,
                winning_proposal.descriptor.lane_block_height,
            )
            .expect("the durable winner remains authoritative");
        assert_eq!(durable.proposal, durable_session.proposal);
        assert_eq!(durable.prepare_qc, durable_session.prepare_qc);
        assert_eq!(durable.commit_qc, durable_session.commit_qc);
    }
    #[test]
    fn historical_alternative_qc_reuses_durable_proposal_identity() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (parent_block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(parent_block.clone())
            .expect("persist the canonical historical carrier");
        let finality = verified_finality_artifact_for_block(&adapter, &keys, &parent_block);
        let _commit_receipt = adapter
            .kura
            .store_v2_finality_artifact(&finality)
            .expect("persist the frozen historical validator PoP authority");
        let committed_parent = ValidBlock::committed_from_replay_signed_block(parent_block.clone());
        commit_test_block_to_state(adapter.state.as_ref(), &committed_parent, &adapter.context);
        let durable_session = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
        };
        adapter
            .kura
            .persist_committed_lane_block_session(&durable_session, &lane_signer_pops(&keys[..3]))
            .expect("persist the first valid quorum proof");
        assert!(
            !adapter
                .kura
                .lane_block_application_receipt_available(&proposal),
            "fixture must stop between certificate and receipt durability"
        );
        let replayed = LaneBlockCertificateV1 {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Commit),
        };
        assert_ne!(
            replayed.prepare_qc.signers_bitmap, durable_session.prepare_qc.signers_bitmap,
            "fixture must use distinct valid quorum subsets"
        );
        adapter.context = successor_context_for_parent(&adapter, &parent_block);
        assert!(proposal.descriptor.proposal_height < adapter.context.height);
        {
            let mut world = adapter.state.world.block();
            for key in &keys {
                world
                    .consensus_keys_by_pk
                    .insert(key.public_key().to_string(), Vec::new());
            }
            world.commit();
        }
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockCertificate(Box::new(replayed)),
                    Some(PeerId::new(keys[0].public_key().clone())),
                ),
                0,
            ),
            V2LaneIngressOutcome::Inserted,
            "the alternate proof must enter the historical recovery owner"
        );
        assert!(matches!(
            adapter
                .service_next_historical_recovery()
                .expect("an alternate quorum proof for the same proposal completes recovery"),
            HistoricalRecoveryServiceOutcome::Complete(_)
        ));
        assert!(adapter.historical_recovery_sessions.is_empty());
        assert!(
            adapter
                .kura
                .lane_block_application_receipt_available(&proposal),
            "alternate proof recovery must finish the interrupted application receipt"
        );
        let durable = adapter
            .kura
            .read_certified_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .expect("retain the first durable quorum proof");
        assert_eq!(durable.prepare_qc, durable_session.prepare_qc);
        assert_eq!(durable.commit_qc, durable_session.commit_qc);
        assert!(!adapter.output_guard.restart_required());
    }
