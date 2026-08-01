
    #[test]
    fn exact_authenticated_retransmission_preserves_capacity_fifo_and_cursor() {
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"coalesced-capacity-context",
            ))),
            height: 9,
            view: 4,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"coalesced-capacity-block")),
            payload_hash: Hash::new(b"coalesced-capacity-payload"),
        };
        let payload = |signature| {
            wire::ConsensusMessageV2Payload::QuorumCertificate(wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                    Hash::new(b"capacity parent state"),
                    Hash::new(b"capacity post state"),
                    Hash::new(b"capacity ordinary writes"),
                    1,
                    Hash::new(b"capacity executed block wire"),
                ),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![signature],
            })
        };
        let authenticated = |signature| {
            AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(payload(
                signature,
            )))
        };
        let queued_wire = wire::ConsensusMessageV2::new(payload(1));
        let transport = wire::ConsensusMessageV2Payload::PayloadManifest(wire::PayloadManifest {
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
            chunk_hashes: vec![Hash::new(b"coalesced capacity chunk")],
            chunk_root: Hash::new(b"coalesced capacity root"),
        });
        assert!(matches!(
            classify_reducer_network_ingress(false, &queued_wire.payload),
            Ok(CommandClass::Progress)
        ));
        assert!(matches!(
            classify_reducer_network_ingress(false, &transport),
            Err(NetworkIngressError::TransportPayload)
        ));
        assert!(matches!(
            classify_reducer_network_ingress(true, &queued_wire.payload),
            Err(NetworkIngressError::FailClosed)
        ));
        assert!(matches!(
            classify_reducer_network_ingress(true, &transport),
            Err(NetworkIngressError::FailClosed)
        ));
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(4, 1, 1));

        assert_eq!(
            ingress
                .enqueue_authenticated(tag(0), CommandClass::Normal, authenticated(1))
                .expect("first wire value enters below the normal boundary"),
            tag(0)
        );
        assert_eq!(
            ingress
                .enqueue_authenticated(tag(1), CommandClass::Normal, authenticated(2))
                .expect("a non-identical wire value uses ordinary capacity"),
            tag(1)
        );
        assert_eq!(
            ingress.check_capacity(CommandClass::Normal),
            Err(EnqueueError::ReservedCapacity)
        );

        let cursor_before = ingress.next_class;
        let tags_before = ingress
            .commands
            .iter()
            .map(|queued| queued.tag)
            .collect::<Vec<_>>();
        assert_eq!(
            ingress
                .enqueue_authenticated(tag(8), CommandClass::Normal, authenticated(1))
                .expect("an exact duplicate coalesces at reserved capacity"),
            tag(0),
            "coalescing deterministically returns the original admission tag"
        );
        assert_eq!(ingress.next_class, cursor_before);
        assert_eq!(
            ingress
                .commands
                .iter()
                .map(|queued| queued.tag)
                .collect::<Vec<_>>(),
            tags_before,
            "coalescing changes neither FIFO ownership nor its tags"
        );
        assert_eq!(
            ingress.enqueue_authenticated(tag(9), CommandClass::Normal, authenticated(3)),
            Err(EnqueueError::ReservedCapacity),
            "a non-identical envelope still obeys the normal boundary"
        );

        ingress
            .enqueue_authenticated(tag(2), CommandClass::Progress, authenticated(3))
            .expect("progress reserve remains independent");
        ingress
            .enqueue_authenticated(tag(3), CommandClass::Completion, authenticated(4))
            .expect("completion reserve fills the final slot");
        assert_eq!(ingress.len(), 4);
        assert_eq!(
            ingress.check_capacity(CommandClass::Completion),
            Err(EnqueueError::Full)
        );
        assert_eq!(ingress.authenticated_wire_tag(&queued_wire), Some(tag(0)));
        assert!(
            ingress
                .check_authenticated_wire_capacity(&queued_wire, CommandClass::Normal, false,)
                .is_ok(),
            "raw equality only opens the authentication attempt at full capacity"
        );
        assert_eq!(
            ingress.check_authenticated_wire_capacity(
                &wire::ConsensusMessageV2::new(payload(5)),
                CommandClass::Normal,
                false,
            ),
            Err(EnqueueError::Full)
        );

        let full_tags = ingress
            .commands
            .iter()
            .map(|queued| queued.tag)
            .collect::<Vec<_>>();
        assert_eq!(
            ingress
                .enqueue_authenticated(tag(10), CommandClass::Normal, authenticated(1))
                .expect("the exact envelope coalesces even when every slot is owned"),
            tag(0)
        );
        assert_eq!(ingress.next_class, cursor_before);
        assert_eq!(
            ingress
                .commands
                .iter()
                .map(|queued| queued.tag)
                .collect::<Vec<_>>(),
            full_tags
        );
        assert!(
            ingress
                .commands
                .iter()
                .all(|queued| queued.eligible_skips == 0)
        );
        assert_eq!(
            ingress.enqueue_authenticated(tag(11), CommandClass::Progress, authenticated(5)),
            Err(EnqueueError::Full),
            "wire inequality cannot inherit the duplicate's full-queue exception"
        );
    }

    #[test]
    fn completion_retries_coalesce_across_ingress_and_busy_deferred_ownership() {
        let directory = TempDir::new().expect("temporary completion-coalescing directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = runtime.round_tag();
        let receipts = |manifest: &wire::PayloadManifest| {
            let durable = DurableBodyReceipt::for_test(
                context.id(),
                manifest.round,
                manifest.subject,
                HashOf::new(manifest),
            );
            let validated = ValidatedBodyReceipt::for_test(durable.clone());
            (durable, validated)
        };

        let ingress_manifest = runtime_manifest(&context, 0x91);
        let (durable, _) = receipts(&ingress_manifest);
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::BodyStored {
                round: ingress_manifest.round,
                subject: ingress_manifest.subject,
                receipt: durable.clone(),
            },
        );
        runtime
            .enqueue_body_stored(
                owner_tag,
                ingress_manifest.round,
                ingress_manifest.subject,
                durable,
            )
            .expect("an exact retransmission coalesces in runtime ingress");
        assert_eq!(runtime.queued_commands(), 1);
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    ingress_manifest.round,
                    ingress_manifest.subject,
                )
                .expect("retire the one coalesced ingress owner"),
            RetiredBodyPipelineCompletions {
                body_available: 0,
                body_stored: 1,
                validation: 0,
                local_proposal: 0,
            }
        );

        let deferred_store = runtime_manifest(&context, 0x92);
        let (durable, _) = receipts(&deferred_store);
        let active_before_store = runtime.driver.all_deferred_admission_ordinals();
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &deferred_store,
                DeferredBodyPipelineStageForTest::BodyStored,
            )
            .expect("stage a Busy-deferred durable-store completion");
        let store_ordinals = runtime
            .driver
            .all_deferred_admission_ordinals()
            .difference(&active_before_store)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(store_ordinals.len(), 1);
        let store_owner = bind_local_deferred_lifecycle_for_test(
            &mut runtime,
            store_ordinals[0],
            b"body-store-pipeline-retirement-owner",
        );
        runtime
            .enqueue_body_stored(
                owner_tag,
                deferred_store.round,
                deferred_store.subject,
                durable,
            )
            .expect("a retransmit coalesces with the Busy-deferred store owner");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(
            runtime
                .minimum_active_lifecycle_ordinal()
                .expect("inspect the exact Busy-deferred store owner"),
            Some(store_owner.lifecycle_ordinal())
        );
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    deferred_store.round,
                    deferred_store.subject,
                )
                .expect("retire the coalesced Busy-deferred store owner"),
            RetiredBodyPipelineCompletions {
                body_available: 0,
                body_stored: 1,
                validation: 0,
                local_proposal: 0,
            }
        );
        assert!(runtime.deferred_lifecycle_ownership.is_empty());
        assert!(runtime.deferred_ingress_ownership.is_empty());
        assert_eq!(
            runtime
                .minimum_active_lifecycle_ordinal()
                .expect("retirement cannot retain a phantom store owner"),
            None
        );

        let deferred_validation = runtime_manifest(&context, 0x93);
        let (_, validated) = receipts(&deferred_validation);
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &deferred_validation,
                DeferredBodyPipelineStageForTest::ValidationSucceeded,
            )
            .expect("stage a Busy-deferred validation completion");
        runtime
            .enqueue_validation_succeeded(
                owner_tag,
                deferred_validation.round,
                deferred_validation.subject,
                validated,
            )
            .expect("a retransmit coalesces with the Busy-deferred validation owner");
        assert_eq!(runtime.queued_commands(), 0);
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                deferred_validation.round,
                deferred_validation.subject,
            )
            .expect("retire the coalesced Busy-deferred validation owner");

        let deferred_proposal = runtime_manifest(&context, 0x94);
        let (durable, validated) = receipts(&deferred_proposal);
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &deferred_proposal,
                DeferredBodyPipelineStageForTest::LocalProposalReady,
            )
            .expect("stage a Busy-deferred local-proposal completion");
        runtime
            .enqueue_local_proposal(owner_tag, deferred_proposal.clone(), durable, validated)
            .expect("a retransmit coalesces with the Busy-deferred proposal owner");
        assert_eq!(runtime.queued_commands(), 0);
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                deferred_proposal.round,
                deferred_proposal.subject,
            )
            .expect("retire the coalesced Busy-deferred proposal owner");
    }

    #[test]
    fn body_available_rebind_rejects_uninstalled_destination_without_mutation() {
        let directory = TempDir::new().expect("temporary uninstalled-rebind directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let source_tag = runtime.round_tag();
        let fabricated = EventTag::new(
            source_tag.height(),
            source_tag.view() + 1,
            Generation::new(source_tag.generation().get() + 1),
        );
        let manifest = runtime_manifest(&context, 0x8B);
        runtime
            .enqueue_body_available(source_tag, manifest.clone())
            .expect("enqueue unique source owner");

        assert_eq!(
            runtime
                .rebind_body_available(source_tag, fabricated, &manifest)
                .expect_err("an uninstalled destination tag must be rejected"),
            "Sumeragi v2 body completion rebind target is not the installed runtime incarnation"
        );
        assert!(
            !runtime.fail_closed,
            "caller contract rejection is recoverable"
        );
        assert_eq!(runtime.round_tag(), source_tag);
        assert_eq!(runtime.queued_commands(), 1);
        assert!(matches!(
            runtime.ingress.commands.front(),
            Some(TaggedCommand {
                tag,
                command: AdapterCommand::BodyAvailable {
                    manifest: queued_manifest,
                },
                ..
            }) if *tag == source_tag && queued_manifest == &manifest
        ));
        assert!(
            runtime
                .retire_body_available(source_tag, &manifest)
                .expect("the untouched source owner remains retireable")
        );
        assert_eq!(runtime.queued_commands(), 0);
    }

    #[test]
    fn body_available_rebind_coalesces_exact_busy_deferred_destination_owner() {
        let directory = TempDir::new().expect("temporary destination-coalescing directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(0),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime for production dispatch");
        runtime
            .enqueue_network(signed_runtime_proposal(&context, &keys, 0x8C))
            .expect("enqueue authenticated proposal");
        let proposal_effects = match runtime.step(now).expect("dispatch proposal") {
            RuntimeStep::Advanced(effects) => effects,
            RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
        };
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("proposal dispatch publishes exact scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::Fifo
        );
        let (source_tag, manifest) = match proposal_effects.as_slice() {
            [
                AdapterEffect::FetchBody {
                    tag,
                    manifest: Some(manifest),
                    ..
                },
            ] => (*tag, manifest.clone()),
            effects => panic!("unexpected proposal effects: {effects:?}"),
        };

        runtime
            .enqueue_body_available(source_tag, manifest.clone())
            .expect("enqueue body reconstruction completion");
        assert!(matches!(
            runtime.step(now).expect("dispatch body reconstruction"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::StoreBody { .. }])
        ));
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("body reconstruction publishes exact scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::Fifo
        );
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        runtime
            .enqueue_body_stored(
                source_tag,
                manifest.round,
                manifest.subject,
                durable.clone(),
            )
            .expect("enqueue durable-store completion");
        assert!(matches!(
            runtime.step(now).expect("dispatch durable-store completion"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::ValidateBody { .. }])
        ));
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("durable-store completion publishes exact scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::Fifo
        );
        runtime
            .enqueue_validation_succeeded(
                source_tag,
                manifest.round,
                manifest.subject,
                ValidatedBodyReceipt::for_test(durable),
            )
            .expect("enqueue validation completion");
        assert!(matches!(
            runtime.step(now).expect("dispatch validation completion"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::Sign { .. }])
        ));
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("validation completion publishes exact scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::Fifo
        );

        let rebound = EventTag::new(
            source_tag.height(),
            source_tag.view() + 1,
            Generation::new(source_tag.generation().get() + 1),
        );
        let active_before_body = runtime.driver.all_deferred_admission_ordinals();
        assert!(
            runtime
                .driver
                .body_available(source_tag, manifest.clone())
                .expect("stage exact completion behind the signer fence")
                .into_effects()
                .is_empty()
        );
        let body_ordinals = runtime
            .driver
            .all_deferred_admission_ordinals()
            .difference(&active_before_body)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(body_ordinals.len(), 1);
        let body_ordinal = body_ordinals[0];
        let body_owner = bind_local_deferred_lifecycle_for_test(
            &mut runtime,
            body_ordinal,
            b"body-available-retirement-owner",
        );
        let evidence = BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: manifest.clone(),
        };
        assert_eq!(
            runtime
                .driver
                .deferred_body_pipeline_completion_ownership(source_tag, &evidence),
            (1, 1),
            "the current tag owns the real Busy-deferred completion"
        );
        observe_enter_view_for_test(&mut runtime, source_tag, rebound, &manifest);
        assert_eq!(
            runtime
                .driver
                .rebind_deferred_body_available(source_tag, rebound, &manifest),
            1,
            "the seam models an exact destination owner already transferred by another path"
        );
        assert_eq!(
            runtime
                .driver
                .deferred_body_pipeline_completion_ownership(rebound, &evidence),
            (1, 1),
            "the destination must be owned by the real Busy-deferred lane"
        );
        stage_completion_for_queue_test(
            &mut runtime,
            source_tag,
            AdapterCommand::BodyAvailable {
                manifest: manifest.clone(),
            },
        );
        assert_eq!(runtime.queued_commands(), 1);

        assert!(
            runtime
                .rebind_body_available(source_tag, rebound, &manifest)
                .expect("exact destination ownership coalesces the source")
        );
        assert!(!runtime.fail_closed);
        assert_eq!(runtime.queued_commands(), 0, "the source owner was retired");
        assert_eq!(
            runtime
                .driver
                .deferred_body_pipeline_completion_ownership(rebound, &evidence),
            (1, 1),
            "coalescing retains exactly one destination owner"
        );
        assert_eq!(
            runtime
                .deferred_lifecycle_ownership
                .get(&body_ordinal)
                .map(RuntimeDeferredLifecycleOwnership::owner),
            Some(&body_owner),
            "coalescing cannot retire the wrapper of the retained Busy owner"
        );
        assert!(
            !runtime
                .rebind_body_available(source_tag, rebound, &manifest)
                .expect("an idempotent retry finds no remaining source owner")
        );
        let same_view_rebound = EventTag::new(
            rebound.height(),
            rebound.view(),
            Generation::new(rebound.generation().get() + 1),
        );
        observe_enter_view_for_test(&mut runtime, rebound, same_view_rebound, &manifest);
        assert!(
            runtime
                .rebind_body_available(rebound, same_view_rebound, &manifest)
                .expect("same-view generation supersession transfers the Busy-deferred owner")
        );
        assert_eq!(
            runtime
                .driver
                .deferred_body_pipeline_completion_ownership(same_view_rebound, &evidence),
            (1, 1),
            "same-view rebinding leaves exactly one Busy-deferred destination"
        );
        assert!(
            runtime
                .deferred_lifecycle_ownership
                .contains_key(&body_ordinal)
        );
        assert!(
            runtime
                .retire_body_available(same_view_rebound, &manifest)
                .expect("the unique destination owner remains retireable")
        );
        assert!(
            !runtime
                .deferred_lifecycle_ownership
                .contains_key(&body_ordinal),
            "retirement cannot leave the drained Busy owner at the global minimum"
        );
        assert!(runtime.deferred_ingress_ownership.is_empty());

        // Exercise the opposite coalescing direction: a Busy source loses to
        // an already-installed FIFO destination. The adapter occurrence and
        // its sealed runtime wrapper must retire in the same transition.
        let retirement_directory =
            TempDir::new().expect("temporary Busy-source coalescing directory");
        let (mut retirement_runtime, retirement_context, _keys) =
            authenticated_network_runtime(&retirement_directory, RuntimeQueueConfig::new(8, 1, 1));
        let retirement_source = retirement_runtime.round_tag();
        let retirement_manifest = runtime_manifest(&retirement_context, 0x8F);
        retirement_runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                retirement_source,
                &retirement_manifest,
                DeferredBodyPipelineStageForTest::BodyAvailable,
            )
            .expect("stage the exact Busy source completion");
        let retirement_ordinals = retirement_runtime
            .driver
            .all_deferred_admission_ordinals()
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(retirement_ordinals.len(), 1);
        let retirement_ordinal = retirement_ordinals[0];
        bind_local_deferred_lifecycle_for_test(
            &mut retirement_runtime,
            retirement_ordinal,
            b"body-available-rebind-retirement-owner",
        );
        let retirement_rebound = EventTag::new(
            retirement_source.height(),
            retirement_source.view() + 1,
            Generation::new(retirement_source.generation().get() + 1),
        );
        observe_enter_view_for_test(
            &mut retirement_runtime,
            retirement_source,
            retirement_rebound,
            &retirement_manifest,
        );
        stage_completion_for_queue_test(
            &mut retirement_runtime,
            retirement_rebound,
            AdapterCommand::BodyAvailable {
                manifest: retirement_manifest.clone(),
            },
        );
        assert!(
            retirement_runtime
                .rebind_body_available(retirement_source, retirement_rebound, &retirement_manifest,)
                .expect("the existing FIFO destination coalesces the Busy source")
        );
        assert!(
            !retirement_runtime
                .deferred_lifecycle_ownership
                .contains_key(&retirement_ordinal),
            "Busy-source coalescing cannot leave its runtime wrapper alive"
        );
        assert!(
            !retirement_runtime
                .driver
                .all_deferred_admission_ordinals()
                .contains(&retirement_ordinal)
        );
        assert_eq!(retirement_runtime.queued_commands(), 1);
        assert!(
            retirement_runtime
                .retire_body_available(retirement_rebound, &retirement_manifest)
                .expect("the retained FIFO destination remains uniquely retireable")
        );
        assert_eq!(retirement_runtime.queued_commands(), 0);
    }

    #[test]
    fn body_available_rebind_destination_conflicts_and_duplicates_fail_closed_before_mutation() {
        {
            let directory = TempDir::new().expect("temporary destination-conflict directory");
            let (mut runtime, context, _keys) =
                authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
            let source_tag = runtime.round_tag();
            let rebound = EventTag::new(
                source_tag.height(),
                source_tag.view() + 1,
                Generation::new(source_tag.generation().get() + 1),
            );
            let manifest = runtime_manifest(&context, 0x8D);
            observe_enter_view_for_test(&mut runtime, source_tag, rebound, &manifest);
            let mut conflicting = manifest.clone();
            conflicting.chunk_hashes[0] = Hash::new(b"conflicting rebound chunk");
            conflicting.chunk_root = Hash::new(b"conflicting rebound root");
            runtime
                .enqueue_body_available(source_tag, manifest.clone())
                .expect("enqueue unique source owner");
            runtime
                .ingress
                .enqueue_canonical_body_available(rebound, conflicting.clone())
                .expect("test seam stages conflicting destination evidence");

            assert_eq!(
                runtime
                    .rebind_body_available(source_tag, rebound, &manifest)
                    .expect_err("conflicting destination evidence must fail closed"),
                "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
            );
            assert!(runtime.fail_closed);
            assert_eq!(runtime.queued_commands(), 2);
            assert!(runtime.ingress.commands.iter().any(|queued| matches!(
                &queued.command,
                AdapterCommand::BodyAvailable { manifest: queued_manifest }
                    if queued.tag == source_tag && queued_manifest == &manifest
            )));
            assert!(runtime.ingress.commands.iter().any(|queued| matches!(
                &queued.command,
                AdapterCommand::BodyAvailable { manifest: queued_manifest }
                    if queued.tag == rebound && queued_manifest == &conflicting
            )));
            assert_eq!(
                runtime
                    .rebind_body_available(source_tag, rebound, &manifest)
                    .expect_err("fail-closed runtime rejects a second conflicting rebind"),
                "Sumeragi v2 runtime is fail-closed"
            );
            assert_eq!(
                runtime.enqueue_application_completed(source_tag, manifest.subject),
                Err(EnqueueError::FailClosed)
            );
            assert!(matches!(
                runtime.step(Instant::now()),
                Err(RuntimeError::FailClosed)
            ));
        }

        {
            let directory = TempDir::new().expect("temporary destination-duplicate directory");
            let (mut runtime, context, _keys) =
                authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
            let source_tag = runtime.round_tag();
            let rebound = EventTag::new(
                source_tag.height(),
                source_tag.view() + 1,
                Generation::new(source_tag.generation().get() + 1),
            );
            let manifest = runtime_manifest(&context, 0x8E);
            observe_enter_view_for_test(&mut runtime, source_tag, rebound, &manifest);
            runtime
                .enqueue_body_available(source_tag, manifest.clone())
                .expect("enqueue unique source owner");
            for _ in 0..2 {
                runtime
                    .ingress
                    .enqueue_canonical_body_available(rebound, manifest.clone())
                    .expect("test seam creates duplicate destination ownership");
            }

            assert_eq!(
                runtime
                    .rebind_body_available(source_tag, rebound, &manifest)
                    .expect_err("duplicate destination ownership must fail closed"),
                "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
            );
            assert!(runtime.fail_closed);
            assert_eq!(runtime.queued_commands(), 3);
            assert_eq!(
                runtime
                    .ingress
                    .commands
                    .iter()
                    .filter(|queued| queued.tag == source_tag)
                    .count(),
                1,
                "destination preflight must retain the source owner"
            );
            assert_eq!(
                runtime
                    .ingress
                    .commands
                    .iter()
                    .filter(|queued| queued.tag == rebound)
                    .count(),
                2,
                "destination preflight must not mutate duplicate owners"
            );
            assert_eq!(
                runtime
                    .rebind_body_available(source_tag, rebound, &manifest)
                    .expect_err("fail-closed runtime rejects a second duplicate rebind"),
                "Sumeragi v2 runtime is fail-closed"
            );
            assert_eq!(
                runtime.enqueue_application_completed(source_tag, manifest.subject),
                Err(EnqueueError::FailClosed)
            );
            assert!(matches!(
                runtime.step(Instant::now()),
                Err(RuntimeError::FailClosed)
            ));
        }
    }

    #[test]
    fn duplicate_body_available_rebind_and_retirement_fail_closed_before_mutation() {
        {
            let directory = TempDir::new().expect("temporary duplicate-rebind directory");
            let (mut runtime, context, _keys) =
                authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
            let owner_tag = runtime.round_tag();
            let manifest = runtime_manifest(&context, 0x8E);
            for _ in 0..2 {
                runtime
                    .ingress
                    .enqueue_canonical_body_available(owner_tag, manifest.clone())
                    .expect("test seam creates duplicate ingress ownership");
            }
            let rebound = EventTag::new(
                owner_tag.height(),
                owner_tag.view() + 1,
                Generation::new(owner_tag.generation().get() + 1),
            );
            observe_enter_view_for_test(&mut runtime, owner_tag, rebound, &manifest);

            assert_eq!(
                runtime
                    .rebind_body_available(owner_tag, rebound, &manifest)
                    .expect_err("duplicate ownership must prevent rebind"),
                "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
            );
            assert!(runtime.fail_closed);
            assert_eq!(runtime.queued_commands(), 2);
            assert!(
                runtime
                    .ingress
                    .commands
                    .iter()
                    .all(|queued| queued.tag == owner_tag),
                "preflight must leave every duplicate owner at its original tag"
            );
            assert_eq!(
                runtime
                    .rebind_body_available(owner_tag, rebound, &manifest)
                    .expect_err("fail-closed runtime must reject a second rebind"),
                "Sumeragi v2 runtime is fail-closed"
            );
            assert_eq!(
                runtime.enqueue_application_completed(owner_tag, manifest.subject),
                Err(EnqueueError::FailClosed)
            );
            assert!(matches!(
                runtime.step(Instant::now()),
                Err(RuntimeError::FailClosed)
            ));
        }

        {
            let directory = TempDir::new().expect("temporary duplicate-retirement directory");
            let (mut runtime, context, _keys) =
                authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
            let owner_tag = runtime.round_tag();
            let manifest = runtime_manifest(&context, 0x8F);
            for _ in 0..2 {
                runtime
                    .ingress
                    .enqueue_canonical_body_available(owner_tag, manifest.clone())
                    .expect("test seam creates duplicate ingress ownership");
            }

            assert_eq!(
                runtime
                    .retire_body_available(owner_tag, &manifest)
                    .expect_err("duplicate ownership must prevent retirement"),
                "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
            );
            assert!(runtime.fail_closed);
            assert_eq!(
                runtime.queued_commands(),
                2,
                "preflight must not mutate duplicate serialized owners"
            );
            assert_eq!(
                runtime
                    .retire_body_available(owner_tag, &manifest)
                    .expect_err("fail-closed runtime must reject a second retirement"),
                "Sumeragi v2 runtime is fail-closed"
            );
            assert_eq!(
                runtime.enqueue_application_completed(owner_tag, manifest.subject),
                Err(EnqueueError::FailClosed)
            );
            assert!(matches!(
                runtime.step(Instant::now()),
                Err(RuntimeError::FailClosed)
            ));
        }
    }

    #[test]
    fn conflicting_body_pipeline_evidence_fails_closed_before_body_available_pruning() {
        let body_directory = TempDir::new().expect("temporary body evidence directory");
        let (mut body_runtime, context, keys) =
            authenticated_network_runtime(&body_directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = body_runtime.round_tag();
        let proposal = signed_runtime_proposal(&context, &keys, 0x95);
        let manifest = match &proposal.payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => proposal.manifest.clone(),
            _ => unreachable!("fixture is a proposal"),
        };
        body_runtime
            .enqueue_network(proposal)
            .expect("enqueue the exact authenticated proposal");
        body_runtime
            .enqueue_body_available(owner_tag, manifest.clone())
            .expect("enqueue the first canonical body completion");
        assert_eq!(body_runtime.queued_commands(), 2);

        let mut conflicting_manifest = manifest.clone();
        conflicting_manifest.chunk_hashes[0] = Hash::new(b"conflicting completion chunk");
        conflicting_manifest.chunk_root = Hash::new(b"conflicting completion root");
        assert_eq!(
            body_runtime.enqueue_body_available(owner_tag, conflicting_manifest),
            Err(EnqueueError::DuplicateCompletionOwnership)
        );
        assert!(body_runtime.fail_closed);
        assert_eq!(
            body_runtime.queued_commands(),
            2,
            "ownership must fail before a conflicting completion prunes the exact proposal"
        );
        assert!(body_runtime.ingress.commands.iter().any(|queued| matches!(
            &queued.command,
            AdapterCommand::Authenticated(authenticated)
                if matches!(
                    authenticated.payload(),
                    wire::ConsensusMessageV2Payload::Proposal(proposal)
                        if proposal.manifest == manifest
                )
        )));
        assert_eq!(
            body_runtime.enqueue_body_available(owner_tag, manifest),
            Err(EnqueueError::FailClosed)
        );

        let stored_directory = TempDir::new().expect("temporary durable evidence directory");
        let (mut stored_runtime, context, _keys) =
            authenticated_network_runtime(&stored_directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = stored_runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x96);
        let exact_receipt = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let mut other_manifest = manifest.clone();
        other_manifest.chunk_hashes[0] = Hash::new(b"different durable receipt chunk");
        other_manifest.chunk_root = Hash::new(b"different durable receipt root");
        let conflicting_receipt = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&other_manifest),
        );
        stage_completion_for_queue_test(
            &mut stored_runtime,
            owner_tag,
            AdapterCommand::BodyStored {
                round: manifest.round,
                subject: manifest.subject,
                receipt: exact_receipt,
            },
        );
        assert_eq!(
            stored_runtime.enqueue_body_stored(
                owner_tag,
                manifest.round,
                manifest.subject,
                conflicting_receipt,
            ),
            Err(EnqueueError::DuplicateCompletionOwnership)
        );
        assert!(stored_runtime.fail_closed);

        let validation_directory = TempDir::new().expect("temporary validation polarity directory");
        let (mut validation_runtime, context, _keys) =
            authenticated_network_runtime(&validation_directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = validation_runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x97);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        stage_completion_for_queue_test(
            &mut validation_runtime,
            owner_tag,
            AdapterCommand::ValidationSucceeded {
                round: manifest.round,
                subject: manifest.subject,
                receipt: ValidatedBodyReceipt::for_test(durable),
            },
        );
        assert_eq!(
            validation_runtime.enqueue_validation_failed(
                owner_tag,
                manifest.round,
                manifest.subject,
            ),
            Err(EnqueueError::DuplicateCompletionOwnership),
            "opposite validation polarity is conflicting evidence"
        );
        assert!(validation_runtime.fail_closed);

        let deferred_failure_directory =
            TempDir::new().expect("temporary deferred validation-failure directory");
        let (mut deferred_failure_runtime, context, _keys) = authenticated_network_runtime(
            &deferred_failure_directory,
            RuntimeQueueConfig::new(8, 1, 1),
        );
        let owner_tag = deferred_failure_runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x9B);
        deferred_failure_runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &manifest,
                DeferredBodyPipelineStageForTest::ValidationFailed,
            )
            .expect("stage Busy-deferred validation failure");
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        assert_eq!(
            deferred_failure_runtime.enqueue_validation_succeeded(
                owner_tag,
                manifest.round,
                manifest.subject,
                ValidatedBodyReceipt::for_test(durable),
            ),
            Err(EnqueueError::DuplicateCompletionOwnership),
            "Busy-deferred failure cannot coalesce an incoming success"
        );
        assert!(deferred_failure_runtime.fail_closed);

        let deferred_success_directory =
            TempDir::new().expect("temporary deferred validation-success directory");
        let (mut deferred_success_runtime, context, _keys) = authenticated_network_runtime(
            &deferred_success_directory,
            RuntimeQueueConfig::new(8, 1, 1),
        );
        let owner_tag = deferred_success_runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x9C);
        deferred_success_runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &manifest,
                DeferredBodyPipelineStageForTest::ValidationSucceeded,
            )
            .expect("stage Busy-deferred validation success");
        assert_eq!(
            deferred_success_runtime.enqueue_validation_failed(
                owner_tag,
                manifest.round,
                manifest.subject,
            ),
            Err(EnqueueError::DuplicateCompletionOwnership),
            "Busy-deferred success cannot coalesce an incoming failure"
        );
        assert!(deferred_success_runtime.fail_closed);

        let atomic_directory = TempDir::new().expect("temporary atomic validation directory");
        let (mut atomic_runtime, context, _keys) =
            authenticated_network_runtime(&atomic_directory, RuntimeQueueConfig::new(3, 1, 1));
        let owner_tag = atomic_runtime.round_tag();
        let manifests = [0x9D, 0x9E, 0x9F, 0xA0].map(|seed| runtime_manifest(&context, seed));
        let failures = manifests
            .iter()
            .map(|manifest| (owner_tag, manifest.round, manifest.subject))
            .collect::<Vec<_>>();
        let next_ordinal_before_wrong_class = atomic_runtime.ingress.next_admission_ordinal;
        let (wrong_tag, wrong_round, wrong_subject) = failures[0];
        assert_eq!(
            atomic_runtime
                .ingress
                .enqueue_completion_batch(vec![TaggedCommand::new(
                    wrong_tag,
                    CommandClass::Normal,
                    AdapterCommand::ValidationFailed {
                        round: wrong_round,
                        subject: wrong_subject,
                    },
                    Instant::now(),
                )]),
            Err(EnqueueError::FailClosed),
            "a batch API cannot relabel non-completion traffic as trusted completion work"
        );
        assert_eq!(atomic_runtime.queued_commands(), 0);
        assert_eq!(
            atomic_runtime.ingress.next_admission_ordinal, next_ordinal_before_wrong_class,
            "rejected batch traffic cannot spend an admission ordinal"
        );
        assert_eq!(
            atomic_runtime.enqueue_validation_failures_atomically(&failures),
            Err(EnqueueError::Full)
        );
        assert_eq!(
            atomic_runtime.queued_commands(),
            0,
            "a capacity failure cannot publish an earlier member of the batch"
        );
        atomic_runtime
            .enqueue_validation_failures_atomically(&failures[..3])
            .expect("the complete fitting batch is admitted atomically");
        assert_eq!(atomic_runtime.queued_commands(), 3);
        for (queued, (tag, round, subject)) in atomic_runtime
            .ingress
            .commands
            .iter()
            .zip(failures.iter().copied())
        {
            assert_eq!(queued.tag, tag);
            assert!(matches!(
                &queued.command,
                AdapterCommand::ValidationFailed {
                    round: queued_round,
                    subject: queued_subject,
                } if *queued_round == round && *queued_subject == subject
            ));
        }
        atomic_runtime
            .enqueue_validation_failures_atomically(&failures[..3])
            .expect("exact pre-owned rows coalesce without spending capacity");
        assert_eq!(atomic_runtime.queued_commands(), 3);

        let conflict_directory =
            TempDir::new().expect("temporary conflicting atomic validation directory");
        let (mut conflict_runtime, conflict_context, _keys) =
            authenticated_network_runtime(&conflict_directory, RuntimeQueueConfig::new(4, 1, 1));
        let conflict_tag = conflict_runtime.round_tag();
        let vacant = runtime_manifest(&conflict_context, 0xA1);
        let conflicting = runtime_manifest(&conflict_context, 0xA2);
        let durable = DurableBodyReceipt::for_test(
            conflict_context.id(),
            conflicting.round,
            conflicting.subject,
            HashOf::new(&conflicting),
        );
        stage_completion_for_queue_test(
            &mut conflict_runtime,
            conflict_tag,
            AdapterCommand::ValidationSucceeded {
                round: conflicting.round,
                subject: conflicting.subject,
                receipt: ValidatedBodyReceipt::for_test(durable),
            },
        );
        assert_eq!(
            conflict_runtime.enqueue_validation_failures_atomically(&[
                (conflict_tag, vacant.round, vacant.subject),
                (conflict_tag, conflicting.round, conflicting.subject),
            ]),
            Err(EnqueueError::DuplicateCompletionOwnership)
        );
        assert_eq!(
            conflict_runtime.queued_commands(),
            1,
            "the vacant prefix cannot become visible before a later conflict"
        );
        assert!(conflict_runtime.fail_closed);
    }

    #[test]
    fn conflicting_local_and_validated_receipts_do_not_coalesce() {
        let validation_directory =
            TempDir::new().expect("temporary execution commitment directory");
        let (mut validation_runtime, context, _keys) =
            authenticated_network_runtime(&validation_directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = validation_runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x98);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let exact_validated = ValidatedBodyReceipt::for_test(durable.clone());
        let conflicting_validated = ValidatedBodyReceipt::for_test_with_commitment(
            durable,
            wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"conflicting parent state"),
                Hash::new(b"conflicting post state"),
                Hash::new(b"conflicting ordinary writes"),
                1,
                Hash::new(b"conflicting executed body"),
            ),
        );
        stage_completion_for_queue_test(
            &mut validation_runtime,
            owner_tag,
            AdapterCommand::ValidationSucceeded {
                round: manifest.round,
                subject: manifest.subject,
                receipt: exact_validated,
            },
        );
        assert_eq!(
            validation_runtime.enqueue_validation_succeeded(
                owner_tag,
                manifest.round,
                manifest.subject,
                conflicting_validated,
            ),
            Err(EnqueueError::DuplicateCompletionOwnership)
        );
        assert!(validation_runtime.fail_closed);

        let proposal_directory = TempDir::new().expect("temporary local proposal directory");
        let (mut proposal_runtime, context, _keys) =
            authenticated_network_runtime(&proposal_directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = proposal_runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x99);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        stage_completion_for_queue_test(
            &mut proposal_runtime,
            owner_tag,
            AdapterCommand::LocalProposalReady {
                manifest: manifest.clone(),
                durable_receipt: durable,
                validated_receipt: validated,
            },
        );

        let mut conflicting_manifest = manifest.clone();
        conflicting_manifest.chunk_hashes[0] = Hash::new(b"conflicting local proposal chunk");
        conflicting_manifest.chunk_root = Hash::new(b"conflicting local proposal root");
        let conflicting_durable = DurableBodyReceipt::for_test(
            context.id(),
            conflicting_manifest.round,
            conflicting_manifest.subject,
            HashOf::new(&conflicting_manifest),
        );
        let conflicting_validated = ValidatedBodyReceipt::for_test(conflicting_durable.clone());
        assert_eq!(
            proposal_runtime.enqueue_local_proposal(
                owner_tag,
                conflicting_manifest,
                conflicting_durable,
                conflicting_validated,
            ),
            Err(EnqueueError::DuplicateCompletionOwnership)
        );
        assert!(proposal_runtime.fail_closed);
    }

    #[test]
    fn applied_body_pipeline_phases_suppress_retries_before_ordinal_allocation() {
        const PHASE_INVENTORY: [&str; 4] = [
            "body_available",
            "body_stored",
            "validation_succeeded",
            "signature_completed",
        ];

        let directory = TempDir::new().expect("temporary production phase-inventory directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(0),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime for production dispatch");
        runtime
            .enqueue_network(signed_runtime_proposal(&context, &keys, 0x9A))
            .expect("enqueue authenticated proposal");
        let proposal_effects = match runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch proposal")
        {
            RuntimeStep::Advanced(effects) => effects,
            RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
        };
        let (tag, manifest) = match proposal_effects.as_slice() {
            [
                AdapterEffect::FetchBody {
                    tag,
                    manifest: Some(manifest),
                    ..
                },
            ] => (*tag, manifest.clone()),
            effects => panic!("unexpected proposal effects: {effects:?}"),
        };
        let mut suppressed_phases = Vec::new();

        runtime
            .enqueue_body_available(tag, manifest.clone())
            .expect("enqueue body reconstruction completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("dispatch body reconstruction"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::StoreBody { .. }])
        ));
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_body_available(tag, manifest.clone())
            .expect("an applied BodyAvailable retry is a monotone stutter");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        suppressed_phases.push("body_available");

        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        runtime
            .enqueue_body_stored(tag, manifest.round, manifest.subject, durable.clone())
            .expect("enqueue durable-store completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("dispatch durable-store completion"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::ValidateBody { .. }])
        ));
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_body_stored(tag, manifest.round, manifest.subject, durable.clone())
            .expect("an applied BodyStored retry is a monotone stutter");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        suppressed_phases.push("body_stored");

        let validated = ValidatedBodyReceipt::for_test(durable);
        runtime
            .enqueue_validation_succeeded(tag, manifest.round, manifest.subject, validated.clone())
            .expect("enqueue validation completion");
        let (signature_tag, signature_preimage) = match runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch validation completion")
        {
            RuntimeStep::Advanced(effects) => match effects.as_slice() {
                [
                    AdapterEffect::Sign {
                        tag,
                        request: SignRequest::Vote(vote),
                    },
                ] => (*tag, vote.signature_preimage()),
                effects => panic!("unexpected validation effects: {effects:?}"),
            },
            RuntimeStep::Idle => panic!("validation completion unexpectedly idle"),
        };
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_validation_succeeded(tag, manifest.round, manifest.subject, validated.clone())
            .expect("an applied validation retry is a monotone stutter");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        suppressed_phases.push("validation_succeeded");

        let signature = Signature::new(keys[0].private_key(), &signature_preimage)
            .payload()
            .to_vec();
        runtime
            .enqueue_signature(signature_tag, signature.clone())
            .expect("enqueue exact signature completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("dispatch exact signature completion"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::Broadcast(_)])
        ));
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_signature(signature_tag, signature)
            .expect("an applied signature retry is a monotone stutter");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        suppressed_phases.push("signature_completed");

        assert_eq!(
            runtime
                .retire_body_pipeline_completions(tag, manifest.round, manifest.subject)
                .expect("no applied callback remains physically owned"),
            RetiredBodyPipelineCompletions::default()
        );
        assert_eq!(suppressed_phases, PHASE_INVENTORY);
    }

    #[test]
    fn applied_validation_failure_suppresses_retry_and_rejects_opposite_outcome() {
        const PHASE_INVENTORY: [&str; 1] = ["validation_failed"];

        let directory = TempDir::new().expect("temporary failed-validation phase directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(0),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime for failed-validation dispatch");
        runtime
            .enqueue_network(signed_runtime_proposal(&context, &keys, 0x9B))
            .expect("enqueue authenticated proposal");
        let (tag, manifest) = match runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch proposal")
        {
            RuntimeStep::Advanced(effects) => match effects.as_slice() {
                [
                    AdapterEffect::FetchBody {
                        tag,
                        manifest: Some(manifest),
                        ..
                    },
                ] => (*tag, manifest.clone()),
                effects => panic!("unexpected proposal effects: {effects:?}"),
            },
            RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
        };
        runtime
            .enqueue_body_available(tag, manifest.clone())
            .expect("enqueue body reconstruction completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("dispatch body reconstruction"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::StoreBody { .. }])
        ));
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        runtime
            .enqueue_body_stored(tag, manifest.round, manifest.subject, durable.clone())
            .expect("enqueue durable-store completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("dispatch durable-store completion"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::ValidateBody { .. }])
        ));

        runtime
            .enqueue_validation_failed(tag, manifest.round, manifest.subject)
            .expect("enqueue deterministic validation failure");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("dispatch deterministic validation failure"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_validation_failed(tag, manifest.round, manifest.subject)
            .expect("an applied failed-validation retry is a monotone stutter");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        assert_eq!(["validation_failed"], PHASE_INVENTORY);

        assert_eq!(
            runtime.enqueue_validation_succeeded(
                tag,
                manifest.round,
                manifest.subject,
                ValidatedBodyReceipt::for_test(durable),
            ),
            Err(EnqueueError::FailClosed),
            "opposite deterministic outcomes for one durable body conflict"
        );
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        assert!(runtime.fail_closed);
    }
