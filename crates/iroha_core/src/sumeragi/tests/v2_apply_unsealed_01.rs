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

    v2_apply_test!(merge_publication_emits_once_across_exact_retry, {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        fixture.execute(&mut store).expect("commit carrier parent");

        let mut entry =
            pending_merge_entry(&fixture.context, 0, b"v2 apply live publication fixture");
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
        let carrier = SignedBlock::from(carrier);
        fixture
            .kura
            .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
            .expect("persist exact merge carrier and sidecar");
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
    });

    v2_apply_test!(
        live_merge_publication_persists_application_receipt_before_retry,
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
            fixture
                .kura
                .store_block(Arc::new(parent.clone()))
                .expect("persist execution-carrier parent");
            fixture
                .kura
                .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
                .expect("persist exact execution carrier and merge log");
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
        let (_parent, entry) =
            merge_entry_with_reservation(&fixture.context, entrypoint, reservation);
        fixture
            .kura
            .append_merge_entry(&entry)
            .expect("persist committed merge history fixture");
        let carrier = body_with_exact_merge_execution_header(&entry);
        fixture.state.record_direct_committed_transactions(
            [reservation.signed_transaction_hash],
            NonZeroUsize::new(1).expect("committed height"),
        );

        assert_eq!(
            finalize_committed_block_merge_reservations(
                fixture.state.as_ref(),
                &queue,
                fixture.kura.as_ref(),
                &carrier,
            )
            .expect("finalize committed merge reservation"),
            1
        );
        assert!(queue.live_lane_reservations().is_empty());
        assert_eq!(
            finalize_committed_block_merge_reservations(
                fixture.state.as_ref(),
                &queue,
                fixture.kura.as_ref(),
                &carrier,
            )
            .expect("repeat exact reservation finalization"),
            0,
            "the post-commit boundary must be idempotent"
        );
    });

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
        fixture.state.record_direct_committed_transactions(
            [reservation.signed_transaction_hash],
            NonZeroUsize::new(1).expect("committed height"),
        );

        let error = finalize_certified_merge_reservations(fixture.state.as_ref(), &queue, &entry)
            .expect_err("bare reservation metadata must fail closed");
        let message = match error {
            V2ReservationLifecycleError::Merge(MergeLedgerCommitError::ExecutionBatchInvalid(
                message,
            )) => message,
            unexpected => panic!("unexpected bare-reservation error: {unexpected}"),
        };
        assert!(
            message.contains("framed Norito"),
            "diagnostic should identify the required framing: {message}"
        );
        assert_eq!(
            queue.live_lane_reservations(),
            vec![reservation],
            "malformed committed evidence must not consume queue ownership"
        );
    });

    v2_apply_test!(
        startup_reconciliation_consumes_replayed_committed_merge_reservation,
        {
            let fixture = ApplyFixture::new();
            let transaction = fixture
                .body
                .external_transactions()
                .next()
                .expect("fixture transaction")
                .clone();
            let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
            let journal_dir = tempfile::tempdir().expect("reservation journal directory");
            let journal_path = journal_dir.path().join("lane-reservations.norito");
            let first_queue = Queue::from_config(QueueConfig::default(), events_sender.clone());
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
            let (reservation, entrypoint) =
                reserve_transaction_for_test(fixture.state.as_ref(), &first_queue, transaction);
            let (parent, entry) =
                merge_entry_with_reservation(&fixture.context, entrypoint, reservation);
            let carrier = body_with_exact_merge_execution_header(&entry);
            fixture
                .kura
                .store_block(Arc::new(parent))
                .expect("persist execution-carrier parent");
            fixture
                .kura
                .store_block_with_merge_entry(Arc::new(carrier), &entry)
                .expect("persist committed merge carrier and exact sidecar");
            drop(first_queue);

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
                .expect("replay reservation QueuePlan claim before pending-tip State repair");
            fixture.state.record_direct_committed_transactions(
                [reservation.signed_transaction_hash],
                NonZeroUsize::new(1).expect("committed height"),
            );
            assert_eq!(replay.restored, 1);
            assert_eq!(replayed_queue.live_lane_reservations(), vec![reservation]);
            assert!(
                replayed_queue.lane_reservation_startup_reconciliation_pending(),
                "replayed committed ownership remains quarantined until State/Kura preflight"
            );
            fixture.kura.reset_merge_query_read_counters_for_test();

            assert_eq!(
                reconcile_lane_reservation_ownership(
                    fixture.state.as_ref(),
                    &replayed_queue,
                    fixture.kura.as_ref(),
                    &verified_context_for_fixture(&fixture, &fixture.context),
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
            let (full_history_scans, _, indexed_lookups) =
                fixture.kura.merge_query_read_counters_for_test();
            assert_eq!(
                full_history_scans, 0,
                "startup reservation reconciliation must not materialize merge history"
            );
            assert_eq!(
                indexed_lookups, 1,
                "startup reconciliation must decode only the exact committed reservation frame"
            );
            assert!(replayed_queue.live_lane_reservations().is_empty());
            assert_eq!(
                reconcile_lane_reservation_ownership(
                    fixture.state.as_ref(),
                    &replayed_queue,
                    fixture.kura.as_ref(),
                    &verified_context_for_fixture(&fixture, &fixture.context),
                )
                .expect("repeat startup reconciliation"),
                LaneReservationReconciliationSummary::default()
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
                fixture.context.chain_id.clone(),
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
            let (parent, first_entry) =
                merge_entry_with_reservation(&fixture.context, first_entrypoint, first);
            let first_carrier = body_with_exact_merge_execution_header(&first_entry);
            fixture
                .kura
                .store_block(Arc::new(parent))
                .expect("persist valid-prefix carrier parent");
            fixture
                .kura
                .store_block_with_merge_entry(Arc::new(first_carrier), &first_entry)
                .expect("persist exact valid-prefix merge binding");
            fixture.state.record_direct_committed_transactions(
                [first.signed_transaction_hash, second.signed_transaction_hash],
                NonZeroUsize::new(1).expect("committed height"),
            );
            let before = queue
                .lane_reservation_reconciliation_snapshot()
                .expect("capture immutable preflight snapshot");

            let error = reconcile_lane_reservation_ownership(
                fixture.state.as_ref(),
                &queue,
                fixture.kura.as_ref(),
                &verified_context_for_fixture(&fixture, &fixture.context),
            )
            .expect_err("missing later merge binding must fail before consuming the valid prefix");
            assert!(matches!(
                error,
                V2ReservationLifecycleError::MissingCommittedBinding { transaction_hash }
                    if transaction_hash == second.signed_transaction_hash
            ));
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

    v2_apply_test!(committed_group_recovery_accepts_exact_forgotten_prefix, {
        let fixture = ApplyFixture::new();
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
                fixture.context.chain_id.clone(),
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
        let keys = members.iter().map(|(_, key)| *key).collect::<Vec<_>>();
        assert_eq!(
            queue
                .commit_lane_reservation(&keys[0])
                .expect("complete the already-forgotten prefix commit"),
            LaneQueueReservationOutcome::Finalized
        );
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("capture committed suffix")
                .ordered_groups[0]
                .ordered_keys,
            keys[1..].to_vec()
        );

        let (parent, entry) = merge_entry_with_reservations(&fixture.context, members);
        let carrier = body_with_exact_merge_execution_header(&entry);
        fixture
            .kura
            .store_block(Arc::new(parent))
            .expect("persist committed suffix carrier parent");
        fixture
            .kura
            .store_block_with_merge_entry(Arc::new(carrier), &entry)
            .expect("persist full committed suffix merge group");
        fixture.state.record_direct_committed_transactions(
            keys.iter().map(|key| key.signed_transaction_hash),
            NonZeroUsize::new(1).expect("committed suffix State height"),
        );

        assert_eq!(
            reconcile_lane_reservation_ownership(
                fixture.state.as_ref(),
                &queue,
                fixture.kura.as_ref(),
                &verified_context_for_fixture(&fixture, &fixture.context),
            )
            .expect("reconcile exact committed suffix"),
            LaneReservationReconciliationSummary {
                recovered: 2,
                finalized_committed: 2,
                ..LaneReservationReconciliationSummary::default()
            }
        );
        assert!(queue.live_lane_reservations().is_empty());
        assert!(queue.lane_reservation_commit_barriers().is_empty());
    });

    v2_apply_test!(mixed_commit_barrier_group_preflights_malformed_later_group, {
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
                fixture.context.chain_id.clone(),
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
            fixture.context.chain_id.clone(),
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
        queue.hold_next_lane_reservation_commit_after_barrier_for_test();
        assert_eq!(
            queue
                .commit_lane_reservation(&first_keys[0])
                .expect("stop first member at durable Commit barrier"),
            LaneQueueReservationOutcome::Finalized
        );
        assert_eq!(
            queue.lane_reservation_commit_barriers(),
            vec![first_keys[0]]
        );

        let (parent, entry) =
            merge_entry_with_reservations(&fixture.context, first_transactions);
        let carrier = body_with_exact_merge_execution_header(&entry);
        fixture
            .kura
            .store_block(Arc::new(parent))
            .expect("persist mixed commit barrier carrier parent");
        fixture
            .kura
            .store_block_with_merge_entry(Arc::new(carrier), &entry)
            .expect("persist exact first committed group");
        fixture.state.record_direct_committed_transactions(
            first_keys
                .iter()
                .map(|key| key.signed_transaction_hash)
                .chain([later_key.signed_transaction_hash]),
            NonZeroUsize::new(1).expect("mixed committed State height"),
        );
        let before = queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture mixed owner preflight snapshot");
        let barriers_before = queue.lane_reservation_commit_barriers();

        let error = reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            &queue,
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &fixture.context),
        )
        .expect_err("malformed later group must stop before consuming mixed first group");
        assert!(matches!(
            error,
            V2ReservationLifecycleError::MissingCommittedBinding { transaction_hash }
                if transaction_hash == later_key.signed_transaction_hash
        ));
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("capture mixed owner post-error snapshot"),
            before
        );
        assert_eq!(queue.lane_reservation_commit_barriers(), barriers_before);
    });

    v2_apply_test!(replayed_mixed_commit_barrier_group_reopens_startup_gate, {
        let fixture = ApplyFixture::new();
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Queue::from_config(QueueConfig::default(), events_sender.clone());
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
                fixture.context.chain_id.clone(),
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
        let keys = transactions
            .iter()
            .map(|(_, key)| *key)
            .collect::<Vec<_>>();
        queue.hold_next_lane_reservation_commit_after_barrier_for_test();
        queue
            .commit_lane_reservation(&keys[0])
            .expect("retain exact replayed Commit barrier");
        let (parent, entry) = merge_entry_with_reservations(&fixture.context, transactions);
        let carrier = body_with_exact_merge_execution_header(&entry);
        fixture
            .kura
            .store_block(Arc::new(parent))
            .expect("persist replayed commit barrier parent");
        fixture
            .kura
            .store_block_with_merge_entry(Arc::new(carrier), &entry)
            .expect("persist replayed commit barrier merge group");
        fixture.state.record_direct_committed_transactions(
            keys.iter().map(|key| key.signed_transaction_hash),
            NonZeroUsize::new(1).expect("replayed commit barrier State height"),
        );
        drop(queue);

        let queue = Queue::from_config(QueueConfig::default(), events_sender);
        let replay = queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("replay mixed Commit/live owner group");
        assert_eq!(replay.restored, 1);
        assert_eq!(replay.commit_barriers, 1);
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
                &verified_context_for_fixture(&fixture, &fixture.context),
            )
            .expect("complete mixed Commit/live recovery"),
            LaneReservationReconciliationSummary {
                recovered: 2,
                finalized_committed: 1,
                ..LaneReservationReconciliationSummary::default()
            }
        );
        assert!(queue.live_lane_reservations().is_empty());
        assert!(queue.lane_reservation_commit_barriers().is_empty());
        assert!(!queue.lane_reservation_startup_reconciliation_pending());
    });

    v2_apply_test!(startup_reconciliation_rejects_partial_state_group_without_mutation, {
        let fixture = ApplyFixture::new();
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
        fixture.state.record_direct_committed_transactions(
            [keys[0].signed_transaction_hash],
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
        assert!(matches!(
            error,
            V2ReservationLifecycleError::PartialCommittedGroup {
                lane_id: LaneId::SINGLE,
                proposal_height: 1,
            }
        ));
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("capture partial-state post-error snapshot"),
            before
        );
        assert!(queue.lane_reservation_commit_barriers().is_empty());
        assert!(queue.lane_reservation_release_barriers().is_empty());
    });

    v2_apply_test!(strict_absence_releases_original_fifo_not_digest_order, {
        let fixture = ApplyFixture::new();
        let producer = KeyPair::try_from_seed(vec![0xB9; 32], Algorithm::BlsNormal)
            .expect("derive strict-absence autonomous producer");
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
        let journal_dir = tempfile::tempdir().expect("strict-absence journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install strict-absence queue-plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install strict-absence reservation journal");
        let (payload, mut expected_fifo) =
            reserve_autonomous_crash_batch(&fixture, &queue, &producer);
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
                fixture.context.chain_id.clone(),
                fixture.service.genesis_account.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(
                Level::INFO,
                format!("strict FIFO digest-order discriminator {index}"),
            )])
            .sign(fixture.genesis_key.private_key());
            expected_fifo.push(transaction.hash());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
            let routing_plan = queue
                .route_plan_with_state(&accepted, fixture.state.as_ref())
                .expect("resolve strict FIFO discriminator route");
            let admission_context = queue
                .plan_admission_context_with_state(fixture.state.as_ref(), &routing_plan)
                .expect("capture strict FIFO discriminator admission context");
            let binding = crate::torii_proxy::QueuePlanAdmissionBindingV2::new(
                fixture.state.chain_id_ref(),
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
                .map(|transaction| transaction.as_ref().hash())
                .collect::<Vec<_>>(),
            expected_fifo,
            "strict absence must restore the exact pre-reservation FIFO sequence"
        );
    });

    v2_apply_test!(finalized_hash_only_carrier_fails_closed_without_releasing_owner, {
        let fixture = ApplyFixture::new();
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
            .persist_lane_executable_payload(&payload, payload.chain_id_hash, payload.epoch)
            .expect("persist exact pruned-carrier payload");
        let mut store = fixture.reopen_body_store();
        fixture
            .execute(&mut store)
            .expect("finalize carrier before body pruning");
        fixture
            .kura
            .force_hash_only_block_for_testing(
                NonZeroUsize::new(1).expect("non-zero pruned carrier height"),
            )
            .expect("evict exact finalized carrier body");
        let before = queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture pruned-carrier ownership snapshot");

        let error = reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &fixture.context),
        )
        .expect_err("hash-only finality cannot prove terminal payload absence");
        assert!(matches!(
            error,
            V2ReservationLifecycleError::MissingCanonicalBody { height: 1 }
        ));
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("capture pruned-carrier post-error snapshot"),
            before
        );
        assert!(
            fixture
                .kura
                .read_autonomous_lane_slot_retirement(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    payload.chain_id_hash,
                    payload.epoch,
                )
                .expect("read pruned-carrier retirement state")
                .is_none()
        );
    });

    v2_apply_test!(canonical_exact_certified_autonomous_group_is_retained, {
        let fixture = ApplyFixture::new();
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
            .expect("install canonical autonomous lane marker");
        fixture
            .kura
            .persist_lane_executable_payload(&payload, payload.chain_id_hash, payload.epoch)
            .expect("persist canonical autonomous payload");
        let envelope = crate::lane_consensus::autonomous_lane_payload_envelope(
            &payload,
            payload.chain_id_hash,
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
                    payload.chain_id_hash,
                    payload.epoch,
                )
                .expect("read canonical certified retirement state")
                .is_none()
        );
    });

    v2_apply_test!(replayed_current_autonomous_group_reopens_startup_gate, {
        let fixture = ApplyFixture::new();
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
        let (payload, _) =
            reserve_canonical_successor_autonomous_batch(&fixture, &queue, &context, 2);
        let unreserved_transaction = TransactionBuilder::new(
            fixture.context.chain_id.clone(),
            fixture.service.genesis_account.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "current recovery startup-gate probe".to_owned(),
        )])
        .sign(fixture.genesis_key.private_key());
        let unreserved_hash = unreserved_transaction.hash();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(unreserved_transaction));
        let routing_plan = queue
            .route_plan_with_state(&accepted, fixture.state.as_ref())
            .expect("resolve startup-gate probe route");
        let admission_context = queue
            .plan_admission_context_with_state(fixture.state.as_ref(), &routing_plan)
            .expect("capture startup-gate probe admission context");
        let binding = crate::torii_proxy::QueuePlanAdmissionBindingV2::new(
            fixture.state.chain_id_ref(),
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
            .persist_lane_executable_payload(&payload, payload.chain_id_hash, payload.epoch)
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

    v2_apply_test!(prior_height_canonical_uncertified_owner_requires_historical_recovery, {
        let fixture = ApplyFixture::new();
        let mut genesis_store = fixture.reopen_body_store();
        fixture
            .execute(&mut genesis_store)
            .expect("commit parent before historical autonomous successor");
        let context = successor_height_context(&fixture);
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
        let journal_dir = tempfile::tempdir().expect("historical autonomous journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install historical autonomous queue-plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
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
            .persist_lane_executable_payload(&payload, payload.chain_id_hash, payload.epoch)
            .expect("persist historical autonomous payload");
        let envelope = crate::lane_consensus::autonomous_lane_payload_envelope(
            &payload,
            payload.chain_id_hash,
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
        let before = queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture historical ownership snapshot");

        let error = reconcile_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &verified_context_for_fixture(&fixture, &next_context),
        )
        .expect_err("prior-height exact carrier without certification needs bounded recovery");
        assert!(matches!(
            error,
            V2ReservationLifecycleError::HistoricalCarrierRecoveryRequired {
                lane_id: LaneId::SINGLE,
                height: 2,
            }
        ));
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("capture historical post-error ownership snapshot"),
            before
        );
        assert!(
            fixture
                .kura
                .read_autonomous_lane_slot_retirement(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    payload.chain_id_hash,
                    payload.epoch,
                )
                .expect("read historical retirement state")
                .is_none()
        );
    });

    v2_apply_test!(pending_merge_split_group_is_rejected, {
        let fixture = ApplyFixture::new();
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
        let mut by_transaction = BTreeMap::new();
        for (index, key) in group.ordered_keys.iter().copied().enumerate() {
            let entry_hash = if index + 1 == group.ordered_keys.len() {
                second_hash
            } else {
                first_hash
            };
            by_transaction.insert(key.signed_transaction_hash, (entry_hash, key));
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
            exact_pending_merge_for_group(&group, &by_transaction, &by_entry),
            Err(V2ReservationLifecycleError::PendingMergeBindingMismatch {
                lane_id: LaneId::SINGLE,
                proposal_height: 1,
            })
        ));
    });

    v2_apply_test!(committed_merge_split_carriers_are_rejected, {
        let fixture = ApplyFixture::new();
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

    v2_apply_test!(canonical_overlap_detects_same_transaction_under_substituted_key, {
        let fixture = ApplyFixture::new();
        let producer = KeyPair::try_from_seed(vec![0xBD; 32], Algorithm::BlsNormal)
            .expect("derive canonical-overlap autonomous producer");
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
        let journal_dir = tempfile::tempdir().expect("canonical-overlap journal directory");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install canonical-overlap queue-plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install canonical-overlap reservation journal");
        let (mut substituted, _) = reserve_autonomous_crash_batch(&fixture, &queue, &producer);
        let snapshot = queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture canonical-overlap ownership snapshot");
        let group = snapshot
            .ordered_groups
            .first()
            .expect("one canonical-overlap group");
        substituted.reservation_keys[0].queue_plan_admission_binding_hash =
            Hash::new(b"substituted canonical QueuePlan binding");

        assert!(
            autonomous_payload_overlaps_group_transaction_identity(&substituted, group),
            "same signed transaction or typed entrypoint must make a substituted key relevant"
        );
        assert!(
            !canonical_payload_contains_group_in_order(&substituted, group),
            "the substituted key must remain ineligible for exact canonical classification"
        );
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("capture canonical-overlap post-check snapshot"),
            snapshot,
            "conflict preflight must not mutate Queue ownership"
        );
        assert!(queue.lane_reservation_commit_barriers().is_empty());
        assert!(queue.lane_reservation_release_barriers().is_empty());
    });

    v2_apply_test!(
        autonomous_reservation_cross_store_crash_matrix_preserves_fifo_exactly_once,
        {
            for boundary in [
                "before_kura_retirement",
                "kura_release_pending",
                "queue_prepared_barrier",
                "kura_released",
                "queue_completion_forgotten",
            ] {
                let fixture = ApplyFixture::new();
                let producer = KeyPair::try_from_seed(vec![0xB7; 32], Algorithm::BlsNormal)
                    .expect("derive autonomous crash producer");
                let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
                let journal_dir =
                    tempfile::tempdir().expect("autonomous reservation crash journals");
                let plan_path = journal_dir.path().join("queue-plans.norito");
                let reservation_path = journal_dir.path().join("lane-reservations.norito");
                let queue = Arc::new(Queue::from_config(
                    QueueConfig::default(),
                    events_sender.clone(),
                ));
                queue
                    .install_plan_journal(&plan_path, 1024 * 1024, true)
                    .expect("install autonomous crash plan journal");
                queue
                    .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
                    .expect("install autonomous crash reservation journal");
                let (payload, expected_fifo) =
                    reserve_autonomous_crash_batch(&fixture, &queue, &producer);
                let descriptor = &payload.origin_proposal.descriptor;
                let lane_config = RuntimeLaneConfig::default();
                fixture
                    .kura
                    .install_lane_incarnation_marker_for_test(
                        lane_config.primary(),
                        descriptor.lane_incarnation,
                        0,
                    )
                    .expect("install autonomous crash lane marker");
                fixture
                    .kura
                    .persist_lane_executable_payload(&payload, payload.chain_id_hash, payload.epoch)
                    .expect("persist autonomous crash payload");
                let mut global_body_store = fixture.reopen_body_store();
                fixture
                    .execute(&mut global_body_store)
                    .expect("finalize the exact global body which omitted the losing payload");
                let retirement =
                    crate::kura::AutonomousLaneSlotRetirementV1::from_payload(&payload);
                let barrier = retirement
                    .queue_release_barrier()
                    .expect("build autonomous crash release barrier");

                match boundary {
                    "before_kura_retirement" => {}
                    "kura_release_pending" => {
                        fixture
                            .kura
                            .persist_autonomous_lane_slot_retirement(
                                &retirement,
                                payload.chain_id_hash,
                                payload.epoch,
                            )
                            .expect("persist Kura ReleasePending boundary");
                    }
                    "queue_prepared_barrier" => {
                        fixture
                            .kura
                            .persist_autonomous_lane_slot_retirement(
                                &retirement,
                                payload.chain_id_hash,
                                payload.epoch,
                            )
                            .expect("persist Kura retirement before Queue barrier");
                        queue
                            .prepare_lane_reservation_release_barrier(&barrier)
                            .expect("persist Queue prepared barrier");
                    }
                    "kura_released" => {
                        fixture
                            .kura
                            .persist_autonomous_lane_slot_retirement(
                                &retirement,
                                payload.chain_id_hash,
                                payload.epoch,
                            )
                            .expect("persist Kura retirement before released claims");
                        queue
                            .prepare_lane_reservation_release_barrier(&barrier)
                            .expect("persist Queue barrier before Kura Released");
                        fixture
                            .kura
                            .finalize_autonomous_lane_slot_release(
                                &retirement,
                                &barrier,
                                payload.chain_id_hash,
                                payload.epoch,
                            )
                            .expect("persist Kura Released boundary");
                    }
                    "queue_completion_forgotten" => {
                        assert_eq!(
                            retire_autonomous_lane_slot_and_release_reservations(
                                fixture.kura.as_ref(),
                                queue.as_ref(),
                                &retirement,
                                payload.chain_id_hash,
                                payload.epoch,
                            )
                            .expect("complete production retirement hand-off"),
                            3
                        );
                    }
                    _ => unreachable!("enumerated crash boundary"),
                }
                assert_eq!(
                    fixture
                        .kura
                        .read_autonomous_lane_slot_retirement(
                            descriptor.lane_id,
                            descriptor.lane_block_height,
                            payload.chain_id_hash,
                            payload.epoch,
                        )
                        .expect("read crash-boundary retirement"),
                    (boundary != "before_kura_retirement").then(|| retirement.clone()),
                    "{boundary}: only post-retirement crash images may start with a Kura tombstone"
                );
                if boundary == "queue_completion_forgotten" {
                    assert!(queue.live_lane_reservations().is_empty());
                    assert!(queue.lane_reservation_release_barriers().is_empty());
                    assert_eq!(queue.queued_len(), 4);
                } else {
                    assert_eq!(queue.live_lane_reservations().len(), 3);
                    assert_eq!(queue.queued_len(), 1);
                    assert_eq!(
                        queue.lane_reservation_release_barriers(),
                        if matches!(boundary, "before_kura_retirement" | "kura_release_pending") {
                            Vec::new()
                        } else {
                            vec![barrier.clone()]
                        },
                        "{boundary}: Queue must expose exactly its durable crash boundary"
                    );
                }
                drop(queue);

                let replayed_queue = Arc::new(Queue::from_config(
                    QueueConfig::default(),
                    events_sender.clone(),
                ));
                let replay = replayed_queue
                    .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
                    .expect("replay first crash-boundary reservation journal");
                replayed_queue
                    .install_plan_journal(&plan_path, 1024 * 1024, true)
                    .expect("install first replay plan journal");
                replayed_queue
                    .replay_plan_journal(fixture.state.as_ref())
                    .expect("replay first crash-boundary queue plans");
                if boundary == "queue_completion_forgotten" {
                    assert_eq!(replay.restored, 0);
                    assert_eq!(replay.release_barriers, 0);
                    assert_eq!(replay.completed_releases, 0);
                } else {
                    assert_eq!(replay.restored, 3);
                    assert_eq!(
                        replay.release_barriers,
                        usize::from(matches!(
                            boundary,
                            "queue_prepared_barrier" | "kura_released"
                        ))
                    );
                    assert_eq!(replay.completed_releases, 0);
                    assert_eq!(
                        replayed_queue.queued_len(),
                        1,
                        "{boundary}: replay must not make lane-owned work selectable"
                    );
                }
                assert_eq!(
                    replayed_queue.lane_reservation_startup_reconciliation_pending(),
                    boundary != "queue_completion_forgotten",
                    "{boundary}: replay quarantine must cover every durable release owner"
                );
                assert_eq!(
                    reconcile_lane_reservation_ownership(
                        fixture.state.as_ref(),
                        replayed_queue.as_ref(),
                        fixture.kura.as_ref(),
                        &verified_context_for_fixture(&fixture, &fixture.context),
                    )
                    .expect("reconcile first cross-store crash image"),
                    if boundary == "queue_completion_forgotten" {
                        LaneReservationReconciliationSummary::default()
                    } else if boundary == "before_kura_retirement" {
                        LaneReservationReconciliationSummary {
                            recovered: 3,
                            released_terminal_loser: 3,
                            ..LaneReservationReconciliationSummary::default()
                        }
                    } else {
                        LaneReservationReconciliationSummary {
                            recovered: 3,
                            resumed_retirement: 3,
                            ..LaneReservationReconciliationSummary::default()
                        }
                    },
                    "{boundary}: reconciliation must transfer each owner exactly once"
                );
                assert!(
                    !replayed_queue.lane_reservation_startup_reconciliation_pending(),
                    "{boundary}: successful release reconciliation must publish the Queue startup gate"
                );
                assert!(replayed_queue.live_lane_reservations().is_empty());
                assert!(
                    replayed_queue
                        .lane_reservation_release_barriers()
                        .is_empty()
                );
                assert_eq!(
                    replayed_queue.queued_len(),
                    4,
                    "{boundary}: release must restore all work without loss"
                );
                assert_eq!(
                    retire_autonomous_lane_slot_and_release_reservations(
                        fixture.kura.as_ref(),
                        replayed_queue.as_ref(),
                        &retirement,
                        payload.chain_id_hash,
                        payload.epoch,
                    )
                    .expect("retry complete production retirement hand-off"),
                    0,
                    "{boundary}: completed hand-off replay must be idempotent"
                );
                drop(replayed_queue);

                let replayed_again = Arc::new(Queue::from_config(
                    QueueConfig::default(),
                    events_sender.clone(),
                ));
                let terminal_replay = replayed_again
                    .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
                    .expect("replay terminal reservation journal");
                assert_eq!(terminal_replay.restored, 0);
                assert_eq!(terminal_replay.release_barriers, 0);
                assert_eq!(terminal_replay.completed_releases, 0);
                replayed_again
                    .install_plan_journal(&plan_path, 1024 * 1024, true)
                    .expect("install terminal replay plan journal");
                replayed_again
                    .replay_plan_journal(fixture.state.as_ref())
                    .expect("replay terminal queue plans");
                assert_eq!(
                    reconcile_lane_reservation_ownership(
                        fixture.state.as_ref(),
                        replayed_again.as_ref(),
                        fixture.kura.as_ref(),
                        &verified_context_for_fixture(&fixture, &fixture.context),
                    )
                    .expect("repeat terminal ownership reconciliation"),
                    LaneReservationReconciliationSummary::default(),
                    "{boundary}: a second restart must not release or commit twice"
                );
                assert_eq!(replayed_again.active_len(), 4);
                assert_eq!(replayed_again.queued_len(), 4);
                assert!(replayed_again.live_lane_reservations().is_empty());

                let replacement_scope = LaneQueueReservationScopeV1 {
                    lane_id: descriptor.lane_id,
                    dataspace_id: descriptor.dataspace_id,
                    lane_incarnation: fixture
                        .state
                        .lane_incarnation_at_height(descriptor.lane_id, 2)
                        .expect("active incarnation for replacement proposal"),
                    proposal_height: 2,
                    lane_block_height: 2,
                    lane_block_view: 0,
                    reservation_owner_hash: Hash::new(
                        format!("replacement owner after {boundary}").as_bytes(),
                    ),
                    proposal_identity_hash: Hash::new(
                        format!("replacement proposal after {boundary}").as_bytes(),
                    ),
                };
                let replacement = replayed_again
                    .reserve_transactions_for_lane(
                        fixture.state.as_ref(),
                        replacement_scope,
                        NonZeroUsize::new(1).expect("one replacement reservation"),
                    )
                    .expect("reserve restored FIFO head under a fresh owner");
                assert_eq!(replacement.len(), 1);
                assert_eq!(
                    replacement[0].key().signed_transaction_hash,
                    expected_fifo[0],
                    "{boundary}: restored work must not be overtaken"
                );
                assert_ne!(
                    replacement[0].key(),
                    &barrier.ordered_keys[0],
                    "{boundary}: replacement ownership must have a fresh exact identity"
                );
                let stale_barrier =
                    replayed_again.prepare_lane_reservation_release_barrier(&barrier);
                assert!(
                    matches!(
                        &stale_barrier,
                        Err(LaneQueueReservationError::Conflict { .. })
                    ),
                    "{boundary}: stale release barrier must fail ABA-safe: {stale_barrier:?}"
                );
                assert_eq!(
                    replayed_again.live_lane_reservations(),
                    vec![*replacement[0].key()],
                    "{boundary}: stale barrier must not disturb replacement ownership"
                );
                assert_eq!(replayed_again.active_len(), 4);
                assert_eq!(replayed_again.queued_len(), 3);

                let mut remaining = Vec::new();
                replayed_again.get_transactions_for_block_with_state(
                    fixture.state.as_ref(),
                    NonZeroUsize::new(3).expect("remaining FIFO length"),
                    &mut remaining,
                );
                let remaining_hashes = remaining
                    .iter()
                    .map(|transaction| transaction.as_ref().hash())
                    .collect::<Vec<_>>();
                assert_eq!(
                    remaining_hashes,
                    expected_fifo[1..],
                    "{boundary}: release replay must preserve FIFO order behind the new owner"
                );
                let observed = std::iter::once(replacement[0].key().signed_transaction_hash)
                    .chain(remaining_hashes)
                    .collect::<BTreeSet<_>>();
                assert_eq!(observed.len(), 4);
                assert_eq!(
                    observed,
                    expected_fifo.iter().copied().collect(),
                    "{boundary}: restart/release must neither lose nor duplicate ownership"
                );
            }
        }
    );

    v2_apply_test!(
        durable_decision_retains_exact_earlier_view_sidecar_and_prunes_losers,
        {
            let fixture = ApplyFixture::new();
            let exact = pending_merge_entry(&fixture.context, 1, b"exact earlier-view sidecar");
            let losing = pending_merge_entry(&fixture.context, 2, b"losing later-view sidecar");
            let exact_hash = fixture
                .kura
                .persist_pending_certified_merge_entry(&exact)
                .expect("persist exact decided sidecar");
            let losing_hash = fixture
                .kura
                .persist_pending_certified_merge_entry(&losing)
                .expect("persist losing sidecar");
            assert_ne!(exact_hash, losing_hash);

            let body = body_with_merge_reference(CertifiedMergeLedgerReference::new(&exact));
            fixture
                .service
                .retain_decided_merge_sidecar(&fixture.context, &body)
                .expect("bind exact sidecar from durable decided body");
            assert_eq!(
                fixture
                    .kura
                    .merge_entry_by_hash(exact_hash)
                    .expect("read exact sidecar after decision binding"),
                Some(exact),
                "the exact earlier-view reference remains protected until finalization"
            );
            assert!(
                fixture
                    .kura
                    .merge_entry_by_hash(losing_hash)
                    .expect("read losing sidecar after decision binding")
                    .is_none(),
                "a durable decision must release every non-referenced sidecar at its height"
            );

            fixture
                .kura
                .prune_finalized_pending_certified_merge_entries(fixture.context.height)
                .expect("finalized height retires the exact protected sidecar");
            assert!(
                fixture
                    .kura
                    .merge_entry_by_hash(exact_hash)
                    .expect("read exact sidecar after finalization")
                    .is_none()
            );
        }
    );

    v2_apply_test!(forged_commit_qc_is_rejected_before_any_durable_mutation, {
        let fixture = ApplyFixture::new();
        let pending = pending_merge_entry(
            &fixture.context,
            2,
            b"pending sidecar must survive unauthenticated Apply",
        );
        let pending_hash = fixture
            .kura
            .persist_pending_certified_merge_entry(&pending)
            .expect("persist pending sidecar before forged Apply");
        let baseline_state_hash =
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref());

        let mut forged_certificate = fixture.task.certificate().clone();
        let first_signature_byte = forged_certificate
            .aggregate_signature
            .first_mut()
            .expect("fixture CommitQC aggregate signature");
        *first_signature_byte ^= 0x80;
        let forged_task = ApplyTask::for_test(
            2,
            fixture.task.tag(),
            fixture.task.subject(),
            forged_certificate,
            fixture.task.validated_receipt().clone(),
        );
        let mut store = fixture.reopen_body_store();

        assert!(matches!(
            fixture
                .service
                .execute(&fixture.context, &mut store, &forged_task),
            Err(V2ApplyError::FinalityCryptography(
                wire::finality::V2QuorumCertificateVerificationError::InvalidAggregateSignature
            ))
        ));
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()),
            baseline_state_hash,
            "an unauthenticated decision must not mutate WSV"
        );
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 0);
        fixture.assert_no_post_apply_sidecars();
        assert_eq!(
            fixture
                .kura
                .merge_entry_by_hash(pending_hash)
                .expect("read pending sidecar after forged Apply"),
            Some(pending),
            "finality verification must precede pending-sidecar pruning"
        );
    });

    v2_apply_test!(
        invalid_commit_aggregate_is_rejected_before_kura_or_wsv_mutation,
        {
            let fixture = ApplyFixture::new();
            let mut certificate = fixture.task.certificate().clone();
            certificate.aggregate_signature[0] ^= 0x80;
            let task = ApplyTask::for_test(
                2,
                fixture.task.tag(),
                fixture.task.subject(),
                certificate,
                fixture.task.validated_receipt().clone(),
            );
            let mut store = fixture.reopen_body_store();

            assert!(matches!(
                fixture.service.execute(&fixture.context, &mut store, &task),
                Err(V2ApplyError::FinalityCryptography(
                    wire::finality::V2QuorumCertificateVerificationError::InvalidAggregateSignature
                ))
            ));
            fixture.assert_no_apply_mutation();
        }
    );

    v2_apply_test!(
        same_body_reproposal_commit_qc_applies_exact_reproposal_body,
        {
            let mut fixture = ApplyFixture::new();
            let mut keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic BLS key")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let mut certificate = fixture.task.certificate().clone();
            certificate.round.view = fixture.body.header().view_change_index().saturating_add(1);
            certificate.proposal_round = certificate.round;
            let preimage = wire::Vote {
                round: certificate.round,
                proposal_round: certificate.proposal_round,
                phase: certificate.phase,
                subject: certificate.subject,
                execution_commitment: certificate.execution_commitment,
                signer: certificate.signers[0],
                signature: Vec::new(),
            }
            .signature_preimage();
            let signatures = certificate
                .signers
                .iter()
                .map(|index| {
                    Signature::try_new(
                        keys[usize::try_from(*index).expect("fixture signer index")].private_key(),
                        &preimage,
                    )
                    .expect("sign same-round reproposal Commit vote")
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
                &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate same-round reproposal Commit votes");
            let later_round = certificate.round;
            let later_tag = EventTag::new(
                fixture.context.height,
                later_round.view,
                fixture.task.tag().generation(),
            );
            let mut store = fixture.reopen_body_store();
            let canonical_wire = fixture
                .body
                .encode_wire()
                .expect("encode unchanged locked body");
            let reproposal_manifest = wire::PayloadManifest::derive(
                &fixture.context,
                later_round,
                fixture.task.subject(),
                u64::try_from(canonical_wire.len()).expect("body length"),
                std::slice::from_ref(&canonical_wire),
            )
            .expect("derive later-round manifest for unchanged locked body");
            let durable = store
                .store(reproposal_manifest, canonical_wire.clone())
                .expect("persist later-round manifest for unchanged locked body");
            let reproposal_receipt = store
                .validate(&durable, |candidate| {
                    assert_eq!(
                        candidate
                            .encode_wire()
                            .expect("encode reproposed candidate"),
                        canonical_wire,
                        "reproposal must retain the exact canonical locked body bytes"
                    );
                    fixture
                        .service
                        .validate_candidate(&fixture.context, candidate)
                })
                .expect("validate unchanged body under the later proposal round");
            assert_eq!(
                reproposal_receipt.durable().round(),
                certificate.proposal_round,
                "the durable receipt must bind the unchanged body to its later reproposal round"
            );
            assert_eq!(
                reproposal_receipt.execution_commitment(),
                fixture.task.validated_receipt().execution_commitment(),
                "view rotation must not change deterministic execution"
            );
            let task = ApplyTask::for_test(
                2,
                later_tag,
                fixture.task.subject(),
                certificate,
                reproposal_receipt,
            );
            fixture.task = task;

            fixture
                .execute(&mut store)
                .expect("reproposal CommitQC applies the exact unchanged body");
            fixture.assert_complete();
        }
    );

    v2_apply_test!(
        invalid_non_signer_durable_pop_is_rejected_before_kura_or_wsv_mutation,
        {
            let mut fixture = ApplyFixture::new();
            fixture.service.validator_set_pops[3][0] ^= 0x80;
            let mut store = fixture.reopen_body_store();

            assert!(matches!(
            fixture.execute(&mut store),
            Err(V2ApplyError::FinalityCryptography(
                wire::finality::V2QuorumCertificateVerificationError::InvalidProofOfPossession {
                    index: 3
                }
            ))
        ));
            fixture.assert_no_apply_mutation();
        }
    );

    v2_apply_test!(block_write_failure_never_advances_wsv_and_retry_is_exact, {
        let fixture = ApplyFixture::new();
        let baseline_state_hash =
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref());
        let mut store = fixture.reopen_body_store();
        fixture.kura.fail_next_block_write_for_tests();
        assert!(matches!(
            fixture.execute(&mut store),
            Err(V2ApplyError::Kura(_))
        ));
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()),
            baseline_state_hash,
            "a failed Kura write must not leak any WSV mutation"
        );
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 0);
        fixture.assert_no_post_apply_sidecars();

        drop(store);
        let mut reopened = fixture.reopen_body_store();
        fixture
            .execute(&mut reopened)
            .expect("retry exact apply after reopening the durable body store");
        fixture.assert_complete();
        let view = fixture.state.view();
        let sumeragi = view.world().parameters().sumeragi();
        assert_eq!(sumeragi.block_cadence_ms().get(), 100);
    });

    v2_apply_test!(height_one_lane_exemption_never_accepts_empty_genesis, {
        let fixture = ApplyFixture::new();
        let proof_policy_bundle = crate::da::active_proof_policy_bundle_at_height(
            &fixture.state.nexus_snapshot(),
            fixture.context.height,
        );
        let invalid = BlockBuilder::new_with_time_source(Vec::new(), TimeSource::new_system())
            .chain(0, None)
            .with_da_proof_policies(Some(proof_policy_bundle))
            .try_sign_with_index(fixture.genesis_key.private_key(), 0)
            .expect("sign empty genesis negative fixture")
            .unpack(|_| {});
        let error = fixture
            .service
            .validate_candidate(&fixture.context, &SignedBlock::from(invalid))
            .expect_err("canonical genesis validation must reject an empty body");
        assert!(
            matches!(&error, V2ApplyError::Validation(message) if message.contains("must have 1 to 16 transactions")),
            "unexpected empty-genesis rejection: {error}"
        );
    });

    v2_apply_test!(
        validation_error_classification_handles_body_without_results,
        {
            let key = KeyPair::try_from_seed(vec![0xD4; 32], Algorithm::Ed25519)
                .expect("derive malformed-body signer");
            let body = SignedBlock::from(
                BlockBuilder::new_with_time_source(Vec::new(), TimeSource::new_system())
                    .chain(0, None)
                    .try_sign_with_index(key.private_key(), 0)
                    .expect("sign no-results body")
                    .unpack(|_| {}),
            );
            assert!(!body.has_results());
            let error = V2ApplyService::classify_candidate_validation_error(
                None,
                &body,
                &BlockValidationError::EmptyBlock,
            );
            assert!(
                matches!(error, V2ApplyError::Validation(message) if message.contains("no committed overlays"))
            );
        }
    );

    v2_apply_test!(
        validation_error_classification_redacts_internal_result_details,
        {
            let fixture = ApplyFixture::new();
            let mut rejected = fixture.body.clone();
            let entry_hashes = rejected
                .external_entrypoints_cloned()
                .map(|entrypoint| entrypoint.hash())
                .collect::<Vec<_>>();
            let secret = "sensitive executor diagnostic";
            let result: TransactionResultInner = Err(TransactionRejectionReason::Validation(
                ValidationFail::InternalError(secret.to_owned()),
            ));
            rejected
                .set_transaction_results(Vec::new(), &entry_hashes, vec![result])
                .expect("attach one rejected result");
            let error = V2ApplyService::classify_candidate_validation_error(
                None,
                &rejected,
                &BlockValidationError::EmptyBlock,
            );
            let V2ApplyError::Validation(message) = error else {
                panic!("unexpected classification")
            };
            assert!(message.contains("rejected transaction result count: 1"));
            assert!(!message.contains(secret));
        }
    );

    v2_apply_test!(
        post_genesis_external_body_without_execution_context_is_rejected,
        {
            let fixture = ApplyFixture::new();
            let mut post_genesis_context = fixture.context.clone();
            post_genesis_context.height = 2;
            let error = fixture
                .service
                .validate_lane_payload_plan(&post_genesis_context, &fixture.body)
                .expect_err("the height-one lane-plan exemption must never apply post-genesis");
            assert!(
                matches!(&error, V2ApplyError::Validation(message) if message.contains("external entrypoints without execution context")),
                "unexpected post-genesis lane-plan rejection: {error}"
            );
        }
    );

    v2_apply_test!(restart_recovers_kura_block_written_before_wsv_commit, {
        let fixture = ApplyFixture::new();
        let baseline_state_hash =
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref());
        let mut store = fixture.reopen_body_store();
        fixture.service.fail_after_kura_store_for_test();
        assert!(matches!(
            fixture.execute(&mut store),
            Err(V2ApplyError::InjectedCrashAfterKuraStore)
        ));
        drop(store);
        let durable = fixture
            .kura
            .get_block(NonZeroUsize::new(1).expect("height"))
            .expect("read production-validated Kura crash image");
        assert!(durable.has_results());
        assert_eq!(durable.results().len(), 1);
        assert!(durable.results().all(|result| result.is_ok()));
        let durable_wire = durable.encode_wire().expect("encode Kura crash image");
        fixture.assert_no_post_apply_sidecars();
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()),
            baseline_state_hash,
            "the Kura-first crash boundary must not leak partial WSV state"
        );

        let mut store = fixture.reopen_body_store();
        fixture
            .execute(&mut store)
            .expect("resume WSV application from exact durable body");
        fixture.assert_complete();
        assert_eq!(
            fixture
                .kura
                .get_block(NonZeroUsize::new(1).expect("height"))
                .expect("read recovered Kura block")
                .encode_wire()
                .expect("encode recovered Kura block"),
            durable_wire,
            "an exact retry must preserve the complete canonical Kura wire"
        );
    });

    v2_apply_test!(native_amx_prepublication_failure_leaves_wsv_unchanged, {
        let fixture = ApplyFixture::new();
        let baseline_state_hash =
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref());
        fixture.kura.fail_next_native_amx_prepublication_for_tests();
        let mut store = fixture.reopen_body_store();
        let error = fixture
            .execute(&mut store)
            .expect_err("inject Native evidence publication failure before WSV staging");
        assert!(matches!(
            &error,
            V2ApplyError::CommittedRecoveryRequired {
                stage: "pre-WSV Native AMX participant evidence publication",
                ..
            }
        ));
        assert!(error.requires_restart_recovery());
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()),
            baseline_state_hash,
            "prepublication failure must not leak the validated State overlay"
        );
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
        assert!(
            fixture
                .kura
                .v2_finality_artifact(fixture.context.height)
                .expect("read pre-WSV finality")
                .is_some(),
            "durable finality must precede Native evidence prepublication"
        );
        assert!(
            fixture
                .kura
                .wsv_checkpoint(fixture.context.height)
                .expect("read absent pre-WSV checkpoint")
                .is_none()
        );
        assert!(
            fixture
                .kura
                .commit_manifest(fixture.context.height)
                .expect("read absent pre-WSV commit manifest")
                .is_none()
        );

        fixture
            .execute(&mut store)
            .expect("retry exact carrier after prepublication failure");
        fixture.assert_complete();
    });

    v2_apply_test!(restart_recovers_kura_lane_body_written_before_wsv_commit, {
        let fixture = ApplyFixture::new_with_lane_payload(true);
        let baseline_state_hash =
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref());
        let ownerships = fixture
            .body
            .execution_context()
            .expect("lane body execution context")
            .lane_payload_ownerships
            .clone();
        assert_eq!(ownerships.len(), 1, "fixture must carry lane ownership");
        let mut store = fixture.reopen_body_store();
        fixture.service.fail_after_kura_store_for_test();
        assert!(matches!(
            fixture.execute(&mut store),
            Err(V2ApplyError::InjectedCrashAfterKuraStore)
        ));
        drop(store);
        let durable = fixture
            .kura
            .get_block(NonZeroUsize::new(1).expect("height"))
            .expect("read production-validated Kura lane crash image");
        assert!(durable.has_results());
        assert_eq!(durable.results().len(), 1);
        assert!(durable.results().all(|result| result.is_ok()));
        let durable_wire = durable.encode_wire().expect("encode Kura lane crash image");
        fixture.assert_no_post_apply_sidecars();
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()),
            baseline_state_hash,
            "the Kura-first lane crash boundary must not leak partial WSV state"
        );
        assert!(
            fixture
                .kura
                .read_lane_block_artifact(ownerships[0].lane_id, ownerships[0].lane_block_height,)
                .is_some(),
            "Kura crash image must include the exact lane sidecar"
        );

        let mut store = fixture.reopen_body_store();
        fixture
            .execute(&mut store)
            .expect("resume exact lane-body WSV application after Kura-first crash");
        fixture.assert_complete();
        assert_eq!(
            fixture
                .kura
                .get_block(NonZeroUsize::new(1).expect("height"))
                .expect("read recovered Kura lane block")
                .encode_wire()
                .expect("encode recovered Kura lane block"),
            durable_wire,
            "an exact lane retry must preserve the complete canonical Kura wire"
        );
    });

    v2_apply_test!(
        conflicting_canonical_kura_block_fails_before_wsv_mutation,
        {
            let fixture = ApplyFixture::new();
            let conflicting_key =
                KeyPair::try_from_seed(vec![0xE1; 32], Algorithm::Ed25519).expect("conflict key");
            let header = BlockHeader::new(
                NonZeroU64::new(1).expect("height"),
                None,
                None,
                None,
                9_999,
                0,
            );
            let signature =
                SignatureOf::try_from_hash(conflicting_key.private_key(), header.hash())
                    .expect("sign conflicting block");
            let conflicting =
                SignedBlock::presigned(BlockSignature::new(0, signature), header, Vec::new());
            assert_ne!(conflicting.hash(), fixture.body.hash());
            fixture
                .kura
                .store_block(conflicting)
                .expect("persist conflicting canonical block");
            let mut store = fixture.reopen_body_store();

            assert!(matches!(
                fixture.execute(&mut store),
                Err(V2ApplyError::KuraConflict)
            ));
            assert_eq!(fixture.state.committed_height(), 0);
            fixture.assert_no_post_apply_sidecars();
        }
    );

    v2_apply_test!(wsv_without_its_canonical_kura_block_fails_closed, {
        let fixture = ApplyFixture::new();
        let artifact = wire::finality::V2FinalityArtifact::new(
            fixture.context.clone(),
            fixture.task.subject(),
            fixture.task.certificate().clone(),
            fixture.service.validator_set_pops.clone(),
        );
        fixture
            .service
            .validate_and_apply(
                &fixture.context,
                fixture.body.clone(),
                false,
                fixture.task.validated_receipt().execution_commitment(),
                &artifact,
            )
            .expect("model corrupted WSV-ahead crash image");
        assert_eq!(fixture.state.committed_height(), 1);
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 0);
        let mut store = fixture.reopen_body_store();

        assert!(matches!(
            fixture.execute(&mut store),
            Err(V2ApplyError::StateAheadOfKura)
        ));
        fixture.assert_no_post_apply_sidecars();
    });

    v2_apply_test!(
        apply_rejects_commit_qc_execution_commitment_drift_before_state_or_kura_write,
        {
            let fixture = ApplyFixture::new();
            let mut certificate = fixture.task.certificate().clone();
            certificate.execution_commitment = wire::ExecutionCommitment::without_topups(
                Hash::new(b"wrong parent state"),
                Hash::new(b"wrong post state"),
                Hash::new(b"wrong ordinary writes"),
                Hash::new(b"wrong executed block wire"),
            );
            let task = ApplyTask::for_test(
                2,
                fixture.task.tag(),
                fixture.task.subject(),
                certificate,
                fixture.task.validated_receipt().clone(),
            );
            let mut store = fixture.reopen_body_store();

            assert!(matches!(
                fixture.service.execute(&fixture.context, &mut store, &task),
                Err(V2ApplyError::ExecutionCommitmentMismatch)
            ));
            assert_eq!(fixture.state.committed_height(), 0);
            assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 0);
            fixture.assert_no_post_apply_sidecars();
        }
    );

    v2_apply_test!(
        fresh_apply_recomputes_and_rejects_a_consistently_forged_marker_and_qc,
        {
            let fixture = ApplyFixture::new();
            let forged_commitment = wire::ExecutionCommitment::without_topups(
                Hash::new(b"forged parent state"),
                Hash::new(b"forged post state"),
                Hash::new(b"forged ordinary writes"),
                Hash::new(b"forged executed block wire"),
            );
            let mut certificate = fixture.task.certificate().clone();
            certificate.execution_commitment = forged_commitment;

            let mut keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic BLS key")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let preimage = wire::Vote {
                round: certificate.round,
                proposal_round: certificate.proposal_round,
                phase: certificate.phase,
                subject: certificate.subject,
                execution_commitment: forged_commitment,
                signer: certificate.signers[0],
                signature: Vec::new(),
            }
            .signature_preimage();
            let signatures = certificate
                .signers
                .iter()
                .map(|index| {
                    Signature::try_new(
                        keys[usize::try_from(*index).expect("fixture signer index")].private_key(),
                        &preimage,
                    )
                    .expect("sign forged execution commitment")
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
                &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate forged Commit votes");

            let forged_validation = ValidatedBodyReceipt::for_test_with_commitment(
                fixture.task.validated_receipt().durable().clone(),
                forged_commitment,
            );
            let task = ApplyTask::for_test(
                2,
                fixture.task.tag(),
                fixture.manifest.subject,
                certificate,
                forged_validation,
            );
            let mut store = fixture.reopen_body_store();

            assert!(matches!(
                fixture.service.execute(&fixture.context, &mut store, &task),
                Err(V2ApplyError::ExecutionCommitmentMismatch)
            ));
            fixture.assert_no_apply_mutation();
        }
    );

    v2_apply_test!(
        checkpoint_write_failure_keeps_wsv_behind_durable_kura_tip,
        {
            let fixture = ApplyFixture::new();
            let mut store = fixture.reopen_body_store();
            fixture.kura.fail_next_wsv_checkpoint_write_for_tests();
            let error = fixture
                .execute(&mut store)
                .expect_err("checkpoint failure follows the durable Kura boundary");
            assert!(
                matches!(
                    &error,
                    V2ApplyError::CommittedRecoveryRequired { stage, .. }
                        if *stage == "pre-WSV recovery checkpoint"
                ),
                "unexpected committed recovery classification: {error:?}"
            );
            assert!(error.requires_restart_recovery());
            assert_eq!(
                fixture.state.committed_height(),
                0,
                "live WSV must not advance without its durable recovery checkpoint"
            );
            assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
            assert!(
                fixture
                    .kura
                    .commit_manifest(fixture.context.height)
                    .expect("read absent manifest")
                    .is_none()
            );
            assert_eq!(
                fixture
                    .kura
                    .v2_finality_artifact(fixture.context.height)
                    .expect("read pre-WSV finality")
                    .expect("finality must precede the WSV checkpoint")
                    .block_hash,
                fixture.body.hash()
            );

            drop(store);
            let mut reopened = fixture.reopen_body_store();
            assert!(
                reopened
                    .validated_recovery_catalog()
                    .contains_key(&(fixture.manifest.round, fixture.manifest.subject)),
                "restart must recover the exact durable validation marker"
            );
            fixture
                .execute(&mut reopened)
                .expect("replay the exact durable tip and publish WSV once");
            fixture.assert_complete();
        }
    );

    v2_apply_test!(
        provider_ingest_archive_failure_after_kura_and_checkpoint_keeps_state_unpublished,
        {
            let mut fixture = ApplyFixture::new();
            let archive_root = direct_archive_tempdir();
            let archive = Arc::new(
                ProviderIngestFinalizedArchiveV1::try_open(
                    archive_root.path(),
                    provider_ingest_archive_bounds(32, 32),
                )
                .expect("open deliberately tiny provider-ingest archive"),
            );
            fixture.service.provider_ingest_finalized_archive = Some(Arc::clone(&archive));
            let mut store = fixture.reopen_body_store();

            let error = fixture
                .execute(&mut store)
                .expect_err("provider-ingest capture must exceed the tiny record bound");
            assert!(
                matches!(
                    &error,
                    V2ApplyError::CommittedRecoveryRequired { stage, .. }
                        if *stage == "provider-ingest finalized archive capture"
                ),
                "unexpected committed recovery classification: {error:?}"
            );
            assert!(error.requires_restart_recovery());
            assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
            assert!(
                fixture
                    .kura
                    .wsv_checkpoint(1)
                    .expect("read staged checkpoint")
                    .is_some(),
                "WSV checkpoint must precede provider-ingest archive capture"
            );
            assert!(
                fixture
                    .kura
                    .v2_finality_artifact(1)
                    .expect("read durable finality")
                    .is_some(),
                "authenticated Kura finality must precede provider-ingest capture"
            );
            assert_eq!(
                fixture.state.committed_height(),
                0,
                "provider-ingest archive failure must precede live State publication"
            );
            assert!(
                archive
                    .activation_floor(&fixture.service.chain_id)
                    .expect("read empty activation floor")
                    .is_none()
            );
        }
    );

    v2_apply_test!(
        provider_ingest_archive_recovery_replays_exact_capture_without_duplicate_generation,
        {
            let mut fixture = ApplyFixture::new();
            let archive_root = direct_archive_tempdir();
            let archive = Arc::new(
                ProviderIngestFinalizedArchiveV1::try_open(
                    archive_root.path(),
                    provider_ingest_archive_bounds(4 * 1024 * 1024, 64 * 1024 * 1024),
                )
                .expect("open provider-ingest archive"),
            );
            fixture.service.provider_ingest_finalized_archive = Some(Arc::clone(&archive));
            fixture
                .service
                .fail_after_provider_ingest_archive_capture_for_test();
            let mut first_store = fixture.reopen_body_store();

            let error = fixture
                .execute(&mut first_store)
                .expect_err("inject crash after provider-ingest archive capture");
            assert!(
                matches!(
                    &error,
                    V2ApplyError::InjectedCrashAfterProviderIngestArchiveCapture
                ),
                "unexpected provider-ingest capture result: {error:?}"
            );
            assert!(error.requires_restart_recovery());
            assert_eq!(fixture.state.committed_height(), 0);
            let generation_after_capture = archive
                .health_generation()
                .expect("read first provider-ingest archive generation");
            let qualification = archive
                .qualify_against_kura_tip(&fixture.service.chain_id, fixture.kura.as_ref(), 0)
                .expect("provider-ingest archive is exact at the durable Kura tip");
            assert_eq!(qualification.activation_floor().height, 1);
            assert_eq!(qualification.archive_tip().height, 1);
            assert_eq!(qualification.kura_tip_height(), 1);
            assert_eq!(qualification.lag_blocks(), 0);
            drop(first_store);

            let (restarted_service, restarted_state) =
                fixture.restart_service_from_last_finalized_snapshot();
            let mut restarted_store = fixture.reopen_body_store();
            restarted_service
                .execute(&fixture.context, &mut restarted_store, &fixture.task)
                .expect("restart replays the exact provider-ingest capture and publishes WSV");
            assert_eq!(restarted_state.committed_height(), 1);
            assert_eq!(
                archive
                    .health_generation()
                    .expect("read replayed provider-ingest archive generation"),
                generation_after_capture,
                "an exact replay must not publish another archive generation"
            );

            let (_, receipt) = restarted_service
                .kura
                .v2_finality_artifact_with_receipt(1)
                .expect("read exact finality receipt")
                .expect("height-one finality receipt");
            let restarted_view = restarted_state.query_view();
            let outcome = archive
                .capture_kura_authenticated_view(&restarted_view, fixture.kura.as_ref(), &receipt)
                .expect("same exact provider-ingest committed view is replay-safe");
            assert_eq!(
                outcome,
                ProviderIngestFinalizedArchiveInsertOutcomeV1::ExactReplay
            );
        }
    );

    v2_apply_test!(
        reputation_archive_failure_after_kura_and_checkpoint_keeps_state_unpublished,
        {
            let mut fixture = ApplyFixture::new_with_reputation_archive();
            let archive_root = direct_archive_tempdir();
            let bounds = ReputationFinalizedArchiveBounds::try_new(32, 16, 32)
                .expect("valid deliberately tiny archive bounds");
            let archive = Arc::new(
                ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                    .expect("open tiny archive"),
            );
            fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));
            let mut store = fixture.reopen_body_store();

            let error = fixture
                .execute(&mut store)
                .expect_err("archive capture must exceed the deliberately tiny bound");
            assert!(
                matches!(
                    &error,
                    V2ApplyError::CommittedRecoveryRequired { stage, .. }
                        if *stage == "reputation finalized archive capture"
                ),
                "unexpected committed recovery classification: {error:?}"
            );
            assert!(error.requires_restart_recovery());
            assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
            assert!(
                fixture
                    .kura
                    .wsv_checkpoint(1)
                    .expect("read staged checkpoint")
                    .is_some(),
                "WSV checkpoint must precede archive capture"
            );
            assert!(
                fixture
                    .kura
                    .v2_finality_artifact(1)
                    .expect("read durable finality")
                    .is_some(),
                "authenticated Kura finality must precede archive capture"
            );
            assert_eq!(
                fixture.state.committed_height(),
                0,
                "archive failure must precede live State publication"
            );
            assert!(
                archive
                    .activation_floor(&fixture.service.chain_id)
                    .expect("read empty activation floor")
                    .is_none()
            );
        }
    );

    v2_apply_test!(
        crash_after_reputation_archive_capture_precedes_state_block_commit,
        {
            let mut fixture = ApplyFixture::new_with_reputation_archive();
            let archive_root = direct_archive_tempdir();
            let bounds =
                ReputationFinalizedArchiveBounds::try_new(4 * 1024 * 1024, 16, 64 * 1024 * 1024)
                    .expect("valid archive bounds");
            let archive = Arc::new(
                ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                    .expect("open archive"),
            );
            fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));
            fixture
                .service
                .fail_after_reputation_archive_capture_for_test();
            let mut store = fixture.reopen_body_store();

            let error = fixture
                .execute(&mut store)
                .expect_err("injected post-capture crash");
            assert!(
                matches!(
                    &error,
                    V2ApplyError::InjectedCrashAfterReputationArchiveCapture
                ),
                "unexpected post-capture result: {error:?}"
            );
            assert_eq!(
                fixture.state.committed_height(),
                0,
                "injected archive-boundary crash must precede StateBlock::commit"
            );
            let qualification = archive
                .qualify_against_kura_tip(&fixture.service.chain_id, fixture.kura.as_ref(), 0)
                .expect("captured archive is exact at the durable Kura tip");
            assert_eq!(qualification.activation_floor().height, 1);
            assert_eq!(qualification.archive_tip().height, 1);
            assert_eq!(qualification.kura_tip_height(), 1);
            assert_eq!(qualification.lag_blocks(), 0);
            assert!(
                fixture
                    .kura
                    .commit_manifest(1)
                    .expect("read absent post-apply manifest")
                    .is_none(),
                "post-apply metadata must remain behind State publication"
            );
        }
    );

    v2_apply_test!(
        reputation_archive_recovery_is_idempotent_without_skipping_height,
        {
            let mut fixture = ApplyFixture::new_with_reputation_archive();
            let archive_root = direct_archive_tempdir();
            let bounds =
                ReputationFinalizedArchiveBounds::try_new(4 * 1024 * 1024, 16, 64 * 1024 * 1024)
                    .expect("valid archive bounds");
            let archive = Arc::new(
                ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                    .expect("open archive"),
            );
            fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));
            fixture
                .service
                .fail_after_reputation_archive_capture_for_test();
            let mut first_store = fixture.reopen_body_store();
            let error = fixture
                .execute(&mut first_store)
                .expect_err("injected post-capture crash");
            assert!(
                matches!(
                    &error,
                    V2ApplyError::InjectedCrashAfterReputationArchiveCapture
                ),
                "unexpected post-capture result: {error:?}"
            );
            let generation_after_capture = archive
                .health_generation()
                .expect("read first archive generation");
            let staged_checkpoint = fixture
                .kura
                .wsv_checkpoint(1)
                .expect("read pre-retry staged checkpoint")
                .expect("archive-boundary crash retains its staged checkpoint");
            assert!(
                fixture
                    .kura
                    .commit_manifest(1)
                    .expect("read pre-retry commit manifest")
                    .is_none(),
                "archive-boundary crash must precede commit-manifest publication"
            );
            drop(first_store);

            let (restarted_service, restarted_state) =
                fixture.restart_service_from_last_finalized_snapshot();
            let mut restarted_store = fixture.reopen_body_store();
            if let Err(error) =
                restarted_service.execute(&fixture.context, &mut restarted_store, &fixture.task)
            {
                let checkpoint_after_retry = fixture.kura.wsv_checkpoint(1);
                let manifest_after_retry = fixture.kura.commit_manifest(1);
                let replay_state_hash =
                    crate::snapshot::canonical_state_snapshot_hash(restarted_state.as_ref());
                panic!(
                    "exact durable replay reuses the archived height: {error:?}; \
                     staged_state_hash={:?}; replay_state_hash={replay_state_hash:?}; \
                     checkpoint_after_retry={checkpoint_after_retry:?}; \
                     manifest_after_retry={manifest_after_retry:?}",
                    staged_checkpoint.state_hash(),
                );
            }
            assert_eq!(restarted_state.committed_height(), 1);
            assert_eq!(
                archive
                    .health_generation()
                    .expect("read replayed archive generation"),
                generation_after_capture,
                "an exact replay must not publish a second archive generation"
            );
            let projection = archive
                .latest_at_or_before(&fixture.service.chain_id, 1)
                .expect("read recovered projection")
                .expect("height one remains archived");
            assert_eq!(projection.key.height, 1);
            let qualification = archive
                .qualify_against_kura_tip(&fixture.service.chain_id, fixture.kura.as_ref(), 0)
                .expect("recovered archive remains contiguous and exact");
            assert_eq!(qualification.activation_floor().height, 1);
            assert_eq!(qualification.archive_tip().height, 1);

            let (_, receipt) = restarted_service
                .kura
                .v2_finality_artifact_with_receipt(1)
                .expect("read exact finality receipt")
                .expect("height one finality receipt");
            let restarted_view = restarted_state.query_view();
            let outcome = archive
                .capture_kura_authenticated_view(&restarted_view, fixture.kura.as_ref(), &receipt)
                .expect("same exact committed view is replay-safe");
            assert_eq!(
                outcome,
                ReputationFinalizedArchiveInsertOutcome::ExactReplay
            );
        }
    );

    v2_apply_test!(
        reputation_archive_virtual_base_allows_commit_owned_successor_capture,
        {
            let mut fixture = ApplyFixture::new_with_reputation_archive();
            let archive_root = direct_archive_tempdir();
            let bounds =
                ReputationFinalizedArchiveBounds::try_new(4 * 1024 * 1024, 16, 64 * 1024 * 1024)
                    .expect("valid retained-capture archive bounds");
            let archive = Arc::new(
                ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                    .expect("open retained-capture archive"),
            );
            fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));

            let mut parent_store = fixture.reopen_body_store();
            fixture
                .execute(&mut parent_store)
                .expect("commit and archive the retention-floor block");
            drop(parent_store);
            let parent = archive
                .latest_at_or_before(&fixture.service.chain_id, 1)
                .expect("read retention-floor projection")
                .expect("height-one archive anchor");
            let fence = archive
                .retention_fence_for(&parent.key)
                .expect("freeze exact authenticated retention fence");
            let retention_authority = ReputationRetentionAuthorityForTest::new();
            let retention_binding = retention_authority.binding();
            let proposal = archive
                .prepare_kura_authenticated_compaction(&fence, fixture.kura.as_ref())
                .expect("prepare exact Kura-authenticated checkpoint");
            let compaction = archive
                .approve_and_install_kura_authenticated_compaction(
                    &proposal,
                    fixture.kura.as_ref(),
                    &retention_binding,
                    &retention_authority,
                )
                .expect("approve and compact the Kura-authenticated height-one prefix");
            assert_eq!(compaction.retention_floor(), &parent.key);
            assert_eq!(
                compaction.generation(),
                fence.expected_generation() + 1,
                "checkpoint-head publication advances the archive generation"
            );
            assert_eq!(
                archive
                    .retention_floor(&fixture.service.chain_id)
                    .expect("read active virtual base"),
                Some(parent.key.clone())
            );

            let mut successor = build_successor_apply_fixture(&fixture);
            let completion = match fixture.service.execute(
                &successor.context,
                &mut successor.store,
                &successor.task,
            ) {
                Ok(completion) => completion,
                Err(error) => {
                    assert!(
                        !matches!(
                            &error,
                            V2ApplyError::CommittedRecoveryRequired { stage, .. }
                                if *stage == "reputation finalized archive capture"
                        ),
                        "retained capture must not degrade to committed recovery: {error:?}"
                    );
                    panic!(
                        "commit-owned H+1 capture failed after virtual-base compaction: {error:?}"
                    );
                }
            };
            assert_eq!(completion.receipt().height(), 2);
            assert_eq!(fixture.state.committed_height(), 2);
            assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 2);
            assert_eq!(
                fixture
                    .kura
                    .get_durable_block_hash(NonZeroUsize::new(2).expect("height two")),
                Some(successor.body.hash())
            );
            let qualification = archive
                .qualify_against_kura_tip(&fixture.service.chain_id, fixture.kura.as_ref(), 0)
                .expect("qualify retained successor against exact Kura tip");
            assert_eq!(qualification.activation_floor().height, 1);
            assert_eq!(qualification.archive_tip().height, 2);
            assert_eq!(
                qualification.checkpoint_digest(),
                Some(compaction.checkpoint_digest())
            );
            assert_eq!(qualification.kura_tip_height(), 2);
            assert_eq!(qualification.lag_blocks(), 0);
            let generation = archive
                .health_generation()
                .expect("read post-successor archive generation");

            drop(successor);
            fixture.service.reputation_finalized_archive = None;
            drop(archive);
            let reopened = ReputationFinalizedArchive::try_open_with_retention_authority(
                archive_root.path(),
                bounds,
                &fixture.service.chain_id,
                fixture.kura.as_ref(),
                &retention_binding,
                &retention_authority,
            )
            .expect("reopen compacted successor archive through sealed authority");
            assert_eq!(
                reopened
                    .health_generation()
                    .expect("read reopened archive generation"),
                generation
            );
            assert_eq!(
                reopened
                    .retention_floor(&fixture.service.chain_id)
                    .expect("read reopened virtual base"),
                Some(parent.key)
            );
            let reopened_qualification = reopened
                .qualify_against_kura_tip(&fixture.service.chain_id, fixture.kura.as_ref(), 0)
                .expect("reopened archive preserves exact predecessor continuity");
            assert_eq!(reopened_qualification.archive_tip().height, 2);
            assert_eq!(
                reopened_qualification.checkpoint_digest(),
                Some(compaction.checkpoint_digest())
            );
            assert_eq!(reopened_qualification.kura_tip_height(), 2);

            let (_, receipt) = fixture
                .kura
                .v2_finality_artifact_with_receipt(2)
                .expect("read height-two finality receipt")
                .expect("height-two finality is durable");
            let view = fixture.state.query_view();
            assert_eq!(
                reopened
                    .capture_kura_authenticated_view(&view, fixture.kura.as_ref(), &receipt)
                    .expect("replay retained H+1 capture after reopen"),
                ReputationFinalizedArchiveInsertOutcome::ExactReplay
            );
            assert_eq!(
                reopened
                    .health_generation()
                    .expect("exact replay preserves archive generation"),
                generation
            );
        }
    );

    v2_apply_test!(
        reputation_retention_restart_recovers_both_cas_publication_boundaries,
        {
            for (failed_load_after_cas, checkpoint_was_published) in [(1, false), (3, true)] {
                let mut fixture = ApplyFixture::new_with_reputation_archive();
                let archive_root = direct_archive_tempdir();
                let bounds = ReputationFinalizedArchiveBounds::try_new(
                    4 * 1024 * 1024,
                    16,
                    64 * 1024 * 1024,
                )
                .expect("valid retention-recovery archive bounds");
                let archive = Arc::new(
                    ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                        .expect("open retention-recovery archive"),
                );
                fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));

                let mut parent_store = fixture.reopen_body_store();
                fixture
                    .execute(&mut parent_store)
                    .expect("commit and archive the retention-floor block");
                drop(parent_store);
                let parent = archive
                    .latest_at_or_before(&fixture.service.chain_id, 1)
                    .expect("read retention-floor projection")
                    .expect("height-one archive anchor");
                let fence = archive
                    .retention_fence_for(&parent.key)
                    .expect("freeze exact retention fence");
                let authority = ReputationRetentionAuthorityForTest::new();
                let binding = authority.binding();
                let proposal = archive
                    .prepare_kura_authenticated_compaction(&fence, fixture.kura.as_ref())
                    .expect("prepare exact retention proposal");
                authority.fail_nth_load_after_next_cas(failed_load_after_cas);

                assert!(matches!(
                    archive.approve_and_install_kura_authenticated_compaction(
                        &proposal,
                        fixture.kura.as_ref(),
                        &binding,
                        &authority,
                    ),
                    Err(ReputationFinalizedArchiveError::RetentionAuthorityCasAmbiguous)
                ));
                assert_eq!(
                    archive
                        .retention_floor(&fixture.service.chain_id)
                        .expect("read ambiguous local retention floor"),
                    checkpoint_was_published.then(|| parent.key.clone())
                );
                assert_eq!(
                    std::fs::read_dir(archive.root().join("anchors"))
                        .expect("read ambiguous anchor namespace")
                        .count(),
                    1,
                    "no physical anchor may be unlinked before the post-publication readback"
                );
                assert_eq!(
                    std::fs::read_dir(archive.root().join("checkpoints"))
                        .expect("read ambiguous checkpoint namespace")
                        .count(),
                    usize::from(checkpoint_was_published)
                );

                fixture.service.reputation_finalized_archive = None;
                drop(archive);
                let recovered = ReputationFinalizedArchive::try_open_with_retention_authority(
                    archive_root.path(),
                    bounds,
                    &fixture.service.chain_id,
                    fixture.kura.as_ref(),
                    &binding,
                    &authority,
                )
                .expect("recover exact externally approved retention state");
                assert_eq!(
                    recovered
                        .retention_floor(&fixture.service.chain_id)
                        .expect("read recovered retention floor"),
                    Some(parent.key.clone())
                );
                assert_eq!(
                    std::fs::read_dir(recovered.root().join("anchors"))
                        .expect("read recovered anchor namespace")
                        .count(),
                    0,
                    "recovery must clean the exact approved physical prefix"
                );
                assert_eq!(
                    std::fs::read_dir(recovered.root().join("checkpoints"))
                        .expect("read recovered checkpoint namespace")
                        .count(),
                    1
                );
                let recovered_generation = recovered
                    .health_generation()
                    .expect("read recovered archive generation");

                drop(recovered);
                let reopened = ReputationFinalizedArchive::try_open_with_retention_authority(
                    archive_root.path(),
                    bounds,
                    &fixture.service.chain_id,
                    fixture.kura.as_ref(),
                    &binding,
                    &authority,
                )
                .expect("repeat exact retention recovery");
                assert_eq!(
                    reopened
                        .health_generation()
                        .expect("read replayed archive generation"),
                    recovered_generation,
                    "recovery replay must be deterministic"
                );
                assert_eq!(
                    reopened
                        .retention_floor(&fixture.service.chain_id)
                        .expect("read replayed retention floor"),
                    Some(parent.key)
                );
            }
        }
    );

    v2_apply_test!(
        crash_after_staged_checkpoint_replays_exact_tip_without_double_apply,
        {
            let fixture = ApplyFixture::new();
            let mut first_process_store = fixture.reopen_body_store();
            fixture.service.fail_after_wsv_checkpoint_for_test();
            let first_error = fixture
                .service
                .execute(&fixture.context, &mut first_process_store, &fixture.task)
                .expect_err("inject crash after checkpoint fsync and before WSV publication");
            assert!(matches!(
                &first_error,
                V2ApplyError::InjectedCrashAfterWsvCheckpoint
            ));
            assert!(first_error.requires_restart_recovery());
            assert_eq!(fixture.state.committed_height(), 0);
            assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
            let staged_checkpoint = fixture
                .kura
                .wsv_checkpoint(1)
                .expect("read staged checkpoint")
                .expect("checkpoint must be durable before WSV publication");
            assert!(
                fixture
                    .kura
                    .commit_manifest(1)
                    .expect("read absent manifest")
                    .is_none(),
                "the pre-WSV checkpoint must remain unbound until State commits"
            );
            assert_eq!(
                fixture
                    .kura
                    .v2_finality_artifact(1)
                    .expect("read pre-WSV finality")
                    .expect("finality must be durable before WSV publication")
                    .block_hash,
                fixture.body.hash()
            );
            let staged_state_hash = staged_checkpoint.state_hash();
            drop(first_process_store);

            // Snapshot publication is gated on the complete
            // checkpoint/manifest/finality tuple, so a process crash reloads
            // the last finalized snapshot (height zero here). The exact
            // durable checkpoint authenticates the overlay replay before live
            // State can cross its commit boundary.
            let (restarted_service, restarted_state) =
                fixture.restart_service_from_last_finalized_snapshot();
            assert_eq!(restarted_state.committed_height(), 0);
            let mut restarted_store = fixture.reopen_body_store();
            restarted_service
                .execute(&fixture.context, &mut restarted_store, &fixture.task)
                .expect("authenticated WAL/body retry reapplies the sole Kura tip");
            assert_eq!(restarted_state.committed_height(), 1);
            let first_artifact = fixture
                .kura
                .v2_finality_artifact(1)
                .expect("read recovered finality")
                .expect("recovery publishes finality");
            assert_eq!(first_artifact.block_hash, fixture.body.hash());
            assert_eq!(
                crate::snapshot::canonical_state_snapshot_hash(restarted_state.as_ref()),
                staged_state_hash,
                "recovery must reproduce the exact pre-commit checkpointed WSV"
            );

            let durable_state_hash =
                crate::snapshot::canonical_state_snapshot_hash(restarted_state.as_ref());
            restarted_service
                .execute(&fixture.context, &mut restarted_store, &fixture.task)
                .expect("an exact post-finality retry is idempotent");
            assert_eq!(
                fixture
                    .kura
                    .v2_finality_artifact(1)
                    .expect("read repeated finality")
                    .as_ref(),
                Some(&first_artifact)
            );
            assert_eq!(
                crate::snapshot::canonical_state_snapshot_hash(restarted_state.as_ref()),
                durable_state_hash,
                "idempotent retry must not execute the block twice"
            );
            fixture.assert_complete_for_state(restarted_state.as_ref());
        }
    );

    v2_apply_test!(restart_recovers_manifest_after_pre_wsv_finality, {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        fixture.kura.fail_next_commit_manifest_write_for_tests();
        let error = fixture
            .execute(&mut store)
            .expect_err("manifest failure follows the irreversible commit boundary");
        assert!(
            matches!(
                &error,
                V2ApplyError::CommittedRecoveryRequired { stage, .. }
                    if *stage == "post-apply metadata"
            ),
            "unexpected committed recovery classification: {error:?}"
        );
        assert!(error.requires_restart_recovery());
        assert_eq!(fixture.state.committed_height(), 1);
        assert!(
            fixture
                .kura
                .wsv_checkpoint(1)
                .expect("read checkpoint")
                .is_some()
        );
        assert!(
            fixture
                .kura
                .commit_manifest(1)
                .expect("read manifest")
                .is_none()
        );
        assert!(
            fixture
                .kura
                .v2_finality_artifact(1)
                .expect("read finality")
                .is_some()
        );

        drop(store);
        let mut reopened = fixture.reopen_body_store();
        fixture.execute(&mut reopened).expect("complete manifest");
        fixture.assert_complete();
    });

    v2_apply_test!(restart_recovers_kura_block_before_pre_wsv_finality, {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        fixture.kura.fail_next_v2_finality_write_for_tests();
        let error = fixture
            .execute(&mut store)
            .expect_err("finality failure follows the irreversible commit boundary");
        assert!(
            matches!(
                &error,
                V2ApplyError::CommittedRecoveryRequired { stage, .. }
                    if *stage == "pre-WSV v2 finality artifact"
            ),
            "unexpected committed recovery classification: {error:?}"
        );
        assert!(error.requires_restart_recovery());
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
        assert!(
            fixture
                .kura
                .wsv_checkpoint(1)
                .expect("read checkpoint")
                .is_none()
        );
        assert!(
            fixture
                .kura
                .commit_manifest(1)
                .expect("read manifest")
                .is_none()
        );
        assert!(
            fixture
                .kura
                .v2_finality_artifact(1)
                .expect("read finality")
                .is_none()
        );

        drop(store);
        let mut reopened = fixture.reopen_body_store();
        fixture
            .execute(&mut reopened)
            .expect("complete pre-WSV finality and apply");
        fixture.assert_complete();
    });

    v2_apply_test!(
        complete_apply_replay_is_idempotent_and_never_advances_twice,
        {
            let fixture = ApplyFixture::new();
            let mut store = fixture.reopen_body_store();
            fixture.execute(&mut store).expect("initial apply");
            let state_hash = crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref());
            let artifact = fixture
                .kura
                .v2_finality_artifact(1)
                .expect("read finality")
                .expect("finality exists");

            fixture.execute(&mut store).expect("idempotent replay");
            fixture.assert_complete();
            assert_eq!(
                crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()),
                state_hash
            );
            assert_eq!(
                fixture
                    .kura
                    .v2_finality_artifact(1)
                    .expect("read repeated finality"),
                Some(artifact)
            );
        }
    );
