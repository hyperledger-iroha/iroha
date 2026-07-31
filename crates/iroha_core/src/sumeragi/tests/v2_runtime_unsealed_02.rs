
    #[test]
    fn exact_authenticated_qc_from_distinct_sources_coalesces_in_one_runtime_slot() {
        let directory = TempDir::new().expect("temporary multi-source QC directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let owner_tag = runtime.round_tag();
        let certificate = signed_runtime_quorum_certificate(&context, &keys, 0xC7);
        let message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
        );
        let first_source = PeerId::new(keys[0].public_key().clone());
        let second_source = PeerId::new(keys[1].public_key().clone());

        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    message.clone(),
                    fair_network_ownership(&message, first_source),
                )
                .expect("the first authenticated carrier owns the runtime command"),
            owner_tag
        );
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    message.clone(),
                    fair_network_ownership(&message, second_source),
                )
                .expect("an exact QC from another source coalesces"),
            owner_tag
        );
        assert_eq!(runtime.queued_commands(), 1);

        let retained = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("the queued QC retains fair-ingress ownership");
        assert!(retained.validate_exact());
        assert_eq!(retained.direct.len(), 2);
        assert!(retained.commit_certificate_response.is_empty());
        assert_ne!(
            retained.direct[0].process_local_projection_hash(),
            retained.direct[1].process_local_projection_hash(),
            "direct carrier projections must retain their distinct authenticated-source identities"
        );

        let mut source_substituted = retained.clone();
        let substituted_source = PeerId::from(KeyPair::random().public_key().clone());
        source_substituted.direct[0].first.wire_key.origin = Some(substituted_source.clone());
        source_substituted.direct[0].first.semantic_origin = Some(substituted_source.clone());
        source_substituted.direct[0].first.authenticated_via = Some(substituted_source.clone());
        source_substituted.direct[0].first.authenticated_source =
            super::super::FairV2IngressSource::Validator(substituted_source.clone());
        source_substituted.direct[0].first.semantic_owner_source =
            super::super::FairV2IngressSource::Validator(substituted_source.clone());
        source_substituted.direct[0].latest.wire_key.origin = Some(substituted_source.clone());
        source_substituted.direct[0].latest.semantic_origin = Some(substituted_source.clone());
        source_substituted.direct[0].latest.authenticated_via = Some(substituted_source.clone());
        source_substituted.direct[0].latest.authenticated_source =
            super::super::FairV2IngressSource::Validator(substituted_source.clone());
        source_substituted.direct[0].latest.semantic_owner_source =
            super::super::FairV2IngressSource::Validator(substituted_source);
        assert!(source_substituted.direct[0].validate_exact());
        assert!(
            !source_substituted.validate_exact(),
            "the retained runtime projection must reject an otherwise exact source substitution"
        );

        let mut reordered = retained.clone();
        reordered.direct.reverse();
        assert!(
            !reordered.validate_exact(),
            "the retained runtime projection must reject carrier-order mutation"
        );
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn exact_authenticated_tc_from_distinct_sources_retains_one_busy_owner() {
        let directory = TempDir::new().expect("temporary multi-source TC directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(4, 1, 1),
            Some(0),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime before authenticated ingress");
        let owner_tag = runtime.round_tag();
        let timeout_effects = runtime
            .driver
            .timeout_elapsed(owner_tag)
            .expect("install a local signing fence")
            .into_effects();
        let (signature_tag, signature_preimage) = match timeout_effects.as_slice() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::TimeoutVote(vote),
                },
            ] => (*tag, vote.signature_preimage()),
            effects => panic!("unexpected timeout effects: {effects:?}"),
        };
        let message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
                signed_runtime_timeout_certificate(&context, &keys),
            ));

        for source in &keys[..2] {
            assert_eq!(
                runtime
                    .enqueue_network_with_ingress_ownership(
                        message.clone(),
                        fair_network_ownership(&message, PeerId::new(source.public_key().clone()),),
                    )
                    .expect("each authenticated TC carrier coalesces"),
                owner_tag
            );
        }
        assert_eq!(runtime.queued_commands(), 1);
        let queued = runtime
            .ingress
            .commands
            .front()
            .and_then(|command| command.ingress_ownership.as_ref())
            .expect("the queued TC retains both fair-ingress carriers");
        assert_eq!(queued.direct.len(), 2);
        assert!(queued.validate_exact());

        assert!(matches!(
            runtime.step(now),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        let fifo_owner = runtime
            .take_last_scheduler_ownership()
            .expect("Busy TC dispatch retains its exact FIFO owner");
        assert!(fifo_owner.validate_exact().is_ok());
        assert_eq!(runtime.deferred_ingress_ownership.len(), 1);
        let deferred = runtime
            .deferred_ingress_ownership
            .values()
            .next()
            .expect("the Busy TC owns one deferred ordinal");
        assert_eq!(deferred.direct.len(), 2);
        assert!(deferred.validate_exact());

        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    message.clone(),
                    fair_network_ownership(&message, PeerId::new(keys[2].public_key().clone()),),
                )
                .expect("a later authenticated carrier merges into the Busy TC"),
            owner_tag
        );
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(
            runtime
                .deferred_ingress_ownership
                .values()
                .next()
                .expect("the Busy TC retains its merged carrier set")
                .direct
                .len(),
            3
        );

        let signature = Signature::new(keys[0].private_key(), &signature_preimage)
            .payload()
            .to_vec();
        runtime
            .enqueue_signature(signature_tag, signature)
            .expect("enqueue the exact signing completion");
        assert!(matches!(
            runtime.step(now),
            Ok(RuntimeStep::Advanced(ref effects))
                if matches!(effects.as_slice(), [AdapterEffect::Broadcast(_)])
        ));
        assert!(runtime.take_last_scheduler_ownership().is_some());
        assert!(matches!(runtime.step(now), Ok(RuntimeStep::Advanced(_))));
        let deferred_owner = runtime
            .take_last_scheduler_ownership()
            .expect("deferred TC service hands off its exact owner");
        assert!(deferred_owner.validate_exact().is_ok());
        let RuntimeSelectedCandidateOwnership::ExactDeferred(deferred) = &deferred_owner.candidate
        else {
            panic!("expected exact deferred TC scheduler ownership")
        };
        assert!(
            deferred
                .ingress_ownership
                .as_ref()
                .is_some_and(|ownership| ownership.direct.len() == 3)
        );
        assert!(runtime.deferred_ingress_ownership.is_empty());
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn same_semantic_qc_with_conflicting_route_authority_fails_closed_atomically() {
        let directory = TempDir::new().expect("temporary conflicting route directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let certificate = signed_runtime_quorum_certificate(&context, &keys, 0xC8);
        let message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
        );
        let source = PeerId::new(keys[0].public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::new(source.clone());
        let first_route = routes.mint(source.clone());
        let conflicting_route = routes
            .forge_equal_ordinal_different_tenure(&first_route, source.clone(), source.clone())
            .expect("fixture owns the conflicting route authority");

        assert!(matches!(
            super::super::InboundBlockMessage::try_from_transport_with_reply_route(
                super::super::message::BlockMessage::V2(message.clone()),
                source.clone(),
                source.clone(),
                conflicting_route.clone(),
            ),
            Err(NetworkReplyRouteError::EqualOrdinalDifferentTenure)
        ));
        let first_ownership = fair_network_ownership_with_route(
            &message,
            source.clone(),
            source.clone(),
            first_route,
        );
        runtime
            .enqueue_network_with_ingress_ownership(message.clone(), first_ownership.clone())
            .expect("the first exact route owns the authenticated QC");
        let retained_before = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("the queued QC retains its first route")
            .clone();

        let mut conflicting_ownership = retained_before.direct[0].clone();
        conflicting_ownership.attempts[0].route = conflicting_route.clone();
        conflicting_ownership.latest.attempts_after[0].route = conflicting_route;
        assert!(
            !conflicting_ownership.validate_exact(),
            "the runtime must reject a carrier whose cursor projection substitutes a forged tenure"
        );
        assert!(matches!(
            runtime.enqueue_network_with_ingress_ownership(message.clone(), conflicting_ownership),
            Err(NetworkIngressError::FailClosed)
        ));
        let retained_after = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("failed merge preserves the first exact route");
        assert_eq!(retained_after, &retained_before);
        assert_eq!(retained_after.direct.len(), 1);
        assert_eq!(
            runtime.fail_closed_reason.as_deref(),
            Some("network ingress changed its authenticated fair-queue ownership")
        );
    }

    #[test]
    fn runtime_ingress_carrier_capacity_returns_backpressure_atomically() {
        let directory = TempDir::new().expect("temporary carrier-capacity directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let certificate = signed_runtime_quorum_certificate(&context, &keys, 0xC9);
        let message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
        );
        let carrier = || {
            let source = PeerId::from(KeyPair::random().public_key().clone());
            fair_network_ownership(&message, source)
        };
        runtime
            .enqueue_network_with_ingress_ownership(message.clone(), carrier())
            .expect("the first disjoint carrier owns the authenticated QC");
        for _ in 1..MAX_RUNTIME_INGRESS_CARRIERS_PER_FORM {
            let candidate = RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, carrier())
                .expect("independent fair-ingress carrier is exact");
            runtime
                .ingress
                .commands
                .front_mut()
                .and_then(|queued| queued.ingress_ownership.as_mut())
                .expect("the queued QC retains its carrier set")
                .merge_downstream(candidate)
                .expect("every protocol-bounded carrier remains exact");
        }
        let retained = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("the queued QC retains the full carrier set");
        assert_eq!(retained.direct.len(), MAX_RUNTIME_INGRESS_CARRIERS_PER_FORM);
        let retained_before = retained.clone();
        let queued_before = runtime.queued_commands();
        let excess_carrier = carrier();

        assert!(matches!(
            runtime.enqueue_network_with_ingress_ownership(message, excess_carrier),
            Err(NetworkIngressError::Backpressure(EnqueueError::Full))
        ));
        let retained_after = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("backpressure preserves the full exact carrier set");
        assert_eq!(retained_after, &retained_before);
        assert_eq!(
            runtime.queued_commands(),
            queued_before,
            "carrier saturation must not create a duplicate runtime command"
        );
        assert!(retained_after.validate_exact());
        assert!(!runtime.fail_closed);
        assert!(runtime.fail_closed_reason.is_none());
    }

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
                execution_commitment: wire::ExecutionCommitment::without_topups(
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

