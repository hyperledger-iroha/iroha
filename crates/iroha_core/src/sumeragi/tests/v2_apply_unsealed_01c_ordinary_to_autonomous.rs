// Ordinary global execution and autonomous merge execution share one applied lane frontier.

v2_apply_test!(
    ordinary_lane_frontier_unblocks_third_autonomous_source_after_timeout_views,
    {
        use crate::sumeragi::{
            LaneRelayMessage,
            v2::{
                AdapterEffect, AdapterFingerprints, DeferredAdmissionOrdinalSource,
                SumeragiV2Adapter,
            },
            v2_lane_work::{
                HistoricalRecoveryServiceOutcome, V2LaneIngressOutcome, V2LaneWorkAdapter,
                V2LaneWorkEffect,
            },
            v2_runtime::{RuntimeQueueConfig, RuntimeStep, SerializedV2Runtime},
        };

        let fixture =
            ApplyFixture::new_for_production_recovered_decision_apply_with_lane_lifecycle();
        fixture
            .execute(&mut fixture.reopen_body_store())
            .expect("commit real genesis");
        let keys = fixture_validator_keys();
        let mut predecessor = None;
        for slot in 1..=2 {
            let context = verified_successor_context_at_fixture_tip(&fixture);
            let mut ordinary = build_apply_fixture_at_context_with_autonomous_payloads(
                &fixture,
                context.context().clone(),
                Vec::new(),
            );
            let bundle = ordinary
                .body
                .execution_context()
                .expect("ordinary execution context");
            assert!(bundle.autonomous_lane_payloads.is_empty());
            assert!(bundle.merge_entry.is_none());
            assert_eq!(bundle.lane_payload_ownerships.len(), 1);
            let ownership = bundle.lane_payload_ownerships[0].clone();
            assert_eq!(ownership.lane_id.as_u32(), 0);
            assert_eq!(ownership.lane_block_height, slot);
            assert_eq!(ownership.previous_lane_block_height, slot - 1);
            let certificate = ordinary_frontier_certificate(&fixture, &ordinary, &keys);
            fixture
                .service
                .execute(&ordinary.context, &mut ordinary.store, &ordinary.task)
                .expect("execute ordinary globally ordered lane transactions");
            assert_eq!(
                fixture
                    .state
                    .unapplied_lane_block_artifact_heights_snapshot_cached()
                    .expect("observe ordinary certificate completion debt")
                    .get(&(ownership.lane_id, ownership.dataspace_id)),
                Some(&slot),
                "ordinary Apply must not impersonate lane certificate completion"
            );
            // Normal successor recovery completes the ordinary certificate and
            // receipt before a later ordinary or autonomous producer may extend it.
            let successor = verified_successor_context_at_fixture_tip(&fixture);
            let mut completion = V2LaneWorkAdapter::new(
                successor.context().clone(),
                PeerId::new(keys[0].public_key().clone()),
                keys[0].clone(),
                false,
                Arc::clone(&fixture.state),
                Arc::clone(&fixture.kura),
                ordinary_frontier_lane_work_limits(),
                None,
            )
            .expect("open observer for exact ordinary certificate recovery");
            assert_eq!(
                completion.accept_lane_message(
                    crate::sumeragi::InboundBlockMessage::from_authenticated_peer(
                        crate::sumeragi::message::BlockMessage::LaneBlockCertificate(Box::new(
                            certificate.clone(),
                        )),
                        PeerId::new(keys[0].public_key().clone()),
                    ),
                    0,
                ),
                V2LaneIngressOutcome::Inserted
            );
            assert!(matches!(
                completion
                    .service_next_historical_recovery()
                    .expect("publish exact ordinary certificate and application receipt"),
                HistoricalRecoveryServiceOutcome::Complete(_)
            ));
            assert!(
                fixture
                    .kura
                    .lane_block_application_receipt_available(&certificate.proposal)
            );
            assert!(
                fixture
                    .state
                    .unapplied_lane_block_artifact_heights_snapshot_cached()
                    .expect("ordinary completion unblocks the exact predecessor")
                    .is_empty()
            );
            predecessor = Some(ownership);
        }
        assert_eq!(fixture.state.committed_height(), 3);
        assert!(fixture.state.merge_ledger.snapshot().is_empty());
        let predecessor = predecessor.expect("ordinary slot two");
        let source_context = verified_successor_context_at_fixture_tip(&fixture);
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
        let queue = fixture_queue(fixture.state.as_ref(), events_sender.clone());
        let journals = tempfile::tempdir().expect("ordinary-to-autonomous journals");
        let plans = journals.path().join("plans.norito");
        let reservations = journals.path().join("reservations.norito");
        queue
            .install_plan_journal(&plans, 1024 * 1024, true)
            .expect("install plans");
        queue
            .install_lane_reservation_journal(&reservations, 1024 * 1024)
            .expect("install reservations");
        let (payload, entrypoints) =
            reserve_canonical_autonomous_batch_at_context_with_instructions(
                &fixture,
                &queue,
                source_context.context(),
                1,
                |_| {
                    vec![InstructionBox::from(Log::new(
                        Level::INFO,
                        "third autonomous lane source".to_owned(),
                    ))]
                },
                false,
                None,
            );
        let descriptor = &payload.origin_proposal.descriptor;
        assert_eq!(descriptor.lane_id.as_u32(), 0);
        assert_eq!(descriptor.lane_block_height, 3);
        assert_eq!(descriptor.previous_lane_block_height, 2);
        assert_eq!(
            descriptor.previous_lane_block_descriptor_hash,
            predecessor.lane_block_descriptor_hash
        );
        let envelope = crate::lane_consensus::autonomous_lane_payload_envelope(
            &payload,
            payload.network_id,
            payload.epoch,
        )
        .expect("encode exact third source");
        let mut source = build_apply_fixture_at_context_with_autonomous_payloads(
            &fixture,
            source_context.context().clone(),
            vec![envelope],
        );
        fixture
            .service
            .execute(&source.context, &mut source.store, &source.task)
            .expect("anchor third autonomous source without executing it");
        assert_eq!(fixture.state.committed_height(), 4);
        assert!(
            entrypoints
                .iter()
                .all(|hash| !fixture.state.has_committed_entrypoint(*hash))
        );
        let active = verified_successor_context_at_fixture_tip(&fixture);
        assert_eq!(active.context().height, 5);

        // Reopen the actual reservation journals as successor startup does.
        drop(queue);
        let queue = fixture_queue(fixture.state.as_ref(), events_sender.clone());
        let replay = queue
            .install_lane_reservation_journal(&reservations, 1024 * 1024)
            .expect("restore exact reservations");
        assert_eq!(replay.restored, 1);
        queue
            .install_plan_journal(&plans, 1024 * 1024, true)
            .expect("restore plans");
        queue
            .replay_plan_journal(fixture.state.as_ref())
            .expect("restore executable owner");
        let planning = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &active,
            None,
        )
        .expect("plan historical source installation");
        let LaneReservationReconciliationPlanning::InstallHistoricalAutonomousRecoveries(
            mut installs,
        ) = planning
        else {
            panic!("third source requires exact historical installation");
        };
        assert_eq!(installs.len(), 1);
        let install = installs.pop().expect("one third-source installation");
        assert_eq!(install.payload.origin_proposal.descriptor, *descriptor);
        assert_eq!(
            install_historical_autonomous_lane_recovery(
                fixture.state.as_ref(),
                fixture.kura.as_ref(),
                &install,
            )
            .expect("install canonical historical source"),
            HistoricalAutonomousLaneRecoveryInstallOutcome::Installed
        );
        let planning = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &active,
            None,
        )
        .expect("reconcile installed source");
        let LaneReservationReconciliationPlanning::Ready(plan) = planning else {
            panic!("installed source must be ready");
        };
        apply_lane_reservation_reconciliation_plan(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            plan,
        )
        .expect("publish historical reservation ownership");
        assert_eq!(
            queue.queued_len(),
            0,
            "the successor has no ordinary transaction wakeup"
        );

        let target_view = 4;
        let local = active.context().leader(target_view);
        let local_key = keys[usize::try_from(local).expect("leader index")].clone();
        let local_peer = PeerId::new(local_key.public_key().clone());
        let limits = ordinary_frontier_lane_work_limits();
        let mut lane_work = V2LaneWorkAdapter::new(
            active.context().clone(),
            local_peer.clone(),
            local_key.clone(),
            true,
            Arc::clone(&fixture.state),
            Arc::clone(&fixture.kura),
            limits,
            None,
        )
        .expect("hydrate third-source lane owner");
        let generation = fixture
            .kura
            .claim_autonomous_lifecycle_process_generation(payload.network_id, &local_peer)
            .expect("claim third-source lifecycle generation");
        let _group = install_live_lifecycle_cursor_for_apply_test(
            fixture.kura.as_ref(),
            &generation,
            &install.payload,
            install.historical_context_id,
            &local_peer,
            &local_key,
        );
        let (certificate, _, _) = terminal_cycle_certificate(&fixture, &install.payload, &keys);
        assert_eq!(
            lane_work.accept_lane_message(
                crate::sumeragi::InboundBlockMessage::from_authenticated_peer(
                    crate::sumeragi::message::BlockMessage::LaneBlockCertificate(Box::new(
                        certificate.clone()
                    )),
                    PeerId::new(keys[0].public_key().clone()),
                ),
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
        assert!(matches!(
            lane_work
                .service_next_historical_recovery()
                .expect("publish certified third source"),
            HistoricalRecoveryServiceOutcome::Complete(_)
        ));
        assert!(
            fixture
                .kura
                .read_certified_lane_block_artifact_read_only(descriptor.lane_id, 3)
                .expect("read certified third source without repair")
                .is_some()
        );

        // Recreate the incident's missing ordinary frontier without changing the
        // certified source. Restore only the exact bytes produced by real Apply.
        let frontier_key: iroha_model_base::state_path::StatePath = format!(
            "merge_lane_frontier_v1_{}_{}_{}",
            descriptor.lane_id.as_u32(),
            descriptor.dataspace_id.as_u64(),
            hex::encode(descriptor.lane_incarnation.as_ref()),
        )
        .parse()
        .expect("exact applied lane frontier key");
        let frontier = {
            let mut world = fixture.state.world.block();
            let frontier = world
                .smart_contract_state
                .get(&frontier_key)
                .cloned()
                .expect("ordinary slots publish the shared applied frontier");
            assert_eq!(
                world.smart_contract_state.remove(frontier_key.clone()),
                Some(frontier.clone())
            );
            world.commit();
            frontier
        };
        assert!(
            !fixture
                .state
                .has_pending_merge_execution_sources(active.context().mode),
            "missing ordinary frontier reproduces the stranded certified third source"
        );
        {
            let mut world = fixture.state.world.block();
            assert!(
                world
                    .smart_contract_state
                    .insert(frontier_key, frontier)
                    .is_none()
            );
            world.commit();
        }
        assert!(
            fixture
                .state
                .has_pending_merge_execution_sources(active.context().mode)
        );
        assert_eq!(queue.queued_len(), 0);

        let wal_dir = tempfile::tempdir().expect("third-source global safety WAL");
        let (adapter, startup) = SumeragiV2Adapter::open(
            wal_dir.path().join("global.wal"),
            active.clone(),
            Some(local),
            Generation::new(1),
            [0xD3; 32],
            AdapterFingerprints {
                node: Hash::new(b"ordinary-frontier node"),
                build: Hash::new(b"ordinary-frontier build"),
                config: Hash::new(b"ordinary-frontier config"),
            },
            DeferredAdmissionOrdinalSource::new(0),
        )
        .expect("open authenticated successor reducer");
        assert!(startup.is_empty());
        let started = std::time::Instant::now();
        let (mut runtime, startup) = SerializedV2Runtime::new(
            adapter,
            startup,
            started,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
        )
        .expect("construct serialized successor runtime");
        assert!(startup.is_empty());
        runtime
            .reconcile_active_view_producer(
                runtime.round_tag(),
                local == active.context().leader(0),
            )
            .expect("bind initial producer");
        runtime
            .arm_live_clocks(started)
            .expect("arm successor clocks");
        lane_work
            .retain_merge_sidecars_for_global_view(0, None, None)
            .expect("initial ordinary refresh");
        let _ = lane_work.drain_effects(usize::MAX);
        for previous_view in 0..target_view {
            let tc = ordinary_frontier_timeout_certificate(active.context(), &keys, previous_view);
            runtime
                .enqueue_network(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::TimeoutCertificate(tc),
                ))
                .expect("admit authenticated timeout certificate");
            let RuntimeStep::Advanced(effects) = runtime
                .step(started)
                .expect("install actual timeout certificate")
            else {
                panic!("authenticated TC must advance the successor view");
            };
            assert!(effects.iter().any(|effect| matches!(effect, AdapterEffect::EnterView { tag, .. } if tag.view() == previous_view + 1)));
            let _ = runtime
                .take_last_scheduler_ownership()
                .expect("consume exact TC scheduler carrier");
            let _ = runtime
                .take_effect_ownership(effects.len())
                .expect("consume exact EnterView effect ownership");
            let _ = runtime.take_leader_wire_runtime_terminals();
            let tag = runtime.round_tag();
            runtime
                .reconcile_active_view_producer(tag, local == active.context().leader(tag.view()))
                .expect("reconcile actual new-view producer");
            lane_work
                .retain_merge_sidecars_for_global_view(tag.view(), None, None)
                .expect("ordinary runner retention after certified view transition");
        }
        assert!(
            runtime
                .local_proposal_admission_available(runtime.round_tag())
                .expect("current leader admission")
        );
        lane_work
            .schedule_retransmission()
            .expect("ordinary retained-source retransmission refresh");
        let share = lane_work
            .drain_effects(usize::MAX)
            .into_iter()
            .find_map(|effect| match effect {
                V2LaneWorkEffect::BroadcastMerge(share)
                    if share.view == target_view && share.signer == local =>
                {
                    Some(share)
                }
                _ => None,
            })
            .expect("normal leader refresh broadcasts the third source at the current view");
        let candidate: crate::merge::MergeLedgerCandidate = norito::decode_canonical(
            share
                .leader_candidate_body
                .as_deref()
                .expect("leader carries canonical candidate"),
        )
        .expect("decode actual broadcast candidate");
        assert_eq!(candidate.carrier_height, 5);
        assert_eq!(candidate.carrier_parent_hash, source.body.hash());
        assert_eq!(candidate.view, target_view);
        let batch = candidate
            .execution_batch
            .as_ref()
            .expect("real autonomous execution work");
        assert_eq!(batch.lanes.len(), 1);
        assert_eq!(batch.lanes[0].proposal, certificate.proposal);
        assert_eq!(
            batch.lanes[0].entrypoint_hashes,
            install.payload.entrypoint_hashes
        );

        for (index, key) in keys
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != usize::try_from(local).expect("leader index"))
            .take(2)
        {
            let mut follower = share.clone();
            follower.signer = u32::try_from(index).expect("follower index");
            follower.leader_candidate_body = None;
            follower.bls_sig = Signature::try_new(key.private_key(), share.message_digest.as_ref())
                .expect("sign exact current-view merge share")
                .payload()
                .to_vec();
            assert_eq!(
                lane_work
                    .accept_relay_message(LaneRelayMessage::MergeSignature(follower), target_view),
                V2LaneIngressOutcome::Inserted
            );
        }
        let header = lane_work
            .merge_carrier_context_header(target_view)
            .expect("current-view merge header");
        let (_, entry, selected_header) = fixture
            .state
            .select_pending_certified_merge_entry_for_round(
                &header,
                candidate.epoch_id,
                crate::state::PendingCertifiedMergeSelection::Any,
                active.context().mode,
            )
            .expect("select through canonical candidate attachment path")
            .expect("normal quorum ingress persisted the certified entry");
        assert_eq!(selected_header, header);
        assert_eq!(crate::merge::MergeLedgerCandidate::from(&entry), candidate);
        let service = V2ApplyService::new(
            Arc::clone(&fixture.state),
            Arc::clone(&queue),
            Arc::clone(&fixture.kura),
            None,
            None,
            fixture.service.block_cadence,
            fixture.service.genesis_account.clone(),
            events_sender,
            fixture.service.validator_set_pops.clone(),
        );
        let mut merge = terminal_cycle_merge_apply_fixture(
            &fixture,
            &service,
            active.context(),
            &entry,
            &header,
            &keys,
        );
        service
            .execute(&merge.context, &mut merge.store, &merge.task)
            .expect("apply the third source through its certified merge carrier");
        assert_eq!(fixture.state.committed_height(), 5);
        assert!(
            entrypoints
                .iter()
                .all(|hash| fixture.state.has_committed_entrypoint(*hash))
        );
        assert_eq!(queue.queued_len(), 0);
        assert!(queue.live_lane_reservations().is_empty());
        let receipt = fixture
            .kura
            .read_lane_block_application_receipt(descriptor.lane_id, 3)
            .expect("exact third-source economic receipt");
        assert_eq!(receipt.proposal, certificate.proposal);
        assert_eq!(receipt.application_block_hash, merge.body.hash());
        assert_eq!(receipt.application_block_height, 5);
    }
);

/// Certify the actual ordinary body with its canonical planned lane descriptor.
fn ordinary_frontier_certificate(
    fixture: &ApplyFixture,
    ordinary: &SuccessorApplyFixture,
    keys: &[KeyPair],
) -> iroha_data_model::block::consensus::LaneBlockCertificateV1 {
    let transaction = ordinary
        .body
        .external_transactions()
        .next()
        .expect("ordinary fixture has one real transaction")
        .clone();
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
    let routing = fixture
        .service
        .queue
        .route_plan_with_state(&accepted, fixture.state.as_ref())
        .expect("route exact ordinary body");
    let route = routing.coordinator_route();
    let entrypoint = Hash::from(accepted.hash_as_entrypoint());
    let plan = super::super::lane_planner::prepare_v2_lane_payload_plan(
        fixture.state.as_ref(),
        fixture.kura.as_ref(),
        &ordinary.context,
        0,
        &ordinary.context.roster[usize::try_from(ordinary.context.leader(0)).unwrap()].validator,
        std::slice::from_ref(&route),
        std::slice::from_ref(&entrypoint),
    )
    .expect("rederive exact ordinary proposal before Apply");
    assert!(plan.unavailable_indices.is_empty());
    assert_eq!(
        plan.ownerships,
        ordinary
            .body
            .execution_context()
            .unwrap()
            .lane_payload_ownerships
    );
    assert_eq!(plan.proposals.len(), 1);
    let proposal = plan.proposals[0].clone().with_payload_block_hint(
        iroha_data_model::block::consensus::LaneBlockProposalPayloadHintV1 {
            proposal_height: ordinary.context.height,
            proposal_view: 0,
            proposal_block_hash: ordinary.body.hash(),
        },
    );
    let qc = |phase| {
        let votes = keys[..3]
            .iter()
            .map(|key| {
                let body = proposal.vote_body(phase);
                crate::lane_consensus::LaneBlockVoteV1 {
                    bls_signature: Signature::try_new(
                        key.private_key(),
                        &body.signature_preimage(),
                    )
                    .expect("sign exact ordinary lane vote")
                    .payload()
                    .to_vec(),
                    body,
                    signer: PeerId::new(key.public_key().clone()),
                    payload_availability_vote: None,
                }
            })
            .collect::<Vec<_>>();
        crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            proposal.vote_body(phase),
            proposal.descriptor.validator_set.clone(),
            &votes,
        )
        .expect("aggregate exactly three ordinary lane votes")
    };
    iroha_data_model::block::consensus::LaneBlockCertificateV1 {
        prepare_qc: qc(CertPhase::Prepare),
        commit_qc: qc(CertPhase::Commit),
        proposal,
    }
}

/// Use the same finite lane-work limits as the existing historical Apply fixture.
fn ordinary_frontier_lane_work_limits() -> crate::sumeragi::v2_lane_work::V2LaneWorkLimits {
    use iroha_config::parameters::defaults::{network, sumeragi};
    let bound = NonZeroUsize::new(8).expect("finite lane work bound");
    crate::sumeragi::v2_lane_work::V2LaneWorkLimits::new(
        bound,
        bound,
        bound,
        bound,
        bound,
        bound,
        bound,
        network::MAX_FRAME_BYTES_CONSENSUS,
        network::MAX_FRAME_BYTES_BLOCK_SYNC,
        sumeragi::V2_AUTHENTICATED_MERGE_QC_CAPACITY,
        sumeragi::V2_MERGE_LEADER_BODY_FRAME_HEADROOM_BYTES,
        sumeragi::V2_AUTONOMOUS_CARRIER_HEADROOM_BYTES,
        sumeragi::V2_AUTONOMOUS_PRODUCER_RECHECK,
        Duration::from_millis(10),
        Duration::from_secs(1),
        sumeragi::V2_HISTORICAL_RECOVERY_STUCK_ATTEMPTS,
        sumeragi::V2_HISTORICAL_RECOVERY_RETRY_TIER_ATTEMPTS,
        sumeragi::V2_HISTORICAL_RECOVERY_MAX_RETRY_TIER,
        sumeragi::V2_SIDECAR_SERVICE_BURST,
        crate::merge_sidecar::MergeSidecarLimits::defaults(),
        crate::merge_sidecar::MergeSigningGuardLimits::defaults(),
        crate::native_amx::NativeAmxSigningGuardLimits::new(
            sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_CAPACITY,
            sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_BYTES,
            sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_ANCHOR_BYTES,
        )
        .expect("default Native AMX signing bounds"),
    )
}

/// Build a real three-of-four timeout certificate for the current frozen context.
fn ordinary_frontier_timeout_certificate(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    view: u64,
) -> wire::TimeoutCertificate {
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view,
    };
    let signers = vec![0, 1, 2];
    let preimage = wire::TimeoutVote {
        round,
        highest_prepare_qc: None,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let signatures = keys[..3]
        .iter()
        .map(|key| {
            Signature::try_new(key.private_key(), &preimage)
                .expect("sign exact timeout vote")
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate three-of-four timeout votes"),
        }],
    }
}
