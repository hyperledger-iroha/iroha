
    #[test]
    fn causal_successors_inherit_root_and_lifecycle_ordinal() {
        let admitted_at = Instant::now();
        let root_tag = tag(0);
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(8, 2, 2));
        ingress
            .enqueue(TaggedCommand::new(
                root_tag,
                CommandClass::Normal,
                FakeCommand::record(1),
                admitted_at,
            ))
            .expect("root candidate is admitted");
        let (root, root_owner) = ingress
            .pop_next_with_ownership()
            .expect("root selection is exact")
            .expect("root candidate is ready");
        assert_eq!(root.lifecycle_ordinal, Some(root_owner.lifecycle_ordinal));

        let successor_tag = EventTag::new(
            root_tag.height(),
            root_tag.view() + 1,
            Generation::new(root_tag.generation().get() + 1),
        );
        for value in [2, 3, 4] {
            ingress
                .enqueue(
                    TaggedCommand::with_causal_origin(
                        successor_tag,
                        CommandClass::Completion,
                        FakeCommand::record(value),
                        admitted_at,
                        root_owner.causal_origin.clone(),
                        root_owner.lifecycle_ordinal,
                    )
                    .expect("causal owner is internally consistent"),
                )
                .expect("causal child is admitted with a unique physical owner");
        }

        let physical_ordinals = ingress
            .commands
            .iter()
            .map(|candidate| {
                assert_eq!(
                    candidate.causal_origin, root_owner.causal_origin,
                    "evidence/view rewriting cannot replace the first-admission root"
                );
                assert_eq!(
                    candidate.lifecycle_ordinal,
                    Some(root_owner.lifecycle_ordinal),
                    "every child inherits one logical lifecycle ordinal"
                );
                candidate
                    .admission_ordinal
                    .expect("every physical child has its own FIFO ordinal")
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(physical_ordinals.len(), 3);

        let unrelated = TaggedCommand::new(
            successor_tag,
            CommandClass::Completion,
            FakeCommand::record(2),
            admitted_at,
        );
        assert!(
            !unrelated
                .causal_origin
                .same_lifecycle(&root_owner.causal_origin),
            "a physically similar command with a different causal root cannot coalesce"
        );
    }

    #[test]
    fn preassigned_batch_lifecycles_require_shared_mint_and_exact_root() {
        let admitted_at = Instant::now();
        let owner_tag = tag(0);
        let unminted_source = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let mut unminted_ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            unminted_source.clone(),
        );
        let unminted_command = FakeCommand::record(1);
        let mut unminted_origin = RuntimeCandidateCausalOrigin::mint(
            owner_tag,
            CommandClass::Completion,
            &unminted_command,
            None,
        );
        assert!(unminted_origin.bind_lifecycle_ordinal(1));
        let unminted = TaggedCommand::with_causal_origin(
            owner_tag,
            CommandClass::Completion,
            unminted_command,
            admitted_at,
            unminted_origin,
            1,
        )
        .expect("construct internally exact but unminted lifecycle");
        assert_eq!(
            unminted_ingress.enqueue_completion_batch(vec![unminted]),
            Err(EnqueueError::FailClosed)
        );
        assert!(unminted_ingress.commands.is_empty());
        assert_eq!(
            unminted_source
                .next_ordinal_for_test()
                .expect("unminted batch rejection preserves the source"),
            Some(1)
        );

        let collision_source = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let mut collision_ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            collision_source.clone(),
        );
        collision_ingress
            .enqueue(TaggedCommand::new(
                owner_tag,
                CommandClass::Normal,
                FakeCommand::record(2),
                admitted_at,
            ))
            .expect("mint one exact lifecycle root");
        let (_, root_owner) = collision_ingress
            .pop_next_with_ownership()
            .expect("select the minted root exactly")
            .expect("root is ready");
        let sibling = TaggedCommand::with_causal_origin(
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(3),
            admitted_at,
            root_owner.causal_origin.clone(),
            root_owner.lifecycle_ordinal,
        )
        .expect("construct one legitimate causal sibling");
        let conflicting_command = FakeCommand::record(4);
        let mut conflicting_origin = RuntimeCandidateCausalOrigin::mint(
            owner_tag,
            CommandClass::Completion,
            &conflicting_command,
            None,
        );
        assert!(conflicting_origin.bind_lifecycle_ordinal(root_owner.lifecycle_ordinal));
        let conflicting = TaggedCommand::with_causal_origin(
            owner_tag,
            CommandClass::Completion,
            conflicting_command,
            admitted_at,
            conflicting_origin,
            root_owner.lifecycle_ordinal,
        )
        .expect("construct a distinct root at the colliding ordinal");
        let next_before_collision = collision_source
            .next_ordinal_for_test()
            .expect("inspect source before batch collision");
        assert_eq!(
            collision_ingress.enqueue_completion_batch(vec![sibling, conflicting]),
            Err(EnqueueError::FailClosed)
        );
        assert!(
            collision_ingress.commands.is_empty(),
            "batch collision must reject atomically"
        );
        assert_eq!(
            collision_source
                .next_ordinal_for_test()
                .expect("batch collision preserves the source"),
            next_before_collision,
            "collision validation must run before reserving physical positions"
        );
    }

    #[test]
    fn restart_dormant_local_fifo_reservation_survives_full_class_churn() {
        let started_at = Instant::now();
        let owner_tag = tag(0);
        let lifecycle_key = Hash::new(b"restart dormant Local FIFO lifecycle");
        let mut driver = FakeDriver::new(owner_tag);
        driver.dormant_local_fifo_reservations =
            vec![RuntimeDormantLocalFifoReservation::completion(
                lifecycle_key,
                1,
                8,
            )];
        let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(1);
        let mut runtime = SerializedV2Runtime::with_driver_and_lifecycle_ordinals(
            driver,
            started_at,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(5, 1, 1),
            Vec::new(),
            lifecycle_ordinals,
        )
        .expect("restart installs exact latent FIFO ownership")
        .0;
        runtime
            .arm_live_clocks(started_at)
            .expect("arm the restarted runtime without advancing its latent owner");
        assert_eq!(
            runtime.remaining_completion_capacity(),
            4,
            "the dormant Local stage consumes one physical completion slot"
        );
        let later_serve = runtime
            .ingress
            .lifecycle_ordinals
            .reserve_one()
            .expect("mint a later exact Serve ticket");
        assert!(
            runtime
                .older_lifecycle_predates_exact_serve(started_at, later_serve)
                .expect("latent FIFO owner participates in the active minimum"),
            "the restart-dormant owner must remain ahead of later Serve work"
        );

        for value in [1, 2] {
            enqueue_fake(
                &mut runtime,
                owner_tag,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .expect("ordinary churn fills only the remaining normal prefix");
        }
        assert_eq!(
            enqueue_fake(
                &mut runtime,
                owner_tag,
                CommandClass::Normal,
                FakeCommand::record(3),
            ),
            Err(EnqueueError::ReservedCapacity),
            "normal churn cannot acquire the dormant target's slot"
        );
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Progress,
            FakeCommand::record(4),
        )
        .expect("progress fills its existing prefix");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(5),
        )
        .expect("a trusted completion fills the last unreserved position");
        assert_eq!(runtime.remaining_completion_capacity(), 0);
        assert!(
            matches!(runtime.step(started_at), Ok(RuntimeStep::Idle)),
            "later Completion, Progress, and Normal commands must idle behind the latent minimum"
        );
        let idle_ownership = runtime
            .take_last_scheduler_ownership()
            .expect("the blocked turn retains exact idle ownership");
        assert_eq!(idle_ownership.selected, RuntimeSelectedOwnerKind::Idle);
        assert!(
            runtime.driver.delivered.is_empty(),
            "no younger physical command may dispatch before exact replacement"
        );

        runtime.driver.admission_preflight_override =
            Some(RuntimeCommandAdmissionPreflight::ReuseDormant {
                causal_lifecycle_key: lifecycle_key,
                admission_ordinal: 1,
                producer_stage: 8,
            });
        let next_before_replay = runtime.ingress.next_admission_ordinal;
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(9),
        )
        .expect("exact retry atomically replaces its latent slot at full capacity");
        assert!(runtime.ingress.dormant_local_fifo_reservations.is_empty());
        assert_eq!(runtime.queued_commands(), 5);
        assert_eq!(runtime.remaining_completion_capacity(), 0);
        assert_eq!(
            runtime.minimum_active_lifecycle_ordinal(),
            Ok(Some(1)),
            "the restored FIFO owner retains the pre-restart lifecycle age"
        );

        let next_after_replay = runtime.ingress.next_admission_ordinal;
        assert_ne!(
            next_after_replay, next_before_replay,
            "the first physical replay receives one fresh FIFO position"
        );
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(9),
        )
        .expect("duplicate exact retry coalesces with the one physical owner");
        assert_eq!(runtime.queued_commands(), 5);
        assert_eq!(
            runtime.ingress.next_admission_ordinal, next_after_replay,
            "coalescing cannot mint another physical admission ordinal"
        );

        let RuntimeStep::Advanced(effects) = runtime
            .step(started_at)
            .expect("the exact replacement becomes the global ready owner")
        else {
            panic!("the exact replacement must dispatch before younger queued work");
        };
        assert!(effects.is_empty());
        let selected = runtime
            .take_last_scheduler_ownership()
            .expect("the replacement dispatch retains exact FIFO ownership");
        assert_eq!(selected.selected, RuntimeSelectedOwnerKind::Fifo);
        assert_eq!(
            runtime.driver.delivered,
            vec![(owner_tag, 9)],
            "the restored target dispatches before every younger physical command"
        );
        assert_eq!(runtime.queued_commands(), 4);

        assert_eq!(
            enqueue_fake(
                &mut runtime,
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(9),
            ),
            Err(EnqueueError::FailClosed),
            "ReuseDormant after latent-slot removal cannot recreate the drained stage"
        );
        assert!(runtime.fail_closed);
        assert_eq!(
            runtime.queued_commands(),
            4,
            "rejected resurrection cannot install another physical owner"
        );
    }

    #[test]
    fn restart_dormant_completion_batch_atomically_replaces_latent_slots() {
        let admitted_at = Instant::now();
        let owner_tag = tag(0);
        let first_key = Hash::new(b"first dormant validation lifecycle");
        let second_key = Hash::new(b"second dormant validation lifecycle");
        let mut ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            RuntimeLifecycleOrdinalSource::after_high_watermark(2),
        );
        ingress
            .install_dormant_local_fifo_reservations(vec![
                RuntimeDormantLocalFifoReservation::completion(first_key, 1, 9),
                RuntimeDormantLocalFifoReservation::completion(second_key, 2, 9),
            ])
            .expect("restart installs two exact completion reservations");
        for value in [1, 2] {
            ingress
                .enqueue(TaggedCommand::new(
                    owner_tag,
                    CommandClass::Completion,
                    FakeCommand::record(value),
                    admitted_at,
                ))
                .expect("ordinary completions fill the unreserved positions");
        }
        assert_eq!(ingress.remaining_capacity(), 0);
        let batch = vec![
            restored_fake_command(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(3),
                first_key,
                1,
                9,
            ),
            restored_fake_command(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(4),
                second_key,
                2,
                9,
            ),
        ];
        ingress
            .enqueue_completion_batch(batch.clone())
            .expect("one atomic batch replaces both latent reservations");
        assert!(ingress.dormant_local_fifo_reservations.is_empty());
        assert_eq!(ingress.len(), 4);
        let next_after_first_batch = ingress.next_admission_ordinal;

        ingress
            .enqueue_completion_batch(batch)
            .expect("repeated exact batch coalesces with physical owners");
        assert_eq!(ingress.len(), 4);
        assert_eq!(
            ingress.next_admission_ordinal, next_after_first_batch,
            "duplicate batch cannot allocate another physical range"
        );
    }

    #[test]
    fn dormant_local_fifo_metadata_rejects_wrong_stage_ordinal_and_capacity() {
        let owner_tag = tag(0);
        let lifecycle_key = Hash::new(b"immutable dormant completion lifecycle");
        let new_ingress = || {
            let mut ingress = BoundedIngress::with_lifecycle_ordinals(
                RuntimeQueueConfig::new(4, 1, 1),
                RuntimeLifecycleOrdinalSource::after_high_watermark(2),
            );
            ingress
                .install_dormant_local_fifo_reservations(vec![
                    RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 8),
                ])
                .expect("install exact dormant metadata");
            ingress
        };

        let mut wrong_stage = new_ingress();
        assert_eq!(
            wrong_stage.enqueue(restored_fake_command(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(1),
                lifecycle_key,
                1,
                9,
            )),
            Err(EnqueueError::FailClosed),
            "a retry cannot change its persisted reducer stage"
        );
        assert_eq!(wrong_stage.remaining_capacity(), 3);

        let mut wrong_ordinal = new_ingress();
        assert_eq!(
            wrong_ordinal.enqueue(restored_fake_command(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(1),
                lifecycle_key,
                2,
                8,
            )),
            Err(EnqueueError::FailClosed),
            "a retry cannot change its immutable lifecycle ordinal"
        );
        assert_eq!(wrong_ordinal.remaining_capacity(), 3);

        let mut over_capacity = BoundedIngress::<FakeCommand>::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            RuntimeLifecycleOrdinalSource::after_high_watermark(5),
        );
        let forged = (1_u128..=5)
            .map(|ordinal| {
                RuntimeDormantLocalFifoReservation::completion(
                    Hash::new(ordinal.to_le_bytes()),
                    ordinal,
                    8,
                )
            })
            .collect();
        assert_eq!(
            over_capacity.install_dormant_local_fifo_reservations(forged),
            Err(EnqueueError::FailClosed),
            "an over-capacity snapshot must fail before live admission"
        );
        assert!(over_capacity.dormant_local_fifo_reservations.is_empty());

        for producer_stage in 0_u8..=u8::MAX {
            if RuntimeDormantLocalFifoReservation::is_local_fifo_stage(producer_stage) {
                continue;
            }
            let mut malformed = BoundedIngress::<FakeCommand>::with_lifecycle_ordinals(
                RuntimeQueueConfig::new(4, 1, 1),
                RuntimeLifecycleOrdinalSource::after_high_watermark(1),
            );
            assert_eq!(
                malformed.install_dormant_local_fifo_reservations(vec![
                    RuntimeDormantLocalFifoReservation::completion(
                        lifecycle_key,
                        1,
                        producer_stage,
                    ),
                ]),
                Err(EnqueueError::FailClosed),
                "nonlocal or unknown stage {producer_stage} cannot forge reserved FIFO capacity"
            );
            assert!(malformed.dormant_local_fifo_reservations.is_empty());
        }
    }

    #[test]
    fn restored_exact_stage_coalesces_at_full_capacity_without_aliasing_successors() {
        let admitted_at = Instant::now();
        let owner_tag = tag(0);
        let lifecycle_key = Hash::new(b"persisted producer lifecycle");
        let mut ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            RuntimeLifecycleOrdinalSource::after_high_watermark(1),
        );
        let restored_with_ordinal = |value, producer_stage, tag, class, lifecycle_ordinal| {
            let command = FakeCommand::record(value);
            let owner = RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
                tag,
                class,
                &command,
                None,
                lifecycle_key,
                lifecycle_ordinal,
            )
            .expect("validated dormant metadata reconstructs one exact owner");
            let mut tagged = TaggedCommand::with_causal_origin(
                tag,
                class,
                command,
                admitted_at,
                owner.causal_origin().clone(),
                owner.lifecycle_ordinal(),
            )
            .expect("restored command binds its persisted ordinal");
            tagged.restored_producer_stage = Some(producer_stage);
            tagged
        };
        let restored_with = |value, producer_stage, tag, class| {
            restored_with_ordinal(value, producer_stage, tag, class, 1)
        };
        let restored = |value, producer_stage| {
            restored_with(value, producer_stage, owner_tag, CommandClass::Completion)
        };
        ingress
            .install_dormant_local_fifo_reservations(vec![
                RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 8),
                RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 9),
            ])
            .expect("restart installs both latent Local FIFO reservations");

        ingress
            .enqueue(restored(1, 8))
            .expect("first restored stage owns one physical position");
        ingress
            .enqueue(restored(2, 9))
            .expect("a distinct causal successor stage shares the lifecycle");
        for value in [3, 4] {
            ingress
                .enqueue(TaggedCommand::new(
                    owner_tag,
                    CommandClass::Completion,
                    FakeCommand::record(value),
                    admitted_at,
                ))
                .expect("fill the remaining physical capacity");
        }
        assert_eq!(ingress.remaining_capacity(), 0);
        let next_before_duplicate = ingress.next_admission_ordinal;

        ingress
            .enqueue(restored(1, 8))
            .expect("the exact restored retry coalesces at full capacity");
        assert_eq!(ingress.len(), 4);
        assert_eq!(
            ingress.next_admission_ordinal, next_before_duplicate,
            "coalescing cannot mint another physical admission ordinal"
        );
        assert_eq!(
            ingress.enqueue(restored_with_ordinal(
                1,
                8,
                owner_tag,
                CommandClass::Completion,
                2,
            )),
            Err(EnqueueError::FailClosed),
            "one restored lifecycle key cannot change its immutable ordinal at the same stage"
        );
        assert_eq!(
            ingress.enqueue(restored_with_ordinal(
                2,
                9,
                owner_tag,
                CommandClass::Completion,
                2,
            )),
            Err(EnqueueError::FailClosed),
            "a restored successor stage cannot change its lifecycle ordinal"
        );
        assert_eq!(
            ingress.enqueue(restored(9, 8)),
            Err(EnqueueError::FailClosed),
            "one persisted lifecycle stage cannot carry conflicting command identity"
        );
        assert_eq!(
            ingress.enqueue(restored_with(1, 8, owner_tag, CommandClass::Progress,)),
            Err(EnqueueError::FailClosed),
            "one persisted lifecycle stage cannot change its service class"
        );
        assert_eq!(
            ingress.enqueue(restored_with(
                1,
                8,
                EventTag::new(
                    owner_tag.height(),
                    owner_tag.view(),
                    Generation::new(owner_tag.generation().get() + 1),
                ),
                CommandClass::Completion,
            )),
            Err(EnqueueError::FailClosed),
            "one queued restart stage cannot change its exact reducer tag"
        );
        let mut changed_origin = restored(1, 8);
        changed_origin.causal_origin.root_ingress_identity =
            Some(Hash::new(b"foreign restored ingress origin"));
        changed_origin.causal_origin.projection_hash =
            runtime_candidate_causal_origin_projection_hash(&changed_origin.causal_origin);
        assert!(changed_origin.validate_admission_identity());
        assert_eq!(
            ingress.enqueue(changed_origin),
            Err(EnqueueError::FailClosed),
            "one persisted lifecycle stage cannot change causal-origin metadata"
        );
        assert_eq!(ingress.len(), 4);
    }

    #[test]
    fn restored_producer_preflight_cannot_change_completion_service_class() {
        let owner_tag = tag(0);
        let mut driver = FakeDriver::new(owner_tag);
        driver.admission_preflight_override =
            Some(RuntimeCommandAdmissionPreflight::ReuseDormant {
                causal_lifecycle_key: Hash::new(b"persisted completion lifecycle"),
                admission_ordinal: 1,
                producer_stage: 5,
            });
        let started_at = Instant::now();
        let mut runtime = runtime(driver, started_at, RuntimeQueueConfig::new(4, 1, 1));

        assert_eq!(
            enqueue_fake(
                &mut runtime,
                owner_tag,
                CommandClass::Progress,
                FakeCommand::record(1),
            ),
            Err(EnqueueError::FailClosed)
        );
        assert!(runtime.fail_closed);
        assert_eq!(
            runtime.ingress.len(),
            0,
            "a caller-class mutation cannot acquire a priority position"
        );
    }

    #[test]
    fn causal_lifecycle_key_ignores_only_process_generation() {
        let first_tag = EventTag::new(9, 4, Generation::new(1));
        let replay_tag = EventTag::new(9, 4, Generation::new(7));
        let different_view = EventTag::new(9, 5, Generation::new(7));
        let command = FakeCommand::record(0xA5);

        let first =
            RuntimeCandidateCausalOrigin::mint(first_tag, CommandClass::Progress, &command, None);
        let replay =
            RuntimeCandidateCausalOrigin::mint(replay_tag, CommandClass::Progress, &command, None);
        let other_view = RuntimeCandidateCausalOrigin::mint(
            different_view,
            CommandClass::Progress,
            &command,
            None,
        );

        assert!(first.same_lifecycle(&replay));
        assert_eq!(first.lifecycle_key, replay.lifecycle_key);
        assert_ne!(
            first.projection_hash, replay.projection_hash,
            "the full diagnostic carrier still records process generation"
        );
        assert!(!first.same_lifecycle(&other_view));
        assert_ne!(first.lifecycle_key, other_view.lifecycle_key);
    }

    #[test]
    fn aggregate_certificate_causal_roots_ignore_signer_carrier_replacement() {
        let (context, keys) = authenticated_runtime_context();
        let owner_tag = tag(0);
        let source_a = PeerId::new(keys[0].public_key().clone());
        let source_b = PeerId::new(keys[1].public_key().clone());
        let tagged_origin = |message: wire::ConsensusMessageV2, source: PeerId| {
            let ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(
                &message,
                fair_runtime_ownership(&message, source.clone(), source),
            )
            .expect("fair ingress yields exact runtime ownership");
            let authenticated = AuthenticatedConsensusMessage::for_test(message);
            assert_eq!(
                authenticated.exact_runtime_command_identity(),
                AdapterCommand::Authenticated(authenticated.clone())
                    .exact_runtime_command_identity(),
                "the authenticated token and adapter wrapper share one exact identity"
            );
            TaggedCommand::with_ingress_ownership(
                owner_tag,
                CommandClass::Progress,
                authenticated,
                Instant::now(),
                ownership,
            )
            .causal_origin
        };

        let qc_a = signed_runtime_quorum_certificate(&context, &keys, 0xD1);
        let mut qc_b = qc_a.clone();
        qc_b.signers.rotate_left(1);
        qc_b.aggregate_signature = vec![0xB2; qc_b.aggregate_signature.len()];
        let qc_origin_a = tagged_origin(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(qc_a)),
            source_a.clone(),
        );
        let qc_origin_b = tagged_origin(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(qc_b)),
            source_b.clone(),
        );
        assert!(qc_origin_a.same_lifecycle(&qc_origin_b));

        let tc_a = signed_runtime_timeout_certificate(&context, &keys);
        let mut tc_b = tc_a.clone();
        tc_b.groups[0].signers.rotate_left(1);
        tc_b.groups[0].aggregate_signature = vec![0xC3; tc_b.groups[0].aggregate_signature.len()];
        let tc_message_a = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(tc_a.clone()),
        );
        let tc_message_b = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(tc_b),
        );
        let exact_tc_a = AdapterCommand::Authenticated(AuthenticatedConsensusMessage::for_test(
            tc_message_a.clone(),
        ))
        .exact_runtime_command_identity()
        .digest();
        let exact_tc_b = AdapterCommand::Authenticated(AuthenticatedConsensusMessage::for_test(
            tc_message_b.clone(),
        ))
        .exact_runtime_command_identity()
        .digest();
        assert_ne!(
            exact_tc_a, exact_tc_b,
            "deep command identity still distinguishes replaceable certificate carriers"
        );
        let tc_origin_a = tagged_origin(tc_message_a, source_a);
        let tc_origin_b = tagged_origin(tc_message_b, source_b.clone());
        assert!(tc_origin_a.same_lifecycle(&tc_origin_b));

        let mut other_round = tc_a;
        other_round.round.view = other_round.round.view.saturating_add(1);
        let other_round_origin = tagged_origin(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
                other_round,
            )),
            source_b,
        );
        assert!(
            !tc_origin_a.same_lifecycle(&other_round_origin),
            "transition-relevant certified round cannot collide with carrier normalization"
        );
    }

    #[test]
    fn class_cursor_advances_from_the_served_class_after_empty_classes() {
        let admitted_at = Instant::now();
        let initial = tag(0);
        let queued = |class, value| {
            TaggedCommand::new(initial, class, FakeCommand::record(value), admitted_at)
        };
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(6, 2, 1));

        ingress
            .enqueue(queued(CommandClass::Normal, 1))
            .expect("normal command fits the bounded ingress");
        let first = ingress.pop_next().expect("normal class is reachable");
        assert_eq!(first.command.record, Some(1));
        assert_eq!(ingress.next_class, CommandClass::Completion);

        ingress
            .enqueue(queued(CommandClass::Normal, 2))
            .expect("second normal command fits the bounded ingress");
        ingress
            .enqueue(queued(CommandClass::Completion, 3))
            .expect("completion reserve remains available");
        let second = ingress.pop_next().expect("completion class is selected");
        assert_eq!(second.command.record, Some(3));
        assert_eq!(ingress.next_class, CommandClass::Progress);

        let third = ingress
            .pop_next()
            .expect("empty progress class is skipped to normal");
        assert_eq!(third.command.record, Some(2));
        assert_eq!(ingress.next_class, CommandClass::Completion);
    }

    #[test]
    fn production_ingress_pop_uses_shared_selector_for_every_ready_mask() {
        let admitted_at = Instant::now();
        let initial = tag(0);
        for cursor in [
            CommandClass::Completion,
            CommandClass::Progress,
            CommandClass::Normal,
        ] {
            for ready_mask in 0u8..8 {
                let completion_ready = ready_mask & 0b001 != 0;
                let progress_ready = ready_mask & 0b010 != 0;
                let normal_ready = ready_mask & 0b100 != 0;
                let expected = select_bounded_service_class(
                    cursor.service_code(),
                    completion_ready,
                    progress_ready,
                    normal_ready,
                );
                let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(6, 2, 1));
                ingress.next_class = cursor;
                for (class, ready) in [
                    (CommandClass::Normal, normal_ready),
                    (CommandClass::Progress, progress_ready),
                    (CommandClass::Completion, completion_ready),
                ] {
                    if ready {
                        ingress
                            .enqueue(TaggedCommand::new(
                                initial,
                                class,
                                FakeCommand::record(class.service_code()),
                                admitted_at,
                            ))
                            .expect("one command per ready class fits reserved ingress");
                    }
                }

                let selected = ingress.pop_next();
                assert_eq!(
                    selected.as_ref().and_then(|queued| queued.command.record),
                    (expected.selected != SERVICE_CLASS_NONE).then_some(expected.selected),
                );
                assert_eq!(ingress.next_class.service_code(), expected.next);
            }
        }
    }

    #[test]
    fn healthy_same_class_fifo_depth_does_not_accrue_service_debt() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );
        for id in 0..4 {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(id),
            )
            .expect("enqueue same-class work");
        }

        let _ = runtime.step(start);
        let queue = runtime.queue_snapshot(start);
        assert_eq!(queue.normal.depth, 3);
        assert_eq!(queue.normal.max_service_debt, 0);
    }

    #[test]
    fn canonical_body_completion_prunes_only_conflicting_queued_proposals() {
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"queued-body-context",
            ))),
            height: 7,
            view: 2,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"queued-body-block")),
            payload_hash: Hash::new(b"queued-body-payload"),
        };
        let layout = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::Plain,
            chunk_size_bytes: 1,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 1,
            max_chunk_count: 1,
        };
        let canonical = wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout,
            chunk_hashes: vec![Hash::new(b"canonical chunk")],
            chunk_root: Hash::new(b"canonical root"),
        };
        let conflicting = wire::PayloadManifest {
            chunk_hashes: vec![Hash::new(b"conflicting chunk")],
            chunk_root: Hash::new(b"conflicting root"),
            ..canonical.clone()
        };
        let other_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"other queued block")),
            payload_hash: Hash::new(b"other queued payload"),
            ..subject
        };
        let other = wire::PayloadManifest {
            subject: other_subject,
            ..conflicting.clone()
        };

        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(8, 1, 1));
        for (command_tag, manifest) in [
            (tag(0), conflicting.clone()),
            (tag(1), canonical.clone()),
            (tag(2), other.clone()),
        ] {
            ingress
                .enqueue(TaggedCommand::new(
                    command_tag,
                    CommandClass::Normal,
                    AdapterCommand::Authenticated(authenticated_proposal_for_test(manifest)),
                    Instant::now(),
                ))
                .expect("queue authenticated proposal");
        }

        ingress
            .enqueue_canonical_body_available(tag(3), canonical.clone())
            .expect("trusted completion prunes its conflicting proposal and appends in FIFO order");
        assert_eq!(ingress.len(), 3);
        assert!(
            ingress.conflicts_with_pending_body_available(&authenticated_proposal_for_test(
                conflicting
            ))
        );
        assert!(
            !ingress
                .conflicts_with_pending_body_available(&authenticated_proposal_for_test(canonical))
        );
        assert!(
            !ingress.conflicts_with_pending_body_available(&authenticated_proposal_for_test(other))
        );

        let retained_tags = ingress
            .commands
            .iter()
            .map(|queued| queued.tag)
            .collect::<Vec<_>>();
        assert_eq!(retained_tags, vec![tag(1), tag(2), tag(3)]);
        let committed = ingress
            .commands
            .back()
            .expect("canonical completion remains at the queue tail");
        assert_eq!(committed.tag, tag(3));
        assert_eq!(committed.class, CommandClass::Completion);
        assert_eq!(committed.admission_ordinal, Some(3));
        assert!(matches!(
            ingress.commands.back().map(|queued| &queued.command),
            Some(AdapterCommand::BodyAvailable { manifest }) if manifest.subject == subject
        ));
    }

    #[test]
    fn unpublished_body_completion_reservation_fences_conflicting_proposals() {
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"reserved-body-context",
            ))),
            height: 8,
            view: 3,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"reserved-body-block")),
            payload_hash: Hash::new(b"reserved-body-payload"),
        };
        let canonical = wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1,
                max_chunk_count: 1,
            },
            chunk_hashes: vec![Hash::new(b"reserved canonical chunk")],
            chunk_root: Hash::new(b"reserved canonical root"),
        };
        let conflicting = wire::PayloadManifest {
            chunk_hashes: vec![Hash::new(b"reserved conflicting chunk")],
            chunk_root: Hash::new(b"reserved conflicting root"),
            ..canonical.clone()
        };
        let canonical_proposal = authenticated_proposal_for_test(canonical.clone());
        let conflicting_proposal = authenticated_proposal_for_test(conflicting);
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(1, 0, 0));

        let reservation = ingress
            .reserve_canonical_body_available(tag(0), canonical)
            .expect("the unpublished completion atomically claims capacity and an ordinal");
        assert_eq!(ingress.len(), 0, "reservation is not reducer-visible");
        assert_eq!(ingress.remaining_capacity(), 0);
        assert_eq!(reservation.admission_ordinal, Some(1));
        assert!(
            ingress.conflicts_with_pending_body_available(&conflicting_proposal),
            "the unpublished canonical manifest must already fence a conflicting proposal"
        );
        assert!(
            !ingress.conflicts_with_pending_body_available(&canonical_proposal),
            "an exact proposal does not conflict with its reserved completion"
        );

        let mut mismatched = reservation.clone();
        mismatched.tag = tag(1);
        assert_eq!(
            ingress.commit_canonical_body_available(mismatched),
            Err(EnqueueError::FailClosed),
            "a stale or mismatched token must not silently lose the completion"
        );
        assert_eq!(ingress.len(), 0);
        assert_eq!(
            ingress.reserved_body_available.as_ref(),
            Some(&reservation),
            "a rejected token preserves the exact unpublished owner"
        );

        ingress
            .commit_canonical_body_available(reservation)
            .expect("the exact reservation token publishes its completion");
        let completion = ingress
            .commands
            .front()
            .expect("commit publishes the already-owned completion slot");
        assert_eq!(completion.admission_ordinal, Some(1));
        assert_eq!(completion.lifecycle_ordinal, Some(1));
        assert!(ingress.conflicts_with_pending_body_available(&conflicting_proposal));
    }

    #[test]
    fn aborted_body_completion_retry_reclaims_the_entire_token_without_reminting() {
        let directory = TempDir::new().expect("temporary body retry directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(3, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0xB1);
        runtime
            .enqueue_network(signed_runtime_proposal(&context, &keys, 0xB3))
            .expect("ordinary ingress occupies its sole unreserved slot");
        runtime
            .enqueue_network(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(
                    signed_runtime_timeout_certificate(&context, &keys),
                ),
            ))
            .expect("certified progress occupies the progress slot");
        assert_eq!(runtime.remaining_completion_capacity(), 1);
        let reservation = runtime
            .reserve_body_available(owner_tag, manifest.clone())
            .expect("reserve one unpublished exact completion");
        assert_eq!(runtime.remaining_completion_capacity(), 0);
        let source_after_reserve = runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect source after body reservation");

        let mut mismatched_abort = reservation.clone();
        mismatched_abort.tag = tag(1);
        runtime.abort_body_available(mismatched_abort);
        assert_eq!(
            runtime.ingress.reserved_body_available.as_ref(),
            Some(&reservation),
            "a mismatched abort has no authority to clear the exact token",
        );
        runtime.abort_body_available(reservation.clone());
        assert_eq!(
            runtime.ingress.reserved_body_available.as_ref(),
            Some(&reservation),
            "abort retains the exact token instead of orphaning its ordinal",
        );
        let retry = runtime
            .reserve_body_available(owner_tag, manifest.clone())
            .expect("exact retry reclaims the unpublished token");
        assert_eq!(retry, reservation);
        assert_eq!(
            runtime
                .ingress
                .lifecycle_ordinals
                .next_ordinal_for_test()
                .expect("inspect source after exact retry"),
            source_after_reserve,
            "exact retry cannot mint a second physical ordinal",
        );

        let competing_ordinal = runtime
            .ingress
            .lifecycle_ordinals
            .reserve_one()
            .expect("advance the shared source through another actor owner");
        let source_before_materialization = runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect actual shared source before materialization");
        assert_eq!(Some(competing_ordinal), source_after_reserve);
        runtime
            .commit_body_available(retry)
            .expect("materialize the exact retained reservation");
        assert_eq!(
            runtime
                .ingress
                .lifecycle_ordinals
                .next_ordinal_for_test()
                .expect("materialization preserves the shared source"),
            source_before_materialization,
            "materialization observes but never advances the current source",
        );
    }

    #[test]
    fn conflicting_body_completion_retry_latches_without_replacing_the_exact_token() {
        let directory = TempDir::new().expect("temporary body conflict directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0xB4);
        let reservation = runtime
            .reserve_body_available(owner_tag, manifest.clone())
            .expect("reserve one unpublished exact completion");
        let source_after_reserve = runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect source after body reservation");
        let conflicting = wire::PayloadManifest {
            chunk_root: Hash::new(b"conflicting retained body root"),
            chunk_hashes: vec![Hash::new(b"conflicting retained body chunk")],
            ..manifest
        };

        assert_eq!(
            runtime.reserve_body_available(owner_tag, conflicting),
            Err(EnqueueError::DuplicateCompletionOwnership),
            "same logical slot with different evidence cannot replace the retained token",
        );
        assert!(runtime.fail_closed);
        assert_eq!(
            runtime.ingress.reserved_body_available.as_ref(),
            Some(&reservation),
        );
        assert_eq!(
            runtime
                .ingress
                .lifecycle_ordinals
                .next_ordinal_for_test()
                .expect("inspect source after rejected conflict"),
            source_after_reserve,
            "conflicting evidence cannot burn a fresh physical ordinal",
        );
    }

    #[test]
    fn dormant_body_reservation_aliases_full_capacity_across_abort_retry_and_commit() {
        let directory = TempDir::new().expect("temporary dormant body retry directory");
        let (_runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let owner_tag = tag(0);
        let manifest = runtime_manifest(&context, 0xB2);
        let lifecycle_key = Hash::new(b"dormant body completion lifecycle");
        let body_command = AdapterCommand::BodyAvailable {
            manifest: manifest.clone(),
        };
        let owner = RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
            owner_tag,
            CommandClass::Completion,
            &body_command,
            None,
            lifecycle_key,
            1,
        )
        .expect("restore exact dormant body owner");
        let dormant = RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 8);
        let source = RuntimeLifecycleOrdinalSource::after_high_watermark(1);
        let mut ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(1, 0, 0),
            source.clone(),
        );
        ingress
            .install_dormant_local_fifo_reservations(vec![dormant])
            .expect("install the full-capacity dormant completion owner");
        assert_eq!(ingress.remaining_capacity(), 0);

        let reservation = ingress
            .reserve_canonical_body_available_internal(
                owner_tag,
                manifest.clone(),
                Some(&owner),
                Some(8),
            )
            .expect("unpublished token aliases the dormant capacity owner");
        assert_eq!(reservation.lifecycle_ordinal, Some(1));
        assert_eq!(reservation.admission_ordinal, Some(2));
        assert_eq!(reservation.dormant_replacement, Some(dormant));
        assert!(ingress.dormant_local_fifo_reservations.contains(&dormant));
        assert_eq!(ingress.remaining_capacity(), 0);
        let source_after_reserve = source
            .next_ordinal_for_test()
            .expect("inspect source after dormant reservation");

        ingress.abort_canonical_body_available(reservation.clone());
        let retry = ingress
            .reserve_canonical_body_available_internal(owner_tag, manifest, Some(&owner), Some(8))
            .expect("exact dormant retry reclaims the whole token");
        assert_eq!(retry, reservation);
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("inspect source after dormant retry"),
            source_after_reserve,
        );
        assert_eq!(
            ingress.reserve_canonical_body_available_internal(
                owner_tag,
                retry.manifest().clone(),
                Some(&owner),
                Some(9),
            ),
            Err(EnqueueError::FailClosed),
            "retry cannot replace the exact dormant stage",
        );
        assert_eq!(ingress.reserved_body_available.as_ref(), Some(&reservation));

        let source_before_failed_commit = source
            .next_ordinal_for_test()
            .expect("inspect source before rejected dormant commit");
        let mut mismatched_commit = retry.clone();
        mismatched_commit.tag = tag(1);
        assert_eq!(
            ingress.commit_canonical_body_available(mismatched_commit),
            Err(EnqueueError::FailClosed),
        );
        assert_eq!(ingress.reserved_body_available.as_ref(), Some(&reservation));
        assert!(ingress.dormant_local_fifo_reservations.contains(&dormant));
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("rejected dormant commit preserves the source"),
            source_before_failed_commit,
        );

        ingress
            .commit_canonical_body_available(retry)
            .expect("materialization atomically replaces token and dormant backing");
        assert!(ingress.reserved_body_available.is_none());
        assert!(ingress.dormant_local_fifo_reservations.is_empty());
        assert_eq!(ingress.len(), 1);
        assert_eq!(ingress.commands[0].admission_ordinal, Some(2));
        assert_eq!(ingress.commands[0].lifecycle_ordinal, Some(1));
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("materialization preserves the source"),
            source_after_reserve,
        );
    }

    #[test]
    fn mismatched_body_completion_commit_fails_closed_without_losing_reservation() {
        let directory = TempDir::new().expect("temporary body reservation directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0xA4);
        let reservation = runtime
            .reserve_body_available(owner_tag, manifest)
            .expect("reserve the exact unpublished completion");
        let exact = reservation.clone();
        let mut mismatched = reservation;
        mismatched.tag = tag(1);

        assert_eq!(
            runtime.commit_body_available(mismatched),
            Err(EnqueueError::FailClosed)
        );
        assert!(runtime.fail_closed);
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(
            runtime.ingress.reserved_body_available.as_ref(),
            Some(&exact),
            "the invalid token cannot consume the exact reserved owner"
        );
    }
