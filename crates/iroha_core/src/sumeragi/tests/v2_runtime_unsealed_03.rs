
#[test]
fn retiring_exact_body_completion_releases_a_capacity_one_ingress_slot() {
    let round = wire::ConsensusRound {
        context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"retired-body-context",
        ))),
        height: 11,
        view: 4,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"retired-body-block")),
        payload_hash: Hash::new(b"retired-body-payload"),
    };
    let layout = wire::DataAvailabilityLayout {
        encoding: wire::PayloadEncoding::ReedSolomon16,
        chunk_size_bytes: 2,
        data_shards: 1,
        parity_shards: 1,
        max_payload_size_bytes: 1,
        max_chunk_count: 2,
    };
    let original = wire::PayloadManifest {
        round,
        subject,
        payload_size_bytes: 1,
        layout,
        chunk_hashes: vec![Hash::new(b"retired chunk"); 2],
        chunk_root: Hash::new(b"retired root"),
    };
    let replacement = wire::PayloadManifest {
        round: wire::ConsensusRound {
            view: round.view + 1,
            ..round
        },
        chunk_hashes: vec![Hash::new(b"replacement chunk"); 2],
        chunk_root: Hash::new(b"replacement root"),
        ..original.clone()
    };
    let original_tag = tag(4);
    let replacement_tag = tag(5);
    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(1, 0, 0));

    ingress
        .enqueue_canonical_body_available(original_tag, original.clone())
        .expect("the original completion claims the sole slot");
    assert_eq!(
        ingress.enqueue_canonical_body_available(replacement_tag, replacement.clone()),
        Err(EnqueueError::Full)
    );
    assert_eq!(
        ingress.retire_canonical_body_available(original_tag, &original),
        1
    );
    assert_eq!(ingress.remaining_capacity(), 1);
    ingress
        .enqueue_canonical_body_available(replacement_tag, replacement.clone())
        .expect("retirement releases the sole completion slot");
    assert_eq!(ingress.len(), 1);
    assert!(matches!(
        ingress.commands.front(),
        Some(TaggedCommand {
            tag,
            command: AdapterCommand::BodyAvailable { manifest },
            ..
        }) if *tag == replacement_tag && manifest == &replacement
    ));
}

#[test]
fn exact_authenticated_progress_retransmission_is_queue_coalesced() {
    let round = wire::ConsensusRound {
        context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"coalesced-progress-context",
        ))),
        height: 7,
        view: 3,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"coalesced-progress-block")),
        payload_hash: Hash::new(b"coalesced-progress-payload"),
    };
    let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"coalesced parent state"),
        Hash::new(b"coalesced post state"),
        Hash::new(b"coalesced ordinary writes"),
        1,
        Hash::new(b"coalesced executed block wire"),
    );
    let payload = wire::ConsensusMessageV2Payload::QuorumCertificate(wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    });
    let authenticated =
        || AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(payload.clone()));
    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(4, 1, 1));

    assert_eq!(
        ingress
            .enqueue_authenticated(tag(0), CommandClass::Progress, authenticated())
            .expect("first authenticated CommitQC owns one queue slot"),
        tag(0)
    );
    let admitted_origin = ingress.commands[0].causal_origin.clone();
    let admitted_lifecycle_ordinal = ingress.commands[0].lifecycle_ordinal;
    assert_eq!(
        ingress
            .enqueue_authenticated(tag(1), CommandClass::Progress, authenticated())
            .expect("equal authenticated retransmission is coalesced"),
        tag(0),
        "a coalesced retransmission returns the original queue owner's tag"
    );
    assert_eq!(ingress.len(), 1);
    assert_eq!(ingress.commands[0].causal_origin, admitted_origin);
    assert_eq!(
        ingress.commands[0].lifecycle_ordinal, admitted_lifecycle_ordinal,
        "an exact transport retry retains the first lifecycle owner"
    );

    let dispatched = ingress
        .pop_next()
        .expect("the sole queued CommitQC is dispatchable");
    assert_eq!(dispatched.class, CommandClass::Progress);
    assert!(matches!(
        dispatched.command,
        AdapterCommand::Authenticated(_)
    ));
    assert_eq!(ingress.len(), 0);

    assert_eq!(
        ingress
            .enqueue_authenticated(tag(2), CommandClass::Progress, authenticated())
            .expect("a later retransmission starts a new ownership interval"),
        tag(2)
    );
    assert_eq!(ingress.len(), 1);
    assert!(
        !ingress.commands[0]
            .causal_origin
            .same_lifecycle(&admitted_origin),
        "a later interval is not spliced into the drained causal root"
    );
}

#[test]
fn runtime_merges_alternate_sources_for_one_semantic_request() {
    let directory = TempDir::new().expect("temporary alternate-source runtime directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
    let message = signed_runtime_proposal(&context, &keys, 0x76);
    let semantic_origin = PeerId::new(keys[0].public_key().clone());
    let source_a = PeerId::new(keys[1].public_key().clone());
    let source_b = PeerId::new(keys[2].public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(source_a.clone(), 2);
    let route_a = routes.mint_via(semantic_origin.clone(), source_a.clone());
    let route_b = routes.mint_via(semantic_origin.clone(), source_b.clone());
    let ownership_a = fair_runtime_ownership_with_reply_route(
        &message,
        semantic_origin.clone(),
        source_a,
        route_a.clone(),
    );
    let ownership_b = fair_runtime_ownership_with_reply_route(
        &message,
        semantic_origin,
        source_b,
        route_b.clone(),
    );

    let owner_tag = runtime
        .enqueue_network_with_ingress_ownership(message.clone(), ownership_a)
        .expect("first source admits the semantic request");
    assert_eq!(
        runtime
            .enqueue_network_with_ingress_ownership(message, ownership_b)
            .expect("alternate source attaches to the retained request"),
        owner_tag
    );
    assert_eq!(runtime.queued_commands(), 1);
    let ownership = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| queued.ingress_ownership.as_ref())
        .expect("coalesced runtime command retains exact source ownership");
    assert!(ownership.validate_exact());
    let projection_hash = ownership.projection_hash;
    let direct = ownership
        .direct
        .first()
        .expect("proposal retains direct fair-ingress ownership");
    assert_eq!(
        direct
            .current_reply_routes()
            .expect("route-aware fair ownership")
            .len(),
        2
    );
    assert!(routes.retire(&route_a));
    let ownership = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| queued.ingress_ownership.as_ref())
        .expect("queued ownership survives a normal source disconnect");
    assert!(ownership.validate_exact());
    assert_eq!(
        ownership.projection_hash, projection_hash,
        "connection liveness is not part of immutable runtime ownership identity"
    );
    assert!(
        ownership
            .direct
            .first()
            .and_then(FairV2IngressOwnershipEvidence::current_reply_routes)
            .is_some_and(|owned| {
                owned.iter().any(|route| route.same_delivery(&route_a))
                    && owned.iter().any(|route| route.same_delivery(&route_b))
            }),
        "retirement is applied only by an authoritative prune receipt"
    );
    assert!(!runtime.fail_closed);
}

#[test]
fn later_same_semantic_fair_retry_retains_runtime_lifecycle_root() {
    let directory = TempDir::new().expect("temporary lifecycle-retry runtime directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
    let message = signed_runtime_proposal(&context, &keys, 0xD1);
    let semantic_origin = PeerId::new(keys[0].public_key().clone());
    let authenticated_via = PeerId::new(keys[1].public_key().clone());
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let retained_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint first fair lifecycle");
    let retry_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint later fair retry lifecycle");
    let retained = fair_runtime_ownership_at_lifecycle(
        fair_runtime_ownership(&message, semantic_origin.clone(), authenticated_via.clone()),
        retained_ordinal,
    );
    let retry = fair_runtime_ownership_at_lifecycle(
        fair_runtime_ownership(&message, semantic_origin, authenticated_via),
        retry_ordinal,
    );

    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), retained)
        .expect("first fair lifecycle enters runtime");
    let physical_ordinal = runtime.ingress.commands[0]
        .admission_ordinal
        .expect("runtime admission owns one physical position");
    let next_before_retry = lifecycle_ordinals
        .next_ordinal_for_test()
        .expect("inspect shared source before coalescing retry");
    runtime
        .enqueue_network_with_ingress_ownership(message, retry)
        .expect("later same-semantic retry coalesces");

    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect shared source after coalescing retry"),
        next_before_retry,
        "runtime coalescence cannot mint a second physical FIFO position"
    );
    let queued = &runtime.ingress.commands[0];
    assert_eq!(queued.admission_ordinal, Some(physical_ordinal));
    assert_eq!(queued.lifecycle_ordinal, Some(retained_ordinal));
    assert_eq!(
        queued.causal_origin.root_lifecycle_ordinal,
        Some(retained_ordinal)
    );
    let ownership = queued
        .ingress_ownership
        .as_ref()
        .expect("coalesced command retains exact fair ownership");
    assert_eq!(
        ownership.earliest_lifecycle_ordinal(),
        Ok(Some(retained_ordinal))
    );
    let carrier = ownership
        .direct
        .first()
        .expect("same semantic retry remains one bounded carrier");
    assert_eq!(carrier.admission_count, 2);
    assert_eq!(carrier.first.lifecycle_ordinal, Some(retained_ordinal));
    assert_eq!(carrier.latest.lifecycle_ordinal, Some(retained_ordinal));
    assert!(ownership.validate_exact());
    assert!(!runtime.fail_closed);
}

#[test]
fn ordinary_fair_predecessor_remains_before_serve_until_runtime_consumes_it() {
    let directory = TempDir::new().expect("temporary fair-to-runtime predecessor directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
    let message = signed_runtime_proposal(&context, &keys, 0xD6);
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let fair_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint ordinary fair-ingress predecessor lifecycle");
    let ownership = fair_runtime_ownership_at_lifecycle(
        fair_runtime_ownership(
            &message,
            PeerId::new(keys[0].public_key().clone()),
            PeerId::new(keys[1].public_key().clone()),
        ),
        fair_ordinal,
    );
    runtime
        .enqueue_network_with_ingress_ownership(message, ownership)
        .expect("transfer ordinary fair predecessor into serialized runtime");
    let serve_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint exact Serve target behind the transferred predecessor");
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime for exact predecessor comparison");
    assert!(
        runtime
            .older_lifecycle_predates_exact_serve(now, serve_ordinal)
            .expect("transferred Fair owner participates in runtime minimum"),
        "the exact Serve target cannot prepare past the transferred predecessor"
    );

    let (_, consumed) = runtime
        .ingress
        .pop_next_with_ownership()
        .expect("runtime predecessor selection remains exact")
        .expect("ordinary Fair predecessor is ready");
    assert_eq!(consumed.lifecycle_ordinal, fair_ordinal);
    assert!(
        !runtime
            .older_lifecycle_predates_exact_serve(now, serve_ordinal)
            .expect("recompute minimum after consuming the predecessor"),
        "Serve becomes eligible only after the transferred lifecycle drains"
    );
    assert!(!runtime.fail_closed);
}

#[test]
fn older_frozen_aggregate_carrier_rebases_queued_runtime_minimum() {
    let directory = TempDir::new().expect("temporary aggregate-rebase runtime directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate(&context, &keys, 0xD2),
        ));
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let older_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint frozen older aggregate lifecycle");
    let newer_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint later independently admissible aggregate lifecycle");
    let newer = fair_runtime_ownership_at_lifecycle(
        fair_network_ownership(&message, PeerId::new(keys[2].public_key().clone())),
        newer_ordinal,
    );
    let older = fair_runtime_ownership_at_lifecycle(
        fair_network_ownership(&message, PeerId::new(keys[1].public_key().clone())),
        older_ordinal,
    );

    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), newer)
        .expect("newer admissible aggregate enters runtime first");
    assert_eq!(
        runtime.ingress.commands[0].lifecycle_ordinal,
        Some(newer_ordinal)
    );
    let physical_ordinal = runtime.ingress.commands[0].admission_ordinal;
    let next_before_older = lifecycle_ordinals
        .next_ordinal_for_test()
        .expect("inspect shared source before older carrier transfer");
    let mut unfrozen_older = older.clone();
    unfrozen_older.runtime_physical_cut = None;
    assert!(unfrozen_older.validate_exact());
    let unfrozen_projection =
        RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, unfrozen_older)
            .expect("pre-dequeue aggregate identity remains exact");
    assert!(!unfrozen_projection.validate_frozen_physical());
    let retained_projection = runtime.ingress.commands[0]
        .ingress_ownership
        .as_ref()
        .expect("newer aggregate retains checked ingress ownership");
    let mut mixed_preview = retained_projection.clone();
    mixed_preview
        .merge_downstream(unfrozen_projection)
        .expect("capacity probe can preview a frozen/unfrozen aggregate merge");
    assert!(mixed_preview.validate_exact());
    assert!(
        !mixed_preview.validate_frozen_physical(),
        "only checked dequeue may promote the preview to mutable runtime ownership"
    );
    runtime
        .enqueue_network_with_ingress_ownership(message, older)
        .expect("older frozen aggregate carrier joins the queued envelope");

    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect shared source after aggregate reconciliation"),
        next_before_older,
        "carrier reconciliation cannot mint another physical command"
    );
    let queued = &runtime.ingress.commands[0];
    assert_eq!(queued.admission_ordinal, physical_ordinal);
    assert_eq!(queued.lifecycle_ordinal, Some(older_ordinal));
    assert_eq!(
        queued.causal_origin.root_lifecycle_ordinal,
        Some(older_ordinal)
    );
    let ownership = queued
        .ingress_ownership
        .as_ref()
        .expect("aggregate command retains both fair carriers");
    assert_eq!(ownership.direct.len(), 2);
    assert_eq!(
        ownership.earliest_lifecycle_ordinal(),
        Ok(Some(older_ordinal))
    );
    assert!(ownership.validate_exact());

    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before exact Serve comparison");
    let serve_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint exact Serve barrier after both aggregate carriers");
    assert!(
        runtime
            .older_lifecycle_predates_exact_serve(now, serve_ordinal)
            .expect("compare reconciled aggregate minimum"),
        "the later-transferred frozen carrier must become the active minimum"
    );
    assert!(!runtime.fail_closed);
}

#[test]
fn network_runtime_rejects_unminted_and_unrelated_colliding_fair_ordinals() {
    let unminted_directory = TempDir::new().expect("temporary unminted-fair runtime directory");
    let (mut unminted_runtime, context, keys) =
        authenticated_network_runtime(&unminted_directory, RuntimeQueueConfig::new(8, 2, 2));
    let source = unminted_runtime.ingress.lifecycle_ordinals.clone();
    let unminted_ordinal = source
        .next_ordinal_for_test()
        .expect("inspect unminted source position")
        .expect("fresh source has a first ordinal");
    let first_message = signed_runtime_proposal(&context, &keys, 0xD3);
    let first_ownership = fair_runtime_ownership_at_lifecycle(
        fair_runtime_ownership(
            &first_message,
            PeerId::new(keys[0].public_key().clone()),
            PeerId::new(keys[1].public_key().clone()),
        ),
        unminted_ordinal,
    );
    assert!(matches!(
        unminted_runtime.enqueue_network_with_ingress_ownership(first_message, first_ownership),
        Err(NetworkIngressError::FailClosed)
    ));
    assert!(unminted_runtime.fail_closed);
    assert_eq!(unminted_runtime.queued_commands(), 0);
    assert_eq!(
        source
            .next_ordinal_for_test()
            .expect("unminted rejection preserves the source"),
        Some(unminted_ordinal)
    );

    let collision_directory = TempDir::new().expect("temporary fair-collision runtime directory");
    let (mut collision_runtime, context, keys) =
        authenticated_network_runtime(&collision_directory, RuntimeQueueConfig::new(8, 2, 2));
    let source = collision_runtime.ingress.lifecycle_ordinals.clone();
    let shared_ordinal = source.reserve_one().expect("mint one exact fair lifecycle");
    let admitted_message = signed_runtime_proposal(&context, &keys, 0xD4);
    let conflicting_message = signed_runtime_proposal(&context, &keys, 0xD5);
    let admitted_ownership = fair_runtime_ownership_at_lifecycle(
        fair_runtime_ownership(
            &admitted_message,
            PeerId::new(keys[0].public_key().clone()),
            PeerId::new(keys[1].public_key().clone()),
        ),
        shared_ordinal,
    );
    let conflicting_ownership = fair_runtime_ownership_at_lifecycle(
        fair_runtime_ownership(
            &conflicting_message,
            PeerId::new(keys[0].public_key().clone()),
            PeerId::new(keys[1].public_key().clone()),
        ),
        shared_ordinal,
    );
    collision_runtime
        .enqueue_network_with_ingress_ownership(admitted_message, admitted_ownership)
        .expect("first exact fair lifecycle enters runtime");
    let next_before_collision = source
        .next_ordinal_for_test()
        .expect("inspect source before unrelated collision");
    assert!(matches!(
        collision_runtime
            .enqueue_network_with_ingress_ownership(conflicting_message, conflicting_ownership,),
        Err(NetworkIngressError::FailClosed)
    ));
    assert!(collision_runtime.fail_closed);
    assert_eq!(collision_runtime.queued_commands(), 1);
    assert_eq!(
        source
            .next_ordinal_for_test()
            .expect("collision rejection preserves the physical source"),
        next_before_collision,
        "unrelated ordinal collision must fail before a FIFO position is minted"
    );
}

#[test]
fn runtime_keeps_identical_wire_requests_from_distinct_semantic_origins_independent() {
    let directory = TempDir::new().expect("temporary distinct-origin runtime directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
    let message = signed_runtime_proposal(&context, &keys, 0x77);
    let origin_a = PeerId::new(keys[0].public_key().clone());
    let origin_b = PeerId::new(keys[1].public_key().clone());
    let source = PeerId::new(keys[2].public_key().clone());
    let ownership_a = fair_runtime_ownership(&message, origin_a, source.clone());
    let ownership_b = fair_runtime_ownership(&message, origin_b, source);

    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), ownership_a)
        .expect("first semantic origin owns one runtime occurrence");
    runtime
        .enqueue_network_with_ingress_ownership(message, ownership_b)
        .expect("distinct semantic origin retains an independent occurrence");
    assert_eq!(runtime.queued_commands(), 2);
    assert!(runtime.ingress.commands.iter().all(|queued| {
        queued
            .ingress_ownership
            .as_ref()
            .is_some_and(RuntimeIngressOwnershipEvidence::validate_exact)
    }));
    let mut commands = runtime.ingress.commands.iter();
    let first = commands.next().expect("first semantic root is retained");
    let second = commands.next().expect("second semantic root is retained");
    assert!(
        !first.causal_origin.same_lifecycle(&second.causal_origin),
        "identical wire bytes from unrelated semantic origins cannot coalesce"
    );
    assert!(!runtime.fail_closed);
}

#[test]
fn busy_deferred_request_merges_alternate_source_and_services_exact_carrier() {
    let directory = TempDir::new().expect("temporary Busy-deferred ownership directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 2, 2),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before authenticated ingress");
    let round_tag = runtime.round_tag();
    let timeout_effects = runtime
        .driver
        .timeout_elapsed(round_tag)
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

    let message = signed_runtime_proposal(&context, &keys, 0x78);
    let semantic_origin = PeerId::new(keys[0].public_key().clone());
    let ownership_a = fair_runtime_ownership(
        &message,
        semantic_origin.clone(),
        PeerId::new(keys[1].public_key().clone()),
    );
    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), ownership_a)
        .expect("first source enters runtime ingress");
    assert!(matches!(
        runtime.step(now),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    let queued_owner = runtime
        .take_last_scheduler_ownership()
        .expect("Busy dispatch retains its exact queue owner");
    assert!(queued_owner.validate_exact().is_ok());
    assert_eq!(runtime.deferred_ingress_ownership.len(), 1);
    let admission_ordinal = *runtime
        .deferred_ingress_ownership
        .keys()
        .next()
        .expect("authenticated Busy owner has an actor-global ordinal");
    let projection_before_alternate =
        runtime.deferred_ingress_ownership[&admission_ordinal].projection_hash;

    let ownership_b = fair_runtime_ownership(
        &message,
        semantic_origin,
        PeerId::new(keys[2].public_key().clone()),
    );
    assert_eq!(
        runtime
            .enqueue_network_with_ingress_ownership(message, ownership_b)
            .expect("alternate source attaches to the Busy owner"),
        round_tag
    );
    assert_eq!(runtime.queued_commands(), 0);
    assert_ne!(
        runtime.deferred_ingress_ownership[&admission_ordinal].projection_hash,
        projection_before_alternate,
        "alternate ownership history must change the exact runtime projection"
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

    let deferred_effects = match runtime.step(now) {
        Ok(RuntimeStep::Advanced(effects)) => effects,
        other => panic!("deferred owner did not receive its service turn: {other:?}"),
    };
    assert!(
        deferred_effects.is_empty()
            || matches!(
                deferred_effects.as_slice(),
                [AdapterEffect::FetchBody { .. }]
            ),
        "the timeout intent may obsolete the proposal, but no unrelated effect may replace it: {deferred_effects:?}"
    );
    let deferred_owner = runtime
        .take_last_scheduler_ownership()
        .expect("deferred service hands off its exact owner");
    let RuntimeSelectedCandidateOwnership::ExactDeferred(deferred) = &deferred_owner.candidate
    else {
        panic!("expected exact deferred scheduler ownership")
    };
    assert!(
        deferred
            .ingress_ownership
            .as_ref()
            .is_some_and(RuntimeIngressOwnershipEvidence::validate_exact)
    );
    assert!(runtime.deferred_ingress_ownership.is_empty());
    assert!(!runtime.fail_closed);
}

#[test]
fn busy_deferred_older_aggregate_rebases_owner_and_rejects_identity_mutation() {
    let directory = TempDir::new().expect("temporary Busy-deferred rebase directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 2, 2),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before Busy-deferred aggregate ingress");
    let owner_tag = runtime.round_tag();
    let timeout = runtime
        .driver
        .timeout_elapsed(owner_tag)
        .expect("install a signer fence before aggregate dispatch");
    assert!(
        matches!(
            timeout.effects(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }]
        ),
        "unexpected timeout effects: {:?}",
        timeout.effects()
    );

    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate(&context, &keys, 0x79),
        ));
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let mutation_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint the oldest identity-mutation carrier");
    let older_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint the older delayed aggregate carrier");
    let newer_ordinal = lifecycle_ordinals
        .reserve_one()
        .expect("mint the newer aggregate carrier admitted first");
    let newer = fair_runtime_ownership_at_lifecycle(
        fair_network_ownership(&message, PeerId::new(keys[2].public_key().clone())),
        newer_ordinal,
    );
    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), newer)
        .expect("newer aggregate carrier enters runtime before the frozen predecessor");
    assert!(matches!(
        runtime.step(now),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    let selected = runtime
        .take_last_scheduler_ownership()
        .expect("Busy dispatch retains the exact queued owner");
    assert!(selected.validate_exact().is_ok());
    let deferred_ordinals = runtime
        .deferred_ingress_ownership
        .keys()
        .copied()
        .collect::<Vec<_>>();
    let [deferred_ordinal] = deferred_ordinals.as_slice() else {
        panic!("aggregate dispatch must retain exactly one Busy-deferred owner")
    };
    let deferred_ordinal = *deferred_ordinal;
    assert_eq!(
        runtime.deferred_ingress_ownership[&deferred_ordinal].earliest_lifecycle_ordinal(),
        Ok(Some(newer_ordinal))
    );
    assert_eq!(
        runtime.deferred_lifecycle_ownership[&deferred_ordinal].lifecycle_ordinal(),
        newer_ordinal
    );
    let frozen_physical_cut = runtime.deferred_lifecycle_ownership[&deferred_ordinal].physical_cut;
    let frozen_source_physical_ordinal =
        runtime.deferred_lifecycle_ownership[&deferred_ordinal].source_physical_ordinal;
    let frozen_runtime_seal = runtime.deferred_lifecycle_ownership[&deferred_ordinal]
        .runtime_seal
        .clone();
    assert_ne!(frozen_physical_cut, 0);
    assert!(frozen_source_physical_ordinal.is_some());

    let older = fair_runtime_ownership_at_lifecycle(
        fair_network_ownership(&message, PeerId::new(keys[1].public_key().clone())),
        older_ordinal,
    );
    assert_eq!(
        runtime
            .enqueue_network_with_ingress_ownership(message.clone(), older)
            .expect("older frozen carrier joins the exact Busy-deferred aggregate"),
        owner_tag
    );
    let merged = runtime
        .deferred_ingress_ownership
        .get(&deferred_ordinal)
        .expect("Busy-deferred aggregate retains the merged carrier set");
    assert_eq!(merged.direct.len(), 2);
    assert_eq!(merged.earliest_lifecycle_ordinal(), Ok(Some(older_ordinal)));
    assert!(merged.validate_exact());
    let rebased_owner = runtime
        .deferred_lifecycle_ownership
        .get(&deferred_ordinal)
        .expect("Busy-deferred aggregate retains its rebased lifecycle owner");
    assert_eq!(rebased_owner.lifecycle_ordinal(), older_ordinal);
    assert_eq!(
        rebased_owner.physical_cut, frozen_physical_cut,
        "logical owner replacement cannot refresh the continuation's physical cut"
    );
    assert_eq!(
        rebased_owner.source_physical_ordinal, frozen_source_physical_ordinal,
        "logical owner replacement cannot replace the source occurrence"
    );
    assert_eq!(
        rebased_owner.runtime_seal, frozen_runtime_seal,
        "logical owner replacement cannot replace the admitted occurrence capability"
    );
    assert_eq!(
        rebased_owner.causal_origin().root_lifecycle_ordinal,
        Some(older_ordinal)
    );
    assert_eq!(
        rebased_owner.causal_origin().root_ingress_identity,
        Some(runtime_ingress_causal_origin_projection_hash(merged))
    );
    assert!(rebased_owner.validate_exact());

    let healthy_owner = rebased_owner.clone();
    let mutation = RuntimeIngressOwnershipEvidence::from_fair_ingress(
        &message,
        fair_runtime_ownership_at_lifecycle(
            fair_network_ownership(&message, PeerId::new(keys[0].public_key().clone())),
            mutation_ordinal,
        ),
    )
    .expect("oldest aggregate carrier has exact runtime ownership");
    assert_eq!(
        mutation.earliest_lifecycle_ordinal(),
        Ok(Some(mutation_ordinal))
    );
    let mut identity_mutated_lifecycle_owner = healthy_owner.owner.clone();
    identity_mutated_lifecycle_owner
        .causal_origin
        .root_ingress_identity = Some(Hash::new(b"mutated Busy-deferred ingress identity"));
    identity_mutated_lifecycle_owner.causal_origin.lifecycle_key =
        runtime_candidate_causal_origin_lifecycle_key(
            &identity_mutated_lifecycle_owner.causal_origin,
        );
    identity_mutated_lifecycle_owner
        .causal_origin
        .projection_hash = runtime_candidate_causal_origin_projection_hash(
        &identity_mutated_lifecycle_owner.causal_origin,
    );
    identity_mutated_lifecycle_owner.projection_hash =
        runtime_lifecycle_owner_projection_hash(&identity_mutated_lifecycle_owner);
    assert!(
        matches!(
            RuntimeDeferredLifecycleOwnership::new(
                identity_mutated_lifecycle_owner,
                healthy_owner.deferred_admission_ordinal,
                healthy_owner.current_ingress,
                healthy_owner.source_physical_ordinal,
                healthy_owner.physical_cut,
                healthy_owner.runtime_seal.clone(),
            ),
            Err(EnqueueError::FailClosed)
        ),
        "the adapter-private seal rejects a coherently rehashed causal identity substitution"
    );
    runtime
        .reconcile_deferred_ingress_ownership(Some((deferred_ordinal, mutation)))
        .expect("the same earlier carrier rebases after restoring the exact identity");
    let final_ingress = &runtime.deferred_ingress_ownership[&deferred_ordinal];
    assert_eq!(final_ingress.direct.len(), 3);
    assert_eq!(
        final_ingress.earliest_lifecycle_ordinal(),
        Ok(Some(mutation_ordinal))
    );
    let final_owner = &runtime.deferred_lifecycle_ownership[&deferred_ordinal];
    assert_eq!(final_owner.lifecycle_ordinal(), mutation_ordinal);
    assert_eq!(final_owner.physical_cut, frozen_physical_cut);
    assert_eq!(
        final_owner.source_physical_ordinal,
        frozen_source_physical_ordinal
    );
    assert_eq!(
        final_owner.runtime_seal, frozen_runtime_seal,
        "repeated aggregate rebasing retains the first admitted occurrence capability"
    );
    assert_eq!(
        final_owner.causal_origin().root_lifecycle_ordinal,
        Some(mutation_ordinal)
    );
    assert_eq!(
        final_owner.causal_origin().root_ingress_identity,
        Some(runtime_ingress_causal_origin_projection_hash(final_ingress))
    );
    assert!(final_owner.validate_exact());
    assert!(!runtime.fail_closed);
}

#[test]
fn distinct_pre_runtime_leader_wire_qc_waits_behind_busy_deferred_owner() {
    let directory = TempDir::new().expect("temporary pre-runtime leader-wire directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 2, 2),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before Busy-deferred aggregate ingress");
    let owner_tag = runtime.round_tag();
    let timeout = runtime
        .driver
        .timeout_elapsed(owner_tag)
        .expect("install a signer fence before aggregate dispatch");
    assert!(matches!(
        timeout.effects(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));

    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate(&context, &keys, 0x7A),
        ));
    let first_source = context.roster[2].validator.clone();
    let second_source = context.roster[1].validator.clone();
    let (_leader_wire_directory, leader_wire_ingress, ownerships) = preowned_leader_wire_ownerships(
        &context,
        &[(message.clone(), first_source)],
        runtime.ingress.lifecycle_ordinals.clone(),
    );
    let [first_ownership]: [FairV2IngressOwnershipEvidence; 1] = ownerships
        .try_into()
        .expect("fixture creates one exact runtime-owned carrier");
    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), first_ownership)
        .expect("first leader-wire carrier enters the runtime");
    assert!(matches!(
        runtime.step(now),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    runtime
        .take_last_scheduler_ownership()
        .expect("Busy dispatch retains the first exact carrier");
    assert_eq!(runtime.deferred_ingress_ownership.len(), 1);

    assert!(matches!(
        leader_wire_ingress.try_push(InboundBlockMessage::new(
            BlockMessage::V2(message.clone()),
            Some(second_source),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));
    let selected = leader_wire_ingress.try_recv_if(|inbound| {
        let BlockMessage::V2(candidate) = inbound.message() else {
            return true;
        };
        let ownership = inbound
            .ingress_ownership()
            .expect("productive fair ingress attaches exact ownership");
        runtime.can_admit_network_message_with_ingress_ownership(candidate, ownership)
    });
    assert!(
        selected.is_none(),
        "a distinct productive leader-wire token must remain physically queued behind the Busy owner"
    );
    assert_eq!(runtime.deferred_ingress_ownership.len(), 1);
    assert!(!runtime.fail_closed);
}

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
    let first_ownership =
        fair_network_ownership_with_route(&message, source.clone(), source.clone(), first_route);
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
