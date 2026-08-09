#[test]
fn committed_lane_status_publisher_tracks_evidence_revisions_and_clear() {
    let _guard = super::super::status::rbc_status_test_guard();
    super::super::status::clear_v2_status();
    let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let mut publisher = super::super::v2_runner::CommittedLaneStatusPublisher::default();
    assert!(publisher.publish_if_changed(&adapter));
    assert!(
        super::super::status::committed_lane_blocks_snapshot().is_empty(),
        "startup must first publish the recovered empty root"
    );
    assert!(
        !publisher.publish_if_changed(&adapter),
        "an unchanged runner turn must not rescan or republish lane status"
    );
    let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
    adapter
        .pending_committed_lanes
        .push_back(CommittedLaneBlockSession {
            prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
            proposal: proposal.clone(),
        });
    adapter.note_committed_lane_status_change();

    assert!(
        publisher.publish_if_changed(&adapter),
        "a newly committed volatile session must publish on the next bounded runner edge"
    );
    let published = super::super::status::committed_lane_blocks_snapshot();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].proposal, proposal);
    assert_eq!(
        published[0].execution_status,
        super::super::status::CommittedLaneBlockExecutionStatus::AwaitingExecutablePayload
    );
    assert!(
        !publisher.publish_if_changed(&adapter),
        "publication must acknowledge the exact adapter/Kura revision"
    );

    adapter
        .kura
        .store_block(block)
        .expect("publish exact canonical payload evidence");
    assert!(publisher.publish_if_changed(&adapter));
    assert_eq!(
        super::super::status::committed_lane_blocks_snapshot()[0].execution_status,
        super::super::status::CommittedLaneBlockExecutionStatus::PayloadAvailableAwaitingExecutor
    );

    let recovered = adapter
        .kura
        .recover_lane_block_payload(&proposal)
        .expect("recover exact lane payload");
    adapter
        .kura
        .persist_lane_block_execution_input(&recovered)
        .expect("persist exact execution input");
    assert!(publisher.publish_if_changed(&adapter));
    assert_eq!(
            super::super::status::committed_lane_blocks_snapshot()[0].execution_status,
            super::super::status::CommittedLaneBlockExecutionStatus::PayloadRecoveredAwaitingStateApplication
        );

    let input = adapter
        .kura
        .read_lane_block_execution_input(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        )
        .expect("read exact execution input");
    let clean_result =
        TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::default()));
    adapter
        .kura
        .persist_lane_block_execution_preflight(
            &input,
            u64::try_from(adapter.state.committed_height()).expect("fixture state height"),
            Some(adapter.state.lane_execution_state_hash()),
            vec![clean_result],
        )
        .expect("persist current exact clean preflight");
    assert!(publisher.publish_if_changed(&adapter));
    assert_eq!(
            super::super::status::committed_lane_blocks_snapshot()[0].execution_status,
            super::super::status::CommittedLaneBlockExecutionStatus::PayloadPreflightedAwaitingStateApplication
        );

    adapter
        .kura
        .persist_lane_block_application_receipt(&proposal)
        .expect("persist exact canonical receipt");
    assert!(publisher.publish_if_changed(&adapter));
    assert_eq!(
        super::super::status::committed_lane_blocks_snapshot()[0].execution_status,
        super::super::status::CommittedLaneBlockExecutionStatus::StateAppliedByCanonicalBlock
    );

    super::super::status::clear_v2_status();
    assert!(
        super::super::status::committed_lane_blocks_snapshot().is_empty(),
        "v2 shutdown/reset must not retain the previous runtime's status root"
    );
}

#[test]
fn recovered_autonomous_certificate_repairs_ready_before_certified_publication() {
    let (mut adapter, keys) = fixture_at_height_inner(wire::ConsensusMode::Permissioned, 2, true);
    let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
    let entrypoint = block
        .external_entrypoints_cloned()
        .next()
        .expect("autonomous recovery fixture entrypoint");
    let accepted = crate::tx::AcceptedTransaction::new_unchecked_entrypoint(
        std::borrow::Cow::Owned(entrypoint.clone()),
    );
    let routing_plan = RoutingPlan::single(RoutingDecision::new(
        proposal.descriptor.lane_id,
        proposal.descriptor.dataspace_id,
    ));
    let mut reservation = crate::queue::LaneQueueReservationKeyV2 {
        version: crate::queue::LaneQueueReservationKeyV2::VERSION,
        signed_transaction_hash: accepted.hash(),
        entrypoint_hash: entrypoint.hash(),
        queue_plan_admission_binding_hash: Hash::new(
            b"autonomous-recovery-queue-plan-admission-binding",
        ),
        routing_plan_digest: routing_plan.digest(),
        coordinator_leg: routing_plan.coordinator_leg(),
        lane_id: proposal.descriptor.lane_id,
        dataspace_id: proposal.descriptor.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        proposal_height: proposal.descriptor.proposal_height,
        lane_block_height: proposal.descriptor.lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        reservation_owner_hash: Hash::new(b"autonomous-recovery-reservation-owner"),
        proposal_identity_hash: proposal.proposal_hash,
    };
    let producer = adapter
        .expected_autonomous_lane_author(&proposal)
        .expect("deterministic autonomous recovery producer")
        .clone();
    bind_canonical_autonomous_reservation_identity(
        &adapter,
        &proposal,
        &producer,
        &mut reservation,
    );
    let producer_key = keys
        .iter()
        .find(|key| key.public_key() == producer.public_key())
        .expect("autonomous recovery producer key");
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        adapter.native_chain_id_hash(),
        adapter.context.epoch,
        proposal.clone(),
        vec![entrypoint],
        vec![reservation],
        vec![routing_plan],
        vec![None],
        producer.clone(),
        producer_key.private_key(),
    )
    .expect("signed autonomous recovery payload");
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneExecutablePayload(payload.clone()),
                Some(producer),
            ),
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );

    adapter
        .kura
        .store_block(block.clone())
        .expect("persist autonomous recovery carrier");
    let proposal_block = block.canonical_resultless_proposal();
    let (locked_round, locked_subject) = global_lock_for_block(&adapter, &proposal_block);
    assert_eq!(
        adapter.mark_global_body_locked(locked_round, locked_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_ne!(
        adapter.bind_locked_global_body(&proposal_block),
        V2LaneIngressOutcome::Rejected
    );
    let prepare_votes = keys
        .iter()
        .map(|key| signed_autonomous_prepare_vote(&proposal, &payload, key, &keys))
        .collect::<Vec<_>>();
    let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        proposal.vote_body(CertPhase::Prepare),
        proposal.descriptor.validator_set.clone(),
        &prepare_votes,
    )
    .expect("recovered READY votes form PrepareQC");
    let recovered = CommittedLaneBlockSession {
        proposal: proposal.clone(),
        prepare_qc: prepare_qc.clone(),
        commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
    };
    adapter.pending_committed_lanes.push_back(recovered);
    assert!(
        adapter
            .kura
            .read_autonomous_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
                adapter.native_chain_id_hash(),
                adapter.context.epoch,
            )
            .is_some_and(|artifact| artifact.availability_certificate.is_none()),
        "direct certificate recovery starts before local READY publication"
    );

    let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    assert_eq!(
        adapter
            .persist_anchored_sessions()
            .expect("repair READY and publish recovered autonomous certificate"),
        1
    );
    assert_eq!(
        adapter
            .kura
            .read_autonomous_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
                adapter.native_chain_id_hash(),
                adapter.context.epoch,
            )
            .and_then(|artifact| artifact.availability_certificate),
        Some(DurableLanePayloadAvailabilityCertificateV1 {
            certificate: prepare_qc.clone(),
        }),
        "READY durability must be repaired before recovered certified publication"
    );
    assert!(
        adapter
            .kura
            .read_certified_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .is_some(),
        "the exact recovered autonomous certificate becomes durable"
    );

    let certified = adapter
        .kura
        .read_certified_lane_block_artifact(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        )
        .expect("read exact autonomous certificate for late-body service");
    let successor_context = successor_context_for_parent(&adapter, &block);
    let source = adapter.local_peer.clone();
    let source_key = adapter.key_pair.clone();
    let requester = certified
        .proposal
        .descriptor
        .validator_set
        .iter()
        .find(|peer| *peer != &source)
        .expect("autonomous committee has a remote late-body requester")
        .clone();
    let state = Arc::clone(&adapter.state);
    let kura = Arc::clone(&adapter.kura);
    let limits = adapter.limits;
    drop(adapter);
    let mut successor = V2LaneWorkAdapter::new(
        successor_context,
        source,
        source_key,
        true,
        state,
        kura,
        limits,
        None,
    )
    .expect("open successor for authenticated autonomous late-body service");
    successor.effects.clear();
    successor.effect_keys.clear();
    let request = LaneHistoricalRecoveryRequestV1 {
        version: LANE_HISTORICAL_RECOVERY_VERSION_V4,
        requester: requester.clone(),
        certificate: Some(LaneBlockCertificateV1 {
            proposal: certified.proposal.clone(),
            prepare_qc: certified.prepare_qc.clone(),
            commit_qc: certified.commit_qc.clone(),
        }),
        signer_pops: certified.signer_pops.clone(),
        kind: LaneHistoricalRecoveryKindV1::AutonomousPayload {
            executable_payload_hash: payload.payload_hash,
            prepare_qc_hash: HashOf::new(&certified.prepare_qc),
            commit_qc_hash: HashOf::new(&certified.commit_qc),
        },
    };
    assert_eq!(
        successor.serve_historical_recovery_request(request.clone(), Some(&requester)),
        V2LaneIngressOutcome::Inserted,
        "the exact durable source must cross the ServeLateBody gate"
    );
    let effect_count = successor.effect_count();
    assert_eq!(
        successor.serve_historical_recovery_request(request.clone(), Some(&requester)),
        V2LaneIngressOutcome::Duplicate,
        "an exact queued late-body retry must remain a stutter"
    );
    assert_eq!(successor.effect_count(), effect_count);
    assert!(matches!(
        successor.drain_effects(1).as_slice(),
        [V2LaneWorkEffect::PostLaneBlock {
            peer,
            message: BlockMessage::LaneHistoricalRecoveryResponse(response),
        }] if peer == &requester
            && response.request_hash == HashOf::new(&request)
            && matches!(
                &response.payload,
                LaneHistoricalRecoveryPayloadV1::AutonomousPayload {
                    payload: served,
                    prepare_qc: served_prepare,
                    commit_qc: served_commit,
                } if served == &payload
                    && served_prepare == &certified.prepare_qc
                    && served_commit == &certified.commit_qc
            )
    ));
    assert!(!successor.output_guard.restart_required());
}
#[test]
fn decided_lane_ownership_blocks_rollover_until_its_session_is_durable() {
    // Result-bearing genesis carries external entrypoints before any lane
    // ownership can exist. Its empty ownership set is complete, not a
    // malformed lane plan or a missing lane certificate.
    {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let transaction_key = KeyPair::try_from_seed(vec![0xE2; 32], Algorithm::Ed25519)
            .expect("external-only transaction key");
        let transaction = TransactionBuilder::new(
            adapter.context.network_id,
            AccountId::new(transaction_key.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .sign(transaction_key.private_key());
        let entrypoint_hash = transaction.hash_as_entrypoint();
        let mut block =
            SignedBlock::genesis(vec![transaction], transaction_key.private_key(), None, None);
        block
            .set_transaction_results(
                Vec::new(),
                &[entrypoint_hash],
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("attach deterministic genesis transaction result");
        assert!(block.header().is_genesis());
        assert_eq!(block.external_entrypoint_count(), 1);
        assert!(block.has_results());
        assert!(block.header().result_merkle_root().is_some());
        assert!(block.execution_context().is_none());
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist canonical external-only carrier");
        let finality_artifact = finality_artifact_for_block(&adapter, &keys, &block);
        assert!(
            adapter
                .durable_lane_rollover_authority(&finality_artifact)
                .expect("validate external-only rollover")
                .is_some(),
            "a canonical block with no lane ownership has no lane durability debt"
        );
    }

    let (mut adapter, keys) = fixture_at_height_inner(wire::ConsensusMode::Permissioned, 2, true);
    let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
    adapter
        .kura
        .store_block(block.clone())
        .expect("persist canonical decided lane carrier");
    let proposal_block = block.canonical_resultless_proposal();
    let (locked_round, decided) = global_lock_for_block(&adapter, &proposal_block);
    assert_eq!(
        adapter.mark_global_body_locked(locked_round, decided),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_eq!(
        adapter.bind_locked_global_body(&proposal_block),
        V2LaneIngressOutcome::Inserted
    );
    let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    adapter
        .retain_merge_sidecars_for_global_view(locked_round.view, Some(decided), Some(decided))
        .expect("install exact global Decision");
    let finality_artifact = finality_artifact_for_block(&adapter, &keys, &block);

    assert!(
        adapter
            .durable_lane_rollover_authority(&finality_artifact)
            .expect("inspect incomplete decided lane boundary")
            .is_none(),
        "a raw ownership must not disappear from an empty rollover authority"
    );

    assert_eq!(proposal.descriptor.validator_count, 4);
    assert_eq!(proposal.descriptor.min_quorum, 3);
    let remote_keys = keys
        .iter()
        .filter(|key| PeerId::new(key.public_key().clone()) != adapter.local_peer)
        .take(2)
        .collect::<Vec<_>>();
    assert_eq!(
        remote_keys.len(),
        2,
        "two survivors plus the local vote must form the 3-of-4 quorum"
    );
    for phase in [CertPhase::Prepare, CertPhase::Commit] {
        for key in &remote_keys {
            let vote = signed_lane_vote(&proposal, phase, key);
            assert_ne!(
                adapter.accept_lane_message(
                    InboundBlockMessage::new(
                        BlockMessage::LaneBlockVote(vote),
                        Some(PeerId::new(key.public_key().clone())),
                    ),
                    locked_round.view,
                ),
                V2LaneIngressOutcome::Rejected,
                "the exact decided carrier must keep accepting quorum progress"
            );
        }
        let _ = adapter.drain_effects(usize::MAX);
    }
    assert_eq!(
        adapter
            .persist_anchored_sessions()
            .expect("persist decided lane certificate and application receipt"),
        1
    );
    let durable = adapter
        .kura
        .read_certified_lane_block_artifact(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        )
        .expect("read exact decided lane certificate");
    assert!(
        adapter
            .kura
            .lane_block_application_receipt_available(&proposal)
    );
    let authority = adapter
        .durable_lane_rollover_authority(&finality_artifact)
        .expect("inspect completed decided lane boundary")
        .expect("the exact durable lane session must release successor activation");
    assert!(
        authority
            .covered_source_hash(
                &finality_artifact,
                &BlockMessage::LaneBlockQc(durable.commit_qc),
            )
            .expect("validate the durable decided CommitQC")
            .is_some(),
        "rollover must cover the decided carrier's exact durable CommitQC"
    );
}

#[test]
fn applied_lane_certificate_retires_alternative_qc_replays_without_weakening_conflicts() {
    let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
    adapter
        .kura
        .store_block(block.clone())
        .expect("persist canonical lane anchor");
    let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);

    let persisted = CommittedLaneBlockSession {
        proposal: proposal.clone(),
        prepare_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
    };
    let persisted_pops = adapter.pops_for_lane_session(&persisted);
    adapter
        .kura
        .persist_committed_lane_block_session(&persisted, &persisted_pops)
        .expect("persist first valid quorum proof");
    assert!(
        adapter
            .kura
            .persist_lane_block_application_receipt_if_ready(&proposal)
            .expect("persist application receipt for the certified proposal")
    );
    adapter
        .kura
        .persist_committed_lane_block_session(&persisted, &persisted_pops)
        .expect("an exact durable duplicate remains idempotent");

    let alternative_qc = CommittedLaneBlockSession {
        proposal: proposal.clone(),
        prepare_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Commit),
    };
    assert_ne!(
        persisted.prepare_qc.signers_bitmap, alternative_qc.prepare_qc.signers_bitmap,
        "fixture must model two valid 3-of-4 certificates for one proposal"
    );
    let alternative_pops = adapter.pops_for_lane_session(&alternative_qc);
    let certificate_error = adapter
        .kura
        .persist_committed_lane_block_session(&alternative_qc, &alternative_pops)
        .expect_err("Kura must not replace the retained certificate bytes");
    assert!(
        certificate_error
            .to_string()
            .contains("different active-incarnation payload")
    );

    let descriptor = &proposal.descriptor;
    let conflicting_proposal = proposal_for_route(
        &adapter,
        &keys,
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
        descriptor.proposal_height,
        descriptor.lane_block_height,
    );
    assert_ne!(
        conflicting_proposal, proposal,
        "fixture must model a different valid body at the occupied lane height"
    );
    let conflicting_body = CommittedLaneBlockSession {
        prepare_qc: lane_qc_for_phase(&conflicting_proposal, &keys[..3], CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&conflicting_proposal, &keys[..3], CertPhase::Commit),
        proposal: conflicting_proposal,
    };
    let conflicting_pops = adapter.pops_for_lane_session(&conflicting_body);
    let body_error = adapter
        .kura
        .persist_committed_lane_block_session(&conflicting_body, &conflicting_pops)
        .expect_err("Kura must reject a different certified body at the same active height");
    assert!(
        body_error
            .to_string()
            .contains("different active-incarnation payload")
    );
    adapter
        .pending_committed_lanes
        .push_back(conflicting_body.clone());
    assert_eq!(
        adapter
            .persist_anchored_sessions()
            .expect("retain a non-canonical conflicting body"),
        0,
        "the exact-proposal shortcut must not retire a different body"
    );
    assert_eq!(
        adapter
            .pending_committed_lanes
            .front()
            .map(|session| &session.proposal),
        Some(&conflicting_body.proposal),
        "a different body must remain pending until it has a canonical anchor"
    );
    adapter.pending_committed_lanes.clear();

    assert!(
        adapter.proposal_body_available(&proposal),
        "an applied peer must keep serving the canonical body to lagging validators"
    );
    assert!(
        adapter
            .canonical_proposal_for_vote_body(&proposal.vote_body(CertPhase::Prepare))
            .is_some(),
        "an applied peer must keep reconstructing canonical recovery evidence"
    );
    adapter
        .pending_committed_lanes
        .push_back(alternative_qc.clone());
    adapter
        .committed_lane_outputs
        .push_back(PendingCommittedLaneOutput {
            session: alternative_qc.clone(),
            next_validator: 0,
        });
    assert_eq!(
        adapter
            .persist_anchored_sessions()
            .expect("retire a replay after matching its durable applied proposal"),
        1,
        "a valid alternative proof must not replace the exact durable certificate"
    );
    assert!(adapter.pending_committed_lanes.is_empty());
    let durable = adapter
        .kura
        .read_certified_lane_block_artifact(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        )
        .expect("retained certified artifact");
    assert_eq!(durable.proposal, persisted.proposal);
    assert_eq!(durable.prepare_qc, persisted.prepare_qc);
    assert_eq!(durable.commit_qc, persisted.commit_qc);

    let finality_artifact = finality_artifact_for_block(&adapter, &keys, &block);
    let authority = adapter
        .durable_lane_rollover_authority(&finality_artifact)
        .expect("inspect exact durable lane rollover authority")
        .expect("build exact durable lane rollover authority");
    let winning_vote = signed_lane_vote(&proposal, CertPhase::Prepare, &keys[3]);
    let winning_certificate = LaneBlockCertificateV1 {
        proposal: proposal.clone(),
        prepare_qc: alternative_qc.prepare_qc.clone(),
        commit_qc: alternative_qc.commit_qc.clone(),
    };
    for message in [
        BlockMessage::LaneBlockProposal(proposal.clone()),
        BlockMessage::LaneBlockVote(winning_vote),
        BlockMessage::LaneBlockQc(alternative_qc.prepare_qc.clone()),
        BlockMessage::LaneBlockQc(alternative_qc.commit_qc.clone()),
        BlockMessage::LaneBlockCertificate(Box::new(winning_certificate)),
    ] {
        assert!(
            authority
                .covered_source_hash(&finality_artifact, &message)
                .expect("validate winning lane rollover output")
                .is_some(),
            "every exact winning lane artifact must share the durable session witness"
        );
    }
    assert!(
        authority
            .covered_source_hash(
                &finality_artifact,
                &BlockMessage::LaneBlockProposal(conflicting_body.proposal.clone()),
            )
            .expect("classify same-height losing lane output")
            .is_some(),
        "the finality artifact must explicitly supersede a non-winning proposal"
    );
    let mut invalid_winning_qc = alternative_qc.commit_qc;
    invalid_winning_qc.bls_aggregate_signature[0] ^= 0x80;
    assert!(
        authority
            .covered_source_hash(
                &finality_artifact,
                &BlockMessage::LaneBlockQc(invalid_winning_qc),
            )
            .is_err(),
        "a winning proposal hash must not hide invalid proof bytes"
    );
}

#[test]
fn same_proposal_shortcut_rejects_unvalidated_certificate_variants() {
    {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist canonical lane anchor");
        let committed = ValidBlock::committed_from_replay_signed_block(block);
        commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
        let pending = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
        };
        let pops = adapter.pops_for_lane_session(&pending);
        adapter
            .kura
            .fail_progress_sidecar_ancestor_sync_attempts_for_tests(0, 1);
        adapter
            .kura
            .persist_committed_lane_block_session(&pending, &pops)
            .expect_err("failed ancestor barrier must leave only readable certificate bytes");
        adapter.pending_committed_lanes.push_back(pending.clone());
        assert_eq!(
            adapter
                .persist_anchored_sessions()
                .expect("the exact retry must repair every failed ancestor barrier"),
            1
        );
        assert!(
            adapter.pending_committed_lanes.is_empty(),
            "a durability-attested retry may retire its volatile reconstruction source"
        );
    }

    #[derive(Clone, Copy, Debug)]
    enum QcUnderTest {
        Prepare,
        Commit,
    }

    impl QcUnderTest {
        fn phase(self) -> CertPhase {
            match self {
                Self::Prepare => CertPhase::Prepare,
                Self::Commit => CertPhase::Commit,
            }
        }

        fn select_mut(self, session: &mut CommittedLaneBlockSession) -> &mut LaneBlockQcV1 {
            match self {
                Self::Prepare => &mut session.prepare_qc,
                Self::Commit => &mut session.commit_qc,
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum InvalidQcVariant {
        ForgedAggregate,
        WrongPhase,
        WrongRound,
        WrongBody,
        WrongBitmap,
        OutOfRangeBitmap,
        InsufficientCount,
        MissingPop,
        InvalidPop,
    }

    for (qc_under_test, variant) in [QcUnderTest::Prepare, QcUnderTest::Commit]
        .into_iter()
        .flat_map(|qc_under_test| {
            [
                InvalidQcVariant::ForgedAggregate,
                InvalidQcVariant::WrongPhase,
                InvalidQcVariant::WrongRound,
                InvalidQcVariant::WrongBody,
                InvalidQcVariant::WrongBitmap,
                InvalidQcVariant::OutOfRangeBitmap,
                InvalidQcVariant::InsufficientCount,
                InvalidQcVariant::MissingPop,
                InvalidQcVariant::InvalidPop,
            ]
            .into_iter()
            .map(move |variant| (qc_under_test, variant))
        })
    {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist canonical lane anchor");
        let committed = ValidBlock::committed_from_replay_signed_block(block);
        commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);

        let retained = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
        };
        adapter
            .kura
            .persist_committed_lane_block_session(&retained, &lane_signer_pops(&keys[..3]))
            .expect("persist retained valid certificate");
        assert!(
            adapter
                .kura
                .persist_lane_block_application_receipt_if_ready(&proposal)
                .expect("persist retained certificate receipt")
        );

        let target_qc = lane_qc_for_phase(&proposal, &keys[1..], qc_under_test.phase());
        let opposite_qc = lane_qc_for_phase(
            &proposal,
            &keys[..3],
            match qc_under_test {
                QcUnderTest::Prepare => CertPhase::Commit,
                QcUnderTest::Commit => CertPhase::Prepare,
            },
        );
        let mut pending = match qc_under_test {
            QcUnderTest::Prepare => CommittedLaneBlockSession {
                proposal: proposal.clone(),
                prepare_qc: target_qc,
                commit_qc: opposite_qc,
            },
            QcUnderTest::Commit => CommittedLaneBlockSession {
                proposal: proposal.clone(),
                prepare_qc: opposite_qc,
                commit_qc: target_qc,
            },
        };
        let valid_candidate = CertifiedLaneBlockArtifact::new(
            pending.clone(),
            adapter.pops_for_lane_session(&pending),
        );
        Kura::validate_certified_lane_block_artifact(&valid_candidate)
            .expect("the unmodified alternative proof must be valid");

        let qc = qc_under_test.select_mut(&mut pending);
        match variant {
            InvalidQcVariant::ForgedAggregate => {
                qc.bls_aggregate_signature[0] ^= 0x80;
            }
            InvalidQcVariant::WrongPhase => {
                qc.body.phase = match qc_under_test {
                    QcUnderTest::Prepare => CertPhase::Commit,
                    QcUnderTest::Commit => CertPhase::Prepare,
                };
            }
            InvalidQcVariant::WrongRound => {
                qc.body.lane_block_view = qc.body.lane_block_view.saturating_add(1);
            }
            InvalidQcVariant::WrongBody => {
                qc.body.subject_hash = Hash::new(b"forged alternative certificate body");
            }
            InvalidQcVariant::WrongBitmap => {
                assert_eq!(
                    qc.signers_bitmap,
                    vec![0b0000_1110],
                    "fixture target QC must select validators 1, 2, and 3"
                );
                qc.signers_bitmap[0] = 0b0000_1101;
            }
            InvalidQcVariant::OutOfRangeBitmap => {
                qc.signers_bitmap[0] |= 0b1000_0000;
            }
            InvalidQcVariant::InsufficientCount => {
                assert_eq!(
                    qc.signers_bitmap,
                    vec![0b0000_1110],
                    "fixture target QC must select validators 1, 2, and 3"
                );
                qc.signers_bitmap[0] = 0b0000_0110;
            }
            InvalidQcVariant::MissingPop | InvalidQcVariant::InvalidPop => {
                let state = Arc::get_mut(&mut adapter.state)
                    .expect("isolated lane adapter uniquely owns its State");
                let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator3".to_owned());
                let mut record = state
                    .world
                    .consensus_keys
                    .view()
                    .get(&id)
                    .expect("signer unique to the target alternative QC")
                    .clone();
                record.pop = match variant {
                    InvalidQcVariant::MissingPop => None,
                    InvalidQcVariant::InvalidPop => Some(vec![0xA5; 96]),
                    _ => unreachable!("matched PoP variants"),
                };
                state.world.consensus_keys.insert(id, record);
            }
        }

        adapter.pending_committed_lanes.push_back(pending.clone());
        let error = adapter
            .persist_anchored_sessions()
            .expect_err("unvalidated proof variant must not use the same-proposal shortcut");
        assert!(
            error
                .to_string()
                .contains("pending committed lane certificate is invalid"),
            "unexpected {qc_under_test:?} {variant:?} rejection: {error}"
        );
        assert_eq!(
            adapter.pending_committed_lanes.front(),
            Some(&pending),
            "rejected {qc_under_test:?} {variant:?} proof must retain its volatile owner for fail-stop diagnosis"
        );
        let durable = adapter
            .kura
            .read_certified_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .expect("retained certificate remains authoritative");
        assert_eq!(durable.prepare_qc, retained.prepare_qc);
        assert_eq!(durable.commit_qc, retained.commit_qc);
    }
}

#[test]
fn alternative_qc_repairs_missing_receipt_from_retained_exact_certificate() {
    let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
    adapter
        .kura
        .store_block(block.clone())
        .expect("persist canonical lane anchor");
    let committed = ValidBlock::committed_from_replay_signed_block(block);
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);

    let retained = CommittedLaneBlockSession {
        proposal: proposal.clone(),
        prepare_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
    };
    let retained_pops = adapter.pops_for_lane_session(&retained);
    adapter
        .kura
        .persist_committed_lane_block_session(&retained, &retained_pops)
        .expect("persist certificate before the simulated crash");
    assert!(
        !adapter
            .kura
            .lane_block_application_receipt_available(&proposal),
        "fixture must stop between certificate and receipt durability"
    );

    let replayed = CommittedLaneBlockSession {
        proposal: proposal.clone(),
        prepare_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Commit),
    };
    assert_ne!(
        retained.prepare_qc.signers_bitmap, replayed.prepare_qc.signers_bitmap,
        "the replay must carry a distinct valid quorum proof"
    );
    adapter.pending_committed_lanes.push_back(replayed);
    assert_eq!(
        adapter
            .persist_anchored_sessions()
            .expect("repair the receipt from the retained exact certificate"),
        1
    );
    assert!(
        adapter
            .kura
            .lane_block_application_receipt_available(&proposal),
        "the retained certificate must finish its interrupted receipt"
    );
    let durable = adapter
        .kura
        .read_certified_lane_block_artifact(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        )
        .expect("retained exact certificate");
    assert_eq!(durable.proposal, retained.proposal);
    assert_eq!(durable.prepare_qc, retained.prepare_qc);
    assert_eq!(durable.commit_qc, retained.commit_qc);
}

#[test]
fn globally_applied_lane_body_without_certificate_remains_recoverable() {
    let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
    assert!(
        adapter
            .lane_sessions
            .proposals_without_commit_qc()
            .iter()
            .all(|pending| pending != &proposal),
        "the adapter must be constructed before the canonical ownership arrives"
    );
    adapter
        .kura
        .store_block(block.clone())
        .expect("persist canonical lane anchor without its certificate");
    let (decided_round, decided_subject) = global_lock_for_block(&adapter, &block);
    let finality_artifact = finality_artifact_for_block(&adapter, &keys, &block);
    let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    adapter
        .retain_merge_sidecars_for_global_view(
            decided_round.view,
            Some(decided_subject),
            Some(decided_subject),
        )
        .expect("install the direct block-sync Decision without binding its lane body");
    assert!(adapter.decision_pending());

    let recovered = CommittedLaneBlockSession {
        proposal: proposal.clone(),
        prepare_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Commit),
    };
    assert!(adapter.proposal_anchor_is_committed_in_state(&proposal));
    assert!(
        adapter
            .kura
            .read_certified_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .is_none(),
        "the globally applied body must begin without lane certificate durability"
    );
    assert!(
        !adapter
            .state
            .certified_lane_block_session_is_applied_or_snapshot_anchored_cached(&recovered),
        "global application alone must not impersonate lane certificate application"
    );
    assert!(
        adapter.proposal_body_available(&proposal),
        "the missing certificate must remain reconstructable from the canonical body"
    );

    assert_eq!(
        adapter
            .persist_anchored_sessions()
            .expect("rehydrate the late-applied canonical ownership"),
        0,
        "no certificate exists yet to persist"
    );
    assert!(
        adapter
            .lane_sessions
            .proposals_without_commit_qc()
            .iter()
            .any(|pending| pending == &proposal),
        "rollover must rehydrate ownership which arrived after adapter construction"
    );
    assert!(
        adapter
            .durable_lane_rollover_authority(&finality_artifact)
            .expect("inspect incomplete decided-lane rollover")
            .is_none(),
        "the decided height must remain open until its lane certificate is durable"
    );
    let _ = adapter.drain_effects(usize::MAX);
    adapter
        .schedule_retransmission()
        .expect("schedule exact missing-certificate discovery");
    assert!(
        adapter.drain_effects(usize::MAX).iter().any(|effect| {
            matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockProposal(pending),
                    ..
                } if pending == &proposal
            )
        }),
        "the rehydrated proposal must become a bounded certificate request source"
    );
    let certificate = LaneBlockCertificateV1 {
        proposal: recovered.proposal.clone(),
        prepare_qc: recovered.prepare_qc.clone(),
        commit_qc: recovered.commit_qc.clone(),
    };
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneBlockCertificate(Box::new(certificate)),
                Some(PeerId::new(keys[1].public_key().clone())),
            ),
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        adapter
            .persist_anchored_sessions()
            .expect("persist recovered certificate and application receipt"),
        1
    );
    let durable = adapter
        .kura
        .read_certified_lane_block_artifact(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        )
        .expect("recovered durable certificate");
    assert_eq!(durable.proposal, recovered.proposal);
    assert_eq!(durable.prepare_qc, recovered.prepare_qc);
    assert_eq!(durable.commit_qc, recovered.commit_qc);
    assert!(
        adapter
            .kura
            .lane_block_application_receipt_available(&proposal),
        "certificate recovery must finish the lane application boundary"
    );
    assert!(
        adapter
            .durable_lane_rollover_authority(&finality_artifact)
            .expect("build recovered decided-lane rollover authority")
            .is_some(),
        "the exact recovered certificate and receipt must release successor activation"
    );
}

#[test]
fn persisted_lane_session_uses_only_selected_qc_signer_pops() {
    let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let (_, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
    let selected_keys = &keys[..3];
    let prepare_qc = lane_qc_for_phase(&proposal, selected_keys, CertPhase::Prepare);
    let commit_qc = lane_qc_for_phase(&proposal, selected_keys, CertPhase::Commit);
    let session = CommittedLaneBlockSession {
        proposal,
        prepare_qc,
        commit_qc,
    };
    let signer_pops = adapter.pops_for_lane_session(&session);
    let expected_signers = selected_keys
        .iter()
        .map(|key| key.public_key().clone())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        signer_pops.keys().cloned().collect::<BTreeSet<_>>(),
        expected_signers,
        "durable proof material must name exactly the bitmap-selected signers"
    );
    let artifact =
        crate::kura::CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    assert_eq!(
        Kura::validate_certified_lane_block_artifact(&artifact),
        Ok(())
    );
    adapter
        .kura
        .persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist a 3-of-4 lane certificate with exact signer PoPs");

    let extra_signer_artifact =
        crate::kura::CertifiedLaneBlockArtifact::new(session.clone(), lane_signer_pops(&keys));
    assert_eq!(
        Kura::validate_certified_lane_block_artifact(&extra_signer_artifact),
        Err("certified lane block signer PoPs do not match QC signers"),
        "a non-signer PoP must remain rejected instead of being persisted as unauthenticated metadata"
    );

    let mut missing_selected_pop = signer_pops;
    missing_selected_pop.remove(selected_keys[0].public_key());
    assert!(matches!(
        crate::lane_consensus::validate_lane_block_qc_aggregate(
            &session.prepare_qc,
            &missing_selected_pop,
        ),
        Err(crate::lane_consensus::LaneBlockQcIngressError::SignerPopMissing)
    ));

    let mut out_of_range_bitmap = session.prepare_qc;
    out_of_range_bitmap.signers_bitmap[0] |= 1_u8 << 4;
    assert!(matches!(
        crate::lane_consensus::validate_lane_block_qc_aggregate(
            &out_of_range_bitmap,
            &lane_signer_pops(&keys),
        ),
        Err(crate::lane_consensus::LaneBlockQcIngressError::SignerBitmapOutOfRange)
    ));
}

#[test]
fn restart_repairs_certified_lane_sidecar_missing_only_application_receipt() {
    let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
    adapter
        .kura
        .store_block(block.clone())
        .expect("persist globally anchored lane block");
    let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);

    let certified = CommittedLaneBlockSession {
        proposal: proposal.clone(),
        prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
    };
    adapter
        .kura
        .persist_committed_lane_block_session(&certified, &lane_signer_pops(&keys))
        .expect("persist certificate before simulated crash");
    assert!(
        !adapter
            .kura
            .lane_block_application_receipt_available(&proposal),
        "fixture must stop after certificate durability but before receipt publication"
    );

    let context = adapter.context.clone();
    let local_peer = adapter.local_peer.clone();
    let local_key = adapter.key_pair.clone();
    let state = Arc::clone(&adapter.state);
    let kura = Arc::clone(&adapter.kura);
    let limits = adapter.limits;
    let recovery = super::super::v2_recovery::PendingKuraApply::for_test(
        context.id(),
        context.height,
        block.hash(),
    );
    let finality_artifact = finality_artifact_for_block(&adapter, &keys, &block);
    drop(adapter);

    let reopened = V2LaneWorkAdapter::new(
        context,
        local_peer,
        local_key,
        true,
        Arc::clone(&state),
        Arc::clone(&kura),
        limits,
        Some(recovery),
    )
    .expect("restart repairs the exact certificate/receipt crash boundary");
    assert!(
        kura.lane_block_application_receipt_available(&proposal),
        "restart must publish the missing canonical application receipt"
    );
    assert!(
        state
            .unapplied_lane_block_artifact_heights_snapshot_cached()
            .is_empty(),
        "the repaired receipt must unblock the next lane-local height"
    );
    assert!(
        reopened.committed_lane_outputs.is_empty(),
        "volatile completed-output ownership must not survive restart"
    );
    assert!(
        reopened
            .durable_lane_rollover_authority(&finality_artifact)
            .expect("reconstruct rollover authority after restart")
            .is_some(),
        "canonical Kura evidence must reconstruct authority without a volatile output queue"
    );
}
