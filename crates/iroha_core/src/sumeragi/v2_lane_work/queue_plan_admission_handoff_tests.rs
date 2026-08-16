macro_rules! qp_lane_case { ($($tokens:tt)*) => { $($tokens)* }; }
fn queue_plan_remote_leader_view(adapter: &V2LaneWorkAdapter) -> wire::View {
    let local = adapter.local_validator_index().expect("local validator");
    (0..u64::try_from(adapter.context.roster.len()).expect("bounded roster") * 2)
        .find(|view| adapter.context.leader(*view) != local)
        .expect("rotating remote leader")
}

fn queue_plan_test_certificate_at_height(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    tag: u8,
    authority_height: u64,
    predecessor_block_hash: Option<HashOf<BlockHeader>>,
) -> (crate::torii_proxy::QueuePlanAdmissionBindingV2, Vec<u8>) {
    let proposal_height = authority_height.checked_add(1).expect("proposal height");
    let routing_plan =
        RoutingPlan::single(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL));
    qp_lane_case! { let route_incarnations = routing_plan.legs().into_iter().map(|leg| { let validator_set = crate::queue::queue_plan_authoritative_peers_in_view_at_height(&adapter.state.view(), leg.route, proposal_height); crate::queue::QueuePlanRouteIncarnationV2 { leg, lane_incarnation: adapter.state.lane_incarnation_at_height(leg.route.lane_id, proposal_height).expect("active route"), validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1, validator_set_hash: HashOf::new(&validator_set), validator_count: u16::try_from(validator_set.len()).expect("validator count"), durability_threshold: u16::try_from(validator_set.len().div_ceil(3)).expect("threshold"), validator_set } }).collect(); let context = crate::queue::QueuePlanAdmissionContextV2 { version: crate::queue::QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V2, authority_height, proposal_height, predecessor_block_hash, routing_plan_digest: routing_plan.digest(), route_incarnations }; }
    qp_lane_case! { let tx_key = KeyPair::try_from_seed(vec![tag.wrapping_add(0x31); 32], Algorithm::Ed25519).expect("transaction key"); let mut tx = TransactionBuilder::new(adapter.context.network_id, AccountId::new(tx_key.public_key().clone()), iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None)); tx.set_creation_time(Duration::from_millis(u64::from(tag) + 1)); let entrypoint = TransactionEntrypoint::External(tx.with_instructions([Log::new(Level::INFO, format!("queue-plan-{tag}"))]).sign(tx_key.private_key())); let binding = crate::torii_proxy::QueuePlanAdmissionBindingV2::new(&adapter.context.network_id, &entrypoint, &routing_plan, context, u64::from(tag) + 100).expect("binding"); }
    let binding_hash = binding.canonical_hash();
    let coordinator = &binding.admission_context.route_incarnations[0];
    qp_lane_case! { let attestations = coordinator.validator_set.iter().take(usize::from(coordinator.durability_threshold)).enumerate().map(|(index, validator)| { let key = keys.iter().find(|key| key.public_key() == validator.public_key()).expect("authority key"); let validator_index = u16::try_from(index).expect("validator index"); let preimage = crate::torii_proxy::queue_plan_admission_attestation_signing_bytes_v2(binding_hash, validator_index).expect("preimage"); crate::torii_proxy::QueuePlanAdmissionAttestationV2 { version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V2, validator_index, signature: Signature::try_new(key.private_key(), &preimage).expect("signature") } }).collect(); let certificate = crate::torii_proxy::QueuePlanAdmissionCertificateV2 { version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V2, binding: binding.clone(), attestations }; (binding, norito::encode_canonical(&certificate).expect("certificate")) }
}

pub(in crate::sumeragi) fn queue_plan_test_certificate(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    tag: u8,
) -> (crate::torii_proxy::QueuePlanAdmissionBindingV2, Vec<u8>) {
    queue_plan_test_certificate_at_height(
        adapter,
        keys,
        tag,
        adapter.context.height - 1,
        adapter
            .context
            .parent_commit_qc
            .as_ref()
            .map(|qc| qc.subject.block_hash),
    )
}

pub(in crate::sumeragi) fn prepare_queue_plan_test(
    adapter: &mut V2LaneWorkAdapter,
    keys: &[KeyPair],
) {
    enable_multilane_nexus(adapter, keys, LaneId::new(1), DataSpaceId::new(7));
}

fn queue_plan_relay(
    adapter: &mut V2LaneWorkAdapter,
    sender: &PeerId,
    bytes: Vec<u8>,
    view: wire::View,
) -> V2LaneIngressOutcome {
    adapter.accept_relay_message(
        LaneRelayMessage::QueuePlanAdmissionCertificate {
            sender: sender.clone(),
            certificate: Arc::new(bytes),
        },
        view,
    )
}

fn assert_queue_plan_kura_source(adapter: &V2LaneWorkAdapter, bytes: &[u8]) {
    let stored = adapter
        .kura
        .pending_queue_plan_admission_certificate(Hash::new(bytes))
        .unwrap();
    assert_eq!(stored.as_deref(), Some(bytes));
}

fn assert_queue_plan_rejected(
    adapter: &mut V2LaneWorkAdapter,
    sender: &PeerId,
    bytes: Vec<u8>,
    view: wire::View,
) {
    assert_eq!(
        queue_plan_relay(adapter, sender, bytes, view),
        V2LaneIngressOutcome::Rejected
    );
}

#[test]
fn queue_plan_nonleader_handoff_targets_frozen_leader_with_exact_bytes() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    prepare_queue_plan_test(&mut adapter, &keys);
    let (_, bytes) = queue_plan_test_certificate(&adapter, &keys, 0x40);
    adapter
        .kura
        .persist_pending_queue_plan_admission_certificate(&bytes)
        .expect("persist");
    let view = queue_plan_remote_leader_view(&adapter);
    let leader = adapter.context.roster[usize::try_from(adapter.context.leader(view)).unwrap()]
        .validator
        .clone();
    adapter
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("view");
    let effect = adapter
        .drain_effects(usize::MAX)
        .into_iter()
        .find_map(|effect| match effect {
            V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
                peer,
                view,
                certificate,
            } => Some((peer, view, certificate)),
            _ => None,
        })
        .expect("handoff");
    assert_eq!(
        (effect.0, effect.1, effect.2.as_slice()),
        (leader, view, bytes.as_slice())
    );
    assert_queue_plan_kura_source(&adapter, &bytes);
}

#[test]
fn queue_plan_leader_stages_exact_handoff_idempotently() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    prepare_queue_plan_test(&mut adapter, &keys);
    adapter
        .retain_merge_sidecars_for_global_view(0, None, None)
        .expect("view");
    adapter.drain_effects(usize::MAX);
    let (_, bytes) = queue_plan_test_certificate(&adapter, &keys, 0x41);
    let sender = PeerId::new(KeyPair::random().public_key().clone());
    assert_eq!(
        queue_plan_relay(&mut adapter, &sender, bytes.clone(), 0),
        V2LaneIngressOutcome::Inserted
    );
    let scans = adapter
        .kura
        .pending_queue_plan_admission_inventory_scans
        .load(Ordering::Relaxed);
    let reads = adapter
        .kura
        .pending_queue_plan_admission_exact_reads
        .load(Ordering::Relaxed);
    let effects = adapter.effect_count();
    assert_eq!(
        queue_plan_relay(&mut adapter, &sender, bytes.clone(), 0),
        V2LaneIngressOutcome::Duplicate
    );
    assert_eq!(adapter.effect_count(), effects);
    assert_eq!(
        adapter
            .kura
            .pending_queue_plan_admission_inventory_scans
            .load(Ordering::Relaxed),
        scans
    );
    assert_eq!(
        adapter
            .kura
            .pending_queue_plan_admission_exact_reads
            .load(Ordering::Relaxed),
        reads + 1
    );
    assert_queue_plan_kura_source(&adapter, &bytes);
}

#[test]
fn queue_plan_handoff_rejects_nonleader_future_stale_conflict_and_corrupt() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    prepare_queue_plan_test(&mut adapter, &keys);
    let sender = adapter.local_peer.clone();
    let (_, valid) = queue_plan_test_certificate(&adapter, &keys, 0x42);
    let remote_view = queue_plan_remote_leader_view(&adapter);
    assert_queue_plan_rejected(&mut adapter, &sender, valid, remote_view);
    let (_, stale) = queue_plan_test_certificate_at_height(
        &adapter,
        &keys,
        0x43,
        adapter.context.height - 1,
        Some(HashOf::from_untyped_unchecked(Hash::new(
            b"stale predecessor",
        ))),
    );
    assert_queue_plan_rejected(&mut adapter, &sender, stale, 0);

    let future_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"future predecessor"));
    {
        let mut hashes = adapter.state.block_hashes.block();
        hashes.push_for_tests(future_hash);
        hashes.commit_for_tests();
    }
    let (_, future) = queue_plan_test_certificate_at_height(
        &adapter,
        &keys,
        0x44,
        adapter.context.height,
        Some(future_hash),
    );
    {
        let hashes = adapter.state.block_hashes.block_and_revert();
        hashes.commit_for_tests();
    }
    assert_queue_plan_rejected(&mut adapter, &sender, future.clone(), 0);
    let future_certificate_hash = Hash::new(&future);
    assert!(
        adapter
            .kura
            .pending_queue_plan_admission_certificate(future_certificate_hash)
            .unwrap()
            .is_none()
    );
    adapter
        .kura
        .persist_pending_queue_plan_admission_certificate(&future)
        .expect("persist validated origin source");
    adapter
        .refresh_merge_candidates(0)
        .expect("defer durable Future");
    assert!(adapter.drain_effects(usize::MAX).is_empty());
    assert_queue_plan_kura_source(&adapter, &future);
    {
        let mut hashes = adapter.state.block_hashes.block();
        hashes.push_for_tests(future_hash);
        hashes.commit_for_tests();
    }
    adapter.context.height += 1;
    let caught_up_view = queue_plan_remote_leader_view(&adapter);
    let leader = adapter.context.roster
        [usize::try_from(adapter.context.leader(caught_up_view)).unwrap()]
    .validator
    .clone();
    assert!(
        adapter
            .reconcile_pending_queue_plan_admissions(caught_up_view)
            .expect("reclassify caught-up Future")
            .is_empty()
    );
    assert!(
        adapter
            .drain_effects(usize::MAX)
            .into_iter()
            .any(|effect| matches!(
                effect,
                V2LaneWorkEffect::PostQueuePlanAdmissionCertificate { peer, view, certificate }
                    if peer == leader && view == caught_up_view && certificate.as_slice() == future
            ))
    );
    adapter.context.height -= 1;
    {
        let hashes = adapter.state.block_hashes.block_and_revert();
        hashes.commit_for_tests();
    }
    adapter
        .kura
        .remove_pending_queue_plan_admission_certificate(future_certificate_hash)
        .expect("clear Future fixture");

    let (binding, conflict) = queue_plan_test_certificate(&adapter, &keys, 0x45);
    let key = format!(
        "queue_plan_admission_v2_{}_{}",
        hex::encode(binding.registry_key().network_id_digest.as_ref()),
        hex::encode(binding.registry_key().entrypoint_hash.as_ref())
    )
    .parse()
    .unwrap();
    let marker = crate::torii_proxy::QueuePlanAdmissionRegistryValueV2 {
        version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_BINDING_VERSION_V2,
        binding_hash: Hash::new(b"other binding"),
    };
    {
        let mut world = adapter.state.world.block();
        world
            .smart_contract_state
            .insert(key, norito::to_bytes(&marker).unwrap());
        world.commit();
    }
    assert_queue_plan_rejected(&mut adapter, &sender, conflict, 0);
    assert_queue_plan_rejected(&mut adapter, &sender, vec![0xFF; 16], 0);
    let pending = adapter
        .kura
        .pending_queue_plan_admission_certificates_bounded(
            adapter.kura.pending_queue_plan_admission_capacity(),
        );
    assert!(pending.unwrap().is_empty());
}

#[test]
fn queue_plan_handoff_cursor_rotates_under_effect_pressure() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    prepare_queue_plan_test(&mut adapter, &keys);
    let view = queue_plan_remote_leader_view(&adapter);
    adapter
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("view");
    adapter.drain_effects(usize::MAX);
    for tag in [0x47, 0x48] {
        let (_, bytes) = queue_plan_test_certificate(&adapter, &keys, tag);
        adapter
            .kura
            .persist_pending_queue_plan_admission_certificate(&bytes)
            .expect("persist");
    }
    adapter.limits.effect_capacity = NonZeroUsize::new(1).unwrap();
    adapter.push_effect(V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
        peer: adapter.local_peer.clone(),
        view: u64::MAX,
        certificate: Arc::new(vec![0xA5]),
    });
    assert!(
        !adapter
            .refresh_pending_queue_plan_admission_handoffs(view)
            .unwrap()
    );
    adapter.drain_effects(usize::MAX);
    let next = |adapter: &mut V2LaneWorkAdapter| {
        assert!(
            !adapter
                .refresh_pending_queue_plan_admission_handoffs(view)
                .unwrap()
        );
        match adapter.drain_effects(1).pop().unwrap() {
            V2LaneWorkEffect::PostQueuePlanAdmissionCertificate { certificate, .. } => certificate,
            other => panic!("unexpected effect {other:?}"),
        }
    };
    let first = next(&mut adapter);
    let second = next(&mut adapter);
    assert_ne!(first, second);
}

include!("autonomous_retirement_and_merge_tests.rs");
