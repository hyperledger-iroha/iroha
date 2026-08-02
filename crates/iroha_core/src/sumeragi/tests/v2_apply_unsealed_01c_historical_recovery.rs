v2_apply_test!(
    historical_autonomous_recovery_reaches_exactly_once_canonical_merge_application,
    {
        use iroha_data_model::block::consensus::SumeragiAutonomousLaneExecutionStage;

        let fixture = ApplyFixture::new_with_lane_lifecycle();
        let assert_fixture_execution_policy = |stage: &str| {
            assert_eq!(
                crate::sumeragi::v2_recovery::committed_execution_policy_hash(
                    fixture.state.as_ref(),
                )
                .expect("derive live apply-fixture execution policy"),
                fixture.context.execution_policy_hash,
                "apply-fixture execution policy drifted {stage}"
            );
        };
        assert_fixture_execution_policy("after setup");
        let mut genesis_store = fixture.reopen_body_store();
        fixture
            .execute(&mut genesis_store)
            .expect("commit parent before historical autonomous end-to-end recovery");
        assert_fixture_execution_policy("after parent commit");
        let context = successor_height_context(&fixture);
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
        let queue = fixture_queue(fixture.state.as_ref(), events_sender.clone());
        let journal_dir =
            tempfile::tempdir().expect("historical autonomous end-to-end journal directory");
        let plan_path = journal_dir.path().join("queue-plans.norito");
        let reservation_path = journal_dir.path().join("lane-reservations.norito");
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install historical autonomous end-to-end QueuePlan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install historical autonomous end-to-end reservation journal");
        let autonomous_asset_id = AssetId::new(
            fixture_reserve_asset_definition(),
            fixture.service.genesis_account.clone(),
        );
        let autonomous_balance = || {
            let view = fixture.state.view();
            view.world
                .asset(&autonomous_asset_id)
                .ok()
                .map(|asset| (**asset.value()).clone())
        };
        assert_eq!(autonomous_balance(), None);
        let (payload, expected_fifo) = reserve_canonical_successor_autonomous_mint_batch(
            &fixture,
            &queue,
            &context,
            2,
            autonomous_asset_id.clone(),
        );
        assert_fixture_execution_policy("after autonomous queue reservation");
        let origin_descriptor = payload.origin_proposal.descriptor.clone();
        let expected_reservation_keys = payload.reservation_keys.clone();
        let expected_routing_plans = payload.routing_plans.clone();
        let expected_reservation_bytes = expected_reservation_keys
            .iter()
            .map(norito::encode_canonical)
            .collect::<Result<Vec<_>, _>>()
            .expect("encode exact FIFO reservation bytes");
        let expected_routing_bytes = expected_routing_plans
            .iter()
            .map(norito::encode_canonical)
            .collect::<Result<Vec<_>, _>>()
            .expect("encode exact autonomous routing-plan bytes");
        let reservation_group = lane_queue_reservation_group_binding_from_ordered_keys(
            expected_reservation_keys.iter(),
        )
        .expect("bind the exact autonomous reservation group");
        assert_eq!(
            expected_fifo,
            expected_reservation_keys
                .iter()
                .map(|key| key.signed_transaction_hash)
                .collect::<Vec<_>>(),
            "reservation keys must retain the original FIFO transaction order"
        );
        assert!(expected_reservation_keys.iter().all(|key| {
            key.lane_id == origin_descriptor.lane_id
                && key.dataspace_id == origin_descriptor.dataspace_id
                && key.lane_incarnation == origin_descriptor.lane_incarnation
                && key.proposal_height == origin_descriptor.proposal_height
                && key.lane_block_height == origin_descriptor.lane_block_height
                && key.lane_block_view == origin_descriptor.lane_block_view
        }));

        let diagnostic_at = |queue: &Queue, stage: SumeragiAutonomousLaneExecutionStage| {
            let rows = fixture
                .state
                .autonomous_lane_execution_diagnostics_with_queue(queue)
                .expect("derive State/Kura/Queue autonomous execution diagnostics");
            let row = rows
                .iter()
                .find(|row| {
                    row.lane_id == reservation_group.identity.lane_id
                        && row.dataspace_id == reservation_group.identity.dataspace_id
                        && row.lane_incarnation == reservation_group.identity.lane_incarnation
                        && row.lane_block_height == reservation_group.identity.lane_block_height
                        && row.lane_block_view == reservation_group.identity.lane_block_view
                        && row.proposal_height == reservation_group.identity.proposal_height
                        && row.reservation_owner_hash
                            == reservation_group.identity.reservation_owner_hash
                        && row.proposal_identity_hash
                            == reservation_group.identity.proposal_identity_hash
                        && row.reservation_group_hash == reservation_group.reservation_group_hash
                })
                .copied()
                .expect("diagnostics retain the exact autonomous reservation identity");
            assert_eq!(row.reservation_count, reservation_group.reservation_count);
            assert_eq!(row.transaction_count, reservation_group.reservation_count);
            assert_eq!(row.highest_durable_stage, stage);
            row.validate()
                .expect("autonomous execution diagnostics row remains self-consistent");
            row
        };

        fixture
            .kura
            .install_lane_incarnation_marker_for_test(
                RuntimeLaneConfig::default().primary(),
                origin_descriptor.lane_incarnation,
                0,
            )
            .expect("install historical autonomous end-to-end lane marker");
        let envelope = crate::lane_consensus::autonomous_lane_payload_envelope(
            &payload,
            payload.chain_id_hash,
            payload.epoch,
        )
        .expect("encode historical autonomous end-to-end envelope");
        let mut successor =
            build_successor_apply_fixture_with_autonomous_payloads(&fixture, vec![envelope]);
        fixture
            .service
            .execute(&successor.context, &mut successor.store, &successor.task)
            .expect("finalize the historical autonomous control-only carrier");
        assert_eq!(fixture.state.committed_height(), 2);
        assert_fixture_execution_policy("after control-only carrier commit");
        let reserved_row = diagnostic_at(
            &queue,
            SumeragiAutonomousLaneExecutionStage::ReservationsDurable,
        );
        assert_eq!(reserved_row.proposal_view, None);
        assert_eq!(reserved_row.proposal_hash, None);
        assert_eq!(reserved_row.descriptor_hash, None);
        assert_eq!(reserved_row.executable_payload_hash, None);
        assert_eq!(reserved_row.source_bundle_hash, None);
        assert_eq!(reserved_row.merge_entry_hash, None);
        assert_eq!(reserved_row.application_block_height, None);

        let active_context = verified_successor_context_after_fixture_tip(&fixture);
        assert_eq!(
            active_context.context().execution_policy_hash,
            crate::sumeragi::v2_recovery::committed_execution_policy_hash(fixture.state.as_ref(),)
                .expect("derive active-height live execution policy"),
            "verified active-height context must bind the live execution policy"
        );
        drop(queue);
        let queue = fixture_queue(fixture.state.as_ref(), events_sender.clone());
        let replay = queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("replay historical autonomous end-to-end reservation owners");
        assert_eq!(replay.restored, expected_reservation_keys.len());
        assert_eq!(
            replay.awaiting_transaction_replay,
            expected_reservation_keys.len()
        );
        assert_eq!(replay.commit_barriers, 0);
        assert_eq!(replay.release_barriers, 0);
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install replayed historical autonomous end-to-end QueuePlan journal");
        queue
            .replay_plan_journal(fixture.state.as_ref())
            .expect("replay historical autonomous end-to-end QueuePlan payloads");
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("read exact replayed autonomous ownership")
                .ordered_groups,
            vec![LaneQueueReservationReconciliationGroupV1 {
                identity: reservation_group.identity,
                ordered_keys: expected_reservation_keys.clone(),
            }]
        );

        let planning = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &active_context,
            None,
        )
        .expect("plan exact historical autonomous recovery from the canonical carrier");
        let LaneReservationReconciliationPlanning::InstallHistoricalAutonomousRecoveries(installs) =
            planning
        else {
            panic!("canonical prior-height autonomous ownership must require installation");
        };
        assert_eq!(installs.len(), 1);
        let install = installs
            .into_iter()
            .next()
            .expect("one historical autonomous recovery installation");
        assert!(install.has_valid_identity());
        assert_eq!(install.historical_context, successor.context);
        assert_eq!(install.canonical_body.height, 2);
        assert_eq!(install.canonical_body.block_hash, successor.body.hash());
        assert_eq!(
            install.payload.origin_proposal.descriptor,
            origin_descriptor
        );
        assert_eq!(install.payload.entrypoints, payload.entrypoints);
        assert_eq!(install.payload.entrypoint_hashes, payload.entrypoint_hashes);
        assert_eq!(install.payload.reservation_keys, expected_reservation_keys);
        assert_eq!(install.payload.routing_plans, expected_routing_plans);
        assert_eq!(
            install.reservation_group,
            LaneQueueReservationReconciliationGroupV1 {
                identity: reservation_group.identity,
                ordered_keys: expected_reservation_keys.clone(),
            }
        );
        let anchored_proposal = install.payload.origin_proposal.clone();
        let anchored_hint = anchored_proposal
            .payload_block_hint
            .expect("historical recovery binds the exact canonical carrier hint");
        assert_eq!(anchored_hint.proposal_height, 2);
        assert_eq!(anchored_hint.proposal_view, 0);
        assert_eq!(anchored_hint.proposal_block_hash, successor.body.hash());

        assert_eq!(
            install_historical_autonomous_lane_recovery(
                fixture.state.as_ref(),
                fixture.kura.as_ref(),
                &install,
            )
            .expect("install exact historical autonomous execution input"),
            HistoricalAutonomousLaneRecoveryInstallOutcome::Installed
        );
        assert_eq!(
            install_historical_autonomous_lane_recovery(
                fixture.state.as_ref(),
                fixture.kura.as_ref(),
                &install,
            )
            .expect("retry exact historical autonomous installation"),
            HistoricalAutonomousLaneRecoveryInstallOutcome::AlreadyInstalled
        );
        let replanning = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &active_context,
            None,
        )
        .expect("replan exact installed historical autonomous recovery");
        let LaneReservationReconciliationPlanning::Ready(plan) = replanning else {
            panic!("installed historical autonomous recovery must make ownership ready");
        };
        assert_eq!(
                apply_lane_reservation_reconciliation_plan(
                    queue.as_ref(),
                    fixture.kura.as_ref(),
                    plan,
                )
                .expect("publish exact historical autonomous reservation reconciliation"),
                LaneReservationReconciliationSummary {
                    recovered: expected_reservation_keys.len(),
                    retained_historical_recovery: expected_reservation_keys.len(),
                    ..LaneReservationReconciliationSummary::default()
                }
            );
        assert!(!queue.lane_reservation_startup_reconciliation_pending());
        let payload_row = diagnostic_at(
            &queue,
            SumeragiAutonomousLaneExecutionStage::ExecutablePayloadDurable,
        );
        assert_eq!(payload_row.proposal_view, Some(0));
        assert_eq!(
            payload_row.proposal_hash,
            Some(anchored_proposal.proposal_hash)
        );
        assert_eq!(
            payload_row.descriptor_hash,
            Some(anchored_proposal.descriptor.descriptor_hash)
        );
        assert_eq!(
            payload_row.executable_payload_hash,
            Some(install.payload.payload_hash)
        );

        let validator_keys = fixture_validator_keys();
        let nonzero = NonZeroUsize::new(8).expect("non-zero lane-work bound");
        let limits = crate::sumeragi::v2_lane_work::V2LaneWorkLimits::new(
                nonzero,
                nonzero,
                nonzero,
                nonzero,
                nonzero,
                nonzero,
                nonzero,
                iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONSENSUS,
                iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC,
                iroha_config::parameters::defaults::sumeragi::V2_AUTHENTICATED_MERGE_QC_CAPACITY,
                iroha_config::parameters::defaults::sumeragi::V2_MERGE_LEADER_BODY_FRAME_HEADROOM_BYTES,
                iroha_config::parameters::defaults::sumeragi::V2_AUTONOMOUS_CARRIER_HEADROOM_BYTES,
                iroha_config::parameters::defaults::sumeragi::V2_AUTONOMOUS_PRODUCER_RECHECK,
                iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_STUCK_ATTEMPTS,
                iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_RETRY_TIER_ATTEMPTS,
                iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_MAX_RETRY_TIER,
                iroha_config::parameters::defaults::sumeragi::V2_SIDECAR_SERVICE_BURST,
                crate::merge_sidecar::MergeSidecarLimits::defaults(),
                crate::merge_sidecar::MergeSigningGuardLimits::defaults(),
                crate::native_amx::NativeAmxSigningGuardLimits::new(
                    iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_CAPACITY,
                    iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_BYTES,
                    iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_ANCHOR_BYTES,
                )
                .expect("default Native AMX signing limits"),
            );
        let local_key = validator_keys[0].clone();
        let local_peer = PeerId::new(local_key.public_key().clone());
        let mut lane_work = crate::sumeragi::v2_lane_work::V2LaneWorkAdapter::new(
            active_context.context().clone(),
            local_peer,
            local_key,
            true,
            Arc::clone(&fixture.state),
            Arc::clone(&fixture.kura),
            limits,
            None,
        )
        .expect("hydrate the historical autonomous recovery in live lane work");

        let availability_body = crate::lane_consensus::lane_payload_availability_body(
            &install.payload,
            &anchored_proposal,
            install.payload.chain_id_hash,
            install.payload.epoch,
        )
        .expect("build exact historical autonomous READY body");
        let prepare_body = anchored_proposal.vote_body(CertPhase::Prepare);
        let prepare_votes = validator_keys
            .iter()
            .take(3)
            .map(|key| {
                let availability_vote =
                    crate::lane_consensus::LanePayloadAvailabilityVoteV1::new_signed(
                        availability_body.clone(),
                        PeerId::new(key.public_key().clone()),
                        fixture.service.validator_set_pops.clone(),
                        key.private_key(),
                    )
                    .expect("sign exact historical autonomous READY vote");
                crate::lane_consensus::LaneBlockVoteV1 {
                    body: prepare_body.clone(),
                    signer: PeerId::new(key.public_key().clone()),
                    bls_signature: Signature::try_new(
                        key.private_key(),
                        &prepare_body.signature_preimage(),
                    )
                    .expect("sign exact historical autonomous Prepare vote")
                    .payload()
                    .to_vec(),
                    payload_availability_vote: Some(availability_vote),
                }
            })
            .collect::<Vec<_>>();
        let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            prepare_body,
            anchored_proposal.descriptor.validator_set.clone(),
            &prepare_votes,
        )
        .expect("aggregate exact historical autonomous PrepareQC");
        let commit_body = anchored_proposal.vote_body(CertPhase::Commit);
        let commit_votes = validator_keys
            .iter()
            .take(3)
            .map(|key| crate::lane_consensus::LaneBlockVoteV1 {
                body: commit_body.clone(),
                signer: PeerId::new(key.public_key().clone()),
                bls_signature: Signature::try_new(
                    key.private_key(),
                    &commit_body.signature_preimage(),
                )
                .expect("sign exact historical autonomous Commit vote")
                .payload()
                .to_vec(),
                payload_availability_vote: None,
            })
            .collect::<Vec<_>>();
        let commit_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            commit_body,
            anchored_proposal.descriptor.validator_set.clone(),
            &commit_votes,
        )
        .expect("aggregate exact historical autonomous CommitQC");
        let certificate = iroha_data_model::block::consensus::LaneBlockCertificateV1 {
            proposal: anchored_proposal.clone(),
            prepare_qc,
            commit_qc,
        };
        assert_eq!(
            lane_work.accept_lane_message(
                crate::sumeragi::InboundBlockMessage::new(
                    crate::sumeragi::message::BlockMessage::LaneBlockCertificate(Box::new(
                        certificate,
                    )),
                    Some(PeerId::new(validator_keys[0].public_key().clone())),
                ),
                0,
            ),
            crate::sumeragi::v2_lane_work::V2LaneIngressOutcome::Inserted
        );
        assert!(matches!(
            lane_work
                .service_next_historical_recovery()
                .expect("persist exact historical autonomous certificate and bundle"),
            crate::sumeragi::v2_lane_work::HistoricalRecoveryServiceOutcome::Complete(_)
        ));
        assert!(!lane_work.has_pending_historical_recovery());

        let source = fixture
            .kura
            .durable_autonomous_lane_merge_source(
                origin_descriptor.lane_id,
                origin_descriptor.lane_block_height,
                install.payload.chain_id_hash,
                install.payload.epoch,
            )
            .expect("read the complete durable autonomous merge source");
        assert_eq!(source.bundle.executable_payload(), &install.payload);
        assert_eq!(source.input.proposal, anchored_proposal);
        assert_eq!(source.input.reservation_keys, expected_reservation_keys);
        assert_eq!(source.input.routing_plans, expected_routing_plans);
        assert_eq!(
            source.input.autonomous_payload_hash,
            Some(install.payload.payload_hash)
        );
        assert_eq!(
            source.source_bundle,
            source
                .bundle
                .encode_framed()
                .expect("re-encode the exact autonomous merge source")
        );
        assert_eq!(
            source.bundle_hash,
            source
                .bundle
                .bundle_hash()
                .expect("re-hash the exact autonomous merge source")
        );
        assert_eq!(
            Kura::decode_autonomous_lane_merge_bundle(
                &source.source_bundle,
                install.payload.chain_id_hash,
                install.payload.epoch,
            )
            .expect("decode the exact durable autonomous merge source"),
            source.bundle
        );
        let certified_row = diagnostic_at(
            &queue,
            SumeragiAutonomousLaneExecutionStage::CertifiedBundleDurable,
        );
        assert_eq!(certified_row.source_bundle_hash, Some(source.bundle_hash));
        assert!(fixture.state.has_pending_merge_execution_sources());

        let application_header = lane_work
            .merge_carrier_context_header(0)
            .expect("derive the exact canonical merge application header");
        let candidate = fixture
            .state
            .build_merge_execution_candidate(application_header.clone())
            .expect("select the exact contiguous certified autonomous source");
        assert_eq!(candidate.carrier_height, 3);
        assert_eq!(candidate.carrier_parent_hash, successor.body.hash());
        let batch = candidate
            .execution_batch
            .as_ref()
            .expect("autonomous candidate carries one execution batch");
        assert_eq!(batch.application_block_header, application_header);
        assert_eq!(batch.lanes.len(), 1);
        let execution = &batch.lanes[0];
        assert_eq!(execution.source_bundle, source.source_bundle);
        assert_eq!(execution.source_bundle_hash, source.bundle_hash);
        assert_eq!(execution.proposal, anchored_proposal);
        assert_eq!(execution.origin_proposal, install.payload.origin_proposal);
        assert_eq!(
            execution.autonomous_payload_hash,
            install.payload.payload_hash
        );
        assert_eq!(execution.reservation_keys, expected_reservation_bytes);
        assert_eq!(execution.routing_plans, expected_routing_bytes);
        assert_eq!(
            execution.entrypoint_hashes,
            install.payload.entrypoint_hashes
        );
        assert_eq!(execution.entrypoints, install.payload.entrypoints);
        assert!(
            execution.results.iter().all(|result| result.is_ok()),
            "every autonomous mint must pre-execute successfully"
        );
        assert_eq!(
            execution.proposal.descriptor.lane_id,
            origin_descriptor.lane_id
        );
        assert_eq!(
            execution.proposal.descriptor.dataspace_id,
            origin_descriptor.dataspace_id
        );
        assert_eq!(
            execution.proposal.descriptor.lane_incarnation,
            origin_descriptor.lane_incarnation
        );
        assert_eq!(
            execution.proposal.descriptor.lane_block_height,
            origin_descriptor.lane_block_height
        );

        let validator_set = active_context
            .context()
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let validator_set_hash = HashOf::new(&validator_set);
        let merge_digest = crate::merge::merge_qc_message_digest(
            &active_context.context().chain_id,
            &candidate,
            VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash,
        );
        let mut bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
        let mut merge_signatures = Vec::new();
        let mut signer_proofs = Vec::new();
        for index in 0_usize..3 {
            bitmap[index / 8] |= 1_u8 << (index % 8);
            merge_signatures.push(
                Signature::try_new(validator_keys[index].private_key(), merge_digest.as_ref())
                    .expect("sign exact autonomous merge candidate")
                    .payload()
                    .to_vec(),
            );
            signer_proofs.push(iroha_data_model::merge::MergeSignerProof {
                signer: u32::try_from(index).expect("merge signer index fits u32"),
                proof_of_possession: iroha_crypto::bls_normal_pop_prove(
                    validator_keys[index].private_key(),
                )
                .expect("derive exact autonomous merge signer PoP"),
            });
        }
        let merge_signature_refs = merge_signatures
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let merge_qc = MergeQuorumCertificate::new(
            candidate.view,
            candidate.epoch_id,
            candidate.carrier_height,
            candidate.carrier_parent_hash,
            crate::merge::merge_chain_id_digest(&active_context.context().chain_id),
            VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash,
            validator_set,
            bitmap,
            signer_proofs,
            iroha_crypto::bls_normal_aggregate_signatures(&merge_signature_refs)
                .expect("aggregate exact autonomous merge signatures"),
            merge_digest,
        );
        let entry = candidate.clone().into_entry(merge_qc);
        crate::sumeragi::v2_lane_work::authenticate_merge_entry_for_height_context(
            active_context.context(),
            &entry,
        )
        .expect("authenticate exact merge QC against the frozen height context");
        fixture
            .state
            .validate_certified_merge_entry_for_global_order(&entry)
            .expect("validate exact autonomous merge entry against current WSV");
        let entry_hash = crate::merge::merge_ledger_entry_hash(&entry);
        assert_eq!(
            fixture
                .kura
                .persist_pending_certified_merge_entry(&entry)
                .expect("persist the exact certified autonomous merge sidecar"),
            entry_hash
        );
        let merge_row = diagnostic_at(
            &queue,
            SumeragiAutonomousLaneExecutionStage::MergeCandidateDurable,
        );
        assert_eq!(merge_row.merge_entry_hash, Some(entry_hash));

        let service = V2ApplyService::new(
            Arc::clone(&fixture.state),
            Arc::clone(&queue),
            Arc::clone(&fixture.kura),
            None,
            None,
            fixture.service.chain_id.clone(),
            fixture.service.block_cadence,
            fixture.service.genesis_account.clone(),
            events_sender.clone(),
            fixture.service.validator_set_pops.clone(),
        );
        let parent = fixture
            .kura
            .get_block(NonZeroUsize::new(2).expect("non-zero carrier parent height"))
            .expect("read exact autonomous control carrier");
        let (_, carrier_time_source) = TimeSource::new_mock(application_header.creation_time());
        let confidential_features = {
            let state_view = fixture.state.view();
            let digest = crate::state::compute_confidential_feature_digest(
                state_view.world(),
                &state_view.zk,
                state_view.sccp_registry.as_ref(),
                active_context.context().height,
            );
            (!digest.is_empty()).then_some(digest)
        };
        let execution_context = BlockExecutionContextBundle::new(Vec::new())
            .with_merge_entry(CertifiedMergeLedgerReference::new(&entry));
        let leader_index = active_context.context().leader(0);
        let leader = usize::try_from(leader_index).expect("height-three leader index");
        let carrier = BlockBuilder::new_with_time_source(Vec::new(), carrier_time_source)
            .chain(0, Some(parent.as_ref()))
            .bind_certified_merge_application_context(&application_header)
            .expect("bind the exact certified autonomous application context")
            .with_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
                &fixture.state.nexus_snapshot(),
                active_context.context().height,
            )))
            .with_confidential_features(confidential_features)
            .with_execution_context(Some(execution_context))
            .try_sign_with_index(
                validator_keys[leader].private_key(),
                u64::from(leader_index),
            )
            .expect("sign the exact autonomous merge carrier")
            .unpack(|_| {});
        let carrier = SignedBlock::from(carrier);
        assert_eq!(carrier.header().height().get(), 3);
        assert_eq!(
            carrier.header().prev_block_hash(),
            Some(successor.body.hash())
        );
        assert_eq!(
            carrier.header().creation_time(),
            application_header.creation_time()
        );
        assert_eq!(carrier.header().view_change_index(), 0);
        let canonical_wire = carrier.encode_wire().expect("encode exact merge carrier");
        let subject = wire::BlockSubject {
            parent_block_hash: Some(successor.body.hash()),
            block_hash: carrier.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let round = wire::ConsensusRound {
            context_id: active_context.context().id(),
            height: 3,
            view: 0,
        };
        let manifest = wire::PayloadManifest::derive(
            active_context.context(),
            round,
            subject,
            u64::try_from(canonical_wire.len()).expect("merge carrier length fits u64"),
            std::slice::from_ref(&canonical_wire),
        )
        .expect("derive exact autonomous merge carrier manifest");
        let execution_commitment = service
            .validate_candidate(active_context.context(), &carrier)
            .expect("deterministically re-execute the certified autonomous batch");
        let mut certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let global_signatures = certificate
            .signers
            .iter()
            .map(|index| {
                Signature::try_new(
                    validator_keys[usize::try_from(*index).expect("global signer index")]
                        .private_key(),
                    &preimage,
                )
                .expect("sign exact autonomous global Commit vote")
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &global_signatures
                .iter()
                .map(Vec::as_slice)
                .collect::<Vec<_>>(),
        )
        .expect("aggregate exact autonomous global Commit votes");
        let body_root = tempfile::tempdir().expect("height-three merge body-store directory");
        let mut body_store = V2BodyStore::open(body_root.path(), active_context.context().clone())
            .expect("open height-three merge body store");
        let durable = body_store
            .store(manifest, canonical_wire)
            .expect("persist exact autonomous merge carrier body");
        let validated = body_store
            .validate(&durable, |candidate| {
                service.validate_candidate(active_context.context(), candidate)
            })
            .expect("persist exact autonomous merge validation marker");
        let task = ApplyTask::for_test(
            3,
            EventTag::new(3, 0, Generation::new(3)),
            subject,
            certificate,
            validated,
        );

        assert_eq!(
            queue.live_lane_reservations().len(),
            expected_reservation_keys.len()
        );
        assert!(queue.lane_reservation_commit_barriers().is_empty());
        assert!(queue.lane_reservation_release_barriers().is_empty());
        service
            .execute(active_context.context(), &mut body_store, &task)
            .expect("atomically apply the exact canonical autonomous merge carrier");
        assert_eq!(fixture.state.committed_height(), 3);
        assert_eq!(
            fixture.state.merge_ledger.snapshot(),
            vec![Arc::new(entry.clone())]
        );
        for transaction_hash in &expected_fifo {
            assert!(fixture.state.has_committed_transaction(*transaction_hash));
        }
        assert_eq!(
            autonomous_balance(),
            Some(iroha_primitives::numeric::Quantity::from(2_u32)),
            "two additive autonomous effects must enter canonical WSV exactly once"
        );
        assert!(queue.live_lane_reservations().is_empty());
        assert!(queue.lane_reservation_commit_barriers().is_empty());
        assert!(queue.lane_reservation_release_barriers().is_empty());
        for key in &expected_reservation_keys {
            assert!(!queue.has_durable_plan_claim_for_test(key.signed_transaction_hash));
        }
        assert!(
            queue.lane_reservation_group_is_finalized_for_diagnostics(&expected_reservation_keys)
        );

        let receipt = fixture
            .kura
            .read_lane_block_application_receipt(
                origin_descriptor.lane_id,
                origin_descriptor.lane_block_height,
            )
            .expect("read one durable autonomous merge application receipt");
        assert_eq!(
            receipt.format,
            crate::kura::LaneBlockApplicationReceiptArtifactFormat::MergeExecution
        );
        assert_eq!(receipt.proposal, anchored_proposal);
        assert_eq!(receipt.application_block_height, 3);
        assert_eq!(receipt.application_block_hash, carrier.hash());
        assert_eq!(receipt.merge_epoch_id, Some(entry.epoch_id));
        assert_eq!(receipt.merge_entry_hash, Some(entry_hash));
        assert_eq!(receipt.merge_carrier_block_height, Some(3));
        assert_eq!(receipt.merge_carrier_block_hash, Some(carrier.hash()));
        assert_eq!(receipt.merge_source_bundle_hash, Some(source.bundle_hash));
        assert_eq!(
            receipt.merge_batch_identity_hash,
            Some(crate::merge::merge_execution_batch_identity_hash(batch))
        );
        assert_eq!(receipt.merge_batch_hash, Some(batch.batch_hash));
        assert_eq!(receipt.merge_base_state_hash, Some(batch.base_state_hash));
        assert_eq!(receipt.merge_write_set_root, Some(batch.write_set_root));
        assert_eq!(
            receipt.merge_expected_post_state_hash,
            Some(batch.expected_post_state_hash)
        );
        assert_eq!(
            receipt.merge_settlement_hash,
            Some(execution.settlement_hash)
        );
        let receipt_hash = HashOf::new(&receipt);
        let finalized_row =
            diagnostic_at(&queue, SumeragiAutonomousLaneExecutionStage::QueueFinalized);
        assert_eq!(
            finalized_row.proposal_hash,
            Some(anchored_proposal.proposal_hash)
        );
        assert_eq!(
            finalized_row.executable_payload_hash,
            Some(install.payload.payload_hash)
        );
        assert_eq!(finalized_row.source_bundle_hash, Some(source.bundle_hash));
        assert_eq!(finalized_row.merge_entry_hash, Some(entry_hash));
        assert_eq!(finalized_row.application_block_height, Some(3));
        assert_eq!(finalized_row.application_block_hash, Some(carrier.hash()));
        assert_eq!(finalized_row.stuck_reason, None);

        service
            .execute(active_context.context(), &mut body_store, &task)
            .expect("retry the exact canonical autonomous merge application");
        assert_eq!(fixture.state.committed_height(), 3);
        assert_eq!(
            fixture.state.merge_ledger.snapshot(),
            vec![Arc::new(entry.clone())]
        );
        assert_eq!(
            autonomous_balance(),
            Some(iroha_primitives::numeric::Quantity::from(2_u32)),
            "an exact Apply retry must not execute autonomous effects twice"
        );
        assert_eq!(
            fixture
                .kura
                .read_lane_block_application_receipt(
                    origin_descriptor.lane_id,
                    origin_descriptor.lane_block_height,
                )
                .as_ref()
                .map(HashOf::new),
            Some(receipt_hash),
            "exact Apply retry must retain one byte-identical durable receipt"
        );
        assert!(queue.live_lane_reservations().is_empty());
        assert!(queue.lane_reservation_commit_barriers().is_empty());
        assert!(queue.lane_reservation_release_barriers().is_empty());
        assert!(
            queue.lane_reservation_group_is_finalized_for_diagnostics(&expected_reservation_keys)
        );

        drop(service);
        drop(queue);
        let terminal_queue = fixture_queue(fixture.state.as_ref(), events_sender);
        let terminal_replay = terminal_queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("replay terminal autonomous reservation journal");
        assert_eq!(terminal_replay.restored, 0);
        assert_eq!(terminal_replay.awaiting_transaction_replay, 0);
        assert_eq!(terminal_replay.commit_barriers, 0);
        assert_eq!(terminal_replay.release_barriers, 0);
        terminal_queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install terminal autonomous QueuePlan journal");
        terminal_queue
            .replay_plan_journal(fixture.state.as_ref())
            .expect("replay terminal autonomous QueuePlan tombstones");
        assert!(terminal_queue.live_lane_reservations().is_empty());
        assert!(terminal_queue.lane_reservation_commit_barriers().is_empty());
        assert!(
            terminal_queue
                .lane_reservation_release_barriers()
                .is_empty()
        );
        for key in &expected_reservation_keys {
            assert!(!terminal_queue.has_durable_plan_claim_for_test(key.signed_transaction_hash));
        }
        assert!(
            terminal_queue
                .lane_reservation_group_is_finalized_for_diagnostics(&expected_reservation_keys)
        );
        assert_eq!(
            autonomous_balance(),
            Some(iroha_primitives::numeric::Quantity::from(2_u32)),
            "terminal journal replay must not execute autonomous effects twice"
        );
        let terminal_row = diagnostic_at(
            &terminal_queue,
            SumeragiAutonomousLaneExecutionStage::QueueFinalized,
        );
        assert_eq!(terminal_row, finalized_row);
    }
);
