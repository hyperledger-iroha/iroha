v2_apply_test!(
    canonical_overlap_detects_same_transaction_under_substituted_key,
    {
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
    }
);

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
            let journal_dir = tempfile::tempdir().expect("autonomous reservation crash journals");
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
                .persist_lane_executable_payload(&payload, payload.network_id, payload.epoch)
                .expect("persist autonomous crash payload");
            let mut global_body_store = fixture.reopen_body_store();
            fixture
                .execute(&mut global_body_store)
                .expect("finalize the exact global body which omitted the losing payload");
            let retirement = crate::kura::AutonomousLaneSlotRetirementV1::from_payload(&payload);
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
                            payload.network_id,
                            payload.epoch,
                        )
                        .expect("persist Kura ReleasePending boundary");
                }
                "queue_prepared_barrier" => {
                    fixture
                        .kura
                        .persist_autonomous_lane_slot_retirement(
                            &retirement,
                            payload.network_id,
                            payload.epoch,
                        )
                        .expect("persist Kura retirement before Queue barrier");
                    let mut substituted_barrier = barrier.clone();
                    substituted_barrier.ordered_keys.swap(0, 1);
                    let substituted_authorization = fixture
                        .kura
                        .authorize_autonomous_lane_queue_release_preparation(
                            &retirement,
                            payload.network_id,
                            payload.epoch,
                        )
                        .expect("authorize the exact Queue barrier before substitution");
                    assert!(matches!(
                        queue.prepare_lane_reservation_release_barrier_with_authorization(
                            &substituted_barrier,
                            substituted_authorization,
                        ),
                        Err(LaneQueueReservationError::InvalidIdentity(_))
                    ));
                    assert!(queue.lane_reservation_release_barriers().is_empty());
                    assert_eq!(queue.live_lane_reservations().len(), 3);
                    let preparation_authorization = fixture
                        .kura
                        .authorize_autonomous_lane_queue_release_preparation(
                            &retirement,
                            payload.network_id,
                            payload.epoch,
                        )
                        .expect("authorize exact Queue prepared barrier");
                    let _durable_queue_barrier = queue
                        .prepare_lane_reservation_release_barrier_with_authorization(
                            &barrier,
                            preparation_authorization,
                        )
                        .expect("persist authorized Queue prepared barrier");
                }
                "kura_released" => {
                    fixture
                        .kura
                        .persist_autonomous_lane_slot_retirement(
                            &retirement,
                            payload.network_id,
                            payload.epoch,
                        )
                        .expect("persist Kura retirement before released claims");
                    let preparation_authorization = fixture
                        .kura
                        .authorize_autonomous_lane_queue_release_preparation(
                            &retirement,
                            payload.network_id,
                            payload.epoch,
                        )
                        .expect("authorize Queue barrier before Kura Released");
                    let durable_queue_barrier = queue
                        .prepare_lane_reservation_release_barrier_with_authorization(
                            &barrier,
                            preparation_authorization,
                        )
                        .expect("persist authorized Queue barrier before Kura Released");
                    let mut substituted_barrier = barrier.clone();
                    substituted_barrier.ordered_keys.swap(0, 1);
                    assert!(
                        fixture
                            .kura
                            .finalize_autonomous_lane_slot_release_with_authorization(
                                &retirement,
                                &substituted_barrier,
                                payload.network_id,
                                payload.epoch,
                                durable_queue_barrier,
                            )
                            .is_err(),
                        "Queue proof must not authorize a substituted barrier"
                    );
                    assert_eq!(
                        queue.lane_reservation_release_barriers(),
                        vec![barrier.clone()]
                    );
                    assert_eq!(queue.live_lane_reservations().len(), 3);
                    let retry_preparation_authorization = fixture
                        .kura
                        .authorize_autonomous_lane_queue_release_preparation(
                            &retirement,
                            payload.network_id,
                            payload.epoch,
                        )
                        .expect("reauthorize the exact prepared Queue barrier");
                    let durable_queue_barrier = queue
                        .prepare_lane_reservation_release_barrier_with_authorization(
                            &barrier,
                            retry_preparation_authorization,
                        )
                        .expect("reopen the exact prepared Queue barrier");
                    let _queue_finalization_authorization = fixture
                        .kura
                        .finalize_autonomous_lane_slot_release_with_authorization(
                            &retirement,
                            &barrier,
                            payload.network_id,
                            payload.epoch,
                            durable_queue_barrier,
                        )
                        .expect("persist authorized Kura Released boundary");
                }
                "queue_completion_forgotten" => {
                    assert_eq!(
                        retire_autonomous_lane_slot_and_release_reservations(
                            fixture.kura.as_ref(),
                            queue.as_ref(),
                            &retirement,
                            payload.network_id,
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
                        payload.network_id,
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
                    payload.network_id,
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
            let stale_barrier = replayed_again.prepare_lane_reservation_release_barrier(&barrier);
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
    autonomous_release_rejects_missing_queue_owner_while_kura_claims_are_pending,
    {
        let fixture = ApplyFixture::new();
        let producer = KeyPair::try_from_seed(vec![0xB8; 32], Algorithm::BlsNormal)
            .expect("derive missing-Queue-owner producer");
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
        let journal_dir = tempfile::tempdir().expect("missing-Queue-owner reservation journals");
        queue
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install missing-Queue-owner queue-plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install missing-Queue-owner reservation journal");
        let (payload, expected_fifo) = reserve_autonomous_crash_batch(&fixture, &queue, &producer);
        let descriptor = &payload.origin_proposal.descriptor;
        let lane_config = RuntimeLaneConfig::default();
        fixture
            .kura
            .install_lane_incarnation_marker_for_test(
                lane_config.primary(),
                descriptor.lane_incarnation,
                0,
            )
            .expect("install missing-Queue-owner lane marker");
        fixture
            .kura
            .persist_lane_executable_payload(&payload, payload.network_id, payload.epoch)
            .expect("persist missing-Queue-owner payload");
        let mut global_body_store = fixture.reopen_body_store();
        fixture
            .execute(&mut global_body_store)
            .expect("finalize global body which omitted the losing payload");
        let retirement = crate::kura::AutonomousLaneSlotRetirementV1::from_payload(&payload);
        let barrier = retirement
            .queue_release_barrier()
            .expect("build missing-Queue-owner release barrier");
        fixture
            .kura
            .persist_autonomous_lane_slot_retirement(
                &retirement,
                payload.network_id,
                payload.epoch,
            )
            .expect("persist missing-Queue-owner ReleasePending boundary");

        assert_eq!(
            queue
                .release_lane_reservations_in_order(&barrier.ordered_keys)
                .expect("construct adversarial missing Queue owner"),
            barrier.ordered_keys.len()
        );
        assert!(queue.live_lane_reservations().is_empty());
        assert!(queue.lane_reservation_release_barriers().is_empty());
        assert_eq!(queue.queued_len(), expected_fifo.len());

        let authorization = fixture
            .kura
            .authorize_autonomous_lane_queue_release_preparation(
                &retirement,
                payload.network_id,
                payload.epoch,
            )
            .expect("authenticate the still-pending Kura claims");
        let failure = match queue
            .prepare_lane_reservation_release_barrier_with_authorization(&barrier, authorization)
        {
            Ok(_) => panic!("pending Kura claims must reject absent Queue ownership"),
            Err(failure) => failure,
        };
        assert!(matches!(
            failure,
            LaneQueueReservationError::InvalidIdentity(ref message)
                if message.contains("missing Queue release ownership")
        ));
        assert!(queue.live_lane_reservations().is_empty());
        assert!(queue.lane_reservation_release_barriers().is_empty());
        assert_eq!(queue.queued_len(), expected_fifo.len());

        let mut observed_fifo = Vec::new();
        queue.get_transactions_for_block_with_state(
            fixture.state.as_ref(),
            NonZeroUsize::new(expected_fifo.len()).expect("non-empty adversarial FIFO"),
            &mut observed_fifo,
        );
        assert_eq!(
            observed_fifo
                .iter()
                .map(|transaction| transaction.as_ref().hash())
                .collect::<Vec<_>>(),
            expected_fifo,
            "failed cross-store authorization must not reorder ordinary FIFO ownership"
        );
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
        let reproposal_manifest = crate::sumeragi::v2_chunks::encode_payload(
            &fixture.context,
            later_round,
            fixture.task.subject(),
            &canonical_wire,
        )
        .expect("derive later-round manifest for unchanged locked body")
        .into_parts()
        .0;
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
        let signature = SignatureOf::try_from_hash(conflicting_key.private_key(), header.hash())
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
            CheckedCarrierApplications::for_block(&fixture.body),
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
        certificate.execution_commitment =
            wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"wrong parent state"),
                Hash::new(b"wrong post state"),
                Hash::new(b"wrong ordinary writes"),
                1,
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
        let forged_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"forged parent state"),
            Hash::new(b"forged post state"),
            Hash::new(b"forged ordinary writes"),
            1,
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
