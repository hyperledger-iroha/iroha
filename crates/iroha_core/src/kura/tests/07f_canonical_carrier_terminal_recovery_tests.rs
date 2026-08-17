fn lifecycle_terminal_bound_payload_for_test(
    template: &LaneExecutablePayloadV1,
    height_context_id: HeightContextId,
    signer: &KeyPair,
) -> LaneExecutablePayloadV1 {
    let local_peer = PeerId::new(signer.public_key().clone());
    let (reservation_owner_hash, proposal_identity_hash) =
        autonomous_lane_reservation_identity_hashes_for_proposal(
            template.network_id,
            height_context_id,
            template.epoch,
            &template.origin_proposal,
            &local_peer,
        )
        .expect("derive terminal-outcome reservation identities");
    let mut reservation_keys = template.reservation_keys.clone();
    for reservation in &mut reservation_keys {
        reservation.reservation_owner_hash = reservation_owner_hash;
        reservation.proposal_identity_hash = proposal_identity_hash;
    }
    LaneExecutablePayloadV1::new_signed_with_reservations(
        template.network_id,
        template.epoch,
        template.origin_proposal.clone(),
        template.entrypoints.clone(),
        reservation_keys,
        template.routing_plans.clone(),
        template.native_amx_receipts.clone(),
        local_peer,
        signer.private_key(),
    )
    .expect("construct terminal-outcome-bound payload")
}
fn install_live_lifecycle_cursor_for_terminal_test(
    kura: &Kura,
    generation: &AutonomousLifecycleProcessGenerationClaim,
    payload: &LaneExecutablePayloadV1,
    height_context_id: HeightContextId,
    signer: &KeyPair,
) -> (
    AutonomousLifecycleAttemptBindingV1,
    LaneQueueReservationGroupBindingV1,
) {
    let local_peer = PeerId::new(signer.public_key().clone());
    let reservation_group =
        lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
            .expect("bind terminal-outcome reservation group");
    let binding = AutonomousLifecycleAttemptBindingV1::from_payload(
        height_context_id,
        payload.origin_proposal.descriptor.lane_block_height,
        payload,
        reservation_group,
        &local_peer,
    )
    .expect("bind terminal-outcome lifecycle attempt");
    let binding_a = canonical_lane_queue_reservation_group_identity_projection(reservation_group);
    let state = ProductionInFlightFirstReleaseStateProjection {
        validator_count: 1,
        producer: 1,
        producer_selected_owner: 1,
        replicated_carrier_owners: 0,
        payload_binding_a: 1,
        binding_a,
        queue: ProductionInFlightFirstReleaseQueueProjection {
            plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
            selected_count: reservation_group.reservation_count,
            reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
        },
        carrier: ProductionInFlightFirstReleaseCarrierProjection {
            kura_active: 1,
            ..ProductionInFlightFirstReleaseCarrierProjection::default()
        },
        session: ProductionInFlightFirstReleaseSessionProjection {
            bodies: 1,
            producer_alive: true,
            ..ProductionInFlightFirstReleaseSessionProjection::default()
        },
        history: ProductionInFlightFirstReleaseHistoryProjection {
            ever_queue_plan_v4: true,
            ever_reservation_v5: true,
            ..ProductionInFlightFirstReleaseHistoryProjection::default()
        },
        decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
        release: ProductionInFlightFirstReleaseReleaseProjection::default(),
    };
    assert!(production_in_flight_first_release_state_kernel(state));
    let unsigned = AutonomousLifecycleCursorUnsignedV2::new(
        1,
        None,
        binding.clone(),
        AutonomousLifecycleCursorPhaseV2::live(generation.generation(), state)
            .expect("construct terminal-outcome Live phase"),
        local_peer,
    )
    .expect("construct terminal-outcome cursor");
    let signature = Signature::try_new(
        signer.private_key(),
        &unsigned
            .signing_preimage()
            .expect("encode terminal-outcome cursor preimage"),
    )
    .expect("sign terminal-outcome cursor");
    let cursor = unsigned
        .finalize(
            <[u8; 96]>::try_from(signature.payload())
                .expect("BLS-normal terminal cursor signature is exactly 96 bytes"),
            &payload.origin_proposal.descriptor.validator_set,
        )
        .expect("finalize terminal-outcome cursor");
    let (_, lease) = kura
        .read_autonomous_lifecycle_cursor(payload, &binding, generation)
        .expect("read absent terminal-outcome cursor")
        .into_parts();
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(lease, cursor.clone())
            .expect("persist terminal-outcome Live cursor")
            .cursor(),
        Some(&cursor),
        "terminal-outcome setup must read back the exact durable Live cursor",
    );
    (binding, reservation_group)
}
fn release_terminal_projection_for_test(
    kura: &Kura,
    payload: &LaneExecutablePayloadV1,
    retirement: &AutonomousLaneSlotRetirementV1,
    barrier: &LaneQueueReservationReleaseBarrierV3,
) -> ProductionInFlightFirstReleaseStateProjection {
    let authorization =
        AutonomousLaneReleaseProjectionContext::from_payload(kura, payload, retirement)
            .and_then(|context| context.queue_finalization_authorization(retirement, barrier))
            .expect("derive exact release terminal projection");
    authorization
        .consume_for_queue(barrier)
        .expect("consume exact release terminal projection")[2]
        .after
}
fn canonical_terminal_payload_for_test(
    lane: &LaneConfigEntry,
    height_context_id: HeightContextId,
    signer: &KeyPair,
    salt: u8,
) -> LaneExecutablePayloadV1 {
    let (_, epoch, template) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, signer);
    let mut builder = TransactionBuilder::new(
        test_network_id(b"kura-autonomous-view-checkpoint"),
        (*SAMPLE_GENESIS_ACCOUNT_ID).clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(10_000 + u64::from(salt)));
    let transaction = builder
        .with_instructions([Log::new(
            Level::INFO,
            format!("canonical terminal carrier lane {salt}"),
        )])
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let entrypoint = TransactionEntrypoint::External(transaction);

    let mut proposal = template.origin_proposal;
    proposal.descriptor.subject_hash =
        Hash::new_from_chunks(&[b"kura:canonical-terminal:subject:v1\0", &[salt]]);
    proposal.descriptor.payload_ownership_hash =
        Hash::new_from_chunks(&[b"kura:canonical-terminal:ownership:v1\0", &[salt]]);
    proposal.descriptor.rbc_instance_hash =
        Hash::new_from_chunks(&[b"kura:canonical-terminal:rbc:v1\0", &[salt]]);
    proposal.descriptor.accepted_transaction_hashes = vec![Hash::from(entrypoint.hash())];
    proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let routing_plan = RoutingPlan::single(crate::queue::RoutingDecision::new(
        lane.lane_id,
        lane.dataspace_id,
    ));
    let local_peer = PeerId::new(signer.public_key().clone());
    let (reservation_owner_hash, proposal_identity_hash) =
        autonomous_lane_reservation_identity_hashes_for_proposal(
            template.network_id,
            height_context_id,
            epoch,
            &proposal,
            &local_peer,
        )
        .expect("derive canonical terminal reservation identities");
    let reservation = LaneQueueReservationKeyV2 {
        version: LaneQueueReservationKeyV2::VERSION,
        entrypoint_hash: entrypoint.hash(),
        queue_plan_admission_binding_hash: Hash::new_from_chunks(&[
            b"kura:canonical-terminal:queue-plan:v1\0",
            &[salt],
        ]),
        routing_plan_digest: routing_plan.digest(),
        coordinator_leg: routing_plan.coordinator_leg(),
        lane_id: lane.lane_id,
        dataspace_id: lane.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        proposal_height: proposal.descriptor.proposal_height,
        lane_block_height: proposal.descriptor.lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        reservation_owner_hash,
        proposal_identity_hash,
    };
    LaneExecutablePayloadV1::new_signed_with_reservations(
        template.network_id,
        epoch,
        proposal,
        vec![entrypoint],
        vec![reservation],
        vec![routing_plan],
        vec![None],
        local_peer,
        signer.private_key(),
    )
    .expect("construct canonical terminal payload")
}
fn canonical_terminal_merge_execution_for_test(
    kura: &Kura,
    payload: &LaneExecutablePayloadV1,
    signer: &KeyPair,
) -> MergeLaneExecution {
    kura.persist_lane_executable_payload(payload, payload.network_id, payload.epoch)
        .expect("persist canonical terminal executable payload");
    let recovered = kura
        .recover_autonomous_lane_block_payload(
            &payload.origin_proposal,
            payload.network_id,
            payload.epoch,
        )
        .expect("recover canonical terminal execution input");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist canonical terminal execution input");
    let availability =
        durable_lane_payload_availability_for_kura(payload, &payload.origin_proposal, signer);
    kura.persist_lane_payload_availability_certificate(
        payload.origin_proposal.descriptor.lane_id,
        payload.origin_proposal.descriptor.lane_block_height,
        availability.clone(),
        payload.network_id,
        payload.epoch,
    )
    .expect("persist canonical terminal READY certificate");
    let (mut session, signer_pops) =
        committed_lane_block_session_for_kura_proposal(&payload.origin_proposal, signer);
    session.prepare_qc = availability.certificate;
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist canonical terminal certified lane source");
    let source = kura
        .durable_autonomous_lane_merge_source(
            payload.origin_proposal.descriptor.lane_id,
            payload.origin_proposal.descriptor.lane_block_height,
            payload.network_id,
            payload.epoch,
        )
        .expect("read canonical terminal durable merge source");
    canonical_terminal_merge_execution_from_durable_source_for_test(payload, source)
}
fn canonical_terminal_merge_execution_from_durable_source_for_test(
    payload: &LaneExecutablePayloadV1,
    source: DurableAutonomousLaneMergeSource,
) -> MergeLaneExecution {
    let DurableAutonomousLaneMergeSource {
        bundle,
        source_bundle,
        bundle_hash,
        input,
    } = source;
    let certified = bundle.certified;
    let results = input
        .entrypoints
        .iter()
        .map(|_| TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::new())))
        .collect::<Vec<_>>();
    let result_hashes = results
        .iter()
        .map(|result| Hash::from(result.hash()))
        .collect::<Vec<_>>();
    let descriptor = &payload.origin_proposal.descriptor;
    let settlement_commitment = LaneBlockCommitment {
        block_height: descriptor.lane_block_height,
        lane_id: descriptor.lane_id,
        lane_incarnation: descriptor.lane_incarnation,
        dataspace_id: descriptor.dataspace_id,
        tx_count: 0,
        total_local_amount: "0".parse().expect("zero local amount"),
        total_xor_due: "0".parse().expect("zero XOR due"),
        total_xor_after_haircut: "0".parse().expect("zero XOR after haircut"),
        total_xor_variance: "0".parse().expect("zero XOR variance"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let settlement_hash = iroha_data_model::nexus::compute_settlement_hash(&settlement_commitment)
        .expect("hash canonical terminal settlement");
    MergeLaneExecution {
        source_bundle,
        source_bundle_hash: bundle_hash,
        proposal: certified.proposal,
        origin_proposal: payload.origin_proposal.clone(),
        prepare_qc: certified.prepare_qc,
        commit_qc: certified.commit_qc,
        signer_proofs: certified
            .signer_pops
            .into_iter()
            .map(|(public_key, proof_of_possession)| {
                iroha_data_model::merge::MergeLaneSignerProof {
                    public_key,
                    proof_of_possession,
                }
            })
            .collect(),
        autonomous_network_id: payload.network_id,
        autonomous_epoch: payload.epoch,
        autonomous_payload_hash: payload.payload_hash,
        entrypoint_hashes: input.entrypoint_hashes,
        entrypoints: input.entrypoints,
        reservation_keys: input
            .reservation_keys
            .iter()
            .map(norito::encode_canonical)
            .collect::<Result<Vec<_>, _>>()
            .expect("encode canonical terminal reservations"),
        routing_plans: input
            .routing_plans
            .iter()
            .map(norito::encode_canonical)
            .collect::<Result<Vec<_>, _>>()
            .expect("encode canonical terminal routing plans"),
        native_amx_receipts: input.native_amx_receipts,
        result_hashes,
        results,
        settlement_hash,
        settlement_commitment,
    }
}
fn canonical_terminal_merge_carrier_for_test(
    execution: MergeLaneExecution,
    merge_epoch: u64,
) -> (Arc<SignedBlock>, Arc<SignedBlock>, MergeLedgerEntry) {
    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let raw_carrier = blocks.next();
    let entrypoint_count =
        u64::try_from(execution.entrypoints.len()).expect("terminal entrypoint count fits u64");
    let executions = vec![execution];
    let base_state_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"canonical terminal single-lane base state"));
    let write_set_root = Hash::new(b"canonical terminal single-lane write set");
    let mut batch = MergeExecutionBatch {
        version: 1,
        base_state_height: 1,
        base_state_hash,
        application_block_header: crate::merge::merge_application_header_from_carrier(
            &raw_carrier.header(),
        ),
        execution_root: crate::merge::merge_execution_root(&executions),
        entrypoint_count,
        entrypoint_merkle_root: crate::merge::merge_execution_entrypoint_merkle_root(&executions)
            .expect("terminal carrier has an entrypoint"),
        result_merkle_root: crate::merge::merge_execution_result_merkle_root(&executions)
            .expect("terminal carrier has a result"),
        lanes: executions,
        application_write_set_root: Hash::new(b"canonical terminal single-lane application writes"),
        write_set_root,
        expected_post_state_hash: crate::merge::merge_expected_post_state_hash(
            1,
            base_state_hash,
            write_set_root,
        ),
        batch_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    batch.batch_hash = crate::merge::merge_execution_batch_hash(&batch);
    let mut merge_entry = sample_merge_entry(merge_epoch);
    merge_entry.epoch_id = merge_epoch;
    merge_entry.execution_batch = Some(batch);
    let bound_carrier = bind_merge_entry_to_carrier(raw_carrier, &mut merge_entry);
    let mut executed_carrier = bound_carrier.as_ref().clone();
    attach_ok_results_to_block(&mut executed_carrier);
    (parent, Arc::new(executed_carrier), merge_entry)
}
fn canonical_terminal_projection_for_test(
    group: LaneQueueReservationGroupBindingV1,
) -> ProductionInFlightFirstReleaseStateProjection {
    let binding_a = canonical_lane_queue_reservation_group_identity_projection(group);
    let projection = ProductionInFlightFirstReleaseStateProjection {
        validator_count: 1,
        producer: 1,
        producer_selected_owner: 1,
        replicated_carrier_owners: 0,
        payload_binding_a: 1,
        binding_a,
        queue: ProductionInFlightFirstReleaseQueueProjection {
            plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_TOMBSTONED,
            selected_count: group.reservation_count,
            reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN,
        },
        carrier: ProductionInFlightFirstReleaseCarrierProjection {
            kura_active: 1,
            execution_input_durable: 1,
            ready_qc_durable: true,
        },
        session: ProductionInFlightFirstReleaseSessionProjection {
            bodies: 1,
            ready_authorized: 1,
            producer_alive: true,
            ..ProductionInFlightFirstReleaseSessionProjection::default()
        },
        history: ProductionInFlightFirstReleaseHistoryProjection {
            ever_queue_plan_v4: true,
            ever_reservation_v5: true,
            ever_execution_input_durable: 1,
            ever_ready_authorized: 1,
            ready_signed: 1,
            ever_ready_qc_durable: true,
            reservation_committed_prefix: group.reservation_count,
            queue_plan_tombstoned_prefix: group.reservation_count,
            reservation_commit_forgotten_prefix: group.reservation_count,
            ..ProductionInFlightFirstReleaseHistoryProjection::default()
        },
        decision: ProductionInFlightFirstReleaseDecisionProjection {
            lane_commit_scope: binding_a,
            lane_commit_owner: 1,
            wsv_committed: true,
            application_count: 1,
            applied_by: 1,
            ..ProductionInFlightFirstReleaseDecisionProjection::default()
        },
        release: ProductionInFlightFirstReleaseReleaseProjection::default(),
    };
    assert!(production_in_flight_first_release_state_kernel(projection));
    projection
}
#[test]
fn lifecycle_release_terminal_outcomes_are_exact_idempotent_and_ordered() {
    let temp_dir = TempDir::new().expect("terminal-outcome temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let local_peer = PeerId::new(signer.public_key().clone());
    let height_context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"kura-terminal-outcome-height-context"),
    ));
    let lanes = [
        lane_config.primary(),
        lane_config.entry(LaneId::new(1)).expect("lane one"),
    ];
    let payloads = lanes
        .iter()
        .map(|lane| {
            let (_, _, template) =
                autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
            lifecycle_terminal_bound_payload_for_test(&template, height_context_id, &signer)
        })
        .collect::<Vec<_>>();
    let network_id = payloads[0].network_id;
    let epoch = payloads[0].epoch;
    let (kura, _) = Kura::new(&config, &lane_config).expect("terminal-outcome Kura");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind terminal-outcome local peer");
    let generation = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim terminal-outcome process generation");
    let mut attempts = Vec::new();
    for (index, (lane, payload)) in lanes.iter().zip(&payloads).enumerate() {
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, payload);
        kura.persist_lane_executable_payload(payload, network_id, epoch)
            .expect("persist terminal-outcome payload");
        let (_, group) = install_live_lifecycle_cursor_for_terminal_test(
            &kura,
            &generation,
            payload,
            height_context_id,
            &signer,
        );
        let retirement = AutonomousLaneSlotRetirementV1::from_payload(payload);
        kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
            .expect("persist terminal-outcome retirement");
        let barrier = retirement
            .queue_release_barrier()
            .expect("derive terminal-outcome Queue barrier");
        kura.finalize_autonomous_lane_slot_release(&retirement, &barrier, network_id, epoch)
            .expect("publish terminal-outcome Released claims");
        if index == 0 {
            assert!(
                kura.persist_autonomous_lifecycle_release_terminal_outcome_pending(
                    &retirement,
                    test_network_id(b"wrong terminal-outcome genesis"),
                    epoch,
                )
                .is_err()
            );
            assert!(
                kura.persist_autonomous_lifecycle_release_terminal_outcome_pending(
                    &retirement,
                    network_id,
                    epoch.saturating_add(1),
                )
                .is_err()
            );
        }
        let _ = kura
            .persist_autonomous_lifecycle_release_terminal_outcome_pending(
                &retirement,
                network_id,
                epoch,
            )
            .expect("persist exact Pending release outcome");
        let path = Kura::autonomous_lifecycle_terminal_outcome_path_for_entry(
            lane,
            temp_dir.path(),
            payload.origin_proposal.descriptor.lane_block_height,
            payload.origin_proposal.descriptor.proposal_height,
        );
        let bytes = fs::read(&path).expect("read Pending terminal outcome");
        let _ = kura
            .persist_autonomous_lifecycle_release_terminal_outcome_pending(
                &retirement,
                network_id,
                epoch,
            )
            .expect("replay exact Pending release outcome");
        assert_eq!(fs::read(&path).expect("reread Pending outcome"), bytes);
        attempts.push((payload.clone(), group, retirement, barrier, path, bytes));
    }
    let (first_payload, first_group, first_retirement, first_barrier, first_path, first_bytes) =
        &attempts[0];
    assert!(
        kura.active_autonomous_lifecycle_attempt_inventory(
            &generation,
            first_group.identity.lane_id,
            first_group.identity.dataspace_id,
            first_group.identity.lane_incarnation,
        )
        .is_err(),
        "default lifecycle inventory must keep every Pending outcome fail-closed",
    );
    let covered = kura
        .active_autonomous_lifecycle_attempt_inventory_with_planner_covered_pending_groups(
            &generation,
            first_group.identity.lane_id,
            first_group.identity.dataspace_id,
            first_group.identity.lane_incarnation,
            std::slice::from_ref(first_group),
        )
        .expect("exact planner-covered Pending group must be source-validated and exposed");
    assert_eq!(covered.len(), 1);
    assert_eq!(covered[0].executable_payload(), first_payload);
    assert!(covered[0].cursor().is_some());
    let observer_covered = kura
        .read_only_active_autonomous_lifecycle_attempt_inventory_with_planner_covered_pending_groups(
            network_id,
            &local_peer,
            first_group.identity.lane_id,
            first_group.identity.dataspace_id,
            first_group.identity.lane_incarnation,
            std::slice::from_ref(first_group),
        )
        .expect("observer inventory exposes the same exact planner-covered attempt");
    assert_eq!(observer_covered.len(), 1);
    assert_eq!(observer_covered[0].executable_payload(), first_payload);
    assert!(observer_covered[0].cursor().is_some());
    let mut substituted_coverage = *first_group;
    substituted_coverage.reservation_group_hash = Hash::new(b"substituted planner coverage");
    assert!(
        kura.active_autonomous_lifecycle_attempt_inventory_with_planner_covered_pending_groups(
            &generation,
            first_group.identity.lane_id,
            first_group.identity.dataspace_id,
            first_group.identity.lane_incarnation,
            &[substituted_coverage],
        )
        .is_err(),
        "non-exact or unused planner coverage must not suppress a Pending attempt",
    );
    let first_pending = Kura::decode_autonomous_lifecycle_terminal_outcome(first_path, first_bytes)
        .expect("decode first Pending terminal outcome");
    assert!(matches!(
        first_pending.stage(),
        AutonomousLifecycleTerminalOutcomeStageV1::Pending { .. }
    ));
    let mut tampered_reserved_terminal = first_pending.clone();
    let AutonomousLifecycleTerminalOutcomeStageV1::Pending { reserved_terminal } =
        &mut tampered_reserved_terminal.body.stage
    else {
        unreachable!("decoded fixture outcome is Pending")
    };
    reserved_terminal.version = AutonomousLifecycleStableStateV1::VERSION;
    assert!(
        AutonomousLifecycleTerminalOutcomeV1::from_body(tampered_reserved_terminal.body).is_err(),
        "Pending must reject any non-canonical reserved terminal payload",
    );
    for case in 0_u8..6 {
        let mut tampered = first_pending.clone();
        match case {
            0 => tampered.body.binding.network_id = test_network_id(b"tampered-terminal-genesis"),
            1 => tampered.body.binding.epoch = epoch.saturating_add(1),
            2 => tampered.body.binding.dataspace_id = DataSpaceId::new(999),
            3 => {
                tampered.body.binding.lane_incarnation = Hash::new(b"tampered terminal incarnation")
            }
            4 => tampered.body.binding.proposal_height = 777,
            5 => {
                tampered
                    .body
                    .binding
                    .reservation_group
                    .reservation_group_hash = Hash::new(b"tampered terminal group")
            }
            _ => unreachable!(),
        }
        let tampered = AutonomousLifecycleTerminalOutcomeV1::from_body(tampered.body)
            .expect("rehash structurally valid tampered terminal outcome");
        fs::write(
            first_path,
            tampered
                .encode_framed()
                .expect("encode tampered terminal outcome"),
        )
        .expect("write tampered terminal outcome");
        assert!(
            kura.pending_autonomous_lifecycle_terminal_outcome_inventory()
                .is_err(),
            "terminal binding tamper case {case} must fail before Queue mutation",
        );
        fs::write(first_path, first_bytes).expect("restore Pending terminal outcome");
    }
    let mut wrong_retirement = first_pending.clone();
    wrong_retirement.body.source = AutonomousLifecycleTerminalOutcomeSourceV1::RetiredRelease {
        retirement_hash: Hash::new(b"tampered terminal retirement"),
    };
    let wrong_retirement = AutonomousLifecycleTerminalOutcomeV1::from_body(wrong_retirement.body)
        .expect("rehash wrong-retirement terminal outcome");
    fs::write(
        first_path,
        wrong_retirement
            .encode_framed()
            .expect("encode wrong-retirement outcome"),
    )
    .expect("write wrong-retirement outcome");
    assert!(
        kura.pending_autonomous_lifecycle_terminal_outcome_inventory()
            .is_err(),
        "a substituted retirement source must fail before Queue mutation",
    );
    fs::write(first_path, first_bytes).expect("restore retirement-bound Pending outcome");
    let recoveries = kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .expect("inventory exact Pending outcomes");
    let routes = recoveries
        .iter()
        .flat_map(AutonomousLifecyclePendingTerminalOutcomeRecovery::route_identities)
        .collect::<Vec<_>>();
    let mut expected_routes = routes.clone();
    expected_routes.sort();
    assert_eq!(
        routes, expected_routes,
        "Pending inventory must be deterministic"
    );
    assert_eq!(routes.len(), 2);
    let expected_pending_hash = recoveries
        .into_iter()
        .find_map(|recovery| match recovery {
            AutonomousLifecyclePendingTerminalOutcomeRecovery::RetiredRelease {
                barrier,
                source_outcome_authorization,
                ..
            } if barrier == *first_barrier => {
                source_outcome_authorization.consume_for_queue(&barrier)
            }
            _ => None,
        })
        .expect("inventory carries the first exact Pending hash");
    assert_eq!(expected_pending_hash, first_pending.outcome_hash);
    let first_terminal =
        release_terminal_projection_for_test(&kura, first_payload, first_retirement, first_barrier);
    let mut swapped = first_pending.clone();
    swapped.body.source = AutonomousLifecycleTerminalOutcomeSourceV1::CanonicalCarrier {
        merge_epoch_id: 9,
        merge_entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"swapped merge entry")),
        carrier_block_height: 1,
        carrier_block_hash: HashOf::from_untyped_unchecked(Hash::new(b"swapped carrier")),
        application_receipt_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"swapped application receipt",
        )),
    };
    let swapped = AutonomousLifecycleTerminalOutcomeV1::from_body(swapped.body)
        .expect("rehash swapped Pending source");
    fs::write(
        first_path,
        swapped
            .encode_framed()
            .expect("encode swapped Pending source"),
    )
    .expect("swap Pending source before completion");
    let swap_error = kura
        .complete_autonomous_lifecycle_terminal_outcome(
            *first_group,
            first_terminal,
            false,
            expected_pending_hash,
        )
        .expect_err("completion must CAS the exact inventoried Pending hash");
    assert!(
        swap_error
            .to_string()
            .contains("exact current source outcome")
    );
    fs::write(first_path, first_bytes).expect("restore exact Pending before completion");
    kura.complete_autonomous_lifecycle_terminal_outcome(
        *first_group,
        first_terminal,
        false,
        expected_pending_hash,
    )
    .expect("complete exact Pending release outcome");
    let complete_bytes = fs::read(first_path).expect("read Complete terminal outcome");
    assert_ne!(&complete_bytes, first_bytes);
    assert_eq!(
        complete_bytes.len(),
        first_bytes.len(),
        "Pending reserves the exact framed length required by Complete",
    );
    let pending_len = u64::try_from(first_bytes.len()).expect("Pending length fits u64");
    let complete_len = u64::try_from(complete_bytes.len()).expect("Complete length fits u64");
    let aggregate_limit = u64::try_from(AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES)
        .expect("aggregate byte limit fits u64");
    assert!(
        Kura::validate_autonomous_lifecycle_terminal_outcome_budget(
            attempts.len(),
            aggregate_limit,
            pending_len,
            complete_len,
            true,
        )
        .is_ok(),
        "a multi-outcome namespace exactly at its stable aggregate bound must retain completion headroom",
    );
    assert!(
        Kura::validate_autonomous_lifecycle_terminal_outcome_budget(
            attempts.len(),
            aggregate_limit,
            pending_len,
            complete_len.saturating_add(1),
            true,
        )
        .is_err(),
        "the near-budget regression must detect any accidental Complete growth",
    );
    let complete = Kura::decode_autonomous_lifecycle_terminal_outcome(first_path, &complete_bytes)
        .expect("decode Complete terminal outcome");
    assert!(complete.is_complete());
    let stale_pending_error = kura
        .complete_autonomous_lifecycle_terminal_outcome(
            *first_group,
            first_terminal,
            false,
            expected_pending_hash,
        )
        .expect_err("Complete retry must reject the superseded Pending hash");
    assert!(
        stale_pending_error
            .to_string()
            .contains("exact current source outcome")
    );
    kura.complete_autonomous_lifecycle_terminal_outcome(
        *first_group,
        first_terminal,
        false,
        complete.outcome_hash,
    )
    .expect("replay Complete release outcome");
    assert_eq!(
        fs::read(first_path).expect("reread Complete outcome"),
        complete_bytes,
        "Complete replay must be a byte-for-byte no-op",
    );
    let (
        second_payload,
        second_group,
        second_retirement,
        second_barrier,
        second_path,
        second_bytes,
    ) = &attempts[1];
    let second_pending =
        Kura::decode_autonomous_lifecycle_terminal_outcome(second_path, second_bytes)
            .expect("decode second Pending terminal outcome");
    let remaining = kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .expect("inventory second Pending after first completion");
    assert_eq!(remaining.len(), 1);
    kura.complete_autonomous_lifecycle_terminal_outcome(
        *second_group,
        release_terminal_projection_for_test(
            &kura,
            second_payload,
            second_retirement,
            second_barrier,
        ),
        false,
        second_pending.outcome_hash,
    )
    .expect("complete second Pending release outcome");
    assert!(
        kura.pending_autonomous_lifecycle_terminal_outcome_inventory()
            .expect("terminal inventory after completion")
            .is_empty()
    );
    drop(kura);
    let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen completed outcomes");
    assert!(
        reopened
            .pending_autonomous_lifecycle_terminal_outcome_inventory()
            .expect("restart terminal inventory")
            .is_empty(),
        "Complete outcomes must stay terminal across restart",
    );
    let missing_artifact_namespace = first_path
        .parent()
        .expect("terminal outcome has an artifact namespace");
    fs::remove_dir_all(missing_artifact_namespace)
        .expect("remove the first lane artifact namespace");
    assert!(
        reopened
            .active_autonomous_lifecycle_attempt_inventory_with_planner_covered_pending_groups(
                &generation,
                first_group.identity.lane_id,
                first_group.identity.dataspace_id,
                first_group.identity.lane_incarnation,
                std::slice::from_ref(first_group),
            )
            .is_err(),
        "a missing artifact namespace must not accept unused planner Pending coverage",
    );
}
#[test]
fn canonical_carrier_terminal_recovery_materializes_and_partitions_the_full_lane_set() {
    let temp_dir = TempDir::new().expect("canonical terminal temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let local_peer = PeerId::new(signer.public_key().clone());
    let height_context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"kura-canonical-terminal-height-context"),
    ));
    let lanes = [
        lane_config.primary(),
        lane_config.entry(LaneId::new(1)).expect("lane one"),
    ];
    let payloads = lanes
        .iter()
        .enumerate()
        .map(|(index, lane)| {
            canonical_terminal_payload_for_test(
                lane,
                height_context_id,
                &signer,
                u8::try_from(index + 1).expect("fixture lane salt fits u8"),
            )
        })
        .collect::<Vec<_>>();
    assert_ne!(
        payloads[0].reservation_keys[0].entrypoint_hash,
        payloads[1].reservation_keys[0].entrypoint_hash,
        "carrier members require distinct entrypoint identities",
    );
    let network_id = payloads[0].network_id;
    let epoch = payloads[0].epoch;
    let (kura, _) = Kura::new(&config, &lane_config).expect("canonical terminal Kura");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind canonical terminal local peer");
    let generation = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim canonical terminal process generation");
    let mut executions = Vec::new();
    let mut groups = Vec::new();
    let mut outcome_paths = Vec::new();
    executions
        .try_reserve_exact(payloads.len())
        .expect("reserve canonical terminal executions");
    groups
        .try_reserve_exact(payloads.len())
        .expect("reserve canonical terminal groups");
    outcome_paths
        .try_reserve_exact(payloads.len())
        .expect("reserve canonical terminal paths");
    for (lane, payload) in lanes.iter().zip(&payloads) {
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, payload);
        executions.push(canonical_terminal_merge_execution_for_test(
            &kura, payload, &signer,
        ));
        let (_, group) = install_live_lifecycle_cursor_for_terminal_test(
            &kura,
            &generation,
            payload,
            height_context_id,
            &signer,
        );
        groups.push(group);
        outcome_paths.push(Kura::autonomous_lifecycle_terminal_outcome_path_for_entry(
            lane,
            temp_dir.path(),
            payload.origin_proposal.descriptor.lane_block_height,
            payload.origin_proposal.descriptor.proposal_height,
        ));
    }
    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let raw_carrier = blocks.next();
    let entrypoint_count = executions
        .iter()
        .try_fold(0_u64, |count, execution| {
            count.checked_add(u64::try_from(execution.entrypoints.len()).ok()?)
        })
        .expect("canonical terminal entrypoint count fits u64");
    let base_state_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"canonical terminal base state"));
    let write_set_root = Hash::new(b"canonical terminal write set");
    let mut batch = MergeExecutionBatch {
        version: 1,
        base_state_height: 1,
        base_state_hash,
        application_block_header: crate::merge::merge_application_header_from_carrier(
            &raw_carrier.header(),
        ),
        execution_root: crate::merge::merge_execution_root(&executions),
        entrypoint_count,
        entrypoint_merkle_root: crate::merge::merge_execution_entrypoint_merkle_root(&executions)
            .expect("canonical terminal carrier has entrypoints"),
        result_merkle_root: crate::merge::merge_execution_result_merkle_root(&executions)
            .expect("canonical terminal carrier has results"),
        lanes: executions,
        application_write_set_root: Hash::new(b"canonical terminal application writes"),
        write_set_root,
        expected_post_state_hash: crate::merge::merge_expected_post_state_hash(
            1,
            base_state_hash,
            write_set_root,
        ),
        batch_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    batch.batch_hash = crate::merge::merge_execution_batch_hash(&batch);
    let mut merge_entry = sample_merge_entry(epoch);
    merge_entry.epoch_id = epoch;
    merge_entry.execution_batch = Some(batch);
    let bound_carrier = bind_merge_entry_to_carrier(raw_carrier, &mut merge_entry);
    let mut executed_carrier = bound_carrier.as_ref().clone();
    attach_ok_results_to_block(&mut executed_carrier);
    let carrier = Arc::new(executed_carrier);
    assert_eq!(
        merge_entry
            .execution_batch
            .as_ref()
            .expect("canonical terminal execution batch")
            .application_block_header,
        crate::merge::merge_application_header_from_carrier(&carrier.header()),
    );
    let carrier_height = carrier.header().height().get();
    let carrier_hash = carrier.hash();
    kura.store_block(parent)
        .expect("store canonical terminal carrier parent");
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &merge_entry)
        .expect("store canonical terminal merge carrier");
    let _ = persist_v2_finality_chain_through(
        &kura,
        NonZeroUsize::new(usize::try_from(carrier_height).expect("carrier height fits usize"))
            .expect("carrier height is non-zero"),
    );
    kura.persist_merge_lane_block_application_receipts(&merge_entry, carrier_height, carrier_hash)
        .expect("persist canonical terminal application receipts");
    assert!(
        outcome_paths.iter().all(|path| !path.exists()),
        "zero-file crash boundary starts without a terminal outcome seed",
    );
    let publication = kura
        .reconstruct_autonomous_lifecycle_canonical_carrier_source_outcomes_for_group(&groups[0])
        .expect("reconstruct zero-file canonical carrier source-outcome set");
    assert_eq!(
        publication.entry_hash(),
        crate::merge::merge_ledger_entry_hash(&merge_entry),
    );
    assert_eq!(
        publication
            .consume_for_v2_apply(&merge_entry)
            .expect("consume exact reconstructed carrier publication")
            .len(),
        2,
    );
    assert!(outcome_paths.iter().all(|path| path.is_file()));
    let initial_recoveries = kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .expect("inventory complete Pending carrier handoff");
    assert_eq!(initial_recoveries.len(), 1);
    let expected_groups = initial_recoveries[0]
        .pending_reservation_groups()
        .expect("complete Pending carrier exposes exact expected groups");
    assert_eq!(expected_groups.len(), 2);
    let initial_stages = kura
        .verify_expected_autonomous_lifecycle_terminal_outcome_stages(network_id, &expected_groups)
        .expect("directly prove both Pending carrier members");
    assert!(initial_stages.iter().all(|stage| {
        stage.source_kind() == AutonomousLifecycleTerminalOutcomeSourceKind::CanonicalCarrier
            && stage.stage() == AutonomousLifecycleTerminalOutcomeDurableStage::Pending
    }));
    fs::remove_file(&outcome_paths[1]).expect("remove strict-prefix second outcome");
    assert!(
        kura.verify_expected_autonomous_lifecycle_terminal_outcome_stages(
            network_id,
            &expected_groups,
        )
        .is_err(),
        "direct expected-stage proof must reject a deleted handoff member without reconstruction",
    );
    let prefix_recoveries = kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .expect("strict-prefix inventory reconstructs every missing carrier member");
    assert_eq!(prefix_recoveries.len(), 1);
    assert_eq!(prefix_recoveries[0].pending_outcome_count(), 2);
    assert_eq!(prefix_recoveries[0].route_identities().len(), 2);
    assert!(outcome_paths[1].is_file());
    let second_bytes = fs::read(&outcome_paths[1]).expect("read second Pending outcome");
    fs::write(&outcome_paths[1], [0xFF]).expect("corrupt later carrier member");
    assert!(
        kura.pending_autonomous_lifecycle_terminal_outcome_inventory()
            .is_err(),
        "a malformed later carrier member must prevent every recovery token from returning",
    );
    fs::write(&outcome_paths[1], &second_bytes).expect("restore second Pending outcome");
    let first_bytes = fs::read(&outcome_paths[0]).expect("read first Pending outcome");
    let first_pending =
        Kura::decode_autonomous_lifecycle_terminal_outcome(&outcome_paths[0], &first_bytes)
            .expect("decode first Pending outcome");
    kura.complete_autonomous_lifecycle_terminal_outcome(
        groups[0],
        canonical_terminal_projection_for_test(groups[0]),
        true,
        first_pending.outcome_hash,
    )
    .expect("complete first canonical carrier member");
    let mixed = kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .expect("inventory mixed Pending and Complete carrier members");
    assert_eq!(mixed.len(), 1);
    assert_eq!(mixed[0].pending_outcome_count(), 1);
    assert_eq!(mixed[0].route_identities().len(), 2);
    let pending_groups = mixed[0]
        .pending_reservation_groups()
        .expect("mixed carrier exposes its exact Pending group");
    assert_eq!(pending_groups.len(), 1);
    assert_eq!(pending_groups[0].binding(), groups[1]);
    assert_eq!(
        pending_groups[0].ordered_keys(),
        payloads[1].reservation_keys.as_slice(),
    );
    let mixed_stages = kura
        .verify_expected_autonomous_lifecycle_terminal_outcome_stages(network_id, &expected_groups)
        .expect("directly prove mixed Complete/Pending carrier stages");
    assert_eq!(
        mixed_stages
            .iter()
            .map(|stage| stage.stage())
            .collect::<Vec<_>>(),
        vec![
            AutonomousLifecycleTerminalOutcomeDurableStage::Complete,
            AutonomousLifecycleTerminalOutcomeDurableStage::Pending,
        ],
    );
    let AutonomousLifecyclePendingTerminalOutcomeRecovery::Canonical(recovery) =
        mixed.into_iter().next().expect("mixed carrier recovery")
    else {
        panic!("mixed carrier recovery must remain canonical")
    };
    let (pending, complete, _, recovered_entry, _, _, recovered_chain) = recovery
        .consume_for_v2_apply()
        .expect("consume mixed carrier recovery partition");
    assert_eq!(pending.len(), 1);
    assert_eq!(complete.len(), 1);
    assert_eq!(recovered_entry, merge_entry);
    assert_eq!(recovered_chain, network_id);
    assert!(
        Kura::decode_autonomous_lifecycle_terminal_outcome(
            &outcome_paths[0],
            &fs::read(&outcome_paths[0]).expect("read Complete first outcome"),
        )
        .expect("decode Complete first outcome")
        .is_complete(),
    );
    let second_pending = Kura::decode_autonomous_lifecycle_terminal_outcome(
        &outcome_paths[1],
        &fs::read(&outcome_paths[1]).expect("read second Pending outcome for completion"),
    )
    .expect("decode second Pending outcome for completion");
    kura.complete_autonomous_lifecycle_terminal_outcome(
        groups[1],
        canonical_terminal_projection_for_test(groups[1]),
        true,
        second_pending.outcome_hash,
    )
    .expect("complete second canonical carrier member");
    let complete_stages = kura
        .verify_expected_autonomous_lifecycle_terminal_outcome_stages(network_id, &expected_groups)
        .expect("directly prove both completed carrier members");
    assert!(complete_stages.iter().all(|stage| {
        stage.source_kind() == AutonomousLifecycleTerminalOutcomeSourceKind::CanonicalCarrier
            && stage.stage() == AutonomousLifecycleTerminalOutcomeDurableStage::Complete
    }));
}
