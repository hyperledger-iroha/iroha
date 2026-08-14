#[test]
fn effect_queue_is_bounded_and_deduplicates_until_drain() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let message = NativeAmxMessage::PrepareRequest(native_request(&adapter, &keys));
    let effect = V2LaneWorkEffect::PostNativeAmx {
        peer: adapter.local_peer.clone(),
        reply_routes: None,
        message,
    };
    assert!(adapter.push_effect(effect.clone()));
    assert!(adapter.push_effect(effect.clone()));
    assert_eq!(adapter.effects.len(), 1);
    assert_eq!(adapter.drain_effects(1).len(), 1);
    assert!(adapter.push_effect(effect));
    assert_eq!(adapter.effects.len(), 1);
}
#[test]
fn duplicate_reply_effect_preserves_exact_source_delivery() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let request = native_request(&adapter, &keys);
    let vote = adapter
        .sign_native_request_once(&request, 0)
        .expect("fixture validator signs one exact Native AMX vote");
    let message = NativeAmxMessage::PrepareVote(vote);
    let peer = adapter.context.roster[1].validator.clone();
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::new(hub);
    let route = route_fixture.mint(peer.clone());
    let effect = V2LaneWorkEffect::PostNativeAmx {
        peer: peer.clone(),
        reply_routes: Some(
            NetworkReplyRoutes::try_from_route(route.clone()).expect("live reply route"),
        ),
        message,
    };
    assert!(adapter.push_effect(effect.clone()));
    assert!(adapter.push_effect(effect));
    assert_eq!(adapter.effects.len(), 1);
    let Some(V2LaneWorkEffect::PostNativeAmx {
        reply_routes: Some(retained),
        ..
    }) = adapter.effects.front()
    else {
        panic!("exact duplicate retains one reply-route set");
    };
    assert_eq!(retained.len(), 1);
    assert!(
        retained
            .iter()
            .any(|retained| retained.same_delivery(&route))
    );
}
#[test]
fn reply_effect_rejects_missing_or_retargeted_route_set() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let request = native_request(&adapter, &keys);
    let vote = adapter
        .sign_native_request_once(&request, 0)
        .expect("fixture validator signs one exact Native AMX vote");
    let message = NativeAmxMessage::PrepareVote(vote);
    let peer = adapter.context.roster[1].validator.clone();
    assert!(!adapter.push_effect(V2LaneWorkEffect::PostNativeAmx {
        peer: peer.clone(),
        reply_routes: None,
        message: message.clone(),
    }));
    let different_target = adapter.context.roster[2].validator.clone();
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::new(hub);
    let retargeted = route_fixture.mint(different_target);
    assert!(!adapter.push_effect(V2LaneWorkEffect::PostNativeAmx {
        peer,
        reply_routes: Some(
            NetworkReplyRoutes::try_from_route(retargeted).expect("live reply route"),
        ),
        message,
    }));
    assert!(adapter.effects.is_empty());
}
#[test]
fn duplicate_reply_effect_updates_only_later_delivery_from_same_source() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let request = native_request(&adapter, &keys);
    let vote = adapter
        .sign_native_request_once(&request, 0)
        .expect("fixture validator signs one exact Native AMX vote");
    let message = NativeAmxMessage::PrepareVote(vote);
    let peer = adapter.context.roster[1].validator.clone();
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::new(hub);
    let first = route_fixture.mint(peer.clone());
    let later = route_fixture
        .redeliver(&first)
        .expect("fixture owns the first route");
    let effect_for = |route| V2LaneWorkEffect::PostNativeAmx {
        peer: peer.clone(),
        reply_routes: Some(NetworkReplyRoutes::try_from_route(route).expect("live reply route")),
        message: message.clone(),
    };
    assert!(adapter.push_effect(effect_for(first.clone())));
    assert!(adapter.push_effect(effect_for(later.clone())));
    assert!(
        !adapter.push_effect(effect_for(first.clone())),
        "a stale delivery must fail without regressing retained ownership"
    );
    let Some(V2LaneWorkEffect::PostNativeAmx {
        reply_routes: Some(retained),
        ..
    }) = adapter.effects.front()
    else {
        panic!("same-source update retains one reply-route set");
    };
    assert_eq!(retained.len(), 1);
    assert!(retained.iter().any(|route| route.same_delivery(&later)));
    assert!(!retained.iter().any(|route| route.same_delivery(&first)));
}
#[test]
fn duplicate_reply_effect_retains_alternate_sources_across_source_update() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let request = native_request(&adapter, &keys);
    let vote = adapter
        .sign_native_request_once(&request, 0)
        .expect("fixture validator signs one exact Native AMX vote");
    let message = NativeAmxMessage::PrepareVote(vote);
    let peer = adapter.context.roster[1].validator.clone();
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::new(hub_a.clone());
    let first_a = route_fixture.mint_via(peer.clone(), hub_a.clone());
    let route_b = route_fixture.mint_via(peer.clone(), hub_b);
    let effect_for = |route| V2LaneWorkEffect::PostNativeAmx {
        peer: peer.clone(),
        reply_routes: Some(NetworkReplyRoutes::try_from_route(route).expect("live reply route")),
        message: message.clone(),
    };
    assert!(adapter.push_effect(effect_for(first_a.clone())));
    assert!(route_fixture.retire(&first_a));
    assert!(adapter.push_effect(effect_for(route_b.clone())));
    let reconnected_a = route_fixture.mint_via(peer.clone(), hub_a);
    assert!(adapter.push_effect(effect_for(reconnected_a.clone())));
    let later_a = route_fixture
        .redeliver(&reconnected_a)
        .expect("fixture owns the reconnected source route");
    assert!(adapter.push_effect(effect_for(later_a.clone())));
    assert!(
        !adapter.push_effect(effect_for(reconnected_a.clone())),
        "a stale source must not reset its own attempt or erase an alternate source"
    );
    let Some(V2LaneWorkEffect::PostNativeAmx {
        reply_routes: Some(retained),
        ..
    }) = adapter.effects.front()
    else {
        panic!("alternate source merge retains one reply-route set");
    };
    assert_eq!(retained.len(), 2);
    assert!(retained.iter().any(|route| route.same_delivery(&later_a)));
    assert!(retained.iter().any(|route| route.same_delivery(&route_b)));
    assert!(
        !retained
            .iter()
            .any(|route| route.same_delivery(&reconnected_a))
    );
    let hub_c = PeerId::new(KeyPair::random().public_key().clone());
    let route_c = route_fixture.mint_via(peer.clone(), hub_c);
    let mut mixed = NetworkReplyRoutes::try_from_route(reconnected_a.clone())
        .expect("stale A is independently live");
    mixed
        .merge(
            &NetworkReplyRoutes::try_from_route(route_b.clone())
                .expect("B is live while constructing the occurrence"),
        )
        .expect("candidate occurrence can carry B");
    mixed
        .merge(&NetworkReplyRoutes::try_from_route(route_c.clone()).expect("new source C is live"))
        .expect("candidate occurrence can carry C");
    assert!(route_fixture.retire(&route_b));
    assert!(
        adapter.push_effect(V2LaneWorkEffect::PostNativeAmx {
            peer: peer.clone(),
            reply_routes: Some(mixed),
            message: message.clone(),
        }),
        "stale and inactive attempts must not suppress an independent live source"
    );
    let Some(V2LaneWorkEffect::PostNativeAmx {
        reply_routes: Some(retained),
        ..
    }) = adapter.effects.front()
    else {
        panic!("a mixed-liveness merge must retain its accepted live sources");
    };
    assert_eq!(retained.len(), 2);
    assert!(retained.iter().any(|route| route.same_delivery(&later_a)));
    assert!(!retained.iter().any(|route| route.same_delivery(&route_b)));
    assert!(retained.iter().any(|route| route.same_delivery(&route_c)));
    assert!(
        !retained
            .iter()
            .any(|route| route.same_delivery(&reconnected_a))
    );
    let retired_only = Some(
        NetworkReplyRoutes::try_from_route(route_c.clone())
            .expect("candidate captures source C before retirement"),
    );
    assert!(route_fixture.retire(&route_c));
    assert!(
        !adapter.push_effect(V2LaneWorkEffect::PostNativeAmx {
            peer,
            reply_routes: retired_only,
            message,
        }),
        "the adapter commits maintenance but reports no retained candidate delivery"
    );
    let Some(V2LaneWorkEffect::PostNativeAmx {
        reply_routes: queued,
        ..
    }) = adapter.effects.front()
    else {
        panic!("queued duplicate retains its route history");
    };
    let retained = queued
        .as_ref()
        .expect("maintenance keeps the live sibling route set");
    assert_eq!(retained.len(), 1);
    assert!(retained.iter().any(|route| route.same_delivery(&later_a)));
    assert!(!retained.iter().any(|route| route.same_delivery(&route_c)));
}
#[test]
fn temporarily_unserviceable_effect_requeues_behind_later_reserved_work() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let message = NativeAmxMessage::PrepareRequest(native_request(&adapter, &keys));
    let first = V2LaneWorkEffect::PostNativeAmx {
        peer: adapter.context.roster[1].validator.clone(),
        reply_routes: None,
        message: message.clone(),
    };
    let second = V2LaneWorkEffect::PostNativeAmx {
        peer: adapter.context.roster[2].validator.clone(),
        reply_routes: None,
        message,
    };
    let first_key = lane_work_effect_key(&first);
    let second_key = lane_work_effect_key(&second);
    assert!(adapter.push_effect(first.clone()));
    assert!(adapter.push_effect(second.clone()));
    assert_eq!(
        adapter.next_effect().as_ref().map(lane_work_effect_key),
        Some(first_key)
    );
    let blocked = adapter
        .drain_effects(1)
        .pop()
        .expect("peeked effect remains drainable");
    assert_eq!(lane_work_effect_key(&blocked), first_key);
    assert!(adapter.requeue_effect(blocked));
    assert_eq!(
        adapter.next_effect().as_ref().map(lane_work_effect_key),
        Some(second_key)
    );
    let second = adapter
        .drain_effects(1)
        .pop()
        .expect("later reserved effect remains queued");
    assert_eq!(lane_work_effect_key(&second), second_key);
    let first = adapter
        .drain_effects(1)
        .pop()
        .expect("requeued effect remains owned");
    assert_eq!(lane_work_effect_key(&first), first_key);
    assert_eq!(adapter.effect_count(), 0);
}
#[test]
fn retransmission_classes_rotate_fairly_at_capacity_one() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let (_, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    assert_eq!(
        adapter.lane_sessions.insert_proposal(proposal.clone()),
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    );
    adapter.locally_bound_lane_proposals.insert(
        proposal.proposal_hash,
        proposal
            .payload_block_hint
            .expect("planned proposal carries its global block hint"),
    );
    let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
    adapter
        .retain_merge_sidecars_for_global_view(candidate.view, None, None)
        .expect("install exact unlocked reducer directive");
    assert!(adapter.drain_effects(usize::MAX).iter().any(|effect| {
        matches!(effect, V2LaneWorkEffect::BroadcastMerge(signature) if signature.message_digest
        == crate::merge::merge_qc_message_digest(
            &adapter.context.network_id,
            &candidate,
            VALIDATOR_SET_HASH_VERSION_V1,
            adapter.frozen_validator_set_hash(),
        ))
    }));
    let request = native_request(&adapter, &keys);
    let body = request.body;
    let peer = adapter
        .context
        .roster
        .iter()
        .map(|entry| &entry.validator)
        .find(|peer| *peer != &adapter.local_peer)
        .expect("fixture has a remote validator")
        .clone();
    adapter.native_requests.insert(
        NativeRequestKey {
            body,
            peer: peer.clone(),
        },
        NativeAmxMessage::PrepareRequest(request),
    );
    adapter.limits.effect_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
    adapter
        .schedule_retransmission()
        .expect("schedule lane retransmission");
    assert!(matches!(
        adapter.drain_effects(usize::MAX).as_slice(),
        [V2LaneWorkEffect::PostLaneBlock { .. }]
    ));
    adapter
        .schedule_retransmission()
        .expect("schedule Native AMX retransmission");
    assert!(matches!(
        adapter.drain_effects(usize::MAX).as_slice(),
        [V2LaneWorkEffect::PostNativeAmx { .. }]
    ));
    adapter
        .schedule_retransmission()
        .expect("schedule merge retransmission");
    assert!(matches!(
        adapter.drain_effects(usize::MAX).as_slice(),
        [V2LaneWorkEffect::BroadcastMerge(_)]
    ));
}
#[test]
fn certified_merge_sidecar_effect_dedup_is_destination_and_payload_bound() {
    let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
    let responder = adapter.context.roster[0].validator.clone();
    let alternate_destination = adapter.context.roster[1].validator.clone();
    let mut request = crate::merge_sidecar::CertifiedMergeSidecarRequestV1 {
        version: crate::merge_sidecar::CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1(
            NonZeroU64::new(1).expect("sidecar stream epoch is non-zero"),
        ),
        semantic_sequence: semantic_sequence(1),
        closed_through: 0,
        request_id: Hash::prehashed([0; Hash::LENGTH]),
        entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"v2-lane-work-sidecar-entry")),
        encoded_len: 128,
        epoch_id: 4,
        reference_digest: Hash::new(b"v2-lane-work-sidecar-reference"),
        requester: adapter.local_peer.clone(),
        responder: responder.clone(),
    };
    request.request_id = request.canonical_request_id();
    let effect = V2LaneWorkEffect::PostCertifiedMergeSidecar {
        peer: responder,
        reply_routes: None,
        message: Arc::new(CertifiedMergeSidecarMessage::Request(request.clone())),
    };
    assert!(adapter.push_effect(effect.clone()));
    assert!(adapter.push_effect(effect.clone()));
    assert_eq!(adapter.effects.len(), 1, "an exact retry is deduplicated");
    assert!(
        adapter.push_effect(V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer: alternate_destination,
            reply_routes: None,
            message: Arc::new(CertifiedMergeSidecarMessage::Request(request.clone())),
        })
    );
    assert_eq!(
        adapter.effects.len(),
        2,
        "the authenticated destination is part of the effect identity"
    );
    let mut distinct_request = request;
    distinct_request.request_id = Hash::new(b"v2-lane-work-sidecar-request-2");
    assert!(
        adapter.push_effect(V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer: distinct_request.responder.clone(),
            reply_routes: None,
            message: Arc::new(CertifiedMergeSidecarMessage::Request(distinct_request)),
        })
    );
    assert_eq!(
        adapter.effects.len(),
        3,
        "the bounded sidecar payload is part of the effect identity"
    );
    assert_eq!(adapter.drain_effects(usize::MAX).len(), 3);
    assert!(adapter.push_effect(effect));
    assert_eq!(
        adapter.effects.len(),
        1,
        "a drained sidecar transport may be retried"
    );
}
