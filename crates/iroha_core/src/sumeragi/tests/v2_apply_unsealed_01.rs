macro_rules! v2_apply_test {
    ($name:ident, $body:block) => {
        #[test]
        fn $name() {
            let handle = crate::sumeragi::sumeragi_thread_builder(concat!(
                "sumeragi-v2-apply-test-",
                stringify!($name)
            ))
            .spawn(move || $body)
            .expect("spawn v2 apply test on the production consensus stack");
            if let Err(payload) = handle.join() {
                std::panic::resume_unwind(payload);
            }
        }
    };
}
v2_apply_test!(
    prospective_autoscale_retirement_queue_veto_rejects_exact_reserved_route,
    {
        let fixture = ApplyFixture::new_with_lane_lifecycle();
        let reservation_lane = install_recreatable_reservation_lane(&fixture);
        let queue = Arc::clone(&fixture.service.queue);
        let journal_dir = tempfile::tempdir().expect("retirement Queue-veto journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install retirement Queue-veto plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install retirement Queue-veto reservation journal");
        let transaction = TransactionBuilder::new(
            fixture.context.network_id,
            fixture.service.genesis_account.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "prospective autoscale retirement Queue veto".to_owned(),
        )])
        .sign(fixture.genesis_key.private_key());
        let (reservation, _) = reserve_transaction_for_lane_test_with_identity(
            fixture.state.as_ref(),
            queue.as_ref(),
            transaction,
            reservation_lane.id,
            reservation_lane.dataspace_id,
            Hash::new(b"retirement Queue-veto owner"),
            Hash::new(b"retirement Queue-veto proposal"),
        );
        assert_eq!(queue.live_lane_reservations(), vec![reservation]);
        let queue_retirement_observer = queue.lock_lane_retirement_observer();
        let error = V2ApplyService::validate_autoscale_retirement_queue_binding(
            &queue_retirement_observer,
            reservation.lane_id,
            reservation.dataspace_id,
            reservation.lane_incarnation,
        )
        .expect_err("the exact reserved route must veto prospective retirement");
        let message = match error {
            V2ApplyError::Validation(message) => message,
            unexpected => panic!("unexpected retirement Queue-veto error: {unexpected}"),
        };
        assert!(message.contains("blocked by local Queue ownership"));
        assert!(message.contains(&format!("lane {}", reservation.lane_id.as_u32())));
        assert!(message.contains(&format!("dataspace {}", reservation.dataspace_id.as_u64())));
        let unrelated_incarnation = Hash::new(b"unrelated retirement incarnation");
        assert_ne!(unrelated_incarnation, reservation.lane_incarnation);
        V2ApplyService::validate_autoscale_retirement_queue_binding(
            &queue_retirement_observer,
            reservation.lane_id,
            reservation.dataspace_id,
            unrelated_incarnation,
        )
        .expect("a reservation from another incarnation must not veto retirement");
        assert_eq!(
            queue.live_lane_reservations(),
            vec![reservation],
            "the read-only veto must preserve exact Queue ownership"
        );
    }
);
v2_apply_test!(merge_publication_emits_once_across_exact_retry, {
    let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
    let mut store = fixture.reopen_body_store();
    fixture.execute(&mut store).expect("commit carrier parent");
    let mut entry = pending_merge_entry(&fixture.context, 0, b"v2 apply live publication fixture");
    entry.epoch_id = 1;
    entry.merge_qc.epoch_id = 1;
    entry.merge_qc.carrier_height = 2;
    entry.merge_qc.carrier_parent_hash = fixture.body.hash();
    entry.merge_qc.view = 0;
    let execution_context = BlockExecutionContextBundle::new(Vec::new())
        .with_merge_entry(CertifiedMergeLedgerReference::new(&entry));
    let carrier = BlockBuilder::new_with_time_source(Vec::new(), TimeSource::new_system())
        .chain(0, Some(&fixture.body))
        .with_execution_context(Some(execution_context))
        .try_sign_with_index(fixture.genesis_key.private_key(), 0)
        .expect("sign merge carrier")
        .unpack(|_| {});
    let carrier = complete_empty_fixture_block(SignedBlock::from(carrier), &fixture.genesis_key);
    fixture
        .kura
        .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
        .expect("persist exact merge carrier and sidecar");
    fixture.persist_exact_v2_finality_chain(&[&carrier]);
    fixture
        .state
        .seed_applied_merge_entry_for_v2_settlement_test(&entry)
        .expect("seed exact post-commit merge state");
    let mut block_hashes = fixture.state.block_hashes.block();
    block_hashes.push_for_tests(carrier.hash());
    block_hashes.commit_for_tests();
    fixture
        .state
        .update_latest_block_header_cache_for_tests(carrier.header().clone());
    let mut events = fixture.service.events_sender.subscribe();
    fixture
        .service
        .publish_committed_block_merge_entry(&carrier)
        .expect("publish live merge entry");
    let event = events.try_recv().expect("receive live merge event");
    let EventBox::Pipeline(iroha_data_model::events::pipeline::PipelineEventBox::Merge(event)) =
        event
    else {
        panic!("v2 apply must publish the merge-ledger event");
    };
    assert_eq!(event.entry, entry);
    assert_eq!(fixture.state.merge_ledger.snapshot().len(), 1);
    fixture
        .service
        .publish_committed_block_merge_entry(&carrier)
        .expect("retry exact live merge publication");
    assert!(matches!(
        events.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    assert_eq!(fixture.state.merge_ledger.snapshot().len(), 1);
    assert_eq!(
        finalize_committed_block_merge_reservations(
            fixture.state.as_ref(),
            fixture.service.queue.as_ref(),
            fixture.kura.as_ref(),
            &carrier,
            fixture.context.network_id,
        )
        .expect("control-only carrier has no Queue cleanup authority to consume"),
        0
    );
});
v2_apply_test!(
    live_merge_publication_persists_application_receipt_before_retry,
    {
        let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        let transaction = fixture
            .body
            .external_transactions()
            .next()
            .expect("fixture transaction")
            .clone();
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Queue::from_config(QueueConfig::default(), events_sender);
        let journal_dir = tempfile::tempdir().expect("reservation journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install queue-plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install reservation journal");
        let (reservation, entrypoint) =
            reserve_transaction_for_test(fixture.state.as_ref(), &queue, transaction);
        let (parent, entry) =
            merge_entry_with_reservation(&fixture.context, entrypoint, reservation);
        let carrier = body_with_exact_merge_execution_header(&entry);
        fixture
            .kura
            .store_block(Arc::new(parent.clone()))
            .expect("persist execution-carrier parent");
        fixture
            .kura
            .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
            .expect("persist exact execution carrier and merge log");
        fixture.persist_exact_v2_finality_chain(&[&parent, &carrier]);
        fixture
            .state
            .seed_applied_merge_entry_for_v2_settlement_test(&entry)
            .expect("seed exact post-commit merge state");
        let mut block_hashes = fixture.state.block_hashes.block();
        block_hashes.push_for_tests(parent.hash());
        block_hashes.push_for_tests(carrier.hash());
        block_hashes.commit_for_tests();
        fixture
            .state
            .update_latest_block_header_cache_for_tests(carrier.header().clone());
        fixture
            .service
            .publish_committed_block_merge_entry(&carrier)
            .expect("publish live execution merge entry");
        let receipt = fixture
            .kura
            .read_lane_block_application_receipt(LaneId::SINGLE, 1)
            .expect("live post-WSV publication must persist the application receipt");
        assert_eq!(
            receipt.format,
            crate::kura::LaneBlockApplicationReceiptArtifactFormat::MergeExecution
        );
        let receipt_hash = HashOf::new(&receipt);
        fixture
            .service
            .publish_committed_block_merge_entry(&carrier)
            .expect("retry exact live execution merge publication");
        assert_eq!(
            fixture
                .kura
                .read_lane_block_application_receipt(LaneId::SINGLE, 1)
                .as_ref()
                .map(HashOf::new),
            Some(receipt_hash),
            "crash retry must preserve one byte-identical receipt"
        );
    }
);
v2_apply_test!(committed_merge_reservation_is_finalized_exactly_once, {
    let fixture = ApplyFixture::new_for_production_recovered_decision_apply_with_lane_lifecycle();
    let transaction = fixture
        .body
        .external_transactions()
        .next()
        .expect("fixture transaction")
        .clone();
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = Queue::from_config(QueueConfig::default(), events_sender);
    let journal_dir = tempfile::tempdir().expect("reservation journal directory");
    queue
        .install_plan_journal(
            journal_dir.path().join("queue-plans.norito"),
            1024 * 1024,
            true,
        )
        .expect("install queue-plan journal");
    queue
        .install_lane_reservation_journal(
            journal_dir.path().join("lane-reservations.norito"),
            1024 * 1024,
        )
        .expect("install reservation journal");
    let (initial_reservation, entrypoint) =
        reserve_transaction_for_test(fixture.state.as_ref(), &queue, transaction);
    let (_, identity_entry) =
        merge_entry_with_reservation(&fixture.context, entrypoint.clone(), initial_reservation);
    let identity_payload = Kura::decode_autonomous_lane_merge_bundle(
        &identity_entry
            .execution_batch
            .as_ref()
            .expect("committed cleanup identity execution batch")
            .lanes[0]
            .source_bundle,
        fixture.context.network_id,
        fixture.context.epoch,
    )
    .expect("decode committed cleanup identity source")
    .autonomous
    .executable_payload;
    let (reservation_owner_hash, proposal_identity_hash) =
        super::super::lane_planner::autonomous_lane_reservation_identity_hashes_for_proposal(
            fixture.context.network_id,
            fixture.context.id(),
            fixture.context.epoch,
            &identity_payload.origin_proposal,
            &identity_payload.producer,
        )
        .expect("derive committed cleanup lifecycle reservation identity");
    queue
        .release_lane_reservation(&initial_reservation)
        .expect("release provisional committed cleanup reservation");
    let corrected = queue
        .reserve_transactions_for_lane(
            fixture.state.as_ref(),
            LaneQueueReservationScopeV1 {
                lane_id: initial_reservation.lane_id,
                dataspace_id: initial_reservation.dataspace_id,
                lane_incarnation: initial_reservation.lane_incarnation,
                proposal_height: initial_reservation.proposal_height,
                lane_block_height: initial_reservation.lane_block_height,
                lane_block_view: initial_reservation.lane_block_view,
                reservation_owner_hash,
                proposal_identity_hash,
            },
            NonZeroUsize::new(1).expect("non-zero committed cleanup reservation limit"),
        )
        .expect("reserve committed cleanup transaction under the exact lifecycle identity");
    assert_eq!(corrected.len(), 1);
    let reservation = *corrected[0].key();
    let (parent, entry) = merge_entry_with_reservation(&fixture.context, entrypoint, reservation);
    let payload = Kura::decode_autonomous_lane_merge_bundle(
        &entry
            .execution_batch
            .as_ref()
            .expect("committed cleanup execution batch")
            .lanes[0]
            .source_bundle,
        fixture.context.network_id,
        fixture.context.epoch,
    )
    .expect("decode committed cleanup autonomous source")
    .autonomous
    .executable_payload;
    assert_eq!(
        payload.origin_proposal, identity_payload.origin_proposal,
        "re-reservation must preserve the exact autonomous proposal identity"
    );
    let local_signer = fixture.validator_keys[0].clone();
    let local_peer = PeerId::new(local_signer.public_key().clone());
    fixture
        .kura
        .bind_local_peer_id(local_peer.clone())
        .expect("bind committed cleanup local peer");
    let generation = fixture
        .kura
        .claim_autonomous_lifecycle_process_generation(fixture.context.network_id, &local_peer)
        .expect("claim committed cleanup lifecycle generation");
    let runtime_lanes =
        RuntimeLaneConfig::from_catalog(&fixture.state.nexus_snapshot().lane_catalog);
    let descriptor = &payload.origin_proposal.descriptor;
    fixture
        .kura
        .install_lane_incarnation_marker_for_test(
            runtime_lanes
                .entry(descriptor.lane_id)
                .expect("committed cleanup runtime lane"),
            descriptor.lane_incarnation,
            0,
        )
        .expect("install committed cleanup lane marker");
    fixture
        .kura
        .persist_lane_executable_payload(&payload, payload.network_id, payload.epoch)
        .expect("persist committed cleanup executable payload");
    let lifecycle_group = install_live_lifecycle_cursor_for_apply_test(
        fixture.kura.as_ref(),
        &generation,
        &payload,
        fixture.context.id(),
        &local_peer,
        &local_signer,
    );
    assert_eq!(
        lifecycle_group,
        lane_queue_reservation_group_binding_from_ordered_keys([reservation].iter())
            .expect("bind committed cleanup reservation group"),
    );
    let carrier = body_with_exact_merge_execution_header(&entry);
    fixture
        .kura
        .store_block(Arc::new(parent.clone()))
        .expect("persist execution-carrier parent");
    fixture
        .kura
        .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
        .expect("persist exact execution carrier and merge log");
    fixture.persist_exact_v2_finality_chain(&[&parent, &carrier]);
    fixture
        .state
        .seed_applied_merge_entry_for_v2_settlement_test(&entry)
        .expect("seed exact post-commit merge state");
    let mut block_hashes = fixture.state.block_hashes.block();
    block_hashes.push_for_tests(parent.hash());
    block_hashes.push_for_tests(carrier.hash());
    block_hashes.commit_for_tests();
    fixture
        .state
        .update_latest_block_header_cache_for_tests(carrier.header().clone());
    fixture
        .service
        .publish_committed_block_merge_entry(&carrier)
        .expect("publish exact committed merge application evidence");
    fixture.state.record_committed_entrypoints_for_tests(
        [reservation.entrypoint_hash],
        NonZeroUsize::new(2).expect("committed carrier height"),
    );
    queue.reconfigure_nexus_with_state(
        &fixture.state.nexus_snapshot(),
        fixture.state.as_ref(),
        None,
    );
    let staged_merge_queue_reservation_hashes =
        certified_merge_queue_reservation_hashes(Some(&entry))
            .expect("project exact staged reservation membership");
    assert_eq!(
        staged_merge_queue_reservation_hashes,
        BTreeSet::from([reservation.entrypoint_hash]),
        "the generic committed-hash cleanup exclusion must bind the exact staged merge member"
    );
    assert!(
        queue.has_durable_plan_claim_for_test(reservation.entrypoint_hash),
        "the reserved transaction must retain its durable QueuePlan claim before cleanup"
    );
    assert_eq!(
        queue.remove_committed_hashes(
            std::iter::once(reservation.entrypoint_hash).filter(|transaction_hash| {
                !staged_merge_queue_reservation_hashes.contains(transaction_hash)
            },),
            None,
        ),
        0,
        "generic committed-hash cleanup must defer autonomous reservation ownership"
    );
    assert!(
        queue.has_durable_plan_claim_for_test(reservation.entrypoint_hash),
        "generic cleanup must not tombstone the autonomous QueuePlan claim"
    );
    assert_eq!(
        queue.live_lane_reservations(),
        vec![reservation],
        "the exact reservation must survive until certified merge finalization"
    );
    assert_eq!(
        finalize_committed_block_merge_reservations(
            fixture.state.as_ref(),
            &queue,
            fixture.kura.as_ref(),
            &carrier,
            fixture.context.network_id,
        )
        .expect("finalize committed merge reservation"),
        1
    );
    assert!(
        !queue.has_durable_plan_claim_for_test(reservation.entrypoint_hash),
        "certified finalization must consume the QueuePlan claim after reservation Commit"
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert_eq!(
        finalize_committed_block_merge_reservations(
            fixture.state.as_ref(),
            &queue,
            fixture.kura.as_ref(),
            &carrier,
            fixture.context.network_id,
        )
        .expect("repeat exact reservation finalization"),
        0,
        "the post-commit boundary must be idempotent"
    );
});
v2_apply_test!(
    committed_merge_group_preflights_all_state_members_before_queue_cleanup,
    {
        let fixture = ApplyFixture::new();
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Queue::from_config(QueueConfig::default(), events_sender);
        let journal_dir = tempfile::tempdir().expect("reservation journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install queue-plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install reservation journal");
        let owner = Hash::new(b"all-member preflight owner");
        let proposal = Hash::new(b"all-member preflight proposal");
        let first = fixture
            .body
            .external_transactions()
            .next()
            .expect("fixture transaction")
            .clone();
        let second = TransactionBuilder::new(
            fixture.context.network_id,
            fixture.service.genesis_account.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "later committed-group member".to_owned(),
        )])
        .sign(fixture.genesis_key.private_key());
        let members = [first, second]
            .into_iter()
            .map(|transaction| {
                let (key, entrypoint) = reserve_transaction_for_test_with_identity(
                    fixture.state.as_ref(),
                    &queue,
                    transaction,
                    owner,
                    proposal,
                );
                (entrypoint, key)
            })
            .collect::<Vec<_>>();
        let keys = members.iter().map(|(_, key)| *key).collect::<Vec<_>>();
        let (_parent, entry) = merge_entry_with_reservations(&fixture.context, members);
        fixture.state.record_committed_entrypoints_for_tests(
            [keys[0].entrypoint_hash],
            NonZeroUsize::new(1).expect("committed height"),
        );
        let reference = CertifiedMergeLedgerReference::new(&entry);
        let applications = authenticated_autonomous_carrier_application_projections(
            &reference,
            &entry,
            fixture.context.network_id,
        )
        .expect("authenticate complete merge group before State membership preflight");
        let error = finalize_certified_merge_reservations_for_test(
            fixture.state.as_ref(),
            &queue,
            &entry,
            applications,
        )
        .expect_err("a missing later State member must reject before Queue mutation");
        assert!(matches!(
            error,
            V2ReservationLifecycleError::UncommittedMergeEntrypoint { entrypoint_hash }
                if entrypoint_hash == keys[1].entrypoint_hash
        ));
        assert_eq!(
            queue.live_lane_reservations().len(),
            2,
            "all group owners must remain live after failed full-State preflight"
        );
        assert!(queue.lane_reservation_commit_barriers().is_empty());
        fixture.state.record_committed_entrypoints_for_tests(
            [keys[1].entrypoint_hash],
            NonZeroUsize::new(1).expect("committed height"),
        );
        let applications = authenticated_autonomous_carrier_application_projections(
            &reference,
            &entry,
            fixture.context.network_id,
        )
        .expect("remint exact authenticated cleanup authority for retry");
        assert_eq!(
            finalize_certified_merge_reservations_for_test(
                fixture.state.as_ref(),
                &queue,
                &entry,
                applications,
            )
            .expect("retry exact fully committed group"),
            2
        );
        assert!(queue.live_lane_reservations().is_empty());
    }
);
v2_apply_test!(
    committed_merge_two_groups_preflight_queue_before_any_cleanup,
    {
        let fixture = ApplyFixture::new_with_lane_lifecycle();
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Queue::from_config(QueueConfig::default(), events_sender);
        let journal_dir = tempfile::tempdir().expect("reservation journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install queue-plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install reservation journal");
        let first_transaction = fixture
            .body
            .external_transactions()
            .next()
            .expect("fixture transaction")
            .clone();
        let second_transaction = TransactionBuilder::new(
            fixture.context.network_id,
            fixture.service.genesis_account.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "later Queue-preflight conflict".to_owned(),
        )])
        .sign(fixture.genesis_key.private_key());
        let (first, first_entrypoint) = reserve_transaction_for_test_with_identity(
            fixture.state.as_ref(),
            &queue,
            first_transaction,
            Hash::new(b"first Queue-preflight owner"),
            Hash::new(b"first Queue-preflight proposal"),
        );
        let later_lane = install_recreatable_reservation_lane(&fixture);
        let (second, second_entrypoint) = reserve_transaction_for_lane_test_with_identity(
            fixture.state.as_ref(),
            &queue,
            second_transaction,
            later_lane.id,
            later_lane.dataspace_id,
            Hash::new(b"later Queue-preflight owner"),
            Hash::new(b"later Queue-preflight proposal"),
        );
        let (_parent, mut entry) =
            merge_entry_with_reservation(&fixture.context, first_entrypoint, first);
        let (_later_parent, mut later_entry) =
            merge_entry_with_reservation(&fixture.context, second_entrypoint, second);
        let mut later_batch = later_entry
            .execution_batch
            .take()
            .expect("later execution batch");
        let batch = entry
            .execution_batch
            .as_mut()
            .expect("first execution batch");
        assert_eq!(
            batch.application_block_header,
            later_batch.application_block_header
        );
        batch.lanes.append(&mut later_batch.lanes);
        batch.entrypoint_count = batch
            .lanes
            .iter()
            .map(|lane| lane.entrypoints.len())
            .sum::<usize>()
            .try_into()
            .expect("two-group entrypoint count fits u64");
        batch.entrypoint_merkle_root =
            crate::merge::merge_execution_entrypoint_merkle_root(&batch.lanes)
                .expect("two-group entrypoint root");
        batch.result_merkle_root = crate::merge::merge_execution_result_merkle_root(&batch.lanes)
            .expect("two-group result root");
        batch.execution_root = crate::merge::merge_execution_root(&batch.lanes);
        batch.batch_hash = crate::merge::merge_execution_batch_hash(batch);
        fixture.state.record_committed_entrypoints_for_tests(
            [first.entrypoint_hash, second.entrypoint_hash],
            NonZeroUsize::new(1).expect("committed height"),
        );
        assert!(queue.remove_routing_plan_for_test(second.entrypoint_hash));
        let reference = CertifiedMergeLedgerReference::new(&entry);
        let applications = authenticated_autonomous_carrier_application_projections(
            &reference,
            &entry,
            fixture.context.network_id,
        )
        .expect("authenticate both autonomous cleanup groups");
        assert_eq!(applications.len(), 2);
        let error = finalize_certified_merge_reservations_for_test(
            fixture.state.as_ref(),
            &queue,
            &entry,
            applications,
        )
        .expect_err("later Queue identity conflict must reject the whole carrier cleanup");
        assert!(matches!(
            error,
            V2ReservationLifecycleError::Queue(LaneQueueReservationError::Conflict { hash })
                if hash == second.entrypoint_hash
        ));
        let live = queue.live_lane_reservations();
        assert_eq!(live.len(), 2);
        assert!(live.contains(&first));
        assert!(live.contains(&second));
        assert!(queue.lane_reservation_commit_barriers().is_empty());
        assert!(
            queue.has_durable_plan_claim_for_test(first.entrypoint_hash),
            "later-group rejection must not tombstone the first QueuePlan"
        );
    }
);
v2_apply_test!(committed_merge_reservation_rejects_bare_norito, {
    let fixture = ApplyFixture::new();
    let transaction = fixture
        .body
        .external_transactions()
        .next()
        .expect("fixture transaction")
        .clone();
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = Queue::from_config(QueueConfig::default(), events_sender);
    let journal_dir = tempfile::tempdir().expect("reservation journal directory");
    queue
        .install_plan_journal(
            journal_dir.path().join("queue-plans.norito"),
            1024 * 1024,
            true,
        )
        .expect("install queue-plan journal");
    queue
        .install_lane_reservation_journal(
            journal_dir.path().join("lane-reservations.norito"),
            1024 * 1024,
        )
        .expect("install reservation journal");
    let (reservation, entrypoint) =
        reserve_transaction_for_test(fixture.state.as_ref(), &queue, transaction);
    let (_parent, mut entry) =
        merge_entry_with_reservation(&fixture.context, entrypoint, reservation);
    let autonomous_carrier = body_with_exact_merge_execution_header(&entry);
    assert_eq!(
        CheckedCarrierApplications::for_block(&autonomous_carrier)
            .consume_for_state_commit(autonomous_carrier.hash(), Some(&entry)),
        Err("checked ApplyCarrier batch identity or cardinality changed before State commit"),
        "an empty checked-transition collection must not authorize an autonomous carrier",
    );
    let encoded = &mut entry
        .execution_batch
        .as_mut()
        .expect("fixture execution batch")
        .lanes[0]
        .reservation_keys[0];
    let bare = reservation.encode();
    assert_ne!(
        *encoded, bare,
        "framed and bare Norito must remain distinct"
    );
    *encoded = bare;
    fixture.state.record_committed_entrypoints_for_tests(
        [reservation.entrypoint_hash],
        NonZeroUsize::new(1).expect("committed height"),
    );
    let reference = CertifiedMergeLedgerReference::new(&entry);
    let message = authenticated_autonomous_carrier_application_projections(
        &reference,
        &entry,
        fixture.context.network_id,
    )
    .expect_err("bare reservation metadata must fail authentication before Queue cleanup");
    assert!(
        message.contains("source bundle") || message.contains("framed Norito"),
        "diagnostic should identify the authenticated framing mismatch: {message}"
    );
    assert_eq!(
        queue.live_lane_reservations(),
        vec![reservation],
        "malformed committed evidence must not consume queue ownership"
    );
});
v2_apply_test!(
    checked_apply_carrier_authorization_binds_exact_state_entry,
    {
        let fixture = ApplyFixture::new();
        let transaction = fixture
            .body
            .external_transactions()
            .next()
            .expect("fixture transaction")
            .clone();
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Queue::from_config(QueueConfig::default(), events_sender);
        let journal_dir = tempfile::tempdir().expect("reservation journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install queue-plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install reservation journal");
        let (reservation, entrypoint) =
            reserve_transaction_for_test(fixture.state.as_ref(), &queue, transaction);
        let (parent, entry) =
            merge_entry_with_reservation(&fixture.context, entrypoint, reservation);
        let carrier = body_with_exact_merge_execution_header(&entry);
        let reference = CertifiedMergeLedgerReference::new(&entry);
        let descriptor = &entry
            .execution_batch
            .as_ref()
            .expect("fixture execution batch")
            .lanes[0]
            .proposal
            .descriptor;
        fixture
            .kura
            .install_lane_incarnation_marker_for_test(
                RuntimeLaneConfig::default().primary(),
                descriptor.lane_incarnation,
                0,
            )
            .expect("install exact post-carrier repair lane marker");
        fixture
            .kura
            .store_block(Arc::new(parent.clone()))
            .expect("persist post-carrier repair parent");
        fixture
            .kura
            .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
            .expect("persist exact post-carrier repair entry and carrier");
        fixture.persist_exact_v2_finality_chain(&[&parent, &carrier]);
        let lane_count = entry
            .execution_batch
            .as_ref()
            .expect("fixture execution batch")
            .lanes
            .len();
        let reservation_group =
            lane_queue_reservation_group_binding_from_ordered_keys([&reservation])
                .expect("fixture reservation group");
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(reservation_group);
        let checked_apply_carrier = || {
            let before = ProductionInFlightFirstReleaseStateProjection {
                validator_count: 4,
                producer: 0b0100,
                producer_selected_owner: 0b0100,
                replicated_carrier_owners: 0b1011,
                payload_binding_a: 0b1111,
                binding_a,
                queue: ProductionInFlightFirstReleaseQueueProjection {
                    plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
                    selected_count: 1,
                    reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
                },
                carrier: ProductionInFlightFirstReleaseCarrierProjection {
                    kura_active: 0b1111,
                    execution_input_durable: 0b1011,
                    ready_qc_durable: true,
                },
                session: ProductionInFlightFirstReleaseSessionProjection {
                    bodies: 0b1111,
                    ready_authorized: 0b1011,
                    crashed: 0,
                    producer_alive: true,
                },
                history: ProductionInFlightFirstReleaseHistoryProjection {
                    ever_queue_plan_v1: true,
                    ever_reservation_v1: true,
                    ever_execution_input_durable: 0b1011,
                    ever_ready_authorized: 0b1011,
                    ready_signed: 0b1011,
                    ever_ready_qc_durable: true,
                    ..ProductionInFlightFirstReleaseHistoryProjection::default()
                },
                decision: ProductionInFlightFirstReleaseDecisionProjection {
                    lane_commit_scope: binding_a,
                    release_scope: CanonicalIdentityProjection::zero(),
                    lane_commit_owner: 0b1000,
                    release_owner: 0,
                    wsv_committed: false,
                    application_count: 0,
                    applied_by: 0,
                },
                release: ProductionInFlightFirstReleaseReleaseProjection::default(),
            };
            let mut after = before;
            after.decision.wsv_committed = true;
            after.decision.application_count = 1;
            after.decision.applied_by = 0b1000;
            let projection = ProductionInFlightFirstReleaseTransitionProjection {
                action: IN_FLIGHT_FIRST_RELEASE_ACTION_APPLY_CARRIER,
                actor: 0b1000,
                target: 0,
                before,
                after,
            };
            let checked = check_production_in_flight_first_release_transition(projection)
                .expect("fixture ApplyCarrier transition");
            (checked, projection)
        };
        let mut exact = CheckedCarrierApplications::for_block(&carrier);
        exact
            .bind_execution_batch(&reference, lane_count)
            .expect("bind exact autonomous execution batch");
        let (checked, projection) = checked_apply_carrier();
        let witness = *checked
            .first_release_witness()
            .expect("production ApplyCarrier checker must attach its V1 witness");
        assert!(
            crate::sumeragi::v2_core::authenticate_production_in_flight_first_release_transition_witness_v1(
                projection,
                witness,
            ),
            "the attached witness must authenticate the exact action, parameters, states, and model source",
        );
        for tampered in [
            crate::sumeragi::v2_core::ProductionInFlightFirstReleaseTransitionWitnessV1 {
                action: IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER,
                ..witness
            },
            crate::sumeragi::v2_core::ProductionInFlightFirstReleaseTransitionWitnessV1 {
                before_state_digest: crate::sumeragi::v2_core::ProductionDigest256Projection {
                    word0: witness.before_state_digest.word0 ^ 1,
                    ..witness.before_state_digest
                },
                ..witness
            },
            crate::sumeragi::v2_core::ProductionInFlightFirstReleaseTransitionWitnessV1 {
                after_state_digest: crate::sumeragi::v2_core::ProductionDigest256Projection {
                    word0: witness.after_state_digest.word0 ^ 1,
                    ..witness.after_state_digest
                },
                ..witness
            },
            crate::sumeragi::v2_core::ProductionInFlightFirstReleaseTransitionWitnessV1 {
                source_identity: crate::sumeragi::v2_core::ProductionDigest256Projection {
                    word0: witness.source_identity.word0 ^ 1,
                    ..witness.source_identity
                },
                ..witness
            },
        ] {
            assert!(
                !crate::sumeragi::v2_core::authenticate_production_in_flight_first_release_transition_witness_v1(
                    projection,
                    tampered,
                ),
                "a tampered witness must fail independent authentication",
            );
        }
        let snapshot_stutter = ProductionInFlightFirstReleaseTransitionProjection {
            action: crate::sumeragi::v2_core::IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT,
            actor: 0,
            target: 0,
            before: projection.before,
            after: projection.before,
        };
        assert!(
            crate::sumeragi::v2_core::check_production_in_flight_first_release_replay_step_v1(
                snapshot_stutter,
                crate::sumeragi::v2_core::ProductionInFlightFirstReleaseReplayStepV1::ComposedNext,
            )
            .is_none(),
            "a named stutter must not pass as an ordinary composed Next step",
        );
        assert!(
            crate::sumeragi::v2_core::check_production_in_flight_first_release_replay_step_v1(
                snapshot_stutter,
                crate::sumeragi::v2_core::ProductionInFlightFirstReleaseReplayStepV1::RecoverReservationSnapshotStutter,
            )
            .is_some(),
            "the exact unchanged snapshot reconstruction must pass its explicit stutter class",
        );
        let changed_snapshot = ProductionInFlightFirstReleaseTransitionProjection {
            after: projection.after,
            ..snapshot_stutter
        };
        assert!(
            crate::sumeragi::v2_core::check_production_in_flight_first_release_replay_step_v1(
                changed_snapshot,
                crate::sumeragi::v2_core::ProductionInFlightFirstReleaseReplayStepV1::RecoverReservationSnapshotStutter,
            )
            .is_none(),
            "a state-changing step must not pass through a stutter classification",
        );
        exact
            .push(checked, projection)
            .expect("retain exact checked ApplyCarrier transition");
        let exact: Box<dyn StateBlockCommitAuthorization> = Box::new(exact);
        assert_eq!(
            exact.consume_for_state_commit(carrier.hash(), Some(&entry)),
            Ok(()),
            "the real trait object must consume its move-only token against the exact State entry"
        );
        let mut wrong_cardinality = CheckedCarrierApplications::for_block(&carrier);
        wrong_cardinality
            .bind_execution_batch(&reference, lane_count.saturating_add(1))
            .expect("bind deliberately mismatched expected cardinality");
        let (checked, projection) = checked_apply_carrier();
        wrong_cardinality
            .push(checked, projection)
            .expect("retain one checked transition for mismatch test");
        assert_eq!(
            wrong_cardinality.consume_for_state_commit(carrier.hash(), Some(&entry)),
            Err("checked ApplyCarrier batch identity or cardinality changed before State commit")
        );
        let mut wrong_entry = CheckedCarrierApplications::for_block(&carrier);
        wrong_entry
            .bind_execution_batch(&reference, lane_count)
            .expect("bind exact reference before State-entry mismatch");
        let (checked, projection) = checked_apply_carrier();
        wrong_entry
            .push(checked, projection)
            .expect("retain checked transition before State-entry mismatch");
        let mut changed_entry = entry.clone();
        changed_entry.epoch_id = changed_entry.epoch_id.saturating_add(1);
        assert_eq!(
            wrong_entry.consume_for_state_commit(carrier.hash(), Some(&changed_entry)),
            Err("checked ApplyCarrier batch identity or cardinality changed before State commit")
        );
        let mut wrong_block = CheckedCarrierApplications::for_block(&carrier);
        wrong_block
            .bind_execution_batch(&reference, lane_count)
            .expect("bind exact reference before carrier mismatch");
        let (checked, projection) = checked_apply_carrier();
        wrong_block
            .push(checked, projection)
            .expect("retain checked transition before carrier mismatch");
        let other_block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"unrelated checked ApplyCarrier State block",
        ));
        assert_eq!(
            wrong_block.consume_for_state_commit(other_block_hash, Some(&entry)),
            Err("checked ApplyCarrier block identity changed before State commit")
        );
        let mut changed_projection = CheckedCarrierApplications::for_block(&carrier);
        changed_projection
            .bind_execution_batch(&reference, lane_count)
            .expect("bind exact reference before projection mismatch");
        let (checked, projection) = checked_apply_carrier();
        changed_projection
            .push(checked, projection)
            .expect("retain checked transition before projection mismatch");
        changed_projection.applications[0]
            .projection
            .after
            .decision
            .applied_by = 0;
        assert_eq!(
            changed_projection.consume_for_state_commit(carrier.hash(), Some(&entry)),
            Err("checked ApplyCarrier projection changed before State commit")
        );
        let network_id = fixture.context.network_id;
        let fresh = authenticated_autonomous_carrier_application_projections(
            &reference, &entry, network_id,
        )
        .expect("derive fresh authenticated ApplyCarrier geometry");
        let reconstructed = authenticated_autonomous_carrier_application_projections(
            &CertifiedMergeLedgerReference::new(&entry),
            &entry,
            network_id,
        )
        .expect("reconstruct authenticated ApplyCarrier geometry from canonical evidence");
        assert_eq!(fresh, reconstructed);
        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].reservation_group, reservation_group);
        assert_eq!(fresh[0].projection.before.validator_count, 4);
        assert_ne!(fresh[0].projection.before.replicated_carrier_owners, 0);
        assert_ne!(fresh[0].projection.before.payload_binding_a, 1);
        let carrier_height = carrier.header().height().get();
        let carrier_hash = carrier.hash();
        let mut repair_authorizations = post_carrier_evidence_repair_authorizations(
            &reference,
            &entry,
            network_id,
            carrier_height,
            carrier_hash,
        )
        .expect("mint exact post-carrier repair authorization");
        assert_eq!(repair_authorizations.len(), 1);
        let repair_projection = repair_authorizations
            .pop()
            .expect("one repair authorization")
            .consume_for_kura(
                reference.entry_hash,
                carrier_height,
                carrier_hash,
                reservation_group,
            )
            .expect("consume repair authorization against exact committed carrier");
        assert_eq!(
            repair_projection.action,
            IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER
        );
        assert_eq!(repair_projection.before, repair_projection.after);
        assert!(repair_projection.before.decision.wsv_committed);
        let wrong_carrier_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"unrelated post-carrier evidence repair block",
        ));
        assert!(
            fixture
                .kura
                .persist_merge_lane_block_application_receipts_from_committed_log_with_authorizations(
                    &entry,
                    post_carrier_evidence_repair_authorizations(
                        &reference,
                        &entry,
                        network_id,
                        carrier_height,
                        wrong_carrier_hash,
                    )
                    .expect("mint wrong-carrier repair authorization"),
                )
                .is_err(),
            "the real Kura sink must reject repair authority for another carrier"
        );
        assert!(
            fixture
                .kura
                .read_lane_block_application_receipt(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                )
                .is_none(),
            "carrier mismatch must fail before receipt publication"
        );
        fixture
            .kura
            .persist_merge_lane_block_application_receipts_from_committed_log_with_authorizations(
                &entry,
                post_carrier_evidence_repair_authorizations(
                    &reference,
                    &entry,
                    network_id,
                    carrier_height,
                    carrier_hash,
                )
                .expect("mint exact sink repair authorization"),
            )
            .expect("the real Kura sink consumes exact repair authority");
        assert!(
            fixture
                .kura
                .read_lane_block_application_receipt(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                )
                .is_some(),
            "exact repair authority must reach receipt publication"
        );
        let exact_projection = fresh[0].projection;
        let authorization = fresh[0]
            .queue_cleanup_authorization()
            .expect("mint authenticated four-validator Queue cleanup authorization");
        assert_eq!(
            authorization.accepted_projection_for_test(),
            exact_projection
        );
        let mut wrong_group = reservation_group;
        wrong_group.reservation_group_hash = Hash::new(b"wrong cleanup reservation group");
        let wrong_authorization =
            AutonomousLaneQueueCarrierCleanupAuthorization::from_projection_for_test(
                wrong_group,
                exact_projection,
            )
            .expect("mint deliberately wrong-group test authorization");
        assert!(
            queue
                .commit_lane_reservation_groups_with_authorization(vec![(
                    vec![reservation],
                    wrong_authorization,
                )])
                .is_err()
        );
        assert_eq!(queue.live_lane_reservations(), vec![reservation]);
        let mut tampered_projection = exact_projection;
        tampered_projection.after.producer = 0b0010;
        assert!(
            AutonomousLaneQueueCarrierCleanupAuthorization::from_projection_for_test(
                reservation_group,
                tampered_projection,
            )
            .is_err(),
            "geometry drift must fail before Queue mutation"
        );
        assert_eq!(queue.live_lane_reservations(), vec![reservation]);
        assert_eq!(
            queue
                .commit_lane_reservation_groups_with_authorization(vec![(
                    vec![reservation],
                    authorization,
                )])
                .expect("consume exact four-validator cleanup authorization"),
            1
        );
        assert!(queue.live_lane_reservations().is_empty());
    }
);
// Establish actual signed local custody before canonical Queue recovery. The
// reservation identities must already be derived from this exact native payload.
fn install_replayed_carrier_lifecycle_for_test(
    fixture: &ApplyFixture,
    entry: &MergeLedgerEntry,
) -> LaneQueueReservationGroupBindingV1 {
    let batch = entry
        .execution_batch
        .as_ref()
        .expect("replayed carrier execution batch");
    assert_eq!(
        batch.lanes.len(),
        1,
        "this fixture contains one exact carrier group"
    );
    let execution = &batch.lanes[0];
    let payload = Kura::decode_autonomous_lane_merge_bundle(
        &execution.source_bundle,
        execution.autonomous_network_id,
        execution.autonomous_epoch,
    )
    .expect("decode actual replayed carrier source")
    .autonomous
    .executable_payload;
    let signer = &fixture.validator_keys[0];
    let local_peer = PeerId::new(signer.public_key().clone());
    fixture
        .kura
        .bind_local_peer_id(local_peer.clone())
        .expect("bind the actual committee lifecycle validator");
    let generation = fixture
        .kura
        .claim_autonomous_lifecycle_process_generation(payload.network_id, &local_peer)
        .expect("claim the exact replayed carrier lifecycle generation");
    fixture
        .kura
        .persist_lane_executable_payload(&payload, payload.network_id, payload.epoch)
        .expect("persist the actual owned replayed carrier payload");
    install_live_lifecycle_cursor_for_apply_test(
        fixture.kura.as_ref(),
        &generation,
        &payload,
        fixture.context.id(),
        &local_peer,
        signer,
    )
}

/// Restore a crashed fixture's durable Queue ownership behind the startup gate.
fn replay_fixture_queue_for_startup(state: &State, journal_dir: &tempfile::TempDir) -> Arc<Queue> {
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = fixture_queue(state, events_sender);
    queue
        .install_lane_reservation_journal(
            journal_dir.path().join("lane-reservations.norito"),
            1024 * 1024,
        )
        .expect("replay crashed fixture reservation journal");
    queue
        .install_plan_journal(
            journal_dir.path().join("queue-plans.norito"),
            1024 * 1024,
            true,
        )
        .expect("reopen crashed fixture QueuePlan journal");
    queue
        .replay_plan_journal(state)
        .expect("restore exact crashed fixture QueuePlan payloads");
    assert!(queue.lane_reservation_startup_reconciliation_pending());
    queue
}

v2_apply_test!(
    startup_reconciliation_completes_before_retirement_and_replays_terminal_journal,
    {
        let fixture =
            ApplyFixture::new_for_production_recovered_decision_apply_with_lane_lifecycle();
        let reservation_lane = install_recreatable_reservation_lane(&fixture);
        let transaction = fixture
            .body
            .external_transactions()
            .next()
            .expect("fixture transaction")
            .clone();
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let journal_dir = tempfile::tempdir().expect("reservation journal directory");
        let journal_path = journal_dir.path().join("lane-reservations.norito");
        let first_queue = fixture_queue(fixture.state.as_ref(), events_sender.clone());
        first_queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install first-process queue-plan journal");
        first_queue
            .install_lane_reservation_journal(&journal_path, 1024 * 1024)
            .expect("install first-process reservation journal");
        let (initial_reservation, entrypoint) = reserve_transaction_for_lane_test_with_identity(
            fixture.state.as_ref(),
            &first_queue,
            transaction,
            reservation_lane.id,
            reservation_lane.dataspace_id,
            Hash::new(b"stale committed reservation owner"),
            Hash::new(b"stale committed reservation proposal"),
        );
        let reservation = bind_exact_fixture_reservation_identity(
            &fixture,
            &first_queue,
            initial_reservation,
            &entrypoint,
        );
        // Reserve an independent uncommitted owner under the same original
        // incarnation before State advances or the lane is recreated. Its
        // durable journals model a separate process that crashes here.
        let stale_transaction = TransactionBuilder::new(
            fixture.context.network_id,
            fixture.service.genesis_account.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "uncommitted stale-incarnation reservation".to_owned(),
        )])
        .sign(fixture.genesis_key.private_key());
        let (stale_events, _stale_receiver) = tokio::sync::broadcast::channel(8);
        let stale_journal_dir = tempfile::tempdir().expect("stale reservation journal directory");
        let stale_plan_path = stale_journal_dir.path().join("queue-plans.norito");
        let stale_reservation_path = stale_journal_dir.path().join("lane-reservations.norito");
        let stale_first_queue = fixture_queue(fixture.state.as_ref(), stale_events.clone());
        stale_first_queue
            .install_plan_journal(&stale_plan_path, 1024 * 1024, true)
            .expect("install stale-owner QueuePlan journal");
        stale_first_queue
            .install_lane_reservation_journal(&stale_reservation_path, 1024 * 1024)
            .expect("install stale-owner reservation journal");
        let (stale_reservation, _) = reserve_transaction_for_lane_test_with_identity(
            fixture.state.as_ref(),
            &stale_first_queue,
            stale_transaction,
            reservation_lane.id,
            reservation_lane.dataspace_id,
            Hash::new(b"uncommitted stale reservation owner"),
            Hash::new(b"uncommitted stale reservation proposal"),
        );
        assert_eq!(reservation.proposal_height, 1);
        assert_eq!(stale_reservation.proposal_height, 1);
        assert_eq!(
            stale_reservation.lane_incarnation, reservation.lane_incarnation,
            "both crash owners must belong to original incarnation A"
        );
        drop(stale_first_queue);
        drop(first_queue);
        let first_queue = replay_fixture_queue_for_startup(fixture.state.as_ref(), &journal_dir);
        let (parent, entry) =
            merge_entry_with_reservation(&fixture.context, entrypoint, reservation);
        let group = install_replayed_carrier_lifecycle_for_test(&fixture, &entry);
        let carrier = body_with_exact_merge_execution_header(&entry);
        fixture
            .kura
            .store_block(Arc::new(parent.clone()))
            .expect("persist execution-carrier parent");
        fixture
            .kura
            .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
            .expect("persist committed merge carrier and exact sidecar");
        fixture.persist_exact_v2_finality_chain(&[&parent, &carrier]);
        fixture
            .state
            .block(parent.header())
            .commit_world_overlay_for_testing()
            .expect("finalize the fixture genesis world before corrupting State history");
        fixture.state.record_committed_entrypoints_for_tests(
            [reservation.entrypoint_hash],
            NonZeroUsize::new(2).expect("exact merge-carrier transaction height"),
        );
        assert_eq!(fixture.state.committed_block_hash_at_height(2), None);
        assert_eq!(
            fixture
                .kura
                .get_durable_block_hash(NonZeroUsize::new(2).expect("carrier height")),
            Some(carrier.hash())
        );
        let missing_history_snapshot = first_queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture ownership before missing-State-history preflight");
        let error = reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            &first_queue,
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &fixture.context),
        )
        .expect_err("a durable Kura carrier absent from committed State history must fail");
        assert!(
            matches!(
                error,
                V2ReservationLifecycleError::CommittedCarrierMismatch {
                    lane_id,
                    proposal_height: 1,
                } if lane_id == reservation_lane.id
            ),
            "cross-store carrier mismatch must preserve Queue ownership: {error:?}",
        );
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(
            first_queue
                .lane_reservation_reconciliation_snapshot()
                .expect("recapture ownership after missing-State-history preflight"),
            missing_history_snapshot,
            "missing canonical State history must not consume Queue ownership"
        );
        // The negative fixture already installed transaction membership at height two.
        // Restore only the deliberately omitted canonical hash history; committing
        // genesis again would violate the existing transaction frontier.
        let mut block_hashes = fixture.state.block_hashes.block();
        block_hashes.push_for_tests(parent.hash());
        block_hashes.push_for_tests(carrier.hash());
        block_hashes.commit_for_tests();
        assert_eq!(fixture.state.committed_height(), 2);
        assert_eq!(fixture.state.latest_block_hash_fast(), Some(carrier.hash()));
        fixture
            .kura
            .persist_merge_lane_block_application_receipts(&entry, 2, carrier.hash())
            .expect("persist the exact applied carrier receipt before lane recreation");
        fixture
            .state
            .record_committed_queue_plan_entrypoints_for_tests(
                [reservation.entrypoint_hash],
                NonZeroUsize::new(2).expect("exact original committed carrier height"),
            )
            .expect(
                "resolve the original committed QueuePlan obligation before recreating its lane",
            );
        let publication = fixture
            .kura
            .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&entry)
            .expect("persist the complete canonical source-outcome set before Queue mutation")
            .expect("one execution carrier has a canonical source-outcome set");
        drop(publication);
        let pending_inventory = fixture
            .kura
            .pending_autonomous_lifecycle_terminal_outcome_inventory()
            .expect("inventory the complete Pending carrier");
        assert_eq!(pending_inventory.len(), 1);
        let expected_groups = pending_inventory[0]
            .pending_reservation_groups()
            .expect("retain the exact Pending group identity")
            .to_vec();
        assert_eq!(expected_groups.len(), 1);
        assert_eq!(expected_groups[0].binding(), group);
        let snapshot = first_queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture pending retirement Queue ownership");
        {
            let observer = first_queue.lock_lane_retirement_observer();
            V2ApplyService::validate_autoscale_retirement_queue_binding(
                &observer,
                reservation.lane_id,
                reservation.dataspace_id,
                reservation.lane_incarnation,
            )
            .expect_err("production retirement must reject the unresolved canonical Queue owner");
        }
        assert_eq!(
            first_queue
                .lane_reservation_reconciliation_snapshot()
                .unwrap(),
            snapshot
        );
        let retirement_error = fixture
            .state
            .apply_lane_lifecycle(&iroha_data_model::nexus::LaneLifecyclePlan {
                additions: vec![reservation_lane.clone()],
                retire: vec![reservation_lane.id],
            })
            .expect_err("actual storage rotation must reject Pending canonical lifecycle debt");
        assert!(
            retirement_error
                .to_string()
                .contains("Pending autonomous lifecycle terminal outcome")
                || retirement_error.to_string().contains("slot retirement"),
            "unexpected terminality refusal: {retirement_error}"
        );
        assert_eq!(
            fixture.state.lane_incarnation(reservation_lane.id),
            Some(reservation.lane_incarnation)
        );
        assert!(
            fixture
                .kura
                .read_lane_block_application_receipt_without_sidecar_repair(
                    reservation.lane_id,
                    reservation.lane_block_height,
                )
                .is_some(),
            "refused retirement must retain its active receipt"
        );
        fixture
            .state
            .transactions
            .overwrite_committed_entrypoint_membership_for_tests(
                reservation.entrypoint_hash,
                NonZeroUsize::new(1).expect("deliberately mismatched State membership height"),
            );
        assert_eq!(
            fixture
                .state
                .committed_entrypoint_height(&reservation.entrypoint_hash),
            NonZeroUsize::new(1)
        );
        drop(first_queue);
        let verified_active_context = verified_successor_context_after_fixture_tip(&fixture);
        let replayed_queue = Queue::from_config(QueueConfig::default(), events_sender);
        let replay = replayed_queue
            .install_lane_reservation_journal(&journal_path, 1024 * 1024)
            .expect("replay first-process reservation journal");
        replayed_queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install replayed queue-plan journal");
        replayed_queue
            .replay_plan_journal(fixture.state.as_ref())
            .expect("replay committed QueuePlan claim before retirement");
        assert_eq!(replay.restored, 1);
        assert_eq!(replayed_queue.live_lane_reservations(), vec![reservation]);
        assert!(
            replayed_queue.lane_reservation_startup_reconciliation_pending(),
            "replayed committed ownership remains quarantined until State/Kura preflight"
        );
        fixture.kura.reset_merge_query_read_counters_for_test();
        let mismatched_snapshot = replayed_queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture cross-store height mismatch ownership");
        let error = reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            &replayed_queue,
            fixture.kura.as_ref(),
            &verified_active_context,
        )
        .expect_err("State membership at another height must not consume ownership");
        assert!(
            matches!(
                error,
                V2ReservationLifecycleError::CommittedCarrierMismatch {
                    lane_id,
                    proposal_height: 1,
                } if lane_id == reservation_lane.id
            ),
            "cross-store carrier mismatch must preserve Queue ownership: {error:?}",
        );
        assert_eq!(
            replayed_queue
                .lane_reservation_reconciliation_snapshot()
                .expect("recapture cross-store height mismatch ownership"),
            mismatched_snapshot
        );
        assert!(replayed_queue.lane_reservation_startup_reconciliation_pending());
        fixture
            .state
            .transactions
            .overwrite_committed_entrypoint_membership_for_tests(
                reservation.entrypoint_hash,
                NonZeroUsize::new(2).expect("exact merge-carrier State height"),
            );
        assert_eq!(
            fixture
                .state
                .committed_entrypoint_height(&reservation.entrypoint_hash),
            NonZeroUsize::new(2)
        );
        assert_eq!(
            reconcile_lane_reservation_ownership(
                fixture.state.as_ref(),
                &replayed_queue,
                fixture.kura.as_ref(),
                &verified_active_context,
            )
            .expect("reconcile replayed committed reservation"),
            LaneReservationReconciliationSummary {
                recovered: 1,
                finalized_committed: 1,
                ..LaneReservationReconciliationSummary::default()
            }
        );
        assert!(
            !replayed_queue.lane_reservation_startup_reconciliation_pending(),
            "successful committed-owner reconciliation must publish the Queue startup gate"
        );
        let (full_history_scans, complete_execution_scans, indexed_lookups) =
            fixture.kura.merge_query_read_counters_for_test();
        assert_eq!(
            full_history_scans, 0,
            "startup reservation reconciliation must not materialize merge history"
        );
        assert_eq!(
            complete_execution_scans, 0,
            "receipt-bound recovery must not scan the complete execution history"
        );
        assert!(
            indexed_lookups > 0,
            "reconciliation must consult its indexed carrier"
        );
        assert_eq!(
            fixture.kura.merge_query_indexed_hashes_for_test(),
            BTreeSet::from([entry.canonical_hash()]),
            "repeated Pending/Queue/Complete checks may revisit only the exact carrier identity"
        );
        assert!(replayed_queue.live_lane_reservations().is_empty());
        let completed_queue_snapshot = replayed_queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture completed owner state before read-only retry");
        let completed_reservation_bytes =
            std::fs::read(&journal_path).expect("read completed reservation journal before retry");
        let completed_plan_bytes = std::fs::read(journal_dir.path().join("queue-plans.norito"))
            .expect("read completed QueuePlan journal before retry");
        assert!(
            fixture
                .kura
                .pending_autonomous_lifecycle_terminal_outcome_inventory()
                .expect("verify no Pending outcome before read-only retry")
                .is_empty()
        );
        assert!(matches!(
            plan_lane_reservation_ownership(
                fixture.state.as_ref(),
                &replayed_queue,
                fixture.kura.as_ref(),
                &verified_active_context,
                None,
            )
            .expect("observe completed startup without minting a mutation plan"),
            LaneReservationReconciliationPlanning::AlreadyCompleted(_)
        ));
        assert_eq!(
            reconcile_lane_reservation_ownership(
                fixture.state.as_ref(),
                &replayed_queue,
                fixture.kura.as_ref(),
                &verified_active_context,
            )
            .expect("repeat startup reconciliation"),
            LaneReservationReconciliationSummary::default()
        );
        assert_eq!(
            std::fs::read(&journal_path).expect("reread reservation journal after observation"),
            completed_reservation_bytes,
            "completed observation must not append, compact or rewrite reservation authority"
        );
        assert_eq!(
            std::fs::read(journal_dir.path().join("queue-plans.norito"))
                .expect("reread QueuePlan journal after observation"),
            completed_plan_bytes,
            "completed observation must not append, compact or rewrite QueuePlan authority"
        );
        assert_eq!(
            replayed_queue
                .lane_reservation_reconciliation_snapshot()
                .unwrap(),
            completed_queue_snapshot,
            "completed observation preserves the exact empty Queue snapshot"
        );
        let stages = fixture
            .kura
            .pending_autonomous_lifecycle_terminal_outcome_inventory()
            .expect("inventory completed canonical source outcomes");
        assert!(
            stages.is_empty(),
            "Queue cleanup must durably complete the Kura source outcome"
        );
        let completed_stages = fixture
            .kura
            .verify_expected_autonomous_lifecycle_terminal_outcome_stages(
                fixture.context.network_id,
                &expected_groups,
            )
            .expect("independently verify the actual native Complete outcome");
        assert_eq!(completed_stages.len(), 1);
        assert_eq!(completed_stages[0].binding(), group);
        assert_eq!(
            completed_stages[0].stage(),
            crate::kura::AutonomousLifecycleTerminalOutcomeDurableStage::Complete
        );
        {
            let observer = replayed_queue.lock_lane_retirement_observer();
            V2ApplyService::validate_autoscale_retirement_queue_binding(
                &observer,
                reservation.lane_id,
                reservation.dataspace_id,
                reservation.lane_incarnation,
            )
            .expect("genuine Queue terminality permits production retirement");
        }
        let (old_incarnation, new_incarnation) =
            replace_recreatable_reservation_lane(fixture.state.as_ref(), &reservation_lane);
        assert_eq!(reservation.lane_incarnation, old_incarnation);
        assert_ne!(reservation.lane_incarnation, new_incarnation);
        assert_ne!(
            fixture
                .state
                .lane_incarnation_at_height(reservation.lane_id, reservation.proposal_height,),
            Some(reservation.lane_incarnation),
            "completed canonical ownership must remain terminal after same-ID recreation"
        );
        assert_eq!(stale_reservation.lane_incarnation, old_incarnation);
        assert_ne!(stale_reservation.lane_incarnation, new_incarnation);
        assert_eq!(
            fixture
                .state
                .lane_incarnation_at_height(reservation_lane.id, 3),
            Some(new_incarnation),
            "replacement incarnation B must activate at the next proposal height"
        );
        drop(replayed_queue);
        let terminal_queue = Queue::from_config(
            QueueConfig::default(),
            fixture.service.events_sender.clone(),
        );
        let terminal_replay = terminal_queue
            .install_lane_reservation_journal(&journal_path, 1024 * 1024)
            .expect("replay actual completed reservation journal after rotation");
        assert_eq!(terminal_replay.restored, 0);
        assert_eq!(terminal_replay.commit_barriers, 0);
        terminal_queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install actual completed QueuePlan journal after rotation");
        terminal_queue
            .replay_plan_journal(fixture.state.as_ref())
            .expect("completed QueuePlan journal survives rotation");
        let verified_active_context = verified_successor_context_after_fixture_tip(&fixture);
        assert_eq!(
            reconcile_lane_reservation_ownership(
                fixture.state.as_ref(),
                &terminal_queue,
                fixture.kura.as_ref(),
                &verified_active_context,
            )
            .expect("terminal replay needs no retired Pending reconstruction"),
            LaneReservationReconciliationSummary::default()
        );
        assert!(!terminal_queue.lane_reservation_startup_reconciliation_pending());
        assert!(
            fixture
                .kura
                .read_lane_block_application_receipt_without_sidecar_repair(
                    reservation.lane_id,
                    reservation.lane_block_height,
                )
                .is_none(),
            "the completed old receipt must stay out of the active incarnation"
        );
        // A distinct uncommitted old-incarnation journal is invalid; it must
        // remain quarantined rather than acquire canonical cleanup authority.
        let stale_replayed_queue = Queue::from_config(QueueConfig::default(), stale_events);
        stale_replayed_queue
            .install_lane_reservation_journal(&stale_reservation_path, 1024 * 1024)
            .expect("replay uncommitted stale owner");
        // A reservation-only replay has no authenticated QueuePlan claim yet.
        // Observe exact quarantined keys and durable bytes without minting the
        // full reconciliation snapshot that requires successful payload replay.
        let stale_owners = stale_replayed_queue.live_lane_reservations();
        assert_eq!(stale_owners, vec![stale_reservation]);
        let stale_reservation_bytes = std::fs::read(&stale_reservation_path)
            .expect("retain the exact stale reservation journal");
        let stale_plan_bytes =
            std::fs::read(&stale_plan_path).expect("retain the exact stale QueuePlan journal");
        assert!(matches!(
            stale_replayed_queue.lane_reservation_reconciliation_snapshot(),
            Err(LaneQueueReservationError::ReconciliationMissingDurableClaim { hash })
                if hash == stale_reservation.entrypoint_hash
        ));
        stale_replayed_queue
            .install_plan_journal(&stale_plan_path, 1024 * 1024, true)
            .expect("install replayed stale-owner QueuePlan journal");
        let plan_error = stale_replayed_queue
            .replay_plan_journal(fixture.state.as_ref())
            .expect_err("uncommitted stale QueuePlan must fail during real startup replay");
        assert_eq!(
            plan_error.kind(),
            std::io::ErrorKind::InvalidData,
            "uncommitted stale QueuePlan replay must fail closed as invalid durable data: {plan_error}"
        );
        assert_eq!(
            stale_replayed_queue.live_lane_reservations(),
            stale_owners,
            "failed stale QueuePlan replay must not mutate reservation ownership"
        );
        let error = reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            &stale_replayed_queue,
            fixture.kura.as_ref(),
            &verified_active_context,
        )
        .expect_err("uncommitted stale-incarnation owner must remain fail-closed");
        assert!(matches!(
            error,
            V2ReservationLifecycleError::Queue(
                LaneQueueReservationError::ReconciliationMissingDurableClaim { hash }
            ) if hash == stale_reservation.entrypoint_hash
        ));
        assert_eq!(
            stale_replayed_queue.live_lane_reservations(),
            stale_owners,
            "stale-owner failure must not mutate Queue ownership"
        );
        assert_eq!(
            std::fs::read(&stale_reservation_path)
                .expect("reread rejected stale reservation journal"),
            stale_reservation_bytes,
            "rejected stale ownership must preserve its complete durable reservation evidence"
        );
        assert_eq!(
            std::fs::read(&stale_plan_path).expect("reread rejected stale QueuePlan journal"),
            stale_plan_bytes,
            "failed startup admission must not rewrite the retained QueuePlan evidence"
        );
        assert!(
            stale_replayed_queue.lane_reservation_startup_reconciliation_pending(),
            "uncommitted stale ownership must keep the startup gate closed"
        );
    }
);
v2_apply_test!(
    startup_reconciliation_validates_every_group_before_mutating_valid_prefix,
    {
        let fixture = ApplyFixture::new();
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Queue::from_config(QueueConfig::default(), events_sender);
        let journal_dir = tempfile::tempdir().expect("reservation journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install queue-plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install reservation journal");
        let first_transaction = fixture
            .body
            .external_transactions()
            .next()
            .expect("fixture transaction")
            .clone();
        let second_transaction = TransactionBuilder::new(
            fixture.context.network_id,
            fixture.service.genesis_account.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "malformed later startup group".to_owned(),
        )])
        .sign(fixture.genesis_key.private_key());
        let (first, first_entrypoint) = reserve_transaction_for_test_with_identity(
            fixture.state.as_ref(),
            &queue,
            first_transaction,
            Hash::new(b"startup valid prefix owner"),
            Hash::new(b"startup valid prefix proposal"),
        );
        let (second, _second_entrypoint) = reserve_transaction_for_test_with_identity(
            fixture.state.as_ref(),
            &queue,
            second_transaction,
            Hash::new(b"startup malformed suffix owner"),
            Hash::new(b"startup malformed suffix proposal"),
        );
        drop(queue);
        let queue = replay_fixture_queue_for_startup(fixture.state.as_ref(), &journal_dir);
        let (parent, first_entry) =
            merge_entry_with_reservation(&fixture.context, first_entrypoint, first);
        let first_carrier = body_with_exact_merge_execution_header(&first_entry);
        fixture
            .kura
            .store_block(Arc::new(parent.clone()))
            .expect("persist valid-prefix carrier parent");
        fixture
            .kura
            .store_block_with_merge_entry(Arc::new(first_carrier.clone()), &first_entry)
            .expect("persist exact valid-prefix merge binding");
        fixture.persist_exact_v2_finality_chain(&[&parent, &first_carrier]);
        commit_exact_fixture_carrier_chain_to_state(&fixture, &parent, &first_carrier);
        fixture.state.record_committed_entrypoints_for_tests(
            [first.entrypoint_hash, second.entrypoint_hash],
            NonZeroUsize::new(2).expect("exact merge-carrier State height"),
        );
        let before = queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture immutable preflight snapshot");
        let verified_active_context = verified_successor_context_after_fixture_tip(&fixture);
        let error = reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            &queue,
            fixture.kura.as_ref(),
            &verified_active_context,
        )
        .expect_err("missing later merge binding must fail before consuming the valid prefix");
        assert!(
            matches!(error, V2ReservationLifecycleError::MissingCommittedBinding { entrypoint_hash }
                if entrypoint_hash == second.entrypoint_hash),
            "the malformed suffix must fail its exact committed binding: {error:?}"
        );
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("capture post-error ownership snapshot"),
            before,
            "a malformed later group must leave every earlier valid owner untouched"
        );
        assert!(queue.lane_reservation_commit_barriers().is_empty());
    }
);
v2_apply_test!(committed_group_recovery_accepts_exact_commit_prefix, {
    let fixture = ApplyFixture::new_with_lane_lifecycle();
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = Queue::from_config(QueueConfig::default(), events_sender);
    let journal_dir = tempfile::tempdir().expect("committed suffix journal directory");
    queue
        .install_plan_journal(
            journal_dir.path().join("queue-plans.norito"),
            1024 * 1024,
            true,
        )
        .expect("install committed suffix QueuePlan journal");
    queue
        .install_lane_reservation_journal(
            journal_dir.path().join("lane-reservations.norito"),
            1024 * 1024,
        )
        .expect("install committed suffix reservation journal");
    let owner = Hash::new(b"committed suffix owner");
    let proposal = Hash::new(b"committed suffix proposal");
    let transactions = std::iter::once(
        fixture
            .body
            .external_transactions()
            .next()
            .expect("fixture transaction")
            .clone(),
    )
    .chain((1_u8..=2).map(|index| {
        TransactionBuilder::new(
            fixture.context.network_id,
            fixture.service.genesis_account.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            format!("committed suffix transaction {index}"),
        )])
        .sign(fixture.genesis_key.private_key())
    }))
    .collect::<Vec<_>>();
    let members = transactions
        .into_iter()
        .map(|transaction| {
            let (key, entrypoint) = reserve_transaction_for_test_with_identity(
                fixture.state.as_ref(),
                &queue,
                transaction,
                owner,
                proposal,
            );
            (entrypoint, key)
        })
        .collect::<Vec<_>>();
    let (_, identity_entry) = merge_entry_with_reservations(&fixture.context, members.clone());
    let identity_execution = &identity_entry
        .execution_batch
        .as_ref()
        .expect("identity batch")
        .lanes[0];
    let identity_payload = Kura::decode_autonomous_lane_merge_bundle(
        &identity_execution.source_bundle,
        identity_execution.autonomous_network_id,
        identity_execution.autonomous_epoch,
    )
    .expect("decode full ordered-group identity")
    .executable_payload()
    .clone();
    let (reservation_owner_hash, proposal_identity_hash) =
        super::super::lane_planner::autonomous_lane_reservation_identity_hashes_for_proposal(
            fixture.context.network_id,
            fixture.context.id(),
            fixture.context.epoch,
            &identity_payload.origin_proposal,
            &identity_payload.producer,
        )
        .expect("derive exact complete-group lifecycle identity");
    for (_, key) in &members {
        queue
            .release_lane_reservation(key)
            .expect("release provisional group identity");
    }
    let first = members[0].1;
    let reserved = queue
        .reserve_transactions_for_lane(
            fixture.state.as_ref(),
            LaneQueueReservationScopeV1 {
                lane_id: first.lane_id,
                dataspace_id: first.dataspace_id,
                lane_incarnation: first.lane_incarnation,
                proposal_height: first.proposal_height,
                lane_block_height: first.lane_block_height,
                lane_block_view: first.lane_block_view,
                reservation_owner_hash,
                proposal_identity_hash,
            },
            NonZeroUsize::new(members.len()).expect("nonempty exact group"),
        )
        .expect("reserve the complete group under its actual lifecycle identity");
    assert_eq!(reserved.len(), members.len());
    let members = members
        .into_iter()
        .zip(reserved)
        .map(|((entrypoint, initial), reserved)| {
            assert_eq!(
                reserved.key().entrypoint_hash,
                initial.entrypoint_hash,
                "re-reservation preserves FIFO"
            );
            (entrypoint, *reserved.key())
        })
        .collect::<Vec<_>>();
    let keys = members.iter().map(|(_, key)| *key).collect::<Vec<_>>();
    assert_eq!(
        queue
            .commit_lane_reservation_group_prefix_for_test(&keys, 1)
            .expect("persist the first reservation Commit-prefix barrier"),
        1
    );
    assert_eq!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture committed suffix")
            .ordered_groups[0]
            .ordered_keys,
        keys[1..].to_vec()
    );
    assert_eq!(queue.lane_reservation_commit_barriers(), vec![keys[0]]);
    let (parent, entry) = merge_entry_with_reservations(&fixture.context, members);
    let execution = &entry
        .execution_batch
        .as_ref()
        .expect("committed prefix execution")
        .lanes[0];
    let payload = Kura::decode_autonomous_lane_merge_bundle(
        &execution.source_bundle,
        execution.autonomous_network_id,
        execution.autonomous_epoch,
    )
    .expect("decode the actual committed prefix payload")
    .executable_payload()
    .clone();
    fixture
        .kura
        .install_lane_incarnation_marker_for_test(
            RuntimeLaneConfig::default().primary(),
            payload.origin_proposal.descriptor.lane_incarnation,
            0,
        )
        .expect("bind the committed prefix lifecycle namespace");
    fixture
        .kura
        .persist_lane_executable_payload(&payload, payload.network_id, payload.epoch)
        .expect("persist the committed prefix lifecycle payload");
    let producer = fixture
        .validator_keys
        .iter()
        .find(|key| key.public_key() == payload.producer.public_key())
        .expect("retain the real fixture producer key");
    install_autonomous_crash_live_cursor(&fixture, &payload, producer);
    let carrier = body_with_exact_merge_execution_header(&entry);
    fixture
        .kura
        .store_block(Arc::new(parent.clone()))
        .expect("persist committed suffix carrier parent");
    fixture
        .kura
        .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
        .expect("persist full committed suffix merge group");
    fixture.persist_exact_v2_finality_chain(&[&parent, &carrier]);
    commit_exact_fixture_carrier_chain_to_state(&fixture, &parent, &carrier);
    fixture
        .kura
        .persist_merge_lane_block_application_receipts(&entry, 2, carrier.hash())
        .expect("persist the exact applied carrier receipt before journal replay");
    fixture
        .state
        .record_committed_queue_plan_entrypoints_for_tests(
            keys.iter().map(|key| key.entrypoint_hash),
            NonZeroUsize::new(2).expect("exact committed suffix carrier State height"),
        )
        .expect("resolve exact committed suffix QueuePlan obligations before restart");
    // Startup reconciliation consumes checked replay receipts from both journals.
    // Reopen the actual durable Commit-prefix/live-suffix history as a new Queue.
    drop(queue);
    let (events_sender, _replay_events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = Queue::from_config(QueueConfig::default(), events_sender);
    let replay = queue
        .install_lane_reservation_journal(
            journal_dir.path().join("lane-reservations.norito"),
            1024 * 1024,
        )
        .expect("replay committed suffix reservation journal");
    assert_eq!((replay.restored, replay.commit_barriers), (2, 1));
    queue
        .install_plan_journal(
            journal_dir.path().join("queue-plans.norito"),
            1024 * 1024,
            true,
        )
        .expect("reopen committed suffix QueuePlan journal");
    queue
        .replay_plan_journal(fixture.state.as_ref())
        .expect("replay the exact committed suffix QueuePlan claims");
    assert!(queue.lane_reservation_startup_reconciliation_pending());
    let verified_active_context = verified_successor_context_after_fixture_tip(&fixture);
    assert_eq!(
        reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            &queue,
            fixture.kura.as_ref(),
            &verified_active_context,
        )
        .expect("reconcile exact committed suffix"),
        LaneReservationReconciliationSummary {
            recovered: 3,
            finalized_committed: 2,
            ..LaneReservationReconciliationSummary::default()
        }
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert!(queue.lane_reservation_commit_barriers().is_empty());
});
v2_apply_test!(
    mixed_commit_barrier_group_preflights_malformed_later_group,
    {
        let fixture = ApplyFixture::new();
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Queue::from_config(QueueConfig::default(), events_sender);
        let journal_dir = tempfile::tempdir().expect("mixed commit barrier journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install mixed commit barrier QueuePlan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install mixed commit barrier reservation journal");
        let first_owner = Hash::new(b"mixed commit barrier first owner");
        let first_proposal = Hash::new(b"mixed commit barrier first proposal");
        let first_transactions = std::iter::once(
            fixture
                .body
                .external_transactions()
                .next()
                .expect("fixture transaction")
                .clone(),
        )
        .chain((1_u8..=2).map(|index| {
            TransactionBuilder::new(
                fixture.context.network_id,
                fixture.service.genesis_account.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(
                Level::INFO,
                format!("mixed commit barrier member {index}"),
            )])
            .sign(fixture.genesis_key.private_key())
        }))
        .map(|transaction| {
            let (key, entrypoint) = reserve_transaction_for_test_with_identity(
                fixture.state.as_ref(),
                &queue,
                transaction,
                first_owner,
                first_proposal,
            );
            (entrypoint, key)
        })
        .collect::<Vec<_>>();
        let first_keys = first_transactions
            .iter()
            .map(|(_, key)| *key)
            .collect::<Vec<_>>();
        let later_transaction = TransactionBuilder::new(
            fixture.context.network_id,
            fixture.service.genesis_account.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "mixed commit barrier malformed later group".to_owned(),
        )])
        .sign(fixture.genesis_key.private_key());
        let (later_key, _) = reserve_transaction_for_test_with_identity(
            fixture.state.as_ref(),
            &queue,
            later_transaction,
            Hash::new(b"mixed commit barrier later owner"),
            Hash::new(b"mixed commit barrier later proposal"),
        );
        assert_eq!(
            queue
                .commit_lane_reservation_group_prefix_for_test(&first_keys, 1)
                .expect("stop first member at durable Commit barrier"),
            1
        );
        assert_eq!(
            queue.lane_reservation_commit_barriers(),
            vec![first_keys[0]]
        );
        drop(queue);
        let queue = replay_fixture_queue_for_startup(fixture.state.as_ref(), &journal_dir);
        let (parent, entry) = merge_entry_with_reservations(&fixture.context, first_transactions);
        let carrier = body_with_exact_merge_execution_header(&entry);
        fixture
            .kura
            .store_block(Arc::new(parent.clone()))
            .expect("persist mixed commit barrier carrier parent");
        fixture
            .kura
            .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
            .expect("persist exact first committed group");
        fixture.persist_exact_v2_finality_chain(&[&parent, &carrier]);
        commit_exact_fixture_carrier_chain_to_state(&fixture, &parent, &carrier);
        fixture.state.record_committed_entrypoints_for_tests(
            first_keys
                .iter()
                .map(|key| key.entrypoint_hash)
                .chain([later_key.entrypoint_hash]),
            NonZeroUsize::new(2).expect("exact mixed carrier State height"),
        );
        let before = queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture mixed owner preflight snapshot");
        let barriers_before = queue.lane_reservation_commit_barriers();
        let verified_active_context = verified_successor_context_after_fixture_tip(&fixture);
        let error = reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            &queue,
            fixture.kura.as_ref(),
            &verified_active_context,
        )
        .expect_err("malformed later group must stop before consuming mixed first group");
        assert!(
            matches!(error, V2ReservationLifecycleError::MissingCommittedBinding { entrypoint_hash }
                if entrypoint_hash == later_key.entrypoint_hash),
            "the malformed later group must fail its exact committed binding: {error:?}"
        );
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("capture mixed owner post-error snapshot"),
            before
        );
        assert_eq!(queue.lane_reservation_commit_barriers(), barriers_before);
    }
);
/// Durable interruption points in one exact two-member Queue cleanup.
enum ReplayedQueueCleanupCrashCut {
    /// Only the first reservation Commit frame has reached the journal.
    MixedCommitLive,
    /// Every Commit frame and the first plan tombstone are durable.
    AfterPlanTombstone,
}

/// Recover each journal cut before allowing its lane incarnation to retire.
fn assert_replayed_queue_cleanup_completes_before_same_id_retirement(
    crash_cut: ReplayedQueueCleanupCrashCut,
) {
    let fixture = ApplyFixture::new_for_production_recovered_decision_apply_with_lane_lifecycle();
    let reservation_lane = install_recreatable_reservation_lane(&fixture);
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = fixture_queue(fixture.state.as_ref(), events_sender.clone());
    let journal_dir = tempfile::tempdir().expect("replayed commit barrier journal directory");
    let plan_path = journal_dir.path().join("queue-plans.norito");
    let reservation_path = journal_dir.path().join("lane-reservations.norito");
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install replayed commit barrier QueuePlan journal");
    queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("install replayed commit barrier reservation journal");
    let owner = Hash::new(b"replayed commit barrier owner");
    let proposal = Hash::new(b"replayed commit barrier proposal");
    let transactions = std::iter::once(
        fixture
            .body
            .external_transactions()
            .next()
            .expect("fixture transaction")
            .clone(),
    )
    .chain(std::iter::once(
        TransactionBuilder::new(
            fixture.context.network_id,
            fixture.service.genesis_account.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "replayed commit barrier live suffix".to_owned(),
        )])
        .sign(fixture.genesis_key.private_key()),
    ))
    .map(|transaction| {
        let (key, entrypoint) = reserve_transaction_for_lane_test_with_identity(
            fixture.state.as_ref(),
            &queue,
            transaction,
            reservation_lane.id,
            reservation_lane.dataspace_id,
            owner,
            proposal,
        );
        (entrypoint, key)
    })
    .collect::<Vec<_>>();
    // The two-member proposal determines one actual producer-owned group.
    // Re-reserve both members under that native identity before publishing custody.
    let (_, provisional) = merge_entry_with_reservations(&fixture.context, transactions.clone());
    let provisional_payload = Kura::decode_autonomous_lane_merge_bundle(
        &provisional.execution_batch.as_ref().unwrap().lanes[0].source_bundle,
        fixture.context.network_id,
        fixture.context.epoch,
    )
    .expect("decode provisional two-member source")
    .autonomous
    .executable_payload;
    let (reservation_owner_hash, proposal_identity_hash) =
        super::super::lane_planner::autonomous_lane_reservation_identity_hashes_for_proposal(
            fixture.context.network_id,
            fixture.context.id(),
            fixture.context.epoch,
            &provisional_payload.origin_proposal,
            &provisional_payload.producer,
        )
        .expect("derive exact two-member lifecycle identity");
    for (_, key) in &transactions {
        queue
            .release_lane_reservation(key)
            .expect("release provisional setup identity");
    }
    let initial = transactions[0].1;
    let corrected = queue
        .reserve_transactions_for_lane(
            fixture.state.as_ref(),
            LaneQueueReservationScopeV1 {
                lane_id: initial.lane_id,
                dataspace_id: initial.dataspace_id,
                lane_incarnation: initial.lane_incarnation,
                proposal_height: initial.proposal_height,
                lane_block_height: initial.lane_block_height,
                lane_block_view: initial.lane_block_view,
                reservation_owner_hash,
                proposal_identity_hash,
            },
            NonZeroUsize::new(2).expect("exact two-member group"),
        )
        .expect("reserve actual two-member producer group");
    assert_eq!(corrected.len(), 2);
    let keys = corrected
        .iter()
        .map(|reserved| *reserved.key())
        .collect::<Vec<_>>();
    let transactions = transactions
        .into_iter()
        .zip(&keys)
        .map(|((entrypoint, old), key)| {
            assert_eq!(
                key.entrypoint_hash, old.entrypoint_hash,
                "exact FIFO membership is preserved"
            );
            (entrypoint, *key)
        })
        .collect();
    let (parent, entry) = merge_entry_with_reservations(&fixture.context, transactions);
    let group = install_replayed_carrier_lifecycle_for_test(&fixture, &entry);
    let carrier = body_with_exact_merge_execution_header(&entry);
    fixture
        .kura
        .store_block(Arc::new(parent.clone()))
        .expect("persist replayed commit barrier parent");
    fixture
        .kura
        .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
        .expect("persist replayed commit barrier merge group");
    fixture.persist_exact_v2_finality_chain(&[&parent, &carrier]);
    commit_exact_fixture_carrier_chain_to_state(&fixture, &parent, &carrier);
    fixture
        .kura
        .persist_merge_lane_block_application_receipts(&entry, 2, carrier.hash())
        .expect("persist the exact applied carrier receipt before journal replay");
    fixture
        .state
        .record_committed_queue_plan_entrypoints_for_tests(
            keys.iter().map(|key| key.entrypoint_hash),
            NonZeroUsize::new(2).expect("exact replayed commit barrier carrier State height"),
        )
        .expect("resolve exact committed QueuePlan obligations before replay");
    let pending = fixture
        .kura
        .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&entry)
        .expect("persist whole-carrier Pending before the first Queue mutation")
        .expect("the execution carrier has a complete Pending set");
    drop(pending);
    let pending_inventory = fixture
        .kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .expect("inventory the exact two-member Pending carrier");
    assert_eq!(pending_inventory.len(), 1);
    let expected_groups = pending_inventory[0]
        .pending_reservation_groups()
        .expect("retain the exact Pending two-member identity")
        .to_vec();
    assert_eq!(expected_groups.len(), 1);
    assert_eq!(expected_groups[0].binding(), group);
    let (expected_restored, expected_commit_barriers, expected_finalized) = match crash_cut {
        ReplayedQueueCleanupCrashCut::MixedCommitLive => {
            // Reproduce a crash between individual durable Commit frames, after
            // exact signed custody, canonical application and Pending publication.
            assert_eq!(
                queue
                    .commit_lane_reservation_group_prefix_for_test(&keys, 1)
                    .expect("persist only the first exact Commit frame"),
                1
            );
            assert_eq!(queue.live_lane_reservations(), vec![keys[1]]);
            assert_eq!(queue.lane_reservation_commit_barriers(), vec![keys[0]]);
            (1, 1, 1)
        }
        ReplayedQueueCleanupCrashCut::AfterPlanTombstone => {
            queue.hold_next_lane_reservation_commit_after_barrier_for_test();
            let crash = finalize_committed_block_merge_reservations(
                fixture.state.as_ref(),
                &queue,
                fixture.kura.as_ref(),
                &carrier,
                fixture.context.network_id,
            )
            .expect_err("the native post-tombstone crash hook withholds terminal Queue evidence");
            assert!(
                crash.to_string().contains("terminal"),
                "unexpected injected cleanup error: {crash:?}"
            );
            (0, 2, 0)
        }
    };
    assert_eq!(
        queue.lane_reservation_commit_barriers().len(),
        expected_commit_barriers
    );
    let snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture genuine interrupted cleanup");
    {
        let observer = queue.lock_lane_retirement_observer();
        V2ApplyService::validate_autoscale_retirement_queue_binding(
            &observer,
            keys[0].lane_id,
            keys[0].dataspace_id,
            keys[0].lane_incarnation,
        )
        .expect_err("production retirement must retain genuine Commit barriers");
    }
    let retirement_error = fixture
        .state
        .apply_lane_lifecycle(&iroha_data_model::nexus::LaneLifecyclePlan {
            additions: vec![reservation_lane.clone()],
            retire: vec![reservation_lane.id],
        })
        .expect_err("Pending canonical lifecycle debt must block actual archive publication");
    assert!(
        retirement_error
            .to_string()
            .contains("Pending autonomous lifecycle terminal outcome")
            || retirement_error.to_string().contains("slot retirement"),
        "unexpected terminality refusal: {retirement_error}"
    );
    assert_eq!(
        queue.lane_reservation_reconciliation_snapshot().unwrap(),
        snapshot
    );
    assert_eq!(
        fixture.state.lane_incarnation(reservation_lane.id),
        Some(keys[0].lane_incarnation)
    );
    drop(queue);
    let verified_active_context = verified_successor_context_after_fixture_tip(&fixture);
    let queue = Queue::from_config(QueueConfig::default(), events_sender);
    let replay = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("replay mixed Commit/live owner group");
    assert_eq!(replay.restored, expected_restored);
    assert_eq!(replay.commit_barriers, expected_commit_barriers);
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install mixed Commit/live QueuePlan journal");
    queue
        .replay_plan_journal(fixture.state.as_ref())
        .expect("replay quarantined mixed Commit/live plans");
    assert!(queue.lane_reservation_startup_reconciliation_pending());
    assert_eq!(
        reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            &queue,
            fixture.kura.as_ref(),
            &verified_active_context,
        )
        .expect("complete mixed Commit/live recovery"),
        LaneReservationReconciliationSummary {
            recovered: 2,
            finalized_committed: expected_finalized,
            ..LaneReservationReconciliationSummary::default()
        }
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert!(queue.lane_reservation_commit_barriers().is_empty());
    assert!(!queue.lane_reservation_startup_reconciliation_pending());
    let completed_stages = fixture
        .kura
        .verify_expected_autonomous_lifecycle_terminal_outcome_stages(
            fixture.context.network_id,
            &expected_groups,
        )
        .expect("verify actual Queue-produced Complete custody");
    assert_eq!(completed_stages.len(), 1);
    assert_eq!(completed_stages[0].binding(), group);
    assert_eq!(
        completed_stages[0].stage(),
        crate::kura::AutonomousLifecycleTerminalOutcomeDurableStage::Complete
    );
    {
        let observer = queue.lock_lane_retirement_observer();
        V2ApplyService::validate_autoscale_retirement_queue_binding(
            &observer,
            keys[0].lane_id,
            keys[0].dataspace_id,
            keys[0].lane_incarnation,
        )
        .expect("native cleanup removes all retirement Queue debt");
    }
    let (old_incarnation, new_incarnation) =
        replace_recreatable_reservation_lane(fixture.state.as_ref(), &reservation_lane);
    assert!(
        keys.iter()
            .all(|key| key.lane_incarnation == old_incarnation)
    );
    assert_ne!(old_incarnation, new_incarnation);
    assert_eq!(
        fixture
            .state
            .lane_incarnation_at_height(reservation_lane.id, 3),
        Some(new_incarnation)
    );
    drop(queue);
    let terminal_queue = Queue::from_config(
        QueueConfig::default(),
        fixture.service.events_sender.clone(),
    );
    let terminal_replay = terminal_queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("reopen the actual completed two-member journal after retirement");
    assert_eq!(terminal_replay.restored, 0);
    assert_eq!(terminal_replay.commit_barriers, 0);
    terminal_queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("reopen completed two-member QueuePlan journal");
    terminal_queue
        .replay_plan_journal(fixture.state.as_ref())
        .expect("replay terminal QueuePlan state");
    let active = verified_successor_context_after_fixture_tip(&fixture);
    assert_eq!(
        reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            &terminal_queue,
            fixture.kura.as_ref(),
            &active,
        )
        .expect("completed replay never reconstructs retired Pending custody"),
        LaneReservationReconciliationSummary::default()
    );
    assert!(!terminal_queue.lane_reservation_startup_reconciliation_pending());
}

v2_apply_test!(replayed_mixed_commit_barrier_group_reopens_startup_gate, {
    assert_replayed_queue_cleanup_completes_before_same_id_retirement(
        ReplayedQueueCleanupCrashCut::MixedCommitLive,
    );
});
v2_apply_test!(
    replayed_partial_queue_cleanup_completes_before_same_id_retirement,
    {
        assert_replayed_queue_cleanup_completes_before_same_id_retirement(
            ReplayedQueueCleanupCrashCut::AfterPlanTombstone,
        );
    }
);
v2_apply_test!(
    startup_reconciliation_rejects_partial_state_group_without_mutation,
    {
        let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        let producer = KeyPair::try_from_seed(vec![0xB8; 32], Algorithm::BlsNormal)
            .expect("derive partial-state autonomous producer");
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
        let journal_dir = tempfile::tempdir().expect("partial-state journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install partial-state queue-plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install partial-state reservation journal");
        let (payload, _) = reserve_autonomous_crash_batch(&fixture, &queue, &producer);
        let keys = payload.reservation_keys.clone();
        drop(queue);
        let queue = replay_fixture_queue_for_startup(fixture.state.as_ref(), &journal_dir);
        fixture.state.record_committed_entrypoints_for_tests(
            [keys[0].entrypoint_hash],
            NonZeroUsize::new(1).expect("partial committed height"),
        );
        let before = queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture partial-state ownership snapshot");
        let error = reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &fixture.context),
        )
        .expect_err("partial atomic reservation group must fail closed");
        assert!(
            matches!(error, V2ReservationLifecycleError::PartialCommittedGroup {
                lane_id: LaneId::SINGLE,
                proposal_height: 1,
            }),
            "partial State membership must fail whole-group preflight: {error:?}"
        );
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("capture partial-state post-error snapshot"),
            before
        );
        assert!(queue.lane_reservation_commit_barriers().is_empty());
        assert!(queue.lane_reservation_release_barriers().is_empty());
    }
);
v2_apply_test!(strict_absence_releases_original_fifo_not_digest_order, {
    let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
    let producer = KeyPair::try_from_seed(vec![0xB9; 32], Algorithm::BlsNormal)
        .expect("derive strict-absence autonomous producer");
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = Arc::new(Queue::from_config(
        QueueConfig::default(),
        events_sender.clone(),
    ));
    let journal_dir = tempfile::tempdir().expect("strict-absence journal directory");
    let plan_path = journal_dir.path().join("queue-plans.norito");
    let reservation_path = journal_dir.path().join("lane-reservations.norito");
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install strict-absence queue-plan journal");
    queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("install strict-absence reservation journal");
    let (payload, mut expected_fifo) = reserve_autonomous_crash_batch(&fixture, &queue, &producer);
    let descriptor = &payload.origin_proposal.descriptor;
    fixture
        .kura
        .install_lane_incarnation_marker_for_test(
            RuntimeLaneConfig::default().primary(),
            descriptor.lane_incarnation,
            0,
        )
        .expect("install strict-absence lane marker");
    let scope = LaneQueueReservationScopeV1 {
        lane_id: descriptor.lane_id,
        dataspace_id: descriptor.dataspace_id,
        lane_incarnation: descriptor.lane_incarnation,
        proposal_height: descriptor.proposal_height,
        lane_block_height: descriptor.lane_block_height,
        lane_block_view: descriptor.lane_block_view,
        reservation_owner_hash: payload.reservation_keys[0].reservation_owner_hash,
        proposal_identity_hash: payload.reservation_keys[0].proposal_identity_hash,
    };
    let mut snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture strict FIFO reconciliation snapshot");
    for index in 4_u8..16 {
        let fifo_keys = snapshot
            .ordered_records
            .iter()
            .map(|record| record.key)
            .collect::<Vec<_>>();
        if queue.live_lane_reservations() != fifo_keys {
            break;
        }
        let transaction = TransactionBuilder::new(
            *fixture.state.network_id_ref(),
            fixture.service.genesis_account.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            format!("strict FIFO digest-order discriminator {index}"),
        )])
        .with_admission_intent(
            iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
        )
        .sign(fixture.genesis_key.private_key());
        expected_fifo.push(transaction.hash_as_entrypoint());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
        let routing_plan = queue
            .route_plan_with_state(&accepted, fixture.state.as_ref())
            .expect("resolve strict FIFO discriminator route");
        let admission_context = queue
            .plan_admission_context_with_state(fixture.state.as_ref(), &routing_plan)
            .expect("capture strict FIFO discriminator admission context");
        let binding = crate::torii_proxy::QueuePlanAdmissionBindingV1::new(
            fixture.state.network_id_ref(),
            accepted.entrypoint(),
            &routing_plan,
            admission_context,
            queue.queue_plan_admission_timestamp_ms(),
        )
        .expect("build strict FIFO discriminator binding");
        queue
            .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
                accepted,
                fixture.state.as_ref(),
                routing_plan,
                &binding,
            )
            .expect("enqueue strict FIFO discriminator");
        install_fixture_queue_plan_registry_value(fixture.state.as_ref(), &binding);
        assert_eq!(
            queue
                .reserve_transactions_for_lane(
                    fixture.state.as_ref(),
                    scope,
                    NonZeroUsize::new(1).expect("one strict FIFO discriminator"),
                )
                .expect("extend strict FIFO reservation group")
                .len(),
            1
        );
        snapshot = queue
            .lane_reservation_reconciliation_snapshot()
            .expect("refresh strict FIFO reconciliation snapshot");
    }
    let fifo_keys = snapshot
        .ordered_records
        .iter()
        .map(|record| record.key)
        .collect::<Vec<_>>();
    assert_ne!(
        queue.live_lane_reservations(),
        fifo_keys,
        "fixture must exercise digest order differing from durable global FIFO"
    );
    let mut store = fixture.reopen_body_store();
    fixture
        .execute(&mut store)
        .expect("finalize canonical body omitting the strictly absent payload");
    let reserved_count = snapshot.ordered_records.len();
    drop(queue);
    let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
    let replay = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("replay strict-absence reservation owners");
    assert_eq!(replay.restored, reserved_count);
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install replayed strict-absence QueuePlan journal");
    queue
        .replay_plan_journal(fixture.state.as_ref())
        .expect("replay strict-absence QueuePlan payloads");
    assert!(queue.lane_reservation_startup_reconciliation_pending());
    let unchanged_snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture owners before rejected direct-release authorities");
    let unchanged_fifo = queue.fifo_snapshot_for_test();
    let unchanged_reservations = std::fs::read(&reservation_path)
        .expect("read reservation journal before rejected direct-release authorities");
    let unchanged_plans = std::fs::read(&plan_path)
        .expect("read QueuePlan journal before rejected direct-release authorities");
    let assert_rejected_release_unchanged = || {
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("read retained owners"),
            unchanged_snapshot,
            "rejected direct-release authority must preserve every reservation owner",
        );
        assert_eq!(queue.fifo_snapshot_for_test(), unchanged_fifo);
        assert_eq!(
            std::fs::read(&reservation_path).expect("read retained reservations"),
            unchanged_reservations
        );
        assert_eq!(
            std::fs::read(&plan_path).expect("read retained QueuePlan"),
            unchanged_plans
        );
        assert!(queue.lane_reservation_startup_reconciliation_pending());
        assert!(!queue.lane_reservation_durability_faulted());
    };
    assert!(matches!(
        queue.release_strictly_absent_lane_reservations_in_order(&fifo_keys, Vec::new()),
        Err(crate::queue::LaneQueueReservationError::InvalidIdentity(_)),
    ));
    assert_rejected_release_unchanged();
    for mutate_identity in [false, true] {
        let planning = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &fixture.context),
            None,
        )
        .expect("mint direct-release authority from actual strict-absence recovery");
        let LaneReservationReconciliationPlanning::Ready(plan) = planning else {
            panic!("strictly absent canonical group must be immediately plannable");
        };
        assert_eq!(plan.direct_release, fifo_keys);
        let authorizations = plan
            .actions
            .into_iter()
            .map(|action| {
                let ReservationReconciliationAction::DirectRelease { authorization, .. } = action
                else {
                    panic!("strict-absence fixture must mint only direct-release authorities");
                };
                authorization
            })
            .collect::<Vec<_>>();
        assert!(!authorizations.is_empty());
        let mut mismatched_keys = fifo_keys.clone();
        if mutate_identity {
            mismatched_keys[0].proposal_identity_hash =
                Hash::new(b"wrong authorized release proposal");
        } else {
            mismatched_keys
                .pop()
                .expect("non-empty strict-absence batch");
        }
        assert!(matches!(
            queue.release_strictly_absent_lane_reservations_in_order(
                &mismatched_keys,
                authorizations
            ),
            Err(crate::queue::LaneQueueReservationError::InvalidIdentity(_)
                | crate::queue::LaneQueueReservationError::Conflict { .. }),
        ));
        assert_rejected_release_unchanged();
    }
    assert_eq!(
        reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &fixture.context),
        )
        .expect("release strictly absent group"),
        LaneReservationReconciliationSummary {
            recovered: reserved_count,
            released_strictly_absent: reserved_count,
            ..LaneReservationReconciliationSummary::default()
        }
    );
    let mut selected = Vec::new();
    queue.get_transactions_for_block_with_state(
        fixture.state.as_ref(),
        NonZeroUsize::new(expected_fifo.len()).expect("non-empty restored FIFO"),
        &mut selected,
    );
    assert_eq!(
        selected
            .iter()
            .map(|transaction| transaction.as_ref().hash_as_entrypoint())
            .collect::<Vec<_>>(),
        expected_fifo,
        "strict absence must restore the exact pre-reservation FIFO sequence"
    );
});
v2_apply_test!(
    terminal_presweep_rejects_unquarantined_nonempty_queue_before_kura_inventory,
    {
        let fixture = ApplyFixture::new();
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Queue::from_config(QueueConfig::default(), events_sender);
        let journal_dir = tempfile::tempdir().expect("terminal pre-sweep journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install terminal pre-sweep QueuePlan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install terminal pre-sweep reservation journal");
        let transaction = fixture
            .body
            .external_transactions()
            .next()
            .expect("fixture transaction")
            .clone();
        let _ = reserve_transaction_for_test_with_identity(
            fixture.state.as_ref(),
            &queue,
            transaction,
            Hash::new(b"terminal pre-sweep owner"),
            Hash::new(b"terminal pre-sweep proposal"),
        );
        let before = queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture unquarantined terminal pre-sweep snapshot");
        assert!(!before.is_empty());
        assert!(!queue.lane_reservation_startup_reconciliation_pending());
        let error = crate::sumeragi::v2_lifecycle_recovery::reconcile_pending_autonomous_lifecycle_terminal_outcomes(
            fixture.state.as_ref(),
            &queue,
            fixture.kura.as_ref(),
            &fixture.context,
        )
        .expect_err("terminal pre-sweep must reject an unquarantined non-empty Queue cut");
        assert!(error.contains("published before terminal-outcome pre-sweep"));
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("recapture rejected terminal pre-sweep snapshot"),
            before,
        );
    }
);
v2_apply_test!(
    empty_startup_plan_skips_canonical_cleanup_and_publishes_its_receipt,
    {
        let fixture = ApplyFixture::new();
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
        let journal_dir = tempfile::tempdir().expect("empty startup journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install empty startup QueuePlan journal");
        let replay = queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install empty startup reservation journal");
        assert_eq!(replay, Default::default());
        assert_eq!(
            queue
                .replay_plan_journal(fixture.state.as_ref())
                .expect("replay empty startup QueuePlan journal"),
            Default::default(),
        );
        assert!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("capture empty startup snapshot")
                .is_empty(),
        );
        assert!(queue.lane_reservation_startup_reconciliation_pending());
        let planning = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &fixture.context),
            None,
        )
        .expect("plan empty startup ownership cut");
        let LaneReservationReconciliationPlanning::Ready(plan) = planning else {
            panic!("empty startup ownership cut must be immediately ready");
        };
        assert_eq!(
            apply_lane_reservation_reconciliation_plan(
                fixture.state.as_ref(),
                queue.as_ref(),
                fixture.kura.as_ref(),
                plan,
            )
            .expect("apply empty startup ownership cut"),
            LaneReservationReconciliationSummary::default(),
        );
        assert!(!queue.lane_reservation_startup_reconciliation_pending());
        let completed_reservations =
            std::fs::read(journal_dir.path().join("lane-reservations.norito"))
                .expect("retain completed empty reservation journal");
        let completed_plans = std::fs::read(journal_dir.path().join("queue-plans.norito"))
            .expect("retain completed empty QueuePlan journal");
        let planning = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &fixture.context),
            None,
        )
        .expect("observe the genuinely completed empty startup");
        let LaneReservationReconciliationPlanning::AlreadyCompleted(observation) = planning else {
            panic!("a completed empty startup must return read-only evidence");
        };
        assert_eq!(
            observe_completed_lane_reservation_reconciliation(
                queue.as_ref(),
                fixture.kura.as_ref(),
                observation,
            )
            .expect("validate the empty Kura/Queue completion observation"),
            LaneReservationReconciliationSummary::default()
        );
        assert_eq!(
            std::fs::read(journal_dir.path().join("lane-reservations.norito")).unwrap(),
            completed_reservations
        );
        assert_eq!(
            std::fs::read(journal_dir.path().join("queue-plans.norito")).unwrap(),
            completed_plans
        );
    }
);
v2_apply_test!(
    deferred_canonical_carrier_owned_and_absent_groups_complete_before_gate_publication,
    {
        let recovery = deferred_canonical_carrier_startup_fixture();
        let summary = apply_lane_reservation_reconciliation_plan(
            recovery.fixture.state.as_ref(),
            recovery.queue.as_ref(),
            recovery.fixture.kura.as_ref(),
            recovery.plan,
        )
        .expect("apply Queue-owned A and terminalize absent sibling B");
        assert_eq!(summary.recovered, 1);
        assert_eq!(summary.finalized_committed, 1);
        assert!(
            recovery
                .queue
                .lane_reservation_reconciliation_snapshot()
                .expect("read completed deferred carrier Queue snapshot")
                .is_empty()
        );
        assert!(
            !recovery
                .queue
                .lane_reservation_startup_reconciliation_pending(),
            "Queue publication opens only after direct A+B Complete proof",
        );
        let stages = recovery
            .fixture
            .kura
            .verify_expected_autonomous_lifecycle_terminal_outcome_stages(
                recovery.fixture.context.network_id,
                &recovery.expected_groups,
            )
            .expect("prove both deferred carrier outcomes Complete");
        assert!(stages.iter().all(|stage| {
            stage.stage() == crate::kura::AutonomousLifecycleTerminalOutcomeDurableStage::Complete
        }));
    }
);
v2_apply_test!(
    deferred_canonical_carrier_missing_after_queue_cleanup_keeps_startup_gate_closed,
    {
        let recovery = deferred_canonical_carrier_startup_fixture();
        let missing_path = recovery.outcome_paths[1].clone();
        crate::sumeragi::v2_lifecycle_recovery::install_deferred_terminal_stage_proof_hook_for_test(
            move || {
                std::fs::remove_file(&missing_path)
                    .expect("delete B only after normal Queue cleanup succeeds");
            },
        );
        let error = apply_lane_reservation_reconciliation_plan(
            recovery.fixture.state.as_ref(),
            recovery.queue.as_ref(),
            recovery.fixture.kura.as_ref(),
            recovery.plan,
        )
        .expect_err("missing post-handoff B must block final receipt publication");
        assert!(matches!(
            error,
            V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization { ref detail }
                if detail.contains("deferred terminal stage proof failed")
        ));
        assert!(
            recovery
                .queue
                .lane_reservation_reconciliation_snapshot()
                .expect("read Queue after injected terminal proof loss")
                .is_empty(),
            "the injected cut must occur after Queue mutation succeeds",
        );
        assert!(
            recovery
                .queue
                .lane_reservation_startup_reconciliation_pending(),
            "missing exact terminal evidence must keep startup publication closed",
        );
        assert!(
            recovery
                .fixture
                .kura
                .verify_expected_autonomous_lifecycle_terminal_outcome_stages(
                    recovery.fixture.context.network_id,
                    &recovery.expected_groups,
                )
                .is_err(),
            "the final exact stage proof must remain fail-closed after B disappears",
        );
    }
);
v2_apply_test!(
    zero_length_canonical_index_is_rejected_before_queue_mutation,
    {
        let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        let producer = KeyPair::try_from_seed(vec![0xBA; 32], Algorithm::BlsNormal)
            .expect("derive pruned-carrier autonomous producer");
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
        let journal_dir = tempfile::tempdir().expect("pruned-carrier journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install pruned-carrier queue-plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install pruned-carrier reservation journal");
        let (payload, _) = reserve_autonomous_crash_batch(&fixture, &queue, &producer);
        let descriptor = &payload.origin_proposal.descriptor;
        fixture
            .kura
            .install_lane_incarnation_marker_for_test(
                RuntimeLaneConfig::default().primary(),
                descriptor.lane_incarnation,
                0,
            )
            .expect("install pruned-carrier lane marker");
        fixture
            .kura
            .persist_lane_executable_payload(&payload, payload.network_id, payload.epoch)
            .expect("persist exact pruned-carrier payload");
        let mut store = fixture.reopen_body_store();
        fixture
            .execute(&mut store)
            .expect("finalize carrier before body pruning");
        let carrier_height = NonZeroUsize::new(1).expect("non-zero pruned carrier height");
        let canonical_body = fixture
            .kura
            .get_block_without_merge_sidecar(carrier_height)
            .expect("capture canonical carrier before body pruning");
        fixture
            .kura
            .force_hash_only_block_for_testing(carrier_height)
            .expect("evict exact finalized carrier body");
        // Startup quarantine is minted by replaying durable ownership into a
        // fresh Queue; the live Queue used to create this crash image is open.
        assert_eq!(Arc::strong_count(&queue), 1);
        drop(queue);
        let (events_sender, _replayed_events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("replay exact pruned-carrier QueuePlan payloads");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("replay exact pruned-carrier reservation ownership");
        queue
            .replay_plan_journal(fixture.state.as_ref())
            .expect("replay exact pruned-carrier durable QueuePlan claims");
        assert!(queue.lane_reservation_startup_reconciliation_pending());
        let before = queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture pruned-carrier ownership snapshot");
        let error = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &fixture.context),
            None,
        )
        .err()
        .expect("zero-length canonical metadata is corruption, not recovery authority");
        assert!(matches!(
            error,
            V2ReservationLifecycleError::Kura(crate::kura::Error::CorruptedBlockLength {
                length: 0,
                ..
            })
        ));
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("recapture rejected recovery ownership"),
            before
        );
        assert!(queue.lane_reservation_startup_reconciliation_pending());
        assert!(
            fixture
                .kura
                .read_autonomous_lane_slot_retirement(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    payload.network_id,
                    payload.epoch,
                )
                .expect("read unchanged retirement state")
                .is_none()
        );
        assert_eq!(
            fixture.kura.get_durable_block_hash(carrier_height),
            Some(canonical_body.hash())
        );
    }
);
v2_apply_test!(
    finalized_hash_only_carrier_plans_recovery_before_queue_mutation,
    {
        let fixture = ApplyFixture::new_with_options_and_retention(
            false,
            false,
            false,
            false,
            NonZeroUsize::new(1).expect("retain the live tail"),
        );
        let mut genesis_store = fixture.reopen_body_store();
        fixture
            .execute(&mut genesis_store)
            .expect("commit the retained genesis parent");
        let context = successor_height_context(&fixture);
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
        let journal_dir = tempfile::tempdir().expect("pruned-carrier journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install pruned-carrier queue-plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install pruned-carrier reservation journal");
        let (payload, _) =
            reserve_canonical_successor_autonomous_batch(&fixture, &queue, &context, 2);
        let descriptor = &payload.origin_proposal.descriptor;
        fixture
            .kura
            .install_lane_incarnation_marker_for_test(
                RuntimeLaneConfig::default().primary(),
                descriptor.lane_incarnation,
                0,
            )
            .expect("install pruned-carrier lane marker");
        fixture
            .kura
            .persist_lane_executable_payload(&payload, payload.network_id, payload.epoch)
            .expect("persist exact pruned-carrier payload");
        let envelope = crate::lane_consensus::autonomous_lane_payload_envelope(
            &payload,
            payload.network_id,
            payload.epoch,
        )
        .expect("encode the exact authenticated successor payload");
        let mut successor =
            build_successor_apply_fixture_with_autonomous_payloads(&fixture, vec![envelope]);
        fixture
            .service
            .execute(&successor.context, &mut successor.store, &successor.task)
            .expect("finalize the actual non-genesis carrier before eviction");
        let carrier_height = NonZeroUsize::new(2).expect("non-genesis carrier height");
        let canonical_body = fixture
            .kura
            .get_block_without_merge_sidecar(carrier_height)
            .expect("capture canonical carrier before body pruning");
        let tail = BlockBuilder::new_with_time_source(Vec::new(), TimeSource::new_system())
            .chain(0, Some(canonical_body.as_ref()))
            .try_sign_with_index(fixture.genesis_key.private_key(), 0)
            .expect("sign the durable tail beyond the finalized carrier")
            .unpack(|_| {});
        let tail = complete_empty_fixture_block(SignedBlock::from(tail), &fixture.genesis_key);
        fixture
            .kura
            .store_block(Arc::new(tail))
            .expect("retain an inline successor tail");
        let (_, payload_len) = fixture
            .kura
            .durable_block_payload_len_by_hash(canonical_body.hash())
            .expect("read exact finality-authenticated eviction length")
            .expect("carrier length");
        assert_eq!(
            fixture
                .kura
                .advertise_required_replicas_for_bench(carrier_height),
            Some(payload_len)
        );
        assert_eq!(
            fixture
                .kura
                .evict_block_bodies(payload_len)
                .expect("evict via actual selected-keeper policy"),
            payload_len
        );
        fixture
            .kura
            .remove_evicted_block_sidecar_for_testing(carrier_height)
            .expect("model remote-only carrier after genuine eviction");
        assert!(
            fixture
                .kura
                .get_block_without_merge_sidecar(carrier_height)
                .is_none()
        );
        assert_eq!(
            fixture
                .kura
                .durable_block_payload_len_by_hash(canonical_body.hash())
                .expect("eviction preserves authenticated canonical wire length"),
            Some((2, payload_len))
        );
        // Startup quarantine is minted by replaying durable ownership into a
        // fresh Queue; the live Queue used to create this crash image is open.
        assert_eq!(Arc::strong_count(&queue), 1);
        drop(queue);
        let (events_sender, _replayed_events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("replay exact pruned-carrier QueuePlan payloads");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("replay exact pruned-carrier reservation ownership");
        queue
            .replay_plan_journal(fixture.state.as_ref())
            .expect("replay exact pruned-carrier durable QueuePlan claims");
        assert!(queue.lane_reservation_startup_reconciliation_pending());
        let before = queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture pruned-carrier ownership snapshot");
        let planning = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &successor.context),
            None,
        )
        .expect("hash-only finality produces an authenticated recovery plan");
        let LaneReservationReconciliationPlanning::RecoverCanonicalBodies(needs) = planning else {
            panic!("hash-only finality must not produce a Queue mutation plan");
        };
        assert_eq!(needs.len(), 1);
        let need = needs[0];
        let finality = fixture
            .kura
            .v2_finality_artifact(2)
            .expect("read pruned-carrier finality")
            .expect("pruned carrier retains finality");
        assert_eq!(need.height, 2);
        assert_eq!(need.block_hash, canonical_body.hash());
        assert_eq!(need.finality_artifact_hash, HashOf::new(&finality));
        assert_eq!(
            need.execution_commitment,
            finality.commit_qc.execution_commitment
        );
        assert_eq!(
            need.executed_block_wire_hash,
            canonical_body
                .executed_block_wire_hash()
                .expect("hash canonical executed block wire")
        );
        let mut collected = BTreeMap::new();
        let mut later = need;
        later.height = 3;
        later.block_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"later canonical executed-block need"));
        collect_canonical_executed_block_need(&mut collected, later)
            .expect("collect later recovery need");
        collect_canonical_executed_block_need(&mut collected, need)
            .expect("collect earlier recovery need");
        collect_canonical_executed_block_need(&mut collected, need)
            .expect("deduplicate byte-identical recovery need");
        assert_eq!(
            collected.keys().copied().collect::<Vec<_>>(),
            vec![2, 3],
            "recovery needs are unique and ordered by canonical height"
        );
        let mut conflicting = need;
        conflicting.executed_block_wire_hash = Hash::new(b"conflicting same-height wire");
        assert!(matches!(
            collect_canonical_executed_block_need(&mut collected, conflicting),
            Err(V2ReservationLifecycleError::CanonicalContextMismatch { height: 2 })
        ));
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("capture pruned-carrier post-error snapshot"),
            before
        );
        assert!(
            queue.lane_reservation_startup_reconciliation_pending(),
            "recovery planning must leave Queue publication closed"
        );
        assert!(
            fixture
                .kura
                .read_autonomous_lane_slot_retirement(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    payload.network_id,
                    payload.epoch,
                )
                .expect("read pruned-carrier retirement state")
                .is_none()
        );
        fixture
            .kura
            .cache_block_body(&canonical_body)
            .expect("restore the exact finality-authenticated carrier body");
        let replanned = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &successor.context),
            None,
        )
        .expect("replan after exact body recovery");
        let LaneReservationReconciliationPlanning::InstallHistoricalAutonomousRecoveries(installs) =
            replanned
        else {
            panic!(
                "body recovery must expose the exact historical installation before Queue mutation"
            );
        };
        assert_eq!(installs.len(), 1);
        assert_eq!(installs[0].canonical_body, need);
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("snapshot after body recovery and historical planning"),
            before,
            "body recovery cannot publish or consume Queue ownership",
        );
        assert_eq!(
            install_historical_autonomous_lane_recovery(
                fixture.state.as_ref(),
                fixture.kura.as_ref(),
                &installs[0],
            )
            .expect("durably install the exact recovered autonomous carrier"),
            HistoricalAutonomousLaneRecoveryInstallOutcome::Installed,
        );
        let ready = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &successor.context),
            None,
        )
        .expect("replan after the complete historical installation");
        let LaneReservationReconciliationPlanning::Ready(plan) = ready else {
            panic!("durable body and historical installation must make Queue reconciliation ready");
        };
        assert!(queue.lane_reservation_startup_reconciliation_pending());
        apply_lane_reservation_reconciliation_plan(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            plan,
        )
        .expect("apply only the fully ready reconciliation plan");
        assert!(
            !queue.lane_reservation_startup_reconciliation_pending(),
            "Queue publication opens only after recovered evidence is replanned and applied"
        );
    }
);
v2_apply_test!(canonical_exact_certified_autonomous_group_is_retained, {
    let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
    let mut genesis_store = fixture.reopen_body_store();
    fixture
        .execute(&mut genesis_store)
        .expect("commit parent before canonical autonomous successor");
    let context = successor_height_context(&fixture);
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = Arc::new(Queue::from_config(
        QueueConfig::default(),
        events_sender.clone(),
    ));
    let journal_dir = tempfile::tempdir().expect("canonical autonomous journal directory");
    let plan_path = journal_dir.path().join("queue-plans.norito");
    let reservation_path = journal_dir.path().join("lane-reservations.norito");
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install canonical autonomous queue-plan journal");
    queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("install canonical autonomous reservation journal");
    let (payload, _) = reserve_canonical_successor_autonomous_batch(&fixture, &queue, &context, 2);
    let descriptor = &payload.origin_proposal.descriptor;
    fixture
        .kura
        .install_lane_incarnation_marker_for_test(
            RuntimeLaneConfig::default().primary(),
            descriptor.lane_incarnation,
            0,
        )
        .expect("install canonical autonomous lane marker");
    fixture
        .kura
        .persist_lane_executable_payload(&payload, payload.network_id, payload.epoch)
        .expect("persist canonical autonomous payload");
    let envelope = crate::lane_consensus::autonomous_lane_payload_envelope(
        &payload,
        payload.network_id,
        payload.epoch,
    )
    .expect("encode canonical autonomous envelope");
    let mut successor =
        build_successor_apply_fixture_with_autonomous_payloads(&fixture, vec![envelope]);
    fixture
        .service
        .execute(&successor.context, &mut successor.store, &successor.task)
        .expect("finalize exact autonomous carrier");
    certify_autonomous_payload_for_test(&fixture, &payload);
    drop(queue);
    let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
    let replay = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("replay certified autonomous reservation owners");
    assert_eq!(replay.restored, payload.reservation_keys.len());
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install certified autonomous QueuePlan journal");
    queue
        .replay_plan_journal(fixture.state.as_ref())
        .expect("replay certified autonomous QueuePlan payloads");
    assert!(queue.lane_reservation_startup_reconciliation_pending());
    assert_eq!(
        reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &successor.context),
        )
        .expect("retain exact canonically certified owner"),
        LaneReservationReconciliationSummary {
            recovered: 2,
            retained_certified: 2,
            ..LaneReservationReconciliationSummary::default()
        }
    );
    assert!(
        !queue.lane_reservation_startup_reconciliation_pending(),
        "retained certified owners must not leave Queue startup frozen"
    );
    assert_eq!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("read retained certified ownership")
            .ordered_records
            .into_iter()
            .map(|record| record.key)
            .collect::<Vec<_>>(),
        payload.reservation_keys
    );
    assert!(queue.lane_reservation_release_barriers().is_empty());
    assert!(
        fixture
            .kura
            .read_autonomous_lane_slot_retirement(
                descriptor.lane_id,
                descriptor.lane_block_height,
                payload.network_id,
                payload.epoch,
            )
            .expect("read canonical certified retirement state")
            .is_none()
    );
});
v2_apply_test!(replayed_current_autonomous_group_reopens_startup_gate, {
    let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
    let mut genesis_store = fixture.reopen_body_store();
    fixture
        .execute(&mut genesis_store)
        .expect("commit parent before current autonomous recovery");
    let context = successor_height_context(&fixture);
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = Arc::new(Queue::from_config(
        QueueConfig::default(),
        events_sender.clone(),
    ));
    let journal_dir = tempfile::tempdir().expect("current autonomous journal directory");
    let plan_path = journal_dir.path().join("queue-plans.norito");
    let reservation_path = journal_dir.path().join("lane-reservations.norito");
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install current autonomous QueuePlan journal");
    queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("install current autonomous reservation journal");
    let (payload, _) = reserve_canonical_successor_autonomous_batch(&fixture, &queue, &context, 2);
    let unreserved_transaction = TransactionBuilder::new(
        *fixture.state.network_id_ref(),
        fixture.service.genesis_account.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "current recovery startup-gate probe".to_owned(),
    )])
    .with_admission_intent(
        iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
    )
    .sign(fixture.genesis_key.private_key());
    let unreserved_hash = unreserved_transaction.hash();
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(unreserved_transaction));
    let routing_plan = queue
        .route_plan_with_state(&accepted, fixture.state.as_ref())
        .expect("resolve startup-gate probe route");
    let admission_context = queue
        .plan_admission_context_with_state(fixture.state.as_ref(), &routing_plan)
        .expect("capture startup-gate probe admission context");
    let binding = crate::torii_proxy::QueuePlanAdmissionBindingV1::new(
        fixture.state.network_id_ref(),
        accepted.entrypoint(),
        &routing_plan,
        admission_context,
        queue.queue_plan_admission_timestamp_ms(),
    )
    .expect("build startup-gate probe binding");
    queue
        .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
            accepted,
            fixture.state.as_ref(),
            routing_plan,
            &binding,
        )
        .expect("enqueue startup-gate probe");
    install_fixture_queue_plan_registry_value(fixture.state.as_ref(), &binding);
    let descriptor = &payload.origin_proposal.descriptor;
    fixture
        .kura
        .install_lane_incarnation_marker_for_test(
            RuntimeLaneConfig::default().primary(),
            descriptor.lane_incarnation,
            0,
        )
        .expect("install current autonomous lane marker");
    fixture
        .kura
        .persist_lane_executable_payload(&payload, payload.network_id, payload.epoch)
        .expect("persist current autonomous payload");
    drop(queue);
    let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
    let replay = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("replay current autonomous owners");
    assert_eq!(replay.restored, payload.reservation_keys.len());
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install replayed current QueuePlan journal");
    queue
        .replay_plan_journal(fixture.state.as_ref())
        .expect("replay current autonomous payload bytes");
    assert!(queue.lane_reservation_startup_reconciliation_pending());
    let mut selected = Vec::new();
    queue.get_transactions_for_block_with_state(
        fixture.state.as_ref(),
        NonZeroUsize::new(1).expect("one startup-gate probe"),
        &mut selected,
    );
    assert!(
        selected.is_empty(),
        "replayed unreserved work must stay quarantined until evidence reconciliation"
    );
    assert_eq!(
        reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &context),
        )
        .expect("retain current autonomous owners"),
        LaneReservationReconciliationSummary {
            recovered: payload.reservation_keys.len(),
            retained_current: payload.reservation_keys.len(),
            ..LaneReservationReconciliationSummary::default()
        }
    );
    assert!(!queue.lane_reservation_startup_reconciliation_pending());
    queue.get_transactions_for_block_with_state(
        fixture.state.as_ref(),
        NonZeroUsize::new(1).expect("one reopened startup-gate probe"),
        &mut selected,
    );
    assert_eq!(
        selected
            .iter()
            .map(|transaction| transaction.as_ref().hash())
            .collect::<Vec<_>>(),
        vec![unreserved_hash],
        "successful retained-current reconciliation must reopen ordinary selection"
    );
    assert_eq!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("read retained current ownership")
            .ordered_groups,
        vec![LaneQueueReservationReconciliationGroupV1 {
            identity: reservation_group_identity(&payload.reservation_keys[0]),
            ordered_keys: payload.reservation_keys,
        }]
    );
});
v2_apply_test!(
    prior_height_canonical_uncertified_owner_requires_historical_recovery,
    {
        let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        let mut genesis_store = fixture.reopen_body_store();
        fixture
            .execute(&mut genesis_store)
            .expect("commit parent before historical autonomous successor");
        let context = successor_height_context(&fixture);
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Arc::new(Queue::from_config(
            QueueConfig::default(),
            events_sender.clone(),
        ));
        let journal_dir = tempfile::tempdir().expect("historical autonomous journal directory");
        let plan_path = journal_dir.path().join("queue-plans.norito");
        let reservation_path = journal_dir.path().join("lane-reservations.norito");
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install historical autonomous queue-plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install historical autonomous reservation journal");
        let (payload, _) =
            reserve_canonical_successor_autonomous_batch(&fixture, &queue, &context, 2);
        let descriptor = &payload.origin_proposal.descriptor;
        fixture
            .kura
            .install_lane_incarnation_marker_for_test(
                RuntimeLaneConfig::default().primary(),
                descriptor.lane_incarnation,
                0,
            )
            .expect("install historical autonomous lane marker");
        fixture
            .kura
            .persist_lane_executable_payload(&payload, payload.network_id, payload.epoch)
            .expect("persist historical autonomous payload");
        let envelope = crate::lane_consensus::autonomous_lane_payload_envelope(
            &payload,
            payload.network_id,
            payload.epoch,
        )
        .expect("encode historical autonomous envelope");
        let mut successor =
            build_successor_apply_fixture_with_autonomous_payloads(&fixture, vec![envelope]);
        fixture
            .service
            .execute(&successor.context, &mut successor.store, &successor.task)
            .expect("finalize historical autonomous carrier");
        let mut next_context = successor.context.clone();
        next_context.height = 3;
        next_context.parent_commit_qc = Some(successor.task.certificate().clone());
        next_context
            .validate()
            .expect("valid next context for historical recovery");
        drop(queue);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
        let replay = queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("replay historical autonomous reservation owners");
        assert_eq!(replay.restored, payload.reservation_keys.len());
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install replayed historical autonomous QueuePlan journal");
        queue
            .replay_plan_journal(fixture.state.as_ref())
            .expect("replay historical autonomous QueuePlan payloads");
        assert!(queue.lane_reservation_startup_reconciliation_pending());
        let before = queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture historical ownership snapshot");
        let planning = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &next_context),
            None,
        )
        .expect("classify prior-height autonomous recovery without mutating Queue");
        let LaneReservationReconciliationPlanning::InstallHistoricalAutonomousRecoveries(installs) =
            planning
        else {
            panic!("prior-height exact autonomous carrier must plan durable installation");
        };
        assert_eq!(installs.len(), 1);
        let install = &installs[0];
        assert!(install.has_valid_identity());
        assert_eq!(install.canonical_body.height, 2);
        assert_eq!(install.historical_context, successor.context);
        assert_eq!(install.historical_context_id, successor.context.id());
        assert_eq!(install.payload.origin_proposal.descriptor, *descriptor);
        assert_eq!(install.payload.entrypoint_hashes, payload.entrypoint_hashes);
        assert_eq!(install.payload.reservation_keys, payload.reservation_keys);
        assert_eq!(
            install.reservation_group,
            LaneQueueReservationReconciliationGroupV1 {
                identity: reservation_group_identity(&payload.reservation_keys[0]),
                ordered_keys: payload.reservation_keys.clone(),
            }
        );
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("capture historical post-error ownership snapshot"),
            before
        );
        assert!(queue.lane_reservation_startup_reconciliation_pending());
        assert!(
            fixture
                .kura
                .read_autonomous_lane_slot_retirement(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    payload.network_id,
                    payload.epoch,
                )
                .expect("read historical retirement state")
                .is_none()
        );
        assert_eq!(
            install_historical_autonomous_lane_recovery(
                fixture.state.as_ref(),
                fixture.kura.as_ref(),
                install,
            )
            .expect("install historical autonomous recovery"),
            HistoricalAutonomousLaneRecoveryInstallOutcome::Installed,
        );
        assert_eq!(
            install_historical_autonomous_lane_recovery(
                fixture.state.as_ref(),
                fixture.kura.as_ref(),
                install,
            )
            .expect("historical autonomous recovery retry is idempotent"),
            HistoricalAutonomousLaneRecoveryInstallOutcome::AlreadyInstalled,
        );
        assert!(
            fixture
                .kura
                .historical_autonomous_lane_recovery_matches(install)
                .expect("read back historical autonomous recovery")
        );
        let lane_config = RuntimeLaneConfig::default();
        let lane = lane_config
            .entry(descriptor.lane_id)
            .expect("historical recovery lane is configured");
        let recovery_path = lane
            .blocks_dir(fixture.kura.store_root())
            .join("lane_artifacts")
            .join("historical_autonomous_recoveries_v1")
            .join(format!(
                "{}.norito",
                hex::encode(install.recovery_id.as_ref())
            ));
        let recovery_bytes = std::fs::read(&recovery_path).expect("read historical recovery seal");
        let mut corrupt_recovery = recovery_bytes.clone();
        corrupt_recovery[0] ^= 0x80;
        std::fs::write(&recovery_path, corrupt_recovery).expect("corrupt historical recovery seal");
        assert!(
            fixture
                .kura
                .historical_autonomous_lane_recovery_matches(install)
                .is_err(),
            "corrupt immutable recovery evidence must fail closed"
        );
        std::fs::write(&recovery_path, recovery_bytes).expect("restore historical recovery seal");
        fixture
            .kura
            .reset_historical_autonomous_recovery_inventory_scans_for_test();
        let replanning = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &next_context),
            None,
        )
        .expect("replan with durable historical recovery");
        let LaneReservationReconciliationPlanning::Ready(plan) = replanning else {
            panic!("durable historical recovery must make the immutable plan ready");
        };
        assert_eq!(
            fixture
                .kura
                .historical_autonomous_recovery_inventory_scans_for_test(),
            1,
            "one planning authority boundary must scan the bounded historical inventory once",
        );
        fixture
            .kura
            .reset_historical_autonomous_recovery_inventory_scans_for_test();
        let summary = apply_lane_reservation_reconciliation_plan(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            plan,
        )
        .expect("publish historical reservation reconciliation");
        assert_eq!(
            fixture
                .kura
                .historical_autonomous_recovery_inventory_scans_for_test(),
            1,
            "one application authority boundary must scan the bounded historical inventory once",
        );
        assert_eq!(
            summary,
            LaneReservationReconciliationSummary {
                recovered: payload.reservation_keys.len(),
                retained_historical_recovery: payload.reservation_keys.len(),
                ..LaneReservationReconciliationSummary::default()
            }
        );
        assert!(!queue.lane_reservation_startup_reconciliation_pending());
    }
);
include!("v2_apply_unsealed_01c_historical_recovery.rs");
include!("v2_apply_unsealed_01c_ordinary_to_autonomous.rs");
v2_apply_test!(pending_merge_split_group_is_rejected, {
    let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
    let producer = KeyPair::try_from_seed(vec![0xBB; 32], Algorithm::BlsNormal)
        .expect("derive pending-split autonomous producer");
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
    let journal_dir = tempfile::tempdir().expect("pending-split journal directory");
    queue
        .install_plan_journal(
            journal_dir.path().join("queue-plans.norito"),
            1024 * 1024,
            true,
        )
        .expect("install pending-split queue-plan journal");
    queue
        .install_lane_reservation_journal(
            journal_dir.path().join("lane-reservations.norito"),
            1024 * 1024,
        )
        .expect("install pending-split reservation journal");
    let (_payload, _) = reserve_autonomous_crash_batch(&fixture, &queue, &producer);
    let group = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture pending-split group")
        .ordered_groups
        .into_iter()
        .next()
        .expect("one pending-split group");
    let first_entry = pending_merge_entry(&fixture.context, 0, b"pending split first");
    let second_entry = pending_merge_entry(&fixture.context, 1, b"pending split second");
    let first_hash = HashOf::new(&first_entry);
    let second_hash = HashOf::new(&second_entry);
    let mut by_entrypoint = BTreeMap::new();
    for (index, key) in group.ordered_keys.iter().copied().enumerate() {
        let entry_hash = if index + 1 == group.ordered_keys.len() {
            second_hash
        } else {
            first_hash
        };
        by_entrypoint.insert(key.entrypoint_hash, (entry_hash, key));
    }
    let by_entry = BTreeMap::from([
        (
            first_hash,
            group.ordered_keys[..group.ordered_keys.len() - 1].to_vec(),
        ),
        (
            second_hash,
            group.ordered_keys[group.ordered_keys.len() - 1..].to_vec(),
        ),
    ]);
    assert!(matches!(
        exact_pending_merge_for_group(&group, &by_entrypoint, &by_entry),
        Err(V2ReservationLifecycleError::PendingMergeBindingMismatch {
            lane_id: LaneId::SINGLE,
            proposal_height: 1,
        })
    ));
});
v2_apply_test!(committed_merge_split_carriers_are_rejected, {
    let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
    let producer = KeyPair::try_from_seed(vec![0xBC; 32], Algorithm::BlsNormal)
        .expect("derive committed-split autonomous producer");
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
    let journal_dir = tempfile::tempdir().expect("committed-split journal directory");
    queue
        .install_plan_journal(
            journal_dir.path().join("queue-plans.norito"),
            1024 * 1024,
            true,
        )
        .expect("install committed-split queue-plan journal");
    queue
        .install_lane_reservation_journal(
            journal_dir.path().join("lane-reservations.norito"),
            1024 * 1024,
        )
        .expect("install committed-split reservation journal");
    let (_payload, _) = reserve_autonomous_crash_batch(&fixture, &queue, &producer);
    let group = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture committed-split group")
        .ordered_groups
        .into_iter()
        .next()
        .expect("one committed-split group");
    let carrier_heights = vec![
        BTreeSet::from([NonZeroUsize::new(2).expect("non-zero first carrier")]),
        BTreeSet::from([NonZeroUsize::new(2).expect("non-zero first carrier")]),
        BTreeSet::from([NonZeroUsize::new(3).expect("non-zero split carrier")]),
    ];
    assert!(matches!(
        exact_committed_carrier_height_for_group(&group, &carrier_heights),
        Err(V2ReservationLifecycleError::CommittedCarrierMismatch {
            lane_id: LaneId::SINGLE,
            proposal_height: 1,
        })
    ));
});
include!("v2_apply_unsealed_01b.rs");
