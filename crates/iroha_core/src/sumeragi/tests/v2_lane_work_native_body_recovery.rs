struct NativeBodyRecoveryPayload {
    transaction: iroha_data_model::transaction::signed::SignedTransaction,
    request: NativeAmxAttestationRequestV2,
    receipt: NativeAmxReceipt,
    routing_plan: RoutingPlan,
    source_id: [u8; Hash::LENGTH],
    entrypoint_hash: HashOf<TransactionEntrypoint>,
}

#[test]
fn native_coordinator_successor_waits_for_missing_applied_half_without_losing_owner() {
    for missing_half in ["receipt", "manifest"] {
        let (mut adapter, _, lane_id, dataspace_id, previous) =
            native_coordinator_after_applied_participant_fixture();
        let request =
            native_coordinator_successor_request(&adapter, lane_id, dataspace_id, &previous);
        let previous_hash = request
            .participant_settlement
            .previous_native_settlement_hash();
        assert!(adapter.native_request_matches_context(&request, 0));
        assert!(
            crate::block::NativeAmxAuthorityContext::native_amx_participant_predecessor_is_current(
                &adapter.state.query_view(),
                &request.participant_proposal,
                previous_hash,
            )
            .expect("complete Native authority permits the exact successor")
        );
        assert_shared_lane_predecessor_is_applied(
            &adapter.state,
            &request.participant_proposal,
            true,
        );
        let slot = plan_autonomous_lane_reservation_slot(
            adapter.state.as_ref(),
            adapter.kura.as_ref(),
            &adapter.context,
            lane_id,
            dataspace_id,
        )
        .expect("actual successor coordinator slot");
        let relay = adapter
            .context
            .roster
            .iter()
            .map(|power| power.validator.clone())
            .find(|peer| peer != &slot.author)
            .expect("distinct authenticated physical relay");
        let mut routes = NetworkReplyRouteTestFixture::new(relay);
        assert!(adapter.register_native_request(
            request.body,
            slot.author.clone(),
            NativeAmxMessage::PrepareRequest(request.clone()),
        ));
        let request_owner = adapter.native_requests.get(&request.body).unwrap().clone();
        let source_claims = adapter.native_request_source_claims.clone();
        let slot_claims = adapter.native_request_slot_claims.clone();
        let effect_keys = adapter.effect_keys.clone();
        let effect_order = adapter
            .effects
            .iter()
            .map(lane_work_effect_key)
            .collect::<Vec<_>>();
        assert!(!effect_order.is_empty(), "retain an actual queued request");
        let retransmit_cursor = adapter.native_retransmit_cursor;
        let admission_cursor = adapter.native_request_admission_cursor;
        assert!(adapter.local_native_claims.is_empty());
        assert_eq!(
            adapter
                .native_signing_guard
                .as_ref()
                .unwrap()
                .record_count_for_test(),
            0
        );

        let artifact_dir = adapter
            .state
            .nexus_snapshot()
            .lane_config
            .entry(lane_id)
            .expect("actual participant storage route")
            .blocks_dir(adapter.kura.store_root())
            .join("lane_artifacts");
        let missing_path = artifact_dir.join(format!(
            "native_amx_{missing_half}_v1_00000000000000000001.norito"
        ));
        let saved = std::fs::read(&missing_path).expect("actual authenticated application half");
        assert!(!saved.is_empty());
        std::fs::remove_file(&missing_path).expect("interrupt only the highest application pair");
        let snapshot_artifacts = || {
            let mut inventory = BTreeMap::new();
            let mut directories = vec![artifact_dir.clone()];
            while let Some(directory) = directories.pop() {
                for entry in std::fs::read_dir(directory).expect("read actual artifact inventory") {
                    let entry = entry.expect("read artifact entry");
                    let path = entry.path();
                    let file_type = entry.file_type().expect("read artifact type");
                    let bytes = if file_type.is_dir() {
                        directories.push(path.clone());
                        None
                    } else {
                        assert!(file_type.is_file(), "fixture artifacts are direct files");
                        Some(std::fs::read(&path).expect("read actual artifact bytes"))
                    };
                    inventory.insert(path, bytes);
                }
            }
            inventory
        };
        let interrupted_artifacts = snapshot_artifacts();
        assert!(!adapter.output_guard.restart_required());
        assert_shared_lane_predecessor_is_applied(
            &adapter.state,
            &request.participant_proposal,
            false,
        );

        // Exercise the latching State authority wrapper first: partial publication
        // must remain a typed pending observation, not poison subsequent signing.
        assert!(
            !crate::block::NativeAmxAuthorityContext::native_amx_participant_predecessor_is_current(
                &adapter.state.query_view(),
                &request.participant_proposal,
                previous_hash,
            )
            .expect("a missing highest half is pending, not corrupt authority")
        );
        assert!(
            !adapter
                .native_coordinator_height_is_current(&request.body)
                .expect("pending Native authority blocks coordinator progression")
        );
        assert!(!adapter.native_request_matches_context(&request, 0));
        assert!(adapter.sign_native_request_once(&request, 0).is_none());
        let pending_reply = routes.mint(slot.author.clone());
        assert_eq!(
            adapter.accept_native_amx(
                slot.author.clone(),
                Some(pending_reply),
                NativeAmxMessage::PrepareRequest(request.clone()),
                0,
            ),
            V2LaneIngressOutcome::Rejected
        );
        assert!(!adapter.output_guard.restart_required());
        assert!(adapter.local_native_claims.is_empty());
        assert_eq!(
            adapter
                .native_signing_guard
                .as_ref()
                .unwrap()
                .record_count_for_test(),
            0,
            "pending evidence must not claim the durable signing slot"
        );
        assert_eq!(adapter.native_requests.len(), 1);
        let retained = adapter.native_requests.get(&request.body).unwrap();
        assert_eq!(retained.message, request_owner.message);
        assert_eq!(retained.expected_peers, request_owner.expected_peers);
        assert_eq!(adapter.native_request_source_claims, source_claims);
        assert_eq!(adapter.native_request_slot_claims, slot_claims);
        assert_eq!(adapter.effect_keys, effect_keys);
        assert_eq!(
            adapter
                .effects
                .iter()
                .map(lane_work_effect_key)
                .collect::<Vec<_>>(),
            effect_order
        );
        assert_eq!(adapter.native_retransmit_cursor, retransmit_cursor);
        assert_eq!(adapter.native_request_admission_cursor, admission_cursor);
        assert_eq!(snapshot_artifacts(), interrupted_artifacts);
        assert!(
            !missing_path.exists(),
            "authority reads must not repair the pair"
        );

        std::fs::write(&missing_path, &saved).expect("restore the exact authenticated half");
        assert_shared_lane_predecessor_is_applied(
            &adapter.state,
            &request.participant_proposal,
            true,
        );
        assert!(
            crate::block::NativeAmxAuthorityContext::native_amx_participant_predecessor_is_current(
                &adapter.state.query_view(),
                &request.participant_proposal,
                previous_hash,
            )
            .expect("restored exact authority permits the original successor")
        );
        assert!(adapter.native_request_matches_context(&request, 0));
        let restored_reply = routes.mint(slot.author.clone());
        assert_eq!(
            adapter.accept_native_amx(
                slot.author.clone(),
                Some(restored_reply),
                NativeAmxMessage::PrepareRequest(request),
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
        let vote = adapter
            .effects
            .iter()
            .find_map(|effect| match effect {
                V2LaneWorkEffect::PostNativeAmx {
                    peer,
                    message: NativeAmxMessage::PrepareVote(vote),
                    ..
                } if peer == &slot.author => Some(vote),
                _ => None,
            })
            .expect("restored authority reaches actual signed vote publication");
        assert_eq!(
            vote.validate_ingress(NativeAmxPhase::Prepare, Some(&adapter.local_peer)),
            Ok(())
        );
        assert_eq!(
            adapter
                .native_signing_guard
                .as_ref()
                .unwrap()
                .record_count_for_test(),
            1
        );
        assert!(!adapter.output_guard.restart_required());
    }
}

fn assert_shared_lane_predecessor_is_applied(
    state: &State,
    proposal: &LaneBlockProposalV1,
    expected: bool,
) {
    assert_eq!(
        state
            .certified_lane_block_predecessor_is_applied_or_snapshot_anchored(proposal)
            .expect("authenticate ordinary shared predecessor"),
        expected,
    );
    assert_eq!(
        state
            .certified_autonomous_lane_block_predecessor_is_globally_applied(proposal)
            .expect("authenticate autonomous shared predecessor"),
        expected,
    );
}

#[test]
fn shared_lane_predecessor_rejects_corrupt_native_application_evidence() {
    for corrupt_half in ["receipt", "manifest"] {
        let (adapter, _, lane_id, dataspace_id, previous) =
            native_coordinator_after_applied_participant_fixture();
        let request =
            native_coordinator_successor_request(&adapter, lane_id, dataspace_id, &previous);
        assert_shared_lane_predecessor_is_applied(
            &adapter.state,
            &request.participant_proposal,
            true,
        );
        let corrupt_path = adapter
            .state
            .nexus_snapshot()
            .lane_config
            .entry(lane_id)
            .expect("actual participant storage route")
            .blocks_dir(adapter.kura.store_root())
            .join("lane_artifacts")
            .join(format!(
                "native_amx_{corrupt_half}_v1_00000000000000000001.norito"
            ));
        assert!(
            !std::fs::read(&corrupt_path)
                .expect("retained authenticated half")
                .is_empty()
        );
        std::fs::write(
            &corrupt_path,
            b"corrupt occupied Native application evidence",
        )
        .expect("corrupt one occupied application half");
        assert!(
            adapter
                .state
                .certified_lane_block_predecessor_is_applied_or_snapshot_anchored(
                    &request.participant_proposal
                )
                .is_err()
        );
        assert!(
            adapter
                .state
                .certified_autonomous_lane_block_predecessor_is_globally_applied(
                    &request.participant_proposal
                )
                .is_err()
        );
        assert_eq!(
            std::fs::read(&corrupt_path).expect("reads must not repair corruption"),
            b"corrupt occupied Native application evidence"
        );
    }
}

fn complete_applied_ordinary_lane_sessions(
    adapter: &mut V2LaneWorkAdapter,
    keys: &[KeyPair],
    block: &SignedBlock,
) {
    let bundle = block
        .execution_context()
        .expect("applied ownership context");
    let before = crate::snapshot::canonical_state_snapshot_hash(adapter.state.as_ref());
    assert!(adapter.pending_committed_lanes.is_empty());
    for ownership in &bundle.lane_payload_ownerships {
        let proposal = proposal_from_ownership(ownership, block.hash())
            .expect("exact applied ordinary ownership");
        assert_shared_lane_predecessor_is_applied(&adapter.state, &proposal, true);
        adapter
            .pending_committed_lanes
            .push_back(committed_lane_session(&proposal, keys));
    }
    assert_eq!(
        adapter
            .persist_anchored_sessions()
            .expect("publish signed ordinary completion and canonical application receipts"),
        bundle.lane_payload_ownerships.len()
    );
    assert!(adapter.pending_committed_lanes.is_empty());
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(adapter.state.as_ref()),
        before,
        "lane completion must not execute the already applied economic effects again"
    );
}

struct NativeBodyRecoveryFixture {
    adapter: V2LaneWorkAdapter,
    carrier: SignedBlock,
    finality: wire::finality::V2FinalityArtifact,
    manifest: crate::sumeragi::exec::NativeAmxApplicationManifestV1,
    marker: crate::state::AppliedNativeAmxParticipantFrontierMarker,
    source_id: [u8; Hash::LENGTH],
    entrypoint_hash: HashOf<TransactionEntrypoint>,
}
fn native_body_recovery_adapter() -> (V2LaneWorkAdapter, Vec<KeyPair>, LaneId, DataSpaceId) {
    native_body_recovery_adapter_with_kura(locked_lane_work_test_kura(
        NonZeroUsize::new(1).expect("retain one carrier body"),
    ))
}
fn native_body_recovery_adapter_with_kura(
    kura: Arc<Kura>,
) -> (V2LaneWorkAdapter, Vec<KeyPair>, LaneId, DataSpaceId) {
    let capacity = NonZeroUsize::new(8).expect("non-zero fixture capacity");
    let limits = V2LaneWorkLimits::new(
        capacity,
        capacity,
        capacity,
        capacity,
        capacity,
        capacity,
        capacity,
        iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONSENSUS,
        iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC,
        iroha_config::parameters::defaults::sumeragi::V2_AUTHENTICATED_MERGE_QC_CAPACITY,
        iroha_config::parameters::defaults::sumeragi::V2_MERGE_LEADER_BODY_FRAME_HEADROOM_BYTES,
        iroha_config::parameters::defaults::sumeragi::V2_AUTONOMOUS_CARRIER_HEADROOM_BYTES,
        iroha_config::parameters::defaults::sumeragi::V2_AUTONOMOUS_PRODUCER_RECHECK,
        std::time::Duration::from_millis(10),
        std::time::Duration::from_secs(1),
        iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_STUCK_ATTEMPTS,
        iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_RETRY_TIER_ATTEMPTS,
        iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_MAX_RETRY_TIER,
        iroha_config::parameters::defaults::sumeragi::V2_SIDECAR_SERVICE_BURST,
        MergeSidecarLimits::defaults(),
        MergeSigningGuardLimits::defaults(),
        NativeAmxSigningGuardLimits::new(
            iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_CAPACITY,
            iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_BYTES,
            iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_ANCHOR_BYTES,
        )
        .expect("default Native AMX signing limits"),
    );
    // Freeze the Native body budget before constructing any parent finality or
    // opening a signing guard. The small lane fixture's 4 KiB DA budget cannot
    // hold the actual grouped Native receipt and its signed participant controls.
    let (mut adapter, keys) = fixture_at_height_inner_with_da_layout(
        wire::ConsensusMode::Permissioned,
        4,
        true,
        limits,
        kura,
        None,
        true,
        wire::recommended_data_availability_layout(),
    );
    let participant_lane = LaneId::new(1);
    let participant_dataspace = DataSpaceId::new(7);
    enable_multilane_nexus(&mut adapter, &keys, participant_lane, participant_dataspace);
    // These routes are ungoverned in the fixture catalog. Match the installed
    // manifest metadata to that catalog so an ordinary Queue rebind preserves
    // the explicitly seeded validator authority.
    let nexus = adapter.state.nexus_snapshot();
    let statuses = adapter
        .state
        .lane_manifests
        .read()
        .statuses()
        .into_iter()
        .map(|mut status| {
            let lane = nexus
                .lane_catalog
                .lanes()
                .iter()
                .find(|lane| lane.id == status.lane)
                .expect("Native fixture manifest has a configured route");
            status.alias = lane.alias.clone();
            status.dataspace = lane.dataspace_id;
            status.visibility = lane.visibility;
            status.storage = lane.storage;
            status.governance = lane.governance.clone();
            (status.lane, status)
        })
        .collect();
    adapter
        .state
        .install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(statuses)));
    adapter.context.nexus_amx_context_hash =
        super::super::v2_recovery::committed_nexus_amx_context_hash(adapter.state.as_ref());
    adapter.context.execution_policy_hash =
        super::super::v2_recovery::committed_execution_policy_hash(adapter.state.as_ref())
            .expect("derive catalog-bound Native fixture policy");
    let entry = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(participant_lane)
        .expect("participant lane storage entry")
        .clone();
    adapter
        .kura
        .reconcile_lane_segments_for_testing(&[&entry], &[], &[])
        .expect("provision participant lane storage");
    let incarnation = adapter
        .state
        .lane_incarnation_at_height(participant_lane, adapter.context.height)
        .expect("participant lane incarnation");
    adapter
        .kura
        .install_lane_incarnation_marker_for_test(&entry, incarnation, 0)
        .expect("install participant lane incarnation marker");
    (adapter, keys, participant_lane, participant_dataspace)
}
struct GroupedNativeCandidateFixture {
    service: crate::sumeragi::v2_apply::V2ApplyService,
    context: wire::HeightContext,
    body: SignedBlock,
    state: Arc<State>,
    kura: Arc<Kura>,
    participant_lane: LaneId,
    participant_dataspace: DataSpaceId,
    participant_incarnation: Hash,
    participant_height: u64,
}

#[allow(clippy::too_many_lines)]
fn native_apply_candidate_body(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    transactions: Vec<iroha_data_model::transaction::signed::SignedTransaction>,
) -> SignedBlock {
    let parent_height = NonZeroUsize::new(
        usize::try_from(adapter.context.height - 1).expect("parent height fits usize"),
    )
    .expect("Native Apply fixture has a committed parent");
    let parent = adapter
        .kura
        .read_block_body(parent_height)
        .expect("authenticate actual Apply parent")
        .expect("retain actual Apply parent body");
    let mut creation_time = parent
        .header()
        .creation_time()
        .checked_add(Duration::from_secs(1))
        .expect("candidate cadence fits Duration");
    for transaction in &transactions {
        creation_time = creation_time.max(
            transaction
                .creation_time()
                .checked_add(Duration::from_millis(1))
                .expect("transaction time fits Duration"),
        );
    }
    let creation_time_ms =
        u64::try_from(creation_time.as_millis()).expect("candidate time fits u64");
    let accepted = transactions
        .iter()
        .cloned()
        .map(|transaction| AcceptedTransaction::new_unchecked(Cow::Owned(transaction)))
        .collect::<Vec<_>>();
    let routing_plans = {
        let view = adapter.state.view();
        accepted
            .iter()
            .map(|transaction| {
                crate::queue::evaluate_policy_plan_with_nexus_and_world_at_block_height(
                    &view.nexus,
                    transaction,
                    view.world(),
                    creation_time_ms,
                    adapter.context.height,
                )
                .expect("derive actual transaction routing plan")
            })
            .collect::<Vec<_>>()
    };
    let candidates = accepted
        .iter()
        .zip(&routing_plans)
        .map(|(transaction, plan)| CandidateDescriptor::new(transaction, plan))
        .collect::<Vec<_>>();
    let coordinator_routes = routing_plans
        .iter()
        .map(RoutingPlan::coordinator_route)
        .collect::<Vec<_>>();
    let candidate_hashes = accepted
        .iter()
        .map(|transaction| Hash::from(transaction.hash_as_entrypoint()))
        .collect::<Vec<_>>();
    let leader_index = usize::try_from(adapter.context.leader(0)).expect("leader fits usize");
    let lane_plan = prepare_v2_lane_payload_plan(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &adapter.context,
        0,
        &adapter.context.roster[leader_index].validator,
        &coordinator_routes,
        &candidate_hashes,
    )
    .expect("derive current shared lane predecessor through production planning");
    assert!(lane_plan.unavailable_indices.is_empty());
    let controls =
        match adapter.prepare_native_participant_controls(&candidates, &lane_plan.proposals) {
            Ok(controls) => controls,
            Err(NativeParticipantControlPreparationError::Storage(error)) => {
                panic!("Native control storage failed: {error}")
            }
            Err(NativeParticipantControlPreparationError::PendingPredecessor(indices)) => {
                panic!("Native control predecessor unexpectedly pending: {indices:?}")
            }
            Err(NativeParticipantControlPreparationError::Unavailable(indices)) => {
                panic!("Native controls unexpectedly unavailable: {indices:?}")
            }
        };
    let external = candidates
        .iter()
        .copied()
        .map(|candidate| {
            let routing_plan = candidate.routing_plan();
            let external = crate::queue::execution_context_for_routing_plan(
                candidate.entrypoint_hash(),
                routing_plan,
            );
            let RoutingPlan::NativeAmx(plan) = routing_plan else {
                return external;
            };
            let coordinator = routing_plan.coordinator_route();
            let coordinator_proposal = lane_plan
                .proposals
                .iter()
                .find(|proposal| {
                    proposal.descriptor.lane_id == coordinator.lane_id
                        && proposal.descriptor.dataspace_id == coordinator.dataspace_id
                })
                .expect("exact coordinator proposal");
            let coordinator_descriptor = &coordinator_proposal.descriptor;
            let mut source_id = [0_u8; Hash::LENGTH];
            source_id.copy_from_slice(candidate.transaction().hash().as_ref());
            let legs = plan
                .participants
                .iter()
                .map(|participant| {
                    let route = participant.route;
                    let control = controls
                        .get(&(route.lane_id, route.dataspace_id))
                        .expect("production participant control");
                    let descriptor = &control.proposal.descriptor;
                    let mut request = native_request_with_distinct_participant(
                        adapter,
                        keys,
                        route.lane_id,
                        route.dataspace_id,
                        coordinator_descriptor.lane_block_height,
                        coordinator_descriptor.previous_lane_block_descriptor_hash,
                    );
                    request.plan_legs = routing_plan.legs();
                    request.coordinator_proposal = coordinator_proposal.clone();
                    request.participant_proposal = control.proposal.clone();
                    request.participant_settlement = control.settlement.clone();
                    let body = &mut request.body;
                    body.source_id = source_id;
                    body.tx_entrypoint_hash = candidate.entrypoint_hash();
                    body.plan_digest = routing_plan.digest();
                    body.coordinator_lane_id = coordinator_descriptor.lane_id;
                    body.coordinator_dataspace_id = coordinator_descriptor.dataspace_id;
                    body.coordinator_lane_incarnation = coordinator_descriptor.lane_incarnation;
                    body.planned_coordinator_block_height =
                        coordinator_descriptor.lane_block_height;
                    body.coordinator_lane_block_view = coordinator_descriptor.lane_block_view;
                    body.coordinator_proposal_hash = coordinator_proposal.proposal_hash;
                    body.participant_lane_id = descriptor.lane_id;
                    body.participant_dataspace_id = descriptor.dataspace_id;
                    body.participant_lane_incarnation = descriptor.lane_incarnation;
                    body.participant_previous_block_height = descriptor.previous_lane_block_height;
                    body.participant_previous_block_descriptor_hash =
                        descriptor.previous_lane_block_descriptor_hash;
                    body.participant_lane_block_height = descriptor.lane_block_height;
                    body.participant_lane_block_view = descriptor.lane_block_view;
                    body.participant_proposal_hash = control.proposal.proposal_hash;
                    body.participant_validator_set_hash = descriptor.validator_set_hash;
                    body.participant_validator_count = descriptor.validator_count;
                    body.participant_min_quorum = descriptor.min_quorum;
                    let settlement_hash = control
                        .settlement
                        .computed_hash()
                        .expect("hash production Native control");
                    body.participant_settlement_commitment = Hash::from(settlement_hash);
                    request
                        .validate_plan_binding()
                        .expect("exact production control binding");
                    assert!(adapter.native_request_matches_context(&request, 0));
                    let prepare_qc = native_qc_for_body(request.body, keys);
                    let mut commit_body = request.body;
                    commit_body.phase = NativeAmxPhase::Commit;
                    NativeAmxLegRecordV2 {
                        lane_id: route.lane_id,
                        dataspace_id: route.dataspace_id,
                        participant_proposal: request.participant_proposal,
                        participant_settlement: request.participant_settlement,
                        participant_settlement_hash: settlement_hash,
                        prepare_qc,
                        commit_qc: native_qc_for_body(commit_body, keys),
                    }
                })
                .collect();
            let receipt = adapter
                .assemble_native_receipt(
                    source_id,
                    coordinator,
                    routing_plan.digest(),
                    coordinator_proposal,
                    legs,
                )
                .expect("assemble exact independently signed Native receipt");
            external.with_native_amx_receipt(receipt)
        })
        .collect();
    let mut header = BlockHeader::new(
        NonZeroU64::new(adapter.context.height).expect("non-zero candidate height"),
        Some(parent.hash()),
        None,
        None,
        creation_time_ms,
        0,
    );
    let confidential_features = {
        let view = adapter.state.view();
        let digest = crate::state::compute_confidential_feature_digest(
            view.world(),
            &view.zk,
            view.sccp_registry.as_ref(),
            adapter.context.height,
        );
        (!digest.is_empty()).then_some(digest)
    };
    header.set_confidential_features(confidential_features);
    let proof_policy_bundle = crate::da::active_proof_policy_bundle_at_height(
        &adapter.state.nexus_snapshot(),
        adapter.context.height,
    );
    let mut builder = BlockBuilder::new(header);
    for transaction in transactions {
        builder.push_transaction(transaction);
    }
    builder.set_da_proof_policies(Some(proof_policy_bundle));
    builder.set_execution_context(Some(
        BlockExecutionContextBundle::new(external)
            .with_lane_payload_ownerships(lane_plan.ownerships),
    ));
    builder
        .try_build_with_signature(
            u64::try_from(leader_index).expect("leader index fits u64"),
            keys[leader_index].private_key(),
        )
        .expect("sign actual Native/ordinary candidate")
        .canonical_resultless_proposal()
}

fn grouped_native_candidate_fixture(
    pending_control_validation_bytes: Option<NonZeroUsize>,
) -> GroupedNativeCandidateFixture {
    grouped_native_candidate_fixture_with_adapter(pending_control_validation_bytes).0
}
#[allow(clippy::too_many_lines)]
fn grouped_native_candidate_fixture_with_adapter(
    pending_control_validation_bytes: Option<NonZeroUsize>,
) -> (
    GroupedNativeCandidateFixture,
    V2LaneWorkAdapter,
    Vec<KeyPair>,
    KeyPair,
) {
    let mut kura =
        locked_lane_work_test_kura(iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY);
    if let Some(aggregate_bytes) = pending_control_validation_bytes {
        Arc::get_mut(&mut kura)
            .expect("fresh grouped Native fixture Kura has one owner")
            .set_pending_control_sidecar_validation_bytes_for_testing(aggregate_bytes);
    }
    let (mut adapter, keys, participant_lane, participant_dataspace) =
        native_body_recovery_adapter_with_kura(kura);
    let mut finality_manifest_root = [0_u8; Hash::LENGTH];
    finality_manifest_root
        .copy_from_slice(Hash::new(b"grouped Native coordinator finality manifest").as_ref());
    Arc::get_mut(&mut adapter.state)
        .expect("fresh grouped Native fixture owns its State")
        .set_axt_policy(
            DataSpaceId::UNIVERSAL,
            iroha_data_model::nexus::AxtPolicyEntry {
                manifest_root: finality_manifest_root,
                target_lane: LaneId::SINGLE,
                active_handle_era: 1,
                next_handle_counter: 1,
                current_slot: 0,
            },
        );
    let transaction_key =
        KeyPair::try_from_seed(vec![0xD8; 32], Algorithm::Ed25519).expect("transaction key");
    let authority = AccountId::new(transaction_key.public_key().clone());
    let authority_domain =
        DomainId::try_new("budgetauthority", "universal").expect("authority domain id");
    let mut world = adapter.state.world.block();
    world.domains.insert(
        authority_domain.clone(),
        Domain::new(authority_domain).build(&authority),
    );
    world.accounts.insert(
        authority.clone(),
        AccountValue::new(AccountDetails::default()),
    );
    // The chain already has three committed parents. Raw domain registration is
    // genesis-only, so seed owned domains and exercise legal metadata writes.
    for (name, dataspace) in [
        ("budgetuniversalone", "universal"),
        ("budgetindependentone", "independent-dataspace"),
        ("budgetuniversaltwo", "universal"),
        ("budgetindependenttwo", "independent-dataspace"),
        ("mixedparticipant2", "independent-dataspace"),
        ("mixedparticipant3", "independent-dataspace"),
        ("mixeduniversalthree", "universal"),
    ] {
        let domain = DomainId::try_new(name, dataspace).expect("owned effect domain");
        world
            .domains
            .insert(domain.clone(), Domain::new(domain).build(&authority));
    }
    world.commit();
    let transaction_time = TimeSource::new_fixed(Duration::from_secs(4));
    let mut transactions = [
        ("budgetuniversalone", "budgetindependentone"),
        ("budgetuniversaltwo", "budgetindependenttwo"),
    ]
    .into_iter()
    .map(|(universal_name, participant_name)| {
        TransactionBuilder::new_with_time_source(
            adapter.context.network_id,
            authority.clone(),
            &transaction_time,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([
            InstructionBox::from(iroha_data_model::isi::SetKeyValue::domain(
                DomainId::try_new(universal_name, "universal")
                    .expect("universal fixture domain id"),
                "native_apply_effect".parse().expect("effect metadata key"),
                1_u64,
            )),
            InstructionBox::from(iroha_data_model::isi::SetKeyValue::domain(
                DomainId::try_new(participant_name, "independent-dataspace")
                    .expect("participant fixture domain id"),
                "native_apply_effect".parse().expect("effect metadata key"),
                1_u64,
            )),
        ])
        .sign(transaction_key.private_key())
    })
    .collect::<Vec<_>>();
    transactions.sort_by_key(|transaction| transaction.hash());
    let body = native_apply_candidate_body(&adapter, &keys, transactions);
    assert!(body.is_resultless_proposal());
    assert_eq!(body.external_entrypoint_count(), 2);
    let bundle = body.execution_context().expect("grouped candidate context");
    assert_eq!(bundle.lane_payload_ownerships.len(), 1);
    assert_eq!(
        bundle.lane_payload_ownerships[0].accepted_candidate_indices,
        vec![0, 1]
    );
    let receipts = bundle
        .external
        .iter()
        .map(|external| {
            external
                .native_amx_receipt
                .as_ref()
                .expect("both grouped sources retain Native routing")
        })
        .collect::<Vec<_>>();
    assert!(receipts[0].source_id < receipts[1].source_id);
    assert_eq!(receipts[0].legs.len(), 1);
    assert_eq!(receipts[1].legs.len(), 1);
    assert_eq!(
        receipts[0].legs[0].participant_proposal,
        receipts[1].legs[0].participant_proposal
    );
    assert_eq!(
        receipts[0].legs[0].participant_settlement,
        receipts[1].legs[0].participant_settlement
    );
    assert_eq!(
        receipts[0].legs[0].participant_settlement.source_ids(),
        &[receipts[0].source_id, receipts[1].source_id]
    );
    let participant_incarnation = adapter
        .state
        .lane_incarnation_at_height(participant_lane, adapter.context.height)
        .expect("active grouped participant incarnation");
    let participant_height = body
        .execution_context()
        .expect("grouped candidate execution context")
        .external[0]
        .native_amx_receipt
        .as_ref()
        .expect("grouped candidate Native receipt")
        .legs[0]
        .participant_proposal
        .descriptor
        .lane_block_height;
    let block_cadence = Duration::from_secs(1);
    let state = Arc::clone(&adapter.state);
    let kura = Arc::clone(&adapter.kura);
    let context = adapter.context.clone();
    let validator_set_pops = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("grouped Native validator PoP")
        })
        .collect::<Vec<_>>();
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
    let queue = Arc::new(Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events_sender.clone(),
    ));
    let service = crate::sumeragi::v2_apply::V2ApplyService::new(
        Arc::clone(&state),
        queue,
        Arc::clone(&kura),
        None,
        None,
        block_cadence,
        authority,
        events_sender,
        validator_set_pops,
    );
    (
        GroupedNativeCandidateFixture {
            service,
            context,
            body,
            state,
            kura,
            participant_lane,
            participant_dataspace,
            participant_incarnation,
            participant_height,
        },
        adapter,
        keys,
        transaction_key,
    )
}

fn native_candidate_apply_task(
    service: &crate::sumeragi::v2_apply::V2ApplyService,
    context: &wire::HeightContext,
    body: &SignedBlock,
    keys: &[KeyPair],
) -> (
    tempfile::TempDir,
    crate::sumeragi::v2_body_store::V2BodyStore,
    crate::sumeragi::v2_effects::ApplyTask,
) {
    use crate::sumeragi::{
        v2_body_store::V2BodyStore,
        v2_core::{EventTag, Generation},
        v2_effects::ApplyTask,
    };
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let canonical_wire = body
        .encode_wire()
        .expect("encode actual resultless candidate");
    let subject = wire::BlockSubject {
        parent_block_hash: body.header().prev_block_hash(),
        block_hash: body.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    let manifest =
        crate::sumeragi::v2_chunks::encode_payload(context, round, subject, &canonical_wire)
            .unwrap_or_else(|error| {
                panic!(
                    "encode signed RS16 candidate payload ({} bytes, layout {:?}): {error}",
                    canonical_wire.len(),
                    context.da_layout,
                )
            })
            .into_parts()
            .0;
    let execution_commitment = service
        .validate_candidate(context, body)
        .expect("execute the actual candidate in a discarded State overlay");
    assert_eq!(context.roster.len(), 4);
    assert_eq!(keys.len(), 4);
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
    let signatures = certificate
        .signers
        .iter()
        .map(|index| {
            let index = usize::try_from(*index).expect("quorum signer index fits usize");
            assert_eq!(
                keys[index].public_key(),
                context.roster[index].validator.public_key()
            );
            Signature::try_new(keys[index].private_key(), &preimage)
                .expect("sign actual execution commitment")
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate exact 2f+1 Commit votes");
    let root = tempfile::tempdir().expect("mixed Native Apply body-store directory");
    let mut store = V2BodyStore::open(root.path(), context.clone())
        .expect("open actual rotating-leader body store");
    let durable = store
        .store(manifest, canonical_wire)
        .expect("store exact candidate body");
    let validated = store
        .validate(&durable, |candidate| {
            service.validate_candidate(context, candidate)
        })
        .expect("persist validation of the exact stored body");
    let task = ApplyTask::for_test(
        context.height,
        EventTag::new(context.height, 0, Generation::new(context.height)),
        subject,
        certificate,
        validated,
    );
    (root, store, task)
}

#[test]
fn native_ordinary_native_chain_applies_real_effects_and_preserves_sparse_native_history() {
    // Exercise Apply on the same bounded worker stack used by the live runtime.
    // Calling the complete execution pipeline directly on libtest's smaller
    // worker bypasses that production boundary and overflows in debug builds.
    let handle = crate::sumeragi::sumeragi_thread_builder("native-ordinary-native-apply")
        .spawn(native_ordinary_native_chain_applies_real_effects_impl)
        .expect("spawn production-budgeted Apply worker");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[allow(clippy::too_many_lines)]
fn native_ordinary_native_chain_applies_real_effects_impl() {
    use crate::kura::NativeAmxParticipantApplicationObservation;
    use iroha_data_model::HasMetadata as _;

    let (fixture, mut adapter, keys, transaction_key) =
        grouped_native_candidate_fixture_with_adapter(None);
    // The production worker binds Kura and every height's adapter to one
    // process-owned guard. Reopening an adapter does not replace that owner.
    let output_guard = Arc::clone(&adapter.output_guard);
    fixture
        .kura
        .bind_consensus_output_guard(Arc::clone(&output_guard))
        .expect("bind the mixed chain's authoritative consensus output guard");
    assert!(output_guard.acquire().is_some());
    let native_layout = wire::recommended_data_availability_layout();
    assert_eq!(fixture.context.da_layout, native_layout);
    assert!(
        fixture
            .body
            .encode_wire()
            .expect("encode grouped Native body")
            .len()
            > 4096,
        "exercise a real Native candidate beyond the small lane fixture's DA budget"
    );
    for height in 1..fixture.context.height {
        let parent = fixture
            .kura
            .v2_finality_artifact(height)
            .expect("read exact fixture parent finality")
            .expect("every fixture parent has published finality");
        assert_eq!(
            parent.height_context.da_layout, native_layout,
            "the selected DA layout must be frozen before parent signing at height {height}"
        );
    }
    let participant_lane = fixture.participant_lane;
    let participant_dataspace = fixture.participant_dataspace;
    let mut first_native_hash = None;
    let mut shared_predecessor = None;
    let mut expected_effects = Vec::new();
    let effect_key: iroha_data_model::name::Name =
        "native_apply_effect".parse().expect("effect metadata key");
    let mut first_native_receipt = None;
    let mut historical_ordinary_session = None;
    let mut historical_native_prefix = None;
    let mut historical_ordinary_proposal = None;
    let installed_manifests = fixture.state.lane_manifests.read().clone();
    let expected_validators = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    for lane_height in 1..=3_u64 {
        assert_eq!(adapter.context.height, lane_height + 3);
        assert_eq!(adapter.context.da_layout, native_layout);
        let body = if lane_height == 1 {
            expected_effects.extend(
                [
                    DomainId::try_new("budgetuniversalone", "universal").unwrap(),
                    DomainId::try_new("budgetindependentone", "independent-dataspace").unwrap(),
                    DomainId::try_new("budgetuniversaltwo", "universal").unwrap(),
                    DomainId::try_new("budgetindependenttwo", "independent-dataspace").unwrap(),
                ]
                .into_iter()
                .map(|domain| (domain, lane_height)),
            );
            fixture.body.clone()
        } else {
            let participant_domain = DomainId::try_new(
                format!("mixedparticipant{lane_height}"),
                "independent-dataspace",
            )
            .expect("participant effect domain");
            let mut instructions = vec![InstructionBox::from(
                iroha_data_model::isi::SetKeyValue::domain(
                    participant_domain.clone(),
                    effect_key.clone(),
                    lane_height,
                ),
            )];
            expected_effects.push((participant_domain, lane_height));
            if lane_height == 3 {
                let universal_domain = DomainId::try_new("mixeduniversalthree", "universal")
                    .expect("coordinator effect domain");
                instructions.push(InstructionBox::from(
                    iroha_data_model::isi::SetKeyValue::domain(
                        universal_domain.clone(),
                        effect_key.clone(),
                        lane_height,
                    ),
                ));
                expected_effects.push((universal_domain, lane_height));
            }
            let time = TimeSource::new_fixed(Duration::from_secs(adapter.context.height));
            let transaction = TransactionBuilder::new_with_time_source(
                adapter.context.network_id,
                AccountId::new(transaction_key.public_key().clone()),
                &time,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions(instructions)
            .sign(transaction_key.private_key());
            native_apply_candidate_body(&adapter, &keys, vec![transaction])
        };
        let bundle = body.execution_context().expect("actual candidate context");
        let proposal = if lane_height == 2 {
            assert!(
                bundle
                    .external
                    .iter()
                    .all(|external| external.native_amx_receipt.is_none())
            );
            let ownership = bundle
                .lane_payload_ownerships
                .iter()
                .find(|ownership| {
                    ownership.lane_id == participant_lane
                        && ownership.dataspace_id == participant_dataspace
                })
                .expect("ordinary transaction owns the participant route");
            proposal_from_ownership(ownership, body.hash()).expect("exact ordinary ownership")
        } else {
            let receipt = bundle.external[0]
                .native_amx_receipt
                .as_ref()
                .expect("Native candidate receipt");
            let leg = receipt
                .legs
                .iter()
                .find(|leg| {
                    leg.lane_id == participant_lane && leg.dataspace_id == participant_dataspace
                })
                .expect("Native participant leg");
            assert_eq!(
                leg.participant_settlement.previous_native_settlement_hash(),
                first_native_hash
            );
            if lane_height == 1 {
                first_native_hash = Some(leg.participant_settlement_hash);
            }
            leg.participant_proposal.clone()
        };
        assert_eq!(proposal.descriptor.lane_block_height, lane_height);
        assert_eq!(
            proposal.descriptor.previous_lane_block_height,
            lane_height - 1
        );
        assert_eq!(
            proposal.descriptor.previous_lane_block_descriptor_hash,
            shared_predecessor
        );
        shared_predecessor = Some(proposal.descriptor.descriptor_hash);
        if lane_height == 2 {
            let prefix = fixture.state.query_view();
            for mode in [
                crate::state::LanePredecessorApplicationMode::AppliedStatePrefix,
                crate::state::LanePredecessorApplicationMode::OrdinaryBodyStatePrefix,
            ] {
                assert!(
                    crate::state::State::lane_block_predecessor_is_applied_for_snapshot(
                        &prefix, &proposal, mode,
                    )
                    .expect("the real applied Native H1 authorizes ordinary H2")
                );
                for first_slot in [false, true] {
                    let mut wrong = proposal.clone();
                    if first_slot {
                        wrong.descriptor.lane_block_height = 1;
                        wrong.descriptor.previous_lane_block_height = 0;
                        wrong.descriptor.previous_lane_block_descriptor_hash = None;
                    } else {
                        wrong.descriptor.previous_lane_block_descriptor_hash =
                            Some(Hash::new(b"competing predecessor"));
                    }
                    wrong.descriptor.descriptor_hash = wrong.descriptor.computed_descriptor_hash();
                    wrong.proposal_hash = wrong.computed_proposal_hash();
                    assert!(
                        !crate::state::State::lane_block_predecessor_is_applied_for_snapshot(
                            &prefix, &wrong, mode,
                        )
                        .expect("competing and occupied-first-slot candidates are ineligible")
                    );
                }
            }
            let directory = fixture
                .state
                .nexus_snapshot()
                .lane_config
                .entry(participant_lane)
                .expect("actual Native route")
                .blocks_dir(fixture.kura.store_root())
                .join("lane_artifacts");
            for half in ["receipt", "manifest"] {
                let path = directory.join(format!("native_amx_{half}_v1_{:020}.norito", 1));
                let saved = std::fs::read(&path).expect("actual Native H1 application half");
                std::fs::remove_file(&path).expect("interrupt the exact predecessor publication");
                for mode in [
                    crate::state::LanePredecessorApplicationMode::AppliedStatePrefix,
                    crate::state::LanePredecessorApplicationMode::OrdinaryBodyStatePrefix,
                ] {
                    assert!(
                        !crate::state::State::lane_block_predecessor_is_applied_for_snapshot(
                            &prefix, &proposal, mode,
                        )
                        .expect("pending Native predecessor cannot become ordinary fallback")
                    );
                }
                std::fs::write(&path, saved).expect("restore the exact fixture application half");
            }
            historical_native_prefix = Some(prefix);
            historical_ordinary_proposal = Some(proposal.clone());
        }

        let context = adapter.context.clone();
        let (_body_root, mut store, task) =
            native_candidate_apply_task(&fixture.service, &context, &body, &keys);
        fixture
            .service
            .execute(&context, &mut store, &task)
            .expect("real Apply commits Native H1, ordinary H2, then Native H3");
        assert_eq!(
            fixture.state.committed_height(),
            usize::try_from(context.height).expect("test height fits the host index")
        );
        let committed = fixture
            .kura
            .read_block_body(NonZeroUsize::new(usize::try_from(context.height).unwrap()).unwrap())
            .expect("authenticate Apply-published finality and canonical wire")
            .expect("Apply retains exact result-bearing body");
        assert!(committed.has_results());
        assert_eq!(committed.hash(), body.hash());
        let rejections = committed
            .errors()
            .map(|(index, error)| (index, format!("{error:?}")))
            .collect::<Vec<_>>();
        assert!(
            rejections.is_empty(),
            "effect transactions must succeed at lane height {lane_height}, global height {}: {rejections:?}",
            context.height
        );
        assert!(
            Arc::ptr_eq(&fixture.state.lane_manifests.read(), &installed_manifests),
            "Apply's Queue refresh must preserve the installed State authority"
        );
        for (lane, dataspace) in [
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (participant_lane, participant_dataspace),
        ] {
            let committee = fixture
                .state
                .resolve_lane_committee_at_height(
                    crate::state::LaneAuthorityRoute::new(lane, dataspace),
                    context.height,
                )
                .expect("real Apply preserves every active route's canonical authority");
            assert_eq!(committee.validators(), expected_validators.as_slice());
        }
        complete_applied_ordinary_lane_sessions(&mut adapter, &keys, &committed);
        if lane_height == 2 {
            historical_ordinary_session = Some(committed_lane_session(&proposal, &keys));
        }
        {
            let view = fixture.state.view();
            for (domain, value) in &expected_effects {
                assert_eq!(
                    view.world()
                        .domain(domain)
                        .expect("seeded effect domain")
                        .metadata()
                        .get(&effect_key),
                    Some(&iroha_primitives::json::Json::new(*value)),
                    "Apply must retain each exact effect for {domain} at lane height {lane_height}"
                );
            }
        }
        assert!(
            fixture
                .state
                .native_amx_participant_frontiers_pending_durable_evidence_snapshot()
                .expect("read completed Native evidence")
                .is_empty()
        );
        assert!(
            fixture
                .state
                .unapplied_lane_block_artifact_heights_snapshot_cached()
                .expect("read shared application frontier")
                .is_empty()
        );
        let tips = fixture
            .state
            .lane_block_artifact_tips_snapshot_cached()
            .expect("read applied ordinary/Native shared frontier");
        assert!(tips.iter().any(|(lane, dataspace, _, height, hash)| {
            *lane == participant_lane
                && *dataspace == participant_dataspace
                && *height == lane_height
                && *hash == shared_predecessor
        }));
        let history = fixture
            .kura
            .read_native_amx_participant_application_history(participant_lane)
            .expect("authenticate sparse Native history produced only by Apply");
        let expected_heights = if lane_height == 3 {
            vec![1, 3]
        } else {
            vec![1]
        };
        assert_eq!(
            history
                .entries()
                .map(|(height, _)| height)
                .collect::<Vec<_>>(),
            expected_heights
        );
        for (_, observation) in history.entries() {
            assert!(matches!(
                observation,
                NativeAmxParticipantApplicationObservation::Applied(_)
            ));
        }
        let NativeAmxParticipantApplicationObservation::Applied(first) = history.get(1).unwrap()
        else {
            panic!("first Native application remains fully authenticated");
        };
        if let Some(expected) = &first_native_receipt {
            assert_eq!(
                first, expected,
                "ordinary application must preserve prior Native authority"
            );
        } else {
            first_native_receipt = Some(first.clone());
        }
        if lane_height == 3 {
            let NativeAmxParticipantApplicationObservation::Applied(last) = history.get(3).unwrap()
            else {
                panic!("third shared lane height is the next applied Native control");
            };
            assert_eq!(
                last.participant_settlement
                    .previous_native_settlement_hash(),
                first_native_hash
            );
            assert_eq!(last.participant_proposal, proposal);
            assert_later_pending_native_preserves_historical_ordinary_application(
                &adapter,
                historical_ordinary_session
                    .as_ref()
                    .expect("real earlier ordinary H2 session"),
                lane_height,
                historical_native_prefix
                    .as_ref()
                    .expect("captured State after Native H1"),
            );
            for mode in [
                crate::state::LanePredecessorApplicationMode::AppliedStatePrefix,
                crate::state::LanePredecessorApplicationMode::OrdinaryBodyStatePrefix,
            ] {
                assert!(crate::state::State::lane_block_predecessor_is_applied_for_snapshot(
                    historical_native_prefix.as_ref().unwrap(),
                    historical_ordinary_proposal.as_ref().unwrap(), mode,
                ).expect("later genuine Native H3 and ordinary H2 cannot replace the supplied H1 prefix"));
            }
            assert!(
                !crate::state::State::lane_block_predecessor_is_applied_for_snapshot(
                    historical_native_prefix.as_ref().unwrap(),
                    historical_ordinary_proposal.as_ref().unwrap(),
                    crate::state::LanePredecessorApplicationMode::CurrentTip,
                )
                .expect(
                    "producer admission must reject a snapshot behind the retained current tip"
                )
            );
            let before = crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref());
            let exact_history = history
                .entries()
                .map(|(height, value)| (height, value.clone()))
                .collect::<Vec<_>>();
            fixture
                .service
                .execute(&context, &mut store, &task)
                .expect("exact completed Apply replay is idempotent");
            assert_eq!(
                crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()),
                before
            );
            assert_eq!(
                fixture
                    .kura
                    .read_native_amx_participant_application_history(participant_lane)
                    .expect("read exact replayed history")
                    .entries()
                    .map(|(height, value)| (height, value.clone()))
                    .collect::<Vec<_>>(),
                exact_history
            );
        }
        assert!(!adapter.output_guard.restart_required());
        let parent = fixture
            .kura
            .v2_finality_artifact(context.height)
            .expect("read Apply-published parent finality")
            .expect("completed Apply has finality");
        let successor_context =
            crate::sumeragi::v2_context::build_successor_height_context_from_state(
                &parent,
                &fixture.state.view(),
                crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(
                    fixture.state.as_ref(),
                ),
            )
            .expect("derive successor exclusively from actual finalized State");
        let restart = LaneAdapterRestartParts::capture(&adapter);
        drop(adapter);
        adapter = V2LaneWorkAdapter::new_with_output_guard(
            successor_context,
            restart.local_peer,
            restart.key_pair,
            true,
            restart.state,
            restart.kura,
            restart.limits,
            None,
            None,
            Arc::clone(&output_guard),
        )
        .expect("reopen consensus across the actual sparse Native chain");
        assert!(
            Arc::ptr_eq(&adapter.output_guard, &output_guard),
            "successor adapters must retain Kura's authoritative output guard"
        );
    }
    assert_eq!(adapter.context.height, 7);
    let directory = fixture
        .state
        .nexus_snapshot()
        .lane_config
        .entry(participant_lane)
        .expect("actual future Native route")
        .blocks_dir(fixture.kura.store_root())
        .join("lane_artifacts");
    let path = directory.join(format!("native_amx_receipt_v1_{:020}.norito", 3));
    let saved = std::fs::read(&path).expect("actual future Native H3 receipt");
    std::fs::write(&path, vec![0xA5; saved.len()]).expect("corrupt the occupied future receipt");
    let result = fixture.kura.consensus_storage_read(
        crate::state::State::lane_block_predecessor_is_applied_for_snapshot(
            historical_native_prefix.as_ref().unwrap(),
            historical_ordinary_proposal.as_ref().unwrap(),
            crate::state::LanePredecessorApplicationMode::OrdinaryBodyStatePrefix,
        )
        .map_err(|error| crate::kura::Error::MergeCarrierConflict(error.to_string())),
    );
    assert!(
        result.is_err(),
        "future corruption is authenticated before prefix selection"
    );
    assert!(adapter.output_guard.restart_required());
    assert!(output_guard.acquire().is_none());
    std::fs::write(path, saved)
        .expect("restore fixture bytes without reopening the fail-stop latch");
    assert!(adapter.output_guard.restart_required());
    assert!(output_guard.acquire().is_none());
}

#[test]
#[allow(clippy::too_many_lines)]
fn grouped_native_amx_prevote_rejects_undersized_evidence_budget_without_kura_or_wsv_mutation() {
    let positive = grouped_native_candidate_fixture(None);
    let positive_state_hash =
        crate::snapshot::canonical_state_snapshot_hash(positive.state.as_ref());
    let commitment = positive
        .service
        .validate_candidate(&positive.context, &positive.body)
        .expect("default evidence budget admits the exact grouped Native candidate");
    assert_eq!(commitment.native_amx_application_manifest_count, 1);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(positive.state.as_ref()),
        positive_state_hash,
        "positive pre-vote validation must discard its WSV overlay"
    );
    let negative = grouped_native_candidate_fixture(Some(
        NonZeroUsize::new(1).expect("non-zero undersized evidence budget"),
    ));
    assert_eq!(positive.context, negative.context);
    assert_eq!(positive.body, negative.body);
    assert_eq!(negative.state.committed_height(), 3);
    assert_eq!(
        negative
            .kura
            .exact_durable_blocks_count()
            .expect("read exact pre-vote durable height"),
        3
    );
    let state_hash_before = crate::snapshot::canonical_state_snapshot_hash(negative.state.as_ref());
    let candidate_height = NonZeroUsize::new(
        usize::try_from(negative.context.height).expect("candidate height fits usize"),
    )
    .expect("candidate height is non-zero");
    assert!(negative.kura.get_block_hash(candidate_height).is_none());
    assert!(
        negative
            .kura
            .wsv_checkpoint(negative.context.height)
            .expect("read pre-vote WSV checkpoint")
            .is_none()
    );
    assert!(
        negative
            .kura
            .commit_manifest(negative.context.height)
            .expect("read pre-vote commit manifest")
            .is_none()
    );
    assert!(
        negative
            .kura
            .v2_finality_artifact(negative.context.height)
            .expect("read pre-vote finality")
            .is_none()
    );
    assert!(
        negative
            .kura
            .read_native_amx_participant_application_receipt(
                negative.participant_lane,
                negative.participant_dataspace,
                negative.participant_incarnation,
                negative.participant_height,
            )
            .is_none()
    );
    let error = negative
        .service
        .validate_candidate(&negative.context, &negative.body)
        .expect_err("one-byte evidence budget must reject before voting");
    match &error {
        crate::sumeragi::v2_apply::V2ApplyError::Validation(message) => {
            assert!(
                message.contains("configured shared stable aggregate byte bound")
                    && message.contains("of 1 bytes"),
                "unexpected grouped Native byte-budget rejection: {message}"
            );
        }
        other => panic!("unexpected grouped Native pre-vote error: {other}"),
    }
    assert!(!error.requires_restart_recovery());
    assert_eq!(negative.state.committed_height(), 3);
    assert_eq!(
        negative
            .kura
            .exact_durable_blocks_count()
            .expect("read exact post-rejection durable height"),
        3
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(negative.state.as_ref()),
        state_hash_before,
        "undersized pre-vote rejection must discard its WSV overlay"
    );
    assert!(negative.kura.get_block_hash(candidate_height).is_none());
    assert!(
        negative
            .kura
            .wsv_checkpoint(negative.context.height)
            .expect("read post-rejection WSV checkpoint")
            .is_none()
    );
    assert!(
        negative
            .kura
            .commit_manifest(negative.context.height)
            .expect("read post-rejection commit manifest")
            .is_none()
    );
    assert!(
        negative
            .kura
            .v2_finality_artifact(negative.context.height)
            .expect("read post-rejection finality")
            .is_none()
    );
    assert!(
        negative
            .kura
            .read_native_amx_participant_application_receipt(
                negative.participant_lane,
                negative.participant_dataspace,
                negative.participant_incarnation,
                negative.participant_height,
            )
            .is_none()
    );
}
fn native_body_recovery_payload(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    participant_lane: LaneId,
    participant_dataspace: DataSpaceId,
) -> NativeBodyRecoveryPayload {
    let transaction_key =
        KeyPair::try_from_seed(vec![0xD7; 32], Algorithm::Ed25519).expect("transaction key");
    let transaction = TransactionBuilder::new(
        adapter.context.network_id,
        AccountId::new(transaction_key.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(transaction_key.private_key());
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let transaction_hash = transaction.hash();
    let mut source_id = [0_u8; Hash::LENGTH];
    source_id.copy_from_slice(transaction_hash.as_ref());
    let mut request = native_request_with_entrypoint(
        native_request_with_distinct_participant(
            adapter,
            keys,
            participant_lane,
            participant_dataspace,
            1,
            None,
        ),
        entrypoint_hash,
    );
    request.body.source_id = source_id;
    request.participant_settlement = request
        .body
        .computed_grouped_participant_settlement(None, &[source_id])
        .expect("derive exact participant settlement");
    let settlement_hash = request
        .participant_settlement
        .computed_hash()
        .expect("hash exact participant settlement");
    request.body.participant_settlement_commitment = Hash::from(settlement_hash);
    request
        .validate_plan_binding()
        .expect("exact Native request binding");
    let prepare_qc = native_qc_for_body(request.body, keys);
    let mut commit_body = request.body;
    commit_body.phase = NativeAmxPhase::Commit;
    let leg = NativeAmxLegRecordV2 {
        lane_id: participant_lane,
        dataspace_id: participant_dataspace,
        participant_proposal: request.participant_proposal.clone(),
        participant_settlement: request.participant_settlement.clone(),
        participant_settlement_hash: settlement_hash,
        prepare_qc,
        commit_qc: native_qc_for_body(commit_body, keys),
    };
    let coordinator = RoutingDecision::new(
        request.body.coordinator_lane_id,
        request.body.coordinator_dataspace_id,
    );
    let participant = RoutingDecision::new(participant_lane, participant_dataspace);
    let routing_plan = RoutingPlan::native_amx(
        coordinator,
        vec![RouteLeg::new(participant, RouteLegRole::Participant)],
    );
    let receipt = adapter
        .assemble_native_receipt(
            source_id,
            coordinator,
            request.body.plan_digest,
            &request.coordinator_proposal,
            vec![leg],
        )
        .expect("assemble exact Native receipt");
    NativeBodyRecoveryPayload {
        transaction,
        request,
        receipt,
        routing_plan,
        source_id,
        entrypoint_hash,
    }
}
fn native_body_recovery_carrier(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    payload: &NativeBodyRecoveryPayload,
) -> SignedBlock {
    let parent_hash = adapter
        .kura
        .get_block(NonZeroUsize::new(3).expect("non-zero parent height"))
        .expect("durable carrier parent")
        .hash();
    let header = BlockHeader::new(
        NonZeroU64::new(adapter.context.height).expect("non-zero carrier height"),
        Some(parent_hash),
        None,
        None,
        adapter.context.height,
        0,
    );
    let leader_index = usize::try_from(adapter.context.leader(0)).expect("leader index fits usize");
    let initial_signature =
        SignatureOf::try_from_hash(keys[leader_index].private_key(), header.hash())
            .expect("sign initial Native carrier");
    let mut carrier = SignedBlock::presigned(
        BlockSignature::new(
            u64::try_from(leader_index).expect("leader index fits u64"),
            initial_signature,
        ),
        header,
        vec![payload.transaction.clone()],
    );
    let coordinator = payload.routing_plan.coordinator_route();
    carrier.set_execution_context(Some(BlockExecutionContextBundle::new(vec![
        ExternalExecutionContext::with_routing_plan(
            payload.entrypoint_hash,
            coordinator.lane_id,
            coordinator.dataspace_id,
            payload.routing_plan.digest(),
            crate::queue::execution_context_legs_for_routing_plan(&payload.routing_plan),
        )
        .with_native_amx_receipt(payload.receipt.clone()),
    ])));
    carrier
        .set_transaction_results(
            Vec::new(),
            &[payload.entrypoint_hash],
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        )
        .expect("attach exact Native carrier result");
    let final_signature =
        SignatureOf::try_from_hash(keys[leader_index].private_key(), carrier.header().hash())
            .expect("sign finalized Native carrier");
    carrier
        .replace_signatures(
            [BlockSignature::new(
                u64::try_from(leader_index).expect("leader index fits u64"),
                final_signature,
            )]
            .into_iter()
            .collect(),
        )
        .expect("replace Native carrier signature");
    carrier
}
fn native_body_recovery_finality(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    carrier: &SignedBlock,
) -> (
    crate::sumeragi::exec::NativeAmxApplicationManifestV1,
    wire::finality::V2FinalityArtifact,
) {
    let manifest =
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(carrier)
            .expect("derive exact Native application manifest");
    assert_eq!(manifest.count(), 1);
    let commitment =
        wire::ExecutionCommitment::new_with_native_amx_application_manifest_without_merge_carrier(
            Hash::new(b"Native generic recovery parent state"),
            Hash::new(b"Native generic recovery post state"),
            Hash::new(b"Native generic recovery ordinary writes"),
            None,
            0,
            wire::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            manifest.root(),
            manifest.count(),
            manifest.executed_block_wire_len(),
            manifest.executed_block_wire_hash(),
        )
        .expect("construct exact Native execution commitment");
    let mut finality = verified_finality_artifact_for_block_with_execution_commitment(
        adapter, keys, carrier, commitment,
    );
    assert_eq!(
        adapter.context.roster.len(),
        4,
        "actual four-validator finality fixture"
    );
    assert_eq!(keys.len(), adapter.context.roster.len());
    for (power, key) in adapter.context.roster.iter().zip(keys) {
        assert_eq!(power.validator.public_key(), key.public_key());
        assert_eq!(key.public_key().try_algorithm(), Ok(Algorithm::BlsNormal));
    }
    let local_signer = adapter
        .context
        .roster
        .iter()
        .position(|power| power.validator == adapter.local_peer)
        .and_then(|index| u32::try_from(index).ok())
        .expect("actual local validator belongs to the frozen roster");
    finality.commit_qc.signers = (0..u32::try_from(adapter.context.roster.len())
        .expect("fixture roster length fits validator index"))
        .filter(|signer| *signer != local_signer)
        .collect();
    assert_eq!(
        u32::try_from(finality.commit_qc.signers.len()).expect("signer count fits u32"),
        finality.height_context.quorum.min_signers,
        "the non-local validators form the exact commit quorum"
    );
    let first_signer = *finality
        .commit_qc
        .signers
        .first()
        .expect("non-local finality quorum has one signer");
    let preimage = finality
        .commit_qc
        .signer_preimage(&adapter.context, first_signer)
        .expect("derive non-local finality signer preimage");
    let signatures = finality
        .commit_qc
        .signers
        .iter()
        .map(|signer| {
            Signature::try_new(
                keys[usize::try_from(*signer).expect("signer index fits usize")].private_key(),
                &preimage,
            )
            .expect("sign non-local finality vote")
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
    finality.commit_qc.aggregate_signature =
        iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .expect("aggregate non-local finality votes");
    finality
        .verify()
        .expect("cryptographically valid non-local finality quorum");
    (manifest, finality)
}
fn persist_and_evict_native_body(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    carrier: &SignedBlock,
    finality: &wire::finality::V2FinalityArtifact,
) -> crate::state::AppliedNativeAmxParticipantFrontierMarker {
    adapter
        .kura
        .store_block(carrier.clone())
        .expect("persist Native carrier");
    let finality_receipt = adapter
        .kura
        .store_v2_finality_artifact(finality)
        .expect("persist Native finality");
    assert_eq!(finality_receipt.height(), carrier.header().height().get());
    assert_eq!(finality_receipt.block_hash(), carrier.hash());
    let committed = ValidBlock::committed_from_replay_signed_block(carrier.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    let checkpoint = crate::snapshot::canonical_state_snapshot_hash(adapter.state.as_ref());
    adapter
        .kura
        .store_wsv_checkpoint(carrier.header().height().get(), carrier.hash(), checkpoint)
        .expect("persist Native WSV checkpoint");
    adapter
        .kura
        .store_commit_manifest(
            crate::kura::CommitManifest::new(
                carrier.header().height().get(),
                carrier.hash(),
                None,
                None,
                checkpoint,
                None,
            )
            .with_authenticated_v2_commit_authority(finality),
        )
        .expect("persist authenticated Native commit manifest");
    let marker = adapter
        .state
        .native_amx_participant_frontiers_pending_durable_evidence_snapshot()
        .expect("inspect pending Native frontier")
        .into_iter()
        .next()
        .expect("one pending Native frontier");
    let leader_index = usize::try_from(adapter.context.leader(0)).expect("leader index fits usize");
    let mut tail_parent = carrier.hash();
    for height in 5..=6 {
        let tail = test_block(height, Some(tail_parent), None, &keys[leader_index]);
        tail_parent = tail.hash();
        adapter
            .kura
            .store_block(tail.clone())
            .expect("persist eviction tail block");
        commit_test_block_to_state(
            adapter.state.as_ref(),
            &ValidBlock::committed_from_replay_signed_block(tail),
            &adapter.context,
        );
    }
    let (carrier_height, payload_len) = adapter
        .kura
        .durable_block_payload_len_by_hash(carrier.hash())
        .expect("inspect durable carrier payload")
        .expect("authenticated carrier payload is present");
    let height = NonZeroUsize::new(usize::try_from(carrier_height).expect("height fits usize"))
        .expect("non-zero carrier height");
    assert_eq!(
        adapter.kura.advertise_required_replicas_for_bench(height),
        Some(payload_len),
        "fixture must install the deterministic selected-keeper quorum"
    );
    assert_eq!(
        adapter
            .kura
            .evict_block_bodies(payload_len)
            .expect("evict exact Native carrier"),
        payload_len
    );
    adapter
        .kura
        .remove_evicted_block_sidecar_for_testing(height)
        .expect("remove local Native carrier sidecar");
    assert!(adapter.kura.get_block(height).is_none());
    marker
}
fn native_body_recovery_fixture() -> NativeBodyRecoveryFixture {
    let (adapter, keys, participant_lane, participant_dataspace) = native_body_recovery_adapter();
    let payload =
        native_body_recovery_payload(&adapter, &keys, participant_lane, participant_dataspace);
    assert_eq!(
        payload.routing_plan.digest(),
        payload.request.body.plan_digest
    );
    let carrier = native_body_recovery_carrier(&adapter, &keys, &payload);
    let (manifest, finality) = native_body_recovery_finality(&adapter, &keys, &carrier);
    let marker = persist_and_evict_native_body(&adapter, &keys, &carrier, &finality);
    NativeBodyRecoveryFixture {
        adapter,
        carrier,
        finality,
        manifest,
        marker,
        source_id: payload.source_id,
        entrypoint_hash: payload.entrypoint_hash,
    }
}
#[test]
fn native_participant_missing_carrier_uses_generic_chunk_recovery_then_repairs_receipt() {
    let fixture = native_body_recovery_fixture();
    let context = &fixture.adapter.context;
    let state = fixture.adapter.state.as_ref();
    let kura = fixture.adapter.kura.as_ref();
    let planning =
        plan_lane_application_evidence_repair(context, state, kura, fixture.adapter.limits)
            .expect("plan missing Native carrier recovery");
    let LaneApplicationEvidenceRepairPlanning::RecoverCanonicalBodies(needs) = planning else {
        panic!("missing Native carrier must enter generic body recovery");
    };
    assert_eq!(needs.len(), 1);
    assert_eq!(needs[0].height, fixture.marker.application_block_height);
    assert_eq!(needs[0].block_hash, fixture.marker.application_block_hash);
    assert_eq!(
        needs[0].finality_artifact_hash,
        HashOf::new(&fixture.finality)
    );
    assert_eq!(
        needs[0].executed_block_wire_hash,
        fixture.manifest.executed_block_wire_hash()
    );
    kura.cache_block_body(&fixture.carrier)
        .expect("simulate authenticated generic chunk assembly");
    let planning =
        plan_lane_application_evidence_repair(context, state, kura, fixture.adapter.limits)
            .expect("replan Native evidence after body recovery");
    let LaneApplicationEvidenceRepairPlanning::Ready(plan) = planning else {
        panic!("recovered body must make the complete publication plan ready");
    };
    let summary = apply_lane_application_evidence_repair(state, kura, plan)
        .expect("publish exact Native application evidence");
    assert_eq!(summary.native_carriers, 1);
    assert_eq!(summary.native_routes, 1);
    assert!(
        state
            .native_amx_participant_frontiers_pending_durable_evidence_snapshot()
            .expect("read repaired Native frontier")
            .is_empty()
    );
    let receipt = kura
        .read_native_amx_participant_application_receipt(
            fixture.marker.lane_id,
            fixture.marker.dataspace_id,
            fixture.marker.lane_incarnation,
            fixture.marker.lane_block_height,
        )
        .expect("read repaired Native receipt");
    assert_eq!(receipt.application_block_hash, fixture.carrier.hash());
    assert_eq!(
        receipt.executed_block_wire_hash,
        fixture.manifest.executed_block_wire_hash()
    );
    assert_eq!(receipt.source_ids, vec![fixture.source_id]);
    assert_eq!(receipt.entrypoint_hashes, vec![fixture.entrypoint_hash]);
    assert_eq!(
        kura.repair_native_amx_participant_application_evidence_for_markers(
            &fixture.carrier,
            &[fixture.marker.clone()],
        )
        .expect("retry exact Native startup evidence repair"),
        1,
        "an exact post-recovery retry must remain idempotent"
    );
    let mut drifted_marker = fixture.marker.clone();
    drifted_marker.participant_proposal_hash =
        Hash::new(b"drifted Native body-recovery participant proposal");
    let error = kura
        .preflight_native_amx_participant_application_evidence_repair(
            &fixture.carrier,
            &[drifted_marker],
            None,
        )
        .expect_err("a drifted State marker must not select carrier evidence");
    assert!(
        error
            .to_string()
            .contains("absent from its authenticated carrier manifest"),
        "unexpected drifted Native marker error: {error}"
    );
    assert_eq!(
        kura.read_native_amx_participant_application_receipt(
            fixture.marker.lane_id,
            fixture.marker.dataspace_id,
            fixture.marker.lane_incarnation,
            fixture.marker.lane_block_height,
        ),
        Some(receipt),
        "failed marker preflight must leave the repaired receipt unchanged"
    );
}
struct MergeNativeProjectionFixture {
    block: SignedBlock,
    entry: iroha_data_model::merge::MergeLedgerEntry,
    source_ids: Vec<[u8; Hash::LENGTH]>,
    routes: Vec<(LaneId, DataSpaceId)>,
}
fn merge_native_projection_lane_qc(
    proposal: &LaneBlockProposalV1,
    phase: CertPhase,
) -> LaneBlockQcV1 {
    let descriptor = &proposal.descriptor;
    LaneBlockQcV1 {
        body: proposal.vote_body(phase),
        validator_set_hash_version: descriptor.validator_set_hash_version,
        validator_set_hash: descriptor.validator_set_hash,
        validator_set: descriptor.validator_set.clone(),
        signers_bitmap: vec![1],
        bls_aggregate_signature: vec![0xA5; 96],
        payload_availability_qc: None,
    }
}
fn merge_native_projection_execution(
    entrypoints: Vec<TransactionEntrypoint>,
    results: Vec<iroha_data_model::transaction::signed::TransactionResult>,
    receipts: Vec<NativeAmxReceipt>,
) -> iroha_data_model::merge::MergeLaneExecution {
    let coordinator_proposal = receipts[0]
        .legs
        .iter()
        .find_map(|leg| {
            matches!(
                crate::native_amx::native_amx_participant_application_role(&receipts[0], leg),
                Ok(crate::native_amx::NativeAmxParticipantApplicationRole::Coordinator)
            )
            .then(|| leg.participant_proposal.clone())
        })
        .unwrap_or_else(|| {
            receipts[0]
                .legs
                .last()
                .expect("merge projection fixture coordinator-shaped leg")
                .participant_proposal
                .clone()
        });
    let source_bundle = b"Native AMX merge projection source".to_vec();
    let control = &receipts[0]
        .legs
        .last()
        .expect("merge projection coordinator control")
        .participant_settlement;
    let settlement = LaneBlockCommitment {
        block_height: control.participant_lane_block_height(),
        lane_id: control.lane_id(),
        lane_incarnation: control.lane_incarnation(),
        dataspace_id: control.dataspace_id(),
        tx_count: control.tx_count(),
        total_local_amount: Quantity::zero(),
        total_xor_due: Quantity::zero(),
        total_xor_after_haircut: Quantity::zero(),
        total_xor_variance: Quantity::zero(),
        swap_metadata: None,
        receipts: control
            .source_ids()
            .iter()
            .map(|source_id| LaneSettlementReceipt {
                source_id: *source_id,
                local_amount: Quantity::zero(),
                xor_due: Quantity::zero(),
                xor_after_haircut: Quantity::zero(),
                xor_variance: Quantity::zero(),
                timestamp_ms: control.authority_context_height(),
            })
            .collect(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: receipts.clone(),
    };
    let settlement_hash = iroha_data_model::nexus::compute_settlement_hash(&settlement)
        .expect("hash merge projection fixture settlement");
    iroha_data_model::merge::MergeLaneExecution {
        source_bundle_hash: Hash::new(&source_bundle),
        source_bundle,
        proposal: coordinator_proposal.clone(),
        origin_proposal: coordinator_proposal.clone(),
        prepare_qc: merge_native_projection_lane_qc(&coordinator_proposal, CertPhase::Prepare),
        commit_qc: merge_native_projection_lane_qc(&coordinator_proposal, CertPhase::Commit),
        signer_proofs: Vec::new(),
        autonomous_network_id: receipts[0].network_id,
        autonomous_epoch: 3,
        autonomous_payload_hash: Hash::new(b"Native AMX merge projection payload"),
        entrypoint_hashes: entrypoints
            .iter()
            .map(|entrypoint| Hash::from(entrypoint.hash()))
            .collect(),
        authenticated_signed_replay_aliases: vec![None; entrypoints.len()],
        entrypoints,
        reservation_keys: vec![Vec::new(); receipts.len()],
        routing_plans: vec![Vec::new(); receipts.len()],
        native_amx_receipts: receipts.into_iter().map(Some).collect(),
        result_hashes: results
            .iter()
            .map(|result| Hash::from(result.hash()))
            .collect(),
        results,
        settlement_commitment: settlement,
        settlement_hash,
        fastpq_transcripts: Vec::new().into(),
    }
}
fn merge_native_projection_batch(
    execution: iroha_data_model::merge::MergeLaneExecution,
    application_block_header: &BlockHeader,
    parent_hash: HashOf<BlockHeader>,
) -> iroha_data_model::merge::MergeExecutionBatch {
    let lanes = vec![execution];
    let base_state_height = application_block_header.height().get() - 1;
    let base_state_hash = parent_hash;
    let execution_root = crate::merge::merge_execution_root(&lanes);
    let entrypoint_merkle_root = crate::merge::merge_execution_entrypoint_merkle_root(&lanes)
        .expect("merge projection fixture entrypoint root");
    let result_merkle_root = crate::merge::merge_execution_result_merkle_root(&lanes)
        .expect("merge projection fixture result root");
    let write_set_root = Hash::new(b"Native AMX merge projection write set");
    let mut batch = iroha_data_model::merge::MergeExecutionBatch {
        version: 1,
        base_state_height,
        base_state_hash,
        application_block_header: application_block_header.clone(),
        entrypoint_count: u64::try_from(lanes[0].entrypoints.len())
            .expect("merge projection fixture entrypoint count fits u64"),
        lanes,
        entrypoint_merkle_root,
        result_merkle_root,
        execution_root,
        application_write_set_root: Hash::new(b"Native AMX merge projection application write set"),
        write_set_root,
        expected_post_state_hash: crate::merge::merge_expected_post_state_hash(
            base_state_height,
            base_state_hash,
            write_set_root,
        ),
        batch_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    batch.batch_hash = crate::merge::merge_execution_batch_hash(&batch);
    batch
}
fn merge_native_projection_entry_and_carrier(
    batch: iroha_data_model::merge::MergeExecutionBatch,
    application_block_header: BlockHeader,
    parent_hash: HashOf<BlockHeader>,
) -> (SignedBlock, iroha_data_model::merge::MergeLedgerEntry) {
    let application_height = application_block_header.height().get();
    let carrier_key = KeyPair::try_from_seed(vec![0x51; 32], Algorithm::BlsNormal)
        .expect("merge projection carrier key");
    let validator_set = vec![PeerId::new(carrier_key.public_key().clone())];
    let entry = iroha_data_model::merge::MergeLedgerEntry {
        version: iroha_data_model::merge::MergeLedgerEntry::VERSION,
        epoch_id: 3,
        lane_catalog_hash: Hash::new(b"Native AMX merge projection lane catalog"),
        active_lanes: Vec::new(),
        lane_authority_catalog: iroha_data_model::merge::MergeLaneAuthorityCatalogV1::default(),
        incarnation_root: Hash::new(b"Native AMX merge projection incarnations"),
        activation_root: Hash::new(b"Native AMX merge projection activations"),
        lane_snapshots: Vec::new(),
        global_state_root: Hash::new(b"Native AMX merge projection global state"),
        merge_qc: iroha_data_model::merge::MergeQuorumCertificate::new(
            application_block_header.view_change_index(),
            3,
            application_height,
            parent_hash,
            iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::new(b"Native AMX merge projection chain"),
            )),
            iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            HashOf::new(&validator_set),
            validator_set,
            vec![1],
            Vec::new(),
            vec![0xA5; 96],
            Hash::new(b"Native AMX merge projection QC"),
        ),
        execution_batch: Some(batch),
        lane_drain_certificates: Vec::new(),
    };
    let signature =
        SignatureOf::try_from_hash(carrier_key.private_key(), application_block_header.hash())
            .expect("sign merge projection carrier");
    let mut block = SignedBlock::presigned(
        BlockSignature::new(0, signature),
        application_block_header,
        Vec::new(),
    );
    block.set_execution_context(Some(
        BlockExecutionContextBundle::new(Vec::new()).with_merge_entry(
            iroha_data_model::block::CertifiedMergeLedgerReference::new(&entry),
        ),
    ));
    block
        .set_transaction_results(Vec::new(), &[], Vec::new())
        .expect("empty merge carrier has a complete result-bearing execution record");
    assert!(block.has_results());
    (block, entry)
}
fn merge_native_projection_fixture(
    mutate_receipts: impl FnOnce(&mut [NativeAmxReceipt]),
) -> MergeNativeProjectionFixture {
    let ordinary_block = crate::sumeragi::exec::result_bearing_native_manifest_block_for_tests();
    let ordinary_manifest =
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(
            &ordinary_block,
        )
        .expect("ordinary Native projection fixture manifest");
    let routes = ordinary_manifest
        .entries()
        .iter()
        .map(|entry| (entry.leaf.lane_id, entry.leaf.dataspace_id))
        .collect::<Vec<_>>();
    let entrypoints = ordinary_block
        .external_entrypoints_cloned()
        .collect::<Vec<_>>();
    let results = ordinary_block.results().cloned().collect::<Vec<_>>();
    let mut receipts = ordinary_block
        .execution_context()
        .expect("ordinary Native projection execution context")
        .external
        .iter()
        .map(|context| {
            context
                .native_amx_receipt
                .clone()
                .expect("ordinary Native projection receipt")
        })
        .collect::<Vec<_>>();
    let source_ids = receipts
        .iter()
        .map(|receipt| receipt.source_id)
        .collect::<Vec<_>>();
    mutate_receipts(&mut receipts);
    let execution = merge_native_projection_execution(entrypoints, results, receipts);
    let application_height = ordinary_block.header().height().get();
    let parent_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"Native AMX merge projection carrier parent"));
    let application_block_header = BlockHeader::new(
        NonZeroU64::new(application_height).expect("non-zero projection fixture height"),
        Some(parent_hash),
        None,
        None,
        application_height,
        ordinary_block.header().view_change_index(),
    );
    let batch = merge_native_projection_batch(execution, &application_block_header, parent_hash);
    let (block, entry) =
        merge_native_projection_entry_and_carrier(batch, application_block_header, parent_hash);
    MergeNativeProjectionFixture {
        block,
        entry,
        source_ids,
        routes,
    }
}
fn merge_native_projection_rebind_single_source_participant(
    leg: &mut NativeAmxLegRecordV2,
    entrypoint_index: u64,
    source_id: [u8; Hash::LENGTH],
    participant_height: u64,
    predecessor_descriptor_hash: Option<Hash>,
) {
    let entrypoint_hash = leg.prepare_qc.body.tx_entrypoint_hash;
    let predecessor_height = participant_height
        .checked_sub(1)
        .expect("merge projection participant height is non-zero");
    let descriptor = &mut leg.participant_proposal.descriptor;
    descriptor.previous_lane_block_height = predecessor_height;
    descriptor.previous_lane_block_descriptor_hash = predecessor_descriptor_hash;
    descriptor.lane_block_height = participant_height;
    descriptor.accepted_candidate_indices = vec![entrypoint_index];
    descriptor.accepted_transaction_hashes = vec![Hash::from(entrypoint_hash)];
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    leg.participant_proposal.proposal_hash = leg.participant_proposal.computed_proposal_hash();
    assert!(leg.participant_settlement.source_ids().contains(&source_id));
    leg.participant_settlement =
        iroha_data_model::block::consensus::NativeAmxParticipantSettlement::try_new(
            leg.participant_settlement.lane_id(),
            leg.participant_settlement.dataspace_id(),
            leg.participant_settlement.lane_incarnation(),
            participant_height,
            leg.participant_settlement.authority_context_height(),
            leg.participant_settlement.previous_native_settlement_hash(),
            vec![source_id],
        )
        .expect("valid single-source merge participant control");
    leg.participant_settlement_hash = leg
        .participant_settlement
        .computed_hash()
        .expect("hash single-source merge projection settlement");
    let descriptor = &leg.participant_proposal.descriptor;
    let participant_lane_id = descriptor.lane_id;
    let participant_dataspace_id = descriptor.dataspace_id;
    let participant_incarnation = descriptor.lane_incarnation;
    let participant_view = descriptor.lane_block_view;
    let proposal_hash = leg.participant_proposal.proposal_hash;
    let settlement_commitment = Hash::from(leg.participant_settlement_hash);
    for body in [&mut leg.prepare_qc.body, &mut leg.commit_qc.body] {
        body.source_id = source_id;
        body.tx_entrypoint_hash = entrypoint_hash;
        body.participant_lane_id = participant_lane_id;
        body.participant_dataspace_id = participant_dataspace_id;
        body.participant_lane_incarnation = participant_incarnation;
        body.participant_previous_block_height = predecessor_height;
        body.participant_previous_block_descriptor_hash = predecessor_descriptor_hash;
        body.participant_lane_block_height = participant_height;
        body.participant_lane_block_view = participant_view;
        body.participant_proposal_hash = proposal_hash;
        body.participant_settlement_commitment = settlement_commitment;
    }
}
fn merge_native_projection_split_participant_heights(
    receipts: &mut [NativeAmxReceipt],
    second_height_delta: u64,
) {
    assert_eq!(receipts.len(), 2);
    let coordinator_route = (receipts[0].lane_id, receipts[0].dataspace_id);
    let participant = receipts[0]
        .legs
        .iter()
        .find(|leg| (leg.lane_id, leg.dataspace_id) != coordinator_route)
        .expect("merge projection separate participant leg");
    let route = (participant.lane_id, participant.dataspace_id);
    let first_height = participant
        .participant_proposal
        .descriptor
        .lane_block_height;
    let first_predecessor_hash = participant
        .participant_proposal
        .descriptor
        .previous_lane_block_descriptor_hash;
    let second_height = first_height
        .checked_add(second_height_delta)
        .expect("participant height fits u64");
    for receipt in receipts.iter_mut() {
        let coordinator_route = (receipt.lane_id, receipt.dataspace_id);
        receipt.legs.retain(|leg| {
            let leg_route = (leg.lane_id, leg.dataspace_id);
            leg_route == route || leg_route == coordinator_route
        });
    }
    let (first, second) = receipts.split_at_mut(1);
    let first_receipt = &mut first[0];
    let first_source_id = first_receipt.source_id;
    let first_leg = first_receipt
        .legs
        .iter_mut()
        .find(|leg| (leg.lane_id, leg.dataspace_id) == route)
        .expect("first-height participant leg");
    merge_native_projection_rebind_single_source_participant(
        first_leg,
        0,
        first_source_id,
        first_height,
        first_predecessor_hash,
    );
    let first_descriptor_hash = first_leg.participant_proposal.descriptor.descriptor_hash;
    let second_receipt = &mut second[0];
    let second_source_id = second_receipt.source_id;
    let second_leg = second_receipt
        .legs
        .iter_mut()
        .find(|leg| (leg.lane_id, leg.dataspace_id) == route)
        .expect("second-height participant leg");
    merge_native_projection_rebind_single_source_participant(
        second_leg,
        1,
        second_source_id,
        second_height,
        Some(first_descriptor_hash),
    );
}
#[test]
fn native_amx_manifest_projects_finality_bound_merge_batch_in_canonical_order() {
    let fixture = merge_native_projection_fixture(|_| {});
    assert!(
        fixture
            .block
            .execution_context()
            .expect("merge carrier execution context")
            .external
            .is_empty(),
        "the merge carrier must not duplicate certified external contexts"
    );
    assert_eq!(
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(
            &fixture.block,
        )
        .expect("ordinary-only projection of autonomous carrier")
        .count(),
        0,
        "autonomous receipts must come only from the exact certified merge entry"
    );
    let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &fixture.block,
        Some(&fixture.entry),
    )
    .expect("project finality-bound merge receipts");
    assert_eq!(manifest.count(), 2);
    assert_eq!(
        manifest
            .entries()
            .iter()
            .map(|entry| (entry.leaf.lane_id, entry.leaf.dataspace_id))
            .collect::<Vec<_>>(),
        fixture.routes
    );
    for entry in manifest.entries() {
        assert_eq!(
            entry
                .leaf
                .members
                .iter()
                .map(|member| (member.entrypoint_index, member.source_id))
                .collect::<Vec<_>>(),
            vec![(0, fixture.source_ids[0]), (1, fixture.source_ids[1])],
            "lane/entrypoint order must be retained while identical routes are grouped"
        );
    }
    let markers = crate::state::State::native_amx_participant_frontier_markers_and_merge_entry(
        &fixture.block,
        Some(&fixture.entry),
    )
    .expect("derive State frontiers from the canonical merge manifest");
    assert_eq!(markers.len(), manifest.entries().len());
    for (marker, entry) in markers.iter().zip(manifest.entries()) {
        let leaf = &entry.leaf;
        assert_eq!(marker.lane_id, leaf.lane_id);
        assert_eq!(marker.dataspace_id, leaf.dataspace_id);
        assert_eq!(marker.lane_incarnation, leaf.lane_incarnation);
        assert_eq!(marker.lane_block_height, leaf.participant_height);
        assert_eq!(marker.participant_proposal_hash, leaf.proposal_hash);
        assert_eq!(marker.participant_settlement_hash, leaf.settlement_hash);
        assert_eq!(
            marker.source_count,
            u64::try_from(leaf.members.len()).expect("fixture member count fits u64")
        );
    }
}
#[test]
fn native_amx_merge_projection_rejects_multiple_participant_heights_in_one_carrier() {
    for second_height_delta in [1_u64, 2_u64] {
        let fixture = merge_native_projection_fixture(|receipts| {
            merge_native_projection_split_participant_heights(receipts, second_height_delta);
        });
        let error = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
            &fixture.block,
            Some(&fixture.entry),
        )
        .expect_err("one carrier must not publish two heights for one participant route");
        assert_eq!(
            error,
            "Native AMX participant route carries more than one height in one application block"
        );
    }
}
#[test]
fn native_amx_merge_projection_rejects_same_height_participant_identity_conflict() {
    let fixture = merge_native_projection_fixture(|receipts| {
        let coordinator_lane_id = receipts[0].lane_id;
        let coordinator_dataspace_id = receipts[0].dataspace_id;
        let participant = receipts[0]
            .legs
            .iter()
            .find(|leg| {
                leg.lane_id != coordinator_lane_id || leg.dataspace_id != coordinator_dataspace_id
            })
            .expect("merge projection separate participant leg");
        let participant_lane_id = participant.lane_id;
        let participant_dataspace_id = participant.dataspace_id;
        let participant_incarnation = participant.participant_proposal.descriptor.lane_incarnation;
        let participant_height = participant
            .participant_proposal
            .descriptor
            .lane_block_height;
        {
            let leg = receipts[1]
                .legs
                .iter_mut()
                .find(|leg| {
                    leg.lane_id == participant_lane_id
                        && leg.dataspace_id == participant_dataspace_id
                })
                .expect("conflicting same-height participant leg");
            assert_eq!(
                leg.participant_proposal.descriptor.lane_incarnation,
                participant_incarnation
            );
            assert_eq!(
                leg.participant_proposal.descriptor.lane_block_height,
                participant_height
            );
            let descriptor = &mut leg.participant_proposal.descriptor;
            descriptor.lane_block_view = descriptor
                .lane_block_view
                .checked_add(1)
                .expect("participant view fits u64");
            descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
            leg.participant_proposal.proposal_hash =
                leg.participant_proposal.computed_proposal_hash();
            let participant_view = leg.participant_proposal.descriptor.lane_block_view;
            let proposal_hash = leg.participant_proposal.proposal_hash;
            for body in [&mut leg.prepare_qc.body, &mut leg.commit_qc.body] {
                body.participant_lane_block_view = participant_view;
                body.participant_proposal_hash = proposal_hash;
            }
        }
        let leg = receipts[1]
            .legs
            .iter()
            .find(|leg| {
                leg.lane_id == participant_lane_id && leg.dataspace_id == participant_dataspace_id
            })
            .expect("drifted separate participant leg");
        assert_eq!(
            crate::native_amx::native_amx_participant_application_role(&receipts[1], leg),
            Ok(crate::native_amx::NativeAmxParticipantApplicationRole::SeparateParticipant)
        );
    });
    let error = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &fixture.block,
        Some(&fixture.entry),
    )
    .expect_err("same-height participant identity drift must fail closed");
    assert!(
        error.contains("participant route carries conflicting proposal/control claims"),
        "{error}"
    );
}
#[test]
fn native_amx_merge_projection_excludes_coordinator_only_receipts() {
    let fixture = merge_native_projection_fixture(|receipts| {
        for receipt in receipts {
            let lane_id = receipt.lane_id;
            let dataspace_id = receipt.dataspace_id;
            receipt
                .legs
                .retain(|leg| leg.lane_id == lane_id && leg.dataspace_id == dataspace_id);
        }
    });
    let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &fixture.block,
        Some(&fixture.entry),
    )
    .expect("coordinator-only merge projection");
    assert_eq!(manifest.count(), 0);
    assert!(
        crate::state::State::native_amx_participant_frontier_markers_and_merge_entry(
            &fixture.block,
            Some(&fixture.entry),
        )
        .expect("coordinator-only State projection")
        .is_empty()
    );
}
#[test]
fn native_amx_merge_projection_rejects_same_route_identity_conflict() {
    let fixture = merge_native_projection_fixture(|receipts| {
        let receipt = &mut receipts[0];
        let lane_id = receipt.lane_id;
        let dataspace_id = receipt.dataspace_id;
        let leg = receipt
            .legs
            .iter_mut()
            .find(|leg| leg.lane_id == lane_id && leg.dataspace_id == dataspace_id)
            .expect("merge projection coordinator leg");
        leg.participant_proposal.descriptor.lane_incarnation =
            Hash::new(b"conflicting merge coordinator incarnation");
        leg.participant_proposal.descriptor.descriptor_hash = leg
            .participant_proposal
            .descriptor
            .computed_descriptor_hash();
        leg.participant_proposal.proposal_hash = leg.participant_proposal.computed_proposal_hash();
        leg.participant_settlement =
            iroha_data_model::block::consensus::NativeAmxParticipantSettlement::try_new(
                leg.participant_settlement.lane_id(),
                leg.participant_settlement.dataspace_id(),
                leg.participant_proposal.descriptor.lane_incarnation,
                leg.participant_settlement.participant_lane_block_height(),
                leg.participant_settlement.authority_context_height(),
                leg.participant_settlement.previous_native_settlement_hash(),
                leg.participant_settlement.source_ids().to_vec(),
            )
            .expect("valid conflicting Native control identity");
        leg.participant_settlement_hash = leg
            .participant_settlement
            .computed_hash()
            .expect("hash conflicting merge coordinator settlement");
        for body in [&mut leg.prepare_qc.body, &mut leg.commit_qc.body] {
            body.participant_lane_incarnation =
                leg.participant_proposal.descriptor.lane_incarnation;
            body.participant_proposal_hash = leg.participant_proposal.proposal_hash;
            body.participant_settlement_commitment = Hash::from(leg.participant_settlement_hash);
        }
    });
    let error = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &fixture.block,
        Some(&fixture.entry),
    )
    .expect_err("same-route identity drift must fail closed");
    assert!(
        error.contains("same-route leg differs from the coordinator identity"),
        "{error}"
    );
}
#[test]
fn native_amx_merge_projection_rejects_duplicate_group_source() {
    let fixture = merge_native_projection_fixture(|receipts| {
        let duplicate_source_id = receipts[0].source_id;
        receipts[1].source_id = duplicate_source_id;
        for leg in &mut receipts[1].legs {
            leg.prepare_qc.body.source_id = duplicate_source_id;
            leg.commit_qc.body.source_id = duplicate_source_id;
        }
    });
    let error = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &fixture.block,
        Some(&fixture.entry),
    )
    .expect_err("duplicate participant source must fail closed");
    assert!(error.contains("repeats a source transaction"), "{error}");
}
#[test]
fn native_amx_merge_projection_matches_decoded_replay_entry() {
    let fixture = merge_native_projection_fixture(|_| {});
    let encoded = norito::to_bytes(&fixture.entry).expect("encode durable merge entry");
    let recovered =
        norito::decode_from_bytes::<iroha_data_model::merge::MergeLedgerEntry>(&encoded)
            .expect("decode durable merge entry");
    let live = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &fixture.block,
        Some(&fixture.entry),
    )
    .expect("live merge projection");
    let restarted = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &fixture.block,
        Some(&recovered),
    )
    .expect("recovered merge projection");
    let ordinary_only =
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(
            &fixture.block,
        )
        .expect("ordinary-only replay projection");
    let witness = iroha_data_model::block::consensus::ExecWitness {
        reads: Vec::new(),
        writes: Vec::new(),
        fastpq_transcripts: Vec::new(),
        fastpq_batches: Vec::new(),
    };
    let lane_finality_manifest =
        crate::sumeragi::exec::LaneFinalityManifestV1::from_result_bearing_block(&fixture.block)
            .expect("merge replay lane-finality manifest");
    let live_commitment = crate::sumeragi::exec::execution_commitment_from_validated_block(
        &witness,
        &live,
        &lane_finality_manifest,
        &fixture.block,
    )
    .expect("live merge replay commitment");
    let restarted_commitment = crate::sumeragi::exec::execution_commitment_from_validated_block(
        &witness,
        &restarted,
        &lane_finality_manifest,
        &fixture.block,
    )
    .expect("decoded merge replay commitment");
    let ordinary_only_commitment =
        crate::sumeragi::exec::execution_commitment_from_validated_block(
            &witness,
            &ordinary_only,
            &lane_finality_manifest,
            &fixture.block,
        )
        .expect("ordinary-only merge replay commitment");
    assert_eq!(ordinary_only.count(), 0);
    assert_ne!(restarted.root(), ordinary_only.root());
    assert_eq!(restarted_commitment, live_commitment);
    assert_ne!(ordinary_only_commitment, live_commitment);
    assert_eq!(restarted.root(), live.root());
    assert_eq!(restarted.count(), live.count());
    assert_eq!(
        restarted
            .entries()
            .iter()
            .map(|entry| entry.leaf.clone())
            .collect::<Vec<_>>(),
        live.entries()
            .iter()
            .map(|entry| entry.leaf.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn shared_lane_first_slot_authenticates_native_publication_before_empty_predecessor() {
    let (empty, keys, lane_id, dataspace_id) = native_body_recovery_adapter();
    let empty_payload = native_body_recovery_payload(&empty, &keys, lane_id, dataspace_id);
    assert_shared_lane_predecessor_is_applied(
        &empty.state,
        &empty_payload.request.participant_proposal,
        true,
    );
    drop(empty);

    for damage in [
        "missing receipt",
        "missing manifest",
        "corrupt receipt",
        "corrupt manifest",
    ] {
        let (adapter, _, lane_id, _, previous) =
            native_coordinator_after_applied_participant_fixture();
        let first = &previous.request.participant_proposal;
        assert_eq!(first.descriptor.lane_block_height, 1);
        assert_eq!(first.descriptor.previous_lane_block_height, 0);
        assert!(
            first
                .descriptor
                .previous_lane_block_descriptor_hash
                .is_none()
        );
        let half = damage.split_once(' ').expect("named fault shape").1;
        let path = adapter
            .state
            .nexus_snapshot()
            .lane_config
            .entry(lane_id)
            .expect("actual Native route")
            .blocks_dir(adapter.kura.store_root())
            .join("lane_artifacts")
            .join(format!("native_amx_{half}_v1_00000000000000000001.norito"));
        let original = std::fs::read(&path).expect("real complete Native half");
        assert!(!original.is_empty());
        if damage.starts_with("missing") {
            std::fs::remove_file(&path).expect("interrupt exact highest publication");
            assert_shared_lane_predecessor_is_applied(&adapter.state, first, false);
            assert!(!adapter.output_guard.restart_required());
            assert!(
                !path.exists(),
                "authority reads never repair the missing half"
            );
        } else {
            let damaged = b"occupied corrupt Native first-slot evidence";
            std::fs::write(&path, damaged).expect("damage one exact occupied half");
            assert!(
                adapter
                    .state
                    .certified_lane_block_predecessor_is_applied_or_snapshot_anchored(first)
                    .is_err(),
                "{damage}"
            );
            assert!(
                adapter
                    .state
                    .certified_autonomous_lane_block_predecessor_is_globally_applied(first)
                    .is_err(),
                "{damage}"
            );
            assert_eq!(std::fs::read(&path).unwrap(), damaged);
        }
    }
}

#[test]
fn native_applied_first_slot_cannot_be_reused_at_lane_signing_or_progress() {
    let (mut adapter, _, lane_id, dataspace_id, previous) =
        native_coordinator_after_applied_participant_fixture();
    let request = native_coordinator_successor_request(&adapter, lane_id, dataspace_id, &previous);
    let successor = request.participant_proposal;
    assert!(
        !adapter
            .lane_application_slot_is_closed(&successor)
            .expect("the exact next shared lane height remains open")
    );
    assert!(
        adapter
            .sign_lane_vote(&successor, CertPhase::Prepare)
            .expect("actual committee can sign the exact shared successor")
            .is_some()
    );

    let mut competing = successor;
    competing.descriptor.lane_block_height = 1;
    competing.descriptor.previous_lane_block_height = 0;
    competing.descriptor.previous_lane_block_descriptor_hash = None;
    competing.descriptor.descriptor_hash = competing.descriptor.computed_descriptor_hash();
    competing.proposal_hash = competing.computed_proposal_hash();
    validate_lane_block_proposal(&competing).expect("valid competing first-slot descriptor");
    assert!(
        competing
            .descriptor
            .validator_set
            .contains(&adapter.local_peer)
    );
    assert!(
        adapter
            .state
            .resolve_lane_committee_at_height(
                crate::state::LaneAuthorityRoute::new(lane_id, dataspace_id),
                competing.descriptor.proposal_height,
            )
            .is_ok_and(
                |committee| committee.validators() == competing.descriptor.validator_set.as_slice()
            )
    );
    assert!(
        adapter
            .lane_application_receipt_at_proposal_slot(&competing)
            .expect("ordinary and merge receipt namespace is independently readable")
            .is_none(),
        "Native authority must not be fabricated as an ordinary receipt"
    );
    assert!(
        adapter
            .lane_application_slot_is_closed(&competing)
            .expect("Native application closes its exact shared prefix")
    );
    assert!(
        !adapter
            .proposal_can_progress(&competing)
            .expect("occupied Native first slot is normal ineligibility")
    );
    for phase in [CertPhase::Prepare, CertPhase::Commit] {
        assert!(
            adapter
                .sign_lane_vote(&competing, phase)
                .expect("valid competing first slot is ordinary rejection")
                .is_none(),
            "Native ownership closes its shared slot before {phase:?} signing"
        );
    }
    assert!(!adapter.output_guard.restart_required());
}

#[derive(Clone, norito::Encode, norito::Decode)]
#[norito(schema_name = "iroha_core::state::AppliedMergeLaneFrontierMarker")]
struct IndependentlyEncodedSharedLaneFrontierForTest {
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    lane_block_descriptor_hash: Hash,
}

#[test]
fn native_application_rejects_valid_but_contradictory_shared_frontier() {
    for fault in ["different descriptor", "missing shared marker"] {
        let (mut adapter, _, lane_id, dataspace_id, previous) =
            native_coordinator_after_applied_participant_fixture();
        let request =
            native_coordinator_successor_request(&adapter, lane_id, dataspace_id, &previous);
        let native = &previous.request.participant_proposal.descriptor;
        let key: iroha_data_model::state_path::StatePath = format!(
            "merge_lane_frontier_v1_{}_{}_{}",
            lane_id.as_u32(),
            dataspace_id.as_u64(),
            hex::encode(native.lane_incarnation.as_ref()),
        )
        .parse()
        .expect("independently derived shared frontier key");
        let mut independent = IndependentlyEncodedSharedLaneFrontierForTest {
            version: 1,
            lane_id,
            dataspace_id,
            lane_incarnation: native.lane_incarnation,
            lane_block_height: native.lane_block_height,
            lane_block_descriptor_hash: native.descriptor_hash,
        };
        let original = adapter
            .state
            .world
            .smart_contract_state
            .view()
            .get(&key)
            .expect("real Apply-published shared marker")
            .clone();
        assert_eq!(
            norito::to_bytes(&independent).unwrap(),
            original,
            "the independent encoder must first reproduce the actual canonical wire"
        );
        assert_eq!(
            adapter
                .state
                .native_amx_participant_application_tips_snapshot()
                .expect("exact actual Native evidence is usable before the fault")
                .len(),
            1
        );
        let mut competing = request.participant_proposal;
        let changed = if fault == "different descriptor" {
            independent.lane_block_descriptor_hash =
                Hash::new(b"independently encoded conflicting shared frontier");
            assert_ne!(
                independent.lane_block_descriptor_hash,
                native.descriptor_hash
            );
            competing.descriptor.previous_lane_block_descriptor_hash =
                Some(independent.lane_block_descriptor_hash);
            competing.descriptor.descriptor_hash = competing.descriptor.computed_descriptor_hash();
            competing.proposal_hash = competing.computed_proposal_hash();
            let bytes = norito::to_bytes(&independent).expect("encode a valid conflicting marker");
            let decoded: IndependentlyEncodedSharedLaneFrontierForTest =
                norito::decode_from_bytes(&bytes).expect("fault remains valid framed Norito");
            assert_eq!(norito::to_bytes(&decoded).unwrap(), bytes);
            Some(bytes)
        } else {
            None
        };
        {
            let mut transaction = adapter.state.world.smart_contract_state.block();
            if let Some(bytes) = &changed {
                transaction.insert(key.clone(), bytes.clone());
            } else {
                transaction.remove(key.clone());
            }
            transaction.commit();
        }
        let error = adapter
            .state
            .native_amx_participant_application_tips_snapshot()
            .expect_err("individually valid replicated markers must agree");
        assert!(
            error
                .to_string()
                .contains("replicated shared lane frontier"),
            "{fault}: {error}"
        );
        assert!(
            adapter
                .state
                .certified_autonomous_lane_block_predecessor_is_globally_applied(&competing)
                .is_err(),
            "the candidate cannot select the conflicting shared frontier"
        );
        assert!(
            State::lane_block_predecessor_is_applied_for_snapshot(
                &adapter.state.query_view(),
                &competing,
                crate::state::LanePredecessorApplicationMode::CurrentTip,
            )
            .is_err(),
            "read-only Native admission must enforce the same marker relation"
        );
        assert!(
            adapter
                .sign_lane_vote(&competing, CertPhase::Prepare)
                .is_err()
        );
        assert!(adapter.output_guard.restart_required());
        assert_eq!(
            adapter
                .state
                .world
                .smart_contract_state
                .view()
                .get(&key)
                .cloned(),
            changed,
            "authority reads must retain the exact contradictory state for recovery"
        );
    }
}

fn assert_later_pending_native_preserves_historical_ordinary_application(
    adapter: &V2LaneWorkAdapter,
    ordinary: &CommittedLaneBlockSession,
    native_height: u64,
    historical_prefix: &impl crate::state::StateReadOnly,
) {
    let descriptor = &ordinary.proposal.descriptor;
    let artifact_dir = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(descriptor.lane_id)
        .expect("actual historical ordinary route")
        .blocks_dir(adapter.kura.store_root())
        .join("lane_artifacts");
    let certificate = adapter
        .kura
        .read_lane_completion_certificate(descriptor.lane_id, descriptor.lane_block_height)
        .expect("read actual historical certificate")
        .expect("retained ordinary certificate");
    let state_hash = crate::snapshot::canonical_state_snapshot_hash(adapter.state.as_ref());
    assert!(
        !adapter
            .state
            .native_amx_participant_application_closes_lane_slot(
                &ordinary.proposal,
                adapter.context.height,
            )
            .expect("a sparse ordinary historical gap keeps its certificate recovery authority")
    );
    for half in ["receipt", "manifest"] {
        let missing = artifact_dir.join(format!("native_amx_{half}_v1_{native_height:020}.norito"));
        let saved = std::fs::read(&missing).expect("actual latest Native application half");
        std::fs::remove_file(&missing).expect("interrupt only the later Native publication");
        assert_eq!(
            adapter
                .state
                .unapplied_native_amx_participant_control_heights_snapshot()
                .expect("later Native publication has recoverable evidence debt")
                .get(&(descriptor.lane_id, descriptor.dataspace_id)),
            Some(&native_height)
        );
        assert!(
            !crate::state::State::lane_block_predecessor_is_applied_for_snapshot(
                historical_prefix,
                &ordinary.proposal,
                crate::state::LanePredecessorApplicationMode::CurrentTip,
            )
            .expect("future pending occupancy blocks producer admission")
        );
        for mode in [
            crate::state::LanePredecessorApplicationMode::AppliedStatePrefix,
            crate::state::LanePredecessorApplicationMode::OrdinaryBodyStatePrefix,
        ] {
            assert!(
                crate::state::State::lane_block_predecessor_is_applied_for_snapshot(
                    historical_prefix,
                    &ordinary.proposal,
                    mode,
                )
                .expect(
                    "future pending publication cannot change historical execution eligibility"
                )
            );
        }
        assert!(
            adapter
                .state
                .certified_lane_block_session_is_applied_or_snapshot_anchored(ordinary)
                .expect("later Native debt cannot erase exact historical ordinary application")
        );
        let repair = adapter
            .state
            .lane_application_certified_repair_snapshot_cached(
                adapter.limits.session_capacity.get(),
            )
            .expect("startup still recognizes the already applied historical ordinary session");
        assert!(
            repair
                .earliest_unapplied
                .iter()
                .all(|session| session.proposal != ordinary.proposal)
        );
        assert_eq!(
            adapter
                .kura
                .read_lane_completion_certificate(descriptor.lane_id, descriptor.lane_block_height,)
                .unwrap()
                .as_ref(),
            Some(&certificate)
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(adapter.state.as_ref()),
            state_hash
        );
        assert!(!adapter.output_guard.restart_required());
        assert!(
            !missing.exists(),
            "historical observation never repairs Native evidence"
        );
        std::fs::write(&missing, &saved)
            .expect("restore exactly the interrupted test fixture half");
        assert!(
            adapter
                .state
                .native_amx_participant_frontiers_pending_durable_evidence_snapshot()
                .expect("exact restored Native authority")
                .is_empty()
        );
    }
}

#[test]
fn autonomous_producer_retains_reservations_until_participant_predecessor_repair() {
    #[derive(Clone)]
    struct NativeRetryRouter(RoutingPlan);
    impl crate::queue::LaneRouter for NativeRetryRouter {
        fn try_route(
            &self,
            _: &dyn crate::queue::TransactionRoutingView,
        ) -> Result<RoutingDecision, crate::queue::RoutingResolveError> {
            Ok(self.0.coordinator_route())
        }
        fn try_route_plan(
            &self,
            _: &dyn crate::queue::TransactionRoutingView,
        ) -> Result<RoutingPlan, crate::queue::RoutingResolveError> {
            Ok(self.0.clone())
        }
        fn try_route_plan_with_view(
            &self,
            _: &dyn crate::queue::TransactionRoutingView,
            _: &crate::state::StateView<'_>,
        ) -> Result<RoutingPlan, crate::queue::RoutingResolveError> {
            Ok(self.0.clone())
        }
        fn try_route_plan_without_state(
            &self,
            _: &dyn crate::queue::TransactionRoutingView,
        ) -> Result<Option<RoutingPlan>, crate::queue::RoutingResolveError> {
            Ok(Some(self.0.clone()))
        }
    }

    let (mut previous_adapter, keys, participant_lane, participant_dataspace, previous) =
        native_coordinator_after_applied_participant_fixture();
    let parent_height = NonZeroUsize::new(previous_adapter.state.committed_height()).unwrap();
    let parent = previous_adapter.kura.get_block(parent_height).unwrap();
    complete_applied_ordinary_lane_sessions(&mut previous_adapter, &keys, &parent);
    let route = previous.routing_plan.coordinator_route();
    let slot = plan_autonomous_lane_reservation_slot(
        previous_adapter.state.as_ref(),
        previous_adapter.kura.as_ref(),
        &previous_adapter.context,
        route.lane_id,
        route.dataspace_id,
    )
    .expect("coordinator predecessor is fully applied before participant interruption");
    let key = keys
        .iter()
        .find(|key| key.public_key() == slot.author.public_key())
        .unwrap()
        .clone();
    let state = Arc::clone(&previous_adapter.state);
    let kura = Arc::clone(&previous_adapter.kura);
    let context = previous_adapter.context.clone();
    let limits = previous_adapter.limits;
    drop(previous_adapter);
    let mut adapter = V2LaneWorkAdapter::new_with_output_guard(
        context,
        slot.author.clone(),
        key,
        true,
        state,
        kura,
        limits,
        None,
        None,
        ConsensusOutputGuard::isolated(),
    )
    .expect("open the exact autonomous producer after predecessor application");
    let queue = Arc::new(Queue::test_with_router_for_routes(
        iroha_config::parameters::actual::Queue::default(),
        &iroha_primitives::time::TimeSource::new_system(),
        Arc::new(NativeRetryRouter(previous.routing_plan.clone())),
        &[
            (route.lane_id, route.dataspace_id),
            (participant_lane, participant_dataspace),
        ],
    ));
    queue.install_lane_manifests(&adapter.state.lane_manifests.read().clone());
    queue.install_test_router_metadata_for_nexus(&adapter.state.nexus_snapshot());
    let journals = tempfile::tempdir().unwrap();
    queue
        .install_lane_reservation_journal(&journals.path().join("reservations.norito"), 1024 * 1024)
        .unwrap();
    queue
        .install_plan_journal(&journals.path().join("plans.norito"), 1024 * 1024, true)
        .unwrap();
    queue.replay_plan_journal(adapter.state.as_ref()).unwrap();
    adapter
        .install_lane_drain_queue(Arc::clone(&queue))
        .unwrap();
    enqueue_autonomous_test_transactions(&adapter, &queue, route.lane_id, route.dataspace_id, 1);
    let reservations = queue
        .reserve_transactions_for_lane_bounded(
            adapter.state.as_ref(),
            slot.selection_authorization().unwrap(),
            LaneQueueReservationSelectionLimits {
                max_transactions: NonZeroUsize::new(1).unwrap(),
                max_scan: NonZeroUsize::new(1).unwrap(),
                max_encoded_bytes: NonZeroU64::new(u64::MAX).unwrap(),
                max_gas: NonZeroU64::new(u64::MAX).unwrap(),
            },
            &BTreeSet::new(),
            LaneQueueReservationRoutingMode::AnyCoordinatorPlan,
        )
        .unwrap();
    assert_eq!(reservations.len(), 1);
    assert!(matches!(
        reservations[0].routing_plan(),
        RoutingPlan::NativeAmx(_)
    ));
    let owned = queue.live_lane_reservations();
    adapter.pending_autonomous_reservation_batches.insert(
        (route.lane_id, route.dataspace_id),
        PendingAutonomousReservationBatch {
            slot,
            reservations,
            envelope_byte_limit: 4 * 1024 * 1024,
        },
    );
    let active_view = (0..2 * adapter
        .state
        .consensus_lane_routes_at_height(adapter.context.height)
        .len() as u64)
        .find(|view| {
            adapter.autonomous_native_coordinator_for_view(*view)
                == Some((route.lane_id, route.dataspace_id))
        })
        .expect("the deterministic Native coordinator rotation selects this route");
    let receipt_path = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(participant_lane)
        .unwrap()
        .blocks_dir(adapter.kura.store_root())
        .join("lane_artifacts")
        .join("native_amx_receipt_v1_00000000000000000001.norito");
    let receipt = std::fs::read(&receipt_path).unwrap();
    std::fs::remove_file(&receipt_path).unwrap();
    adapter.next_autonomous_producer_tick = Instant::now();
    adapter
        .schedule_autonomous_lane_production(active_view, autonomous_test_candidate_limits(1, 1))
        .unwrap();
    assert_eq!(queue.live_lane_reservations(), owned);
    assert!(
        adapter
            .pending_autonomous_reservation_batches
            .contains_key(&(route.lane_id, route.dataspace_id))
    );
    assert!(
        !adapter
            .autonomous_production_attempted_routes
            .contains(&(route.lane_id, route.dataspace_id))
    );
    assert!(
        adapter.native_requests.is_empty(),
        "no participant request precedes its exact predecessor"
    );
    assert!(!adapter.output_guard.restart_required());

    std::fs::write(&receipt_path, receipt).unwrap();
    adapter.next_autonomous_producer_tick = Instant::now();
    adapter
        .schedule_autonomous_lane_production(active_view, autonomous_test_candidate_limits(1, 1))
        .unwrap();
    assert_eq!(queue.live_lane_reservations(), owned);
    assert!(
        adapter
            .pending_autonomous_reservation_batches
            .contains_key(&(route.lane_id, route.dataspace_id))
    );
    assert!(
        !adapter.native_requests.is_empty(),
        "the retained batch resumes actual Native request production after repair"
    );
    assert!(
        !adapter
            .autonomous_production_attempted_routes
            .contains(&(route.lane_id, route.dataspace_id))
    );
    assert!(!adapter.output_guard.restart_required());
}
