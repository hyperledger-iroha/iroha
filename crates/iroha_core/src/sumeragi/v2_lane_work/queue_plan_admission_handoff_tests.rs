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
) -> (crate::torii_proxy::QueuePlanAdmissionBindingV1, Vec<u8>) {
    let proposal_height = authority_height.checked_add(1).expect("proposal height");
    let routing_plan =
        RoutingPlan::single(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL));
    qp_lane_case! { let route_incarnations = routing_plan.legs().into_iter().map(|leg| { let validator_set = crate::queue::queue_plan_authoritative_peers_in_view_at_height(&adapter.state.view(), leg.route, proposal_height).expect("route authority"); crate::queue::QueuePlanRouteIncarnationV1 { leg, lane_incarnation: adapter.state.lane_incarnation_at_height(leg.route.lane_id, proposal_height).expect("active route"), validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1, validator_set_hash: HashOf::new(&validator_set), validator_count: u16::try_from(validator_set.len()).expect("validator count"), durability_threshold: u16::try_from(validator_set.len().div_ceil(3)).expect("threshold"), validator_set } }).collect(); let context = crate::queue::QueuePlanAdmissionContextV1 { version: crate::queue::QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V1, authority_height, proposal_height, predecessor_block_hash, routing_plan_digest: routing_plan.digest(), route_incarnations }; }
    qp_lane_case! { let tx_key = KeyPair::try_from_seed(vec![tag.wrapping_add(0x31); 32], Algorithm::Ed25519).expect("transaction key"); let mut tx = TransactionBuilder::new(adapter.context.network_id, AccountId::new(tx_key.public_key().clone()), iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None)); tx.set_creation_time(Duration::from_millis(u64::from(tag) + 1)); let entrypoint = TransactionEntrypoint::External(tx.with_instructions([Log::new(Level::INFO, format!("queue-plan-{tag}"))]).sign(tx_key.private_key())); let binding = crate::torii_proxy::QueuePlanAdmissionBindingV1::new(&adapter.context.network_id, &entrypoint, &routing_plan, context, u64::from(tag) + 100).expect("binding"); }
    let binding_hash = binding.canonical_hash();
    let coordinator = &binding.admission_context.route_incarnations[0];
    qp_lane_case! { let attestations = coordinator.validator_set.iter().take(usize::from(coordinator.durability_threshold)).enumerate().map(|(index, validator)| { let key = keys.iter().find(|key| key.public_key() == validator.public_key()).expect("authority key"); let validator_index = u16::try_from(index).expect("validator index"); let preimage = crate::torii_proxy::queue_plan_admission_attestation_signing_bytes_v1(binding_hash, validator_index).expect("preimage"); crate::torii_proxy::QueuePlanAdmissionAttestationV1 { version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V1, validator_index, signature: Signature::try_new(key.private_key(), &preimage).expect("signature") } }).collect(); let certificate = crate::torii_proxy::QueuePlanAdmissionCertificateV1 { version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V1, binding: binding.clone(), attestations }; (binding, norito::encode_canonical(&certificate).expect("certificate")) }
}

pub(in crate::sumeragi) fn queue_plan_test_certificate(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    tag: u8,
) -> (crate::torii_proxy::QueuePlanAdmissionBindingV1, Vec<u8>) {
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
    assert!(
        adapter
            .refresh_pending_queue_plan_admission_handoffs(view)
            .expect("service the separately scheduled durable admission handoff")
    );
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
fn queue_plan_exact_marker_retains_certificate_until_transaction_application() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    prepare_queue_plan_test(&mut adapter, &keys);
    let (binding, bytes) = queue_plan_test_certificate(&adapter, &keys, 0x46);
    adapter
        .state
        .install_queue_plan_pending_binding_for_test(&binding)
        .expect("install exact marker and pending transaction obligation");
    let certificate_hash = Hash::new(&bytes);
    assert!(
        adapter
            .kura
            .pending_queue_plan_admission_certificate(certificate_hash)
            .expect("inspect missing exact-marker handoff")
            .is_none()
    );
    let sender = PeerId::new(KeyPair::random().public_key().clone());
    assert_eq!(
        queue_plan_relay(&mut adapter, &sender, bytes.clone(), 0),
        V2LaneIngressOutcome::Inserted,
        "an exact pending WSV marker must not be mistaken for an applied transaction"
    );
    assert_eq!(
        queue_plan_relay(&mut adapter, &sender, bytes.clone(), 0),
        V2LaneIngressOutcome::Duplicate,
        "the recovered sidecar must remain idempotent"
    );

    assert!(
        adapter
            .reconcile_pending_queue_plan_admissions(0)
            .expect("reconcile exact pending marker")
            .is_empty()
    );
    assert_queue_plan_kura_source(&adapter, &bytes);
    let indexed = adapter
        .state
        .pending_queue_plan_admission_gossip_certificates()
        .expect("index exact pending handoff");
    assert_eq!(
        indexed
            .get(&binding.canonical_hash())
            .map(|certificate| certificate.as_slice()),
        Some(bytes.as_slice())
    );
}

#[test]
fn queue_plan_handoff_retains_future_but_rejects_nonleader_stale_conflict_and_corrupt() {
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
    let mut self_declared =
        norito::decode_from_bytes::<crate::torii_proxy::QueuePlanAdmissionCertificateV1>(&future)
            .expect("decode future certificate for adversarial roster mutation");
    let compromised_key = keys.first().expect("fixture current authority key");
    let self_declared_peer = PeerId::from(compromised_key.public_key().clone());
    let coordinator = self_declared
        .binding
        .admission_context
        .route_incarnations
        .first_mut()
        .expect("single-route future certificate");
    coordinator.validator_set = vec![self_declared_peer];
    coordinator.validator_count = 1;
    coordinator.durability_threshold = 1;
    coordinator.validator_set_hash = HashOf::new(&coordinator.validator_set);
    let self_declared_binding_hash = self_declared.binding.canonical_hash();
    let signing_bytes = crate::torii_proxy::queue_plan_admission_attestation_signing_bytes_v1(
        self_declared_binding_hash,
        0,
    )
    .expect("encode self-declared future attestation preimage");
    self_declared.attestations = vec![crate::torii_proxy::QueuePlanAdmissionAttestationV1 {
        version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V1,
        validator_index: 0,
        signature: Signature::try_new(compromised_key.private_key(), &signing_bytes)
            .expect("sign self-declared future certificate"),
    }];
    let self_declared =
        norito::encode_canonical(&self_declared).expect("encode self-declared future certificate");
    assert_queue_plan_rejected(&mut adapter, &sender, self_declared.clone(), 0);
    assert!(
        adapter
            .kura
            .pending_queue_plan_admission_certificate(Hash::new(&self_declared))
            .expect("inspect rejected self-declared Future")
            .is_none(),
        "a self-declared future roster must not consume durable Kura capacity"
    );
    assert_eq!(
        queue_plan_relay(&mut adapter, &sender, future.clone(), 0),
        V2LaneIngressOutcome::Inserted,
        "the current leader must durably park an authenticated Future certificate"
    );
    let future_certificate_hash = Hash::new(&future);
    assert_queue_plan_kura_source(&adapter, &future);
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
    let marker = crate::torii_proxy::QueuePlanAdmissionRegistryValueV1 {
        version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_BINDING_VERSION_V1,
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
fn queue_plan_handoff_retires_future_after_current_source_incarnation_drifts() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    prepare_queue_plan_test(&mut adapter, &keys);
    let queue = Arc::new(Queue::test(
        iroha_config::parameters::actual::Queue::default(),
        &iroha_primitives::time::TimeSource::new_system(),
    ));
    adapter
        .install_lane_drain_queue(queue)
        .expect("install queue needed for exact stale-claim reconciliation");
    let sender = adapter.local_peer.clone();
    let future_authority_height = adapter
        .context
        .height
        .checked_add(1)
        .expect("two-step future authority height");
    let (_, future) = queue_plan_test_certificate_at_height(
        &adapter,
        &keys,
        0x49,
        future_authority_height,
        Some(HashOf::from_untyped_unchecked(Hash::new(
            b"two-step future predecessor",
        ))),
    );
    assert_eq!(
        queue_plan_relay(&mut adapter, &sender, future.clone(), 0),
        V2LaneIngressOutcome::Inserted
    );
    assert_queue_plan_kura_source(&adapter, &future);

    let _ = adapter.state.set_lane_incarnation_for_test(
        LaneId::SINGLE,
        Hash::new(b"future source incarnation rotated before catch-up"),
    );
    assert!(
        adapter
            .reconcile_pending_queue_plan_admissions(0)
            .expect("source-authority drift must retire instead of wedging reconciliation")
            .is_empty()
    );
    assert!(
        adapter
            .kura
            .pending_queue_plan_admission_certificate(Hash::new(&future))
            .expect("inspect retired future certificate")
            .is_none(),
        "a no-longer-authenticated Future must release its bounded Kura slot"
    );
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

#[test]
fn queue_plan_handoff_preserves_fresh_admission_before_height_adapter_rollover() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    prepare_queue_plan_test(&mut adapter, &keys);
    let queue = Arc::new(Queue::test(
        iroha_config::parameters::actual::Queue::default(),
        &iroha_primitives::time::TimeSource::new_system(),
    ));
    adapter
        .install_lane_drain_queue(queue)
        .expect("install exact queue owner for pending admission reconciliation");
    let sender = adapter.local_peer.clone();
    let successor_parent = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"post-WSV predecessor before process-height rollover",
    ));
    {
        let mut hashes = adapter.state.block_hashes.block();
        hashes.push_for_tests(successor_parent);
        hashes.commit_for_tests();
    }
    let (_, certificate) = queue_plan_test_certificate_at_height(
        &adapter,
        &keys,
        0x4A,
        adapter.context.height,
        Some(successor_parent),
    );
    {
        let hashes = adapter.state.block_hashes.block_and_revert();
        hashes.commit_for_tests();
    }
    assert_eq!(
        queue_plan_relay(&mut adapter, &sender, certificate.clone(), 0),
        V2LaneIngressOutcome::Inserted,
        "authenticate and durably retain the future certificate before WSV publication"
    );
    assert_queue_plan_kura_source(&adapter, &certificate);
    {
        let mut hashes = adapter.state.block_hashes.block();
        hashes.push_for_tests(successor_parent);
        hashes.commit_for_tests();
    }
    let current_carrier_height = adapter.context.height.checked_add(1).unwrap();
    assert!(matches!(
        adapter
            .state
            .classify_pending_queue_plan_admission(&certificate, current_carrier_height)
            .expect("classify using the current State frontier")
            .1,
        PendingQueuePlanAdmissionDisposition::EligibleAbsent
    ));

    // WSV can publish before the asynchronous Apply owner completes process-height rollover.
    // An old adapter cannot terminalize the next height's authenticated admission.
    assert!(
        adapter
            .reconcile_pending_queue_plan_admissions(0)
            .expect("old adapter must defer a current-State admission")
            .is_empty()
    );
    assert_queue_plan_kura_source(&adapter, &certificate);

    adapter.context.height = current_carrier_height;
    let current_view = queue_plan_remote_leader_view(&adapter);
    let current_leader = adapter.context.roster
        [usize::try_from(adapter.context.leader(current_view)).unwrap()]
    .validator
    .clone();
    assert!(
        adapter
            .reconcile_pending_queue_plan_admissions(current_view)
            .expect("handoff from the current height adapter")
            .is_empty()
    );
    assert!(adapter.drain_effects(usize::MAX).into_iter().any(|effect| {
        matches!(effect, V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
            peer, view, certificate: observed,
        } if peer == current_leader && view == current_view && observed.as_slice() == certificate)
    }));
    assert_queue_plan_kura_source(&adapter, &certificate);
}

fn queue_plan_materialized_certificate_for_binding(
    binding: &crate::torii_proxy::QueuePlanAdmissionBindingV1,
    keys: &[KeyPair],
) -> Vec<u8> {
    let coordinator = &binding.admission_context.route_incarnations[0];
    assert_eq!(coordinator.validator_set.len(), 4);
    assert_eq!(coordinator.durability_threshold, 2);
    let attestations = (0..coordinator.durability_threshold)
        .map(|validator_index| {
            let validator = &coordinator.validator_set[usize::from(validator_index)];
            let key = keys
                .iter()
                .find(|key| key.public_key() == validator.public_key())
                .expect("exact frozen authority key");
            let preimage = crate::torii_proxy::queue_plan_admission_attestation_signing_bytes_v1(
                binding.canonical_hash(),
                validator_index,
            )
            .unwrap();
            crate::torii_proxy::QueuePlanAdmissionAttestationV1 {
                version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V1,
                validator_index,
                signature: Signature::try_new(key.private_key(), &preimage).unwrap(),
            }
        })
        .collect();
    norito::encode_canonical(&crate::torii_proxy::QueuePlanAdmissionCertificateV1 {
        version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V1,
        binding: binding.clone(),
        attestations,
    })
    .unwrap()
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one exact admission is traced through durable Queue ownership and both height adapters"
)]
fn queue_plan_handoff_preserves_materialized_fifo_before_height_adapter_rollover() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    prepare_queue_plan_test(&mut adapter, &keys);
    let journal_dir = tempfile::tempdir().unwrap();
    let journal_path = journal_dir.path().join("post-wsv-reservations.norito");
    let plan_path = journal_path.with_extension("plans.norito");
    let queue = install_autonomous_test_queue(
        &mut adapter,
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        &journal_path,
    );
    let successor_parent = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"materialized-post-WSV-parent-before-process-height-rollover",
    ));
    {
        let mut hashes = adapter.state.block_hashes.block();
        hashes.push_for_tests(successor_parent);
        hashes.commit_for_tests();
    }
    let key = KeyPair::try_from_seed(vec![0xB8; 32], Algorithm::Ed25519).unwrap();
    let authority = AccountId::new(key.public_key().clone());
    {
        let mut world = adapter.state.world.block();
        world.accounts.insert(
            authority.clone(),
            AccountValue::new(AccountDetails::default()),
        );
        world.commit();
    }
    let transaction = TransactionBuilder::new(
        adapter.context.network_id,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_admission_intent(
        iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
    )
    .with_instructions([Log::new(
        Level::INFO,
        "post-wsv-exact-queued-owner".to_owned(),
    )])
    .sign(key.private_key());
    let accepted =
        crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(transaction));
    let routing_plan = queue
        .route_plan_with_state(&accepted, &adapter.state)
        .unwrap();
    let context = queue
        .plan_admission_context_with_state(&adapter.state, &routing_plan)
        .unwrap();
    assert_eq!(context.authority_height, adapter.context.height);
    let current_height = context.proposal_height;
    let binding = crate::torii_proxy::QueuePlanAdmissionBindingV1::new(
        adapter.state.network_id_ref(),
        accepted.entrypoint(),
        &routing_plan,
        context,
        queue.queue_plan_admission_timestamp_ms_for(&accepted),
    )
    .unwrap();
    let exact_claim = queue
        .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
            accepted.clone(),
            &adapter.state,
            routing_plan,
            &binding,
        )
        .expect("materialize the exact current-State transaction and fsync its journal claim");
    let certificate = queue_plan_materialized_certificate_for_binding(&binding, &keys);
    crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v1(
        adapter.state.network_id_ref(),
        &certificate,
    )
    .expect("fixture retains real exact-roster quorum authentication");
    adapter
        .kura
        .persist_pending_queue_plan_admission_certificate(&certificate)
        .expect("durably retain the exact authenticated handoff source");
    let before_journal = std::fs::read(&plan_path).unwrap();
    let before_fifo = queue.fifo_snapshot_for_test();
    assert_eq!(before_fifo, vec![binding.entrypoint_hash]);
    assert_eq!((queue.active_len(), queue.queued_len()), (1, 1));
    assert!(
        adapter
            .reconcile_pending_queue_plan_admissions(0)
            .expect("the old adapter defers a valid current-State admission")
            .is_empty()
    );
    assert_queue_plan_kura_source(&adapter, &certificate);
    assert_eq!(std::fs::read(&plan_path).unwrap(), before_journal);
    assert_eq!(queue.fifo_snapshot_for_test(), before_fifo);
    assert_eq!((queue.active_len(), queue.queued_len()), (1, 1));
    assert_eq!(
        queue
            .durable_plan_admission_claim_with_state(&accepted, &adapter.state)
            .unwrap(),
        Some(exact_claim.clone())
    );

    adapter.context.height = current_height;
    let view = queue_plan_remote_leader_view(&adapter);
    let leader = adapter.context.roster[usize::try_from(adapter.context.leader(view)).unwrap()]
        .validator
        .clone();
    assert!(
        adapter
            .reconcile_pending_queue_plan_admissions(view)
            .unwrap()
            .is_empty()
    );
    assert!(adapter.drain_effects(usize::MAX).into_iter().any(|effect| {
        matches!(effect, V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
            peer, view: observed_view, certificate: observed,
        } if peer == leader && observed_view == view && observed.as_slice() == certificate)
    }));
    assert_queue_plan_kura_source(&adapter, &certificate);
    assert_eq!(std::fs::read(&plan_path).unwrap(), before_journal);
    assert_eq!(queue.fifo_snapshot_for_test(), before_fifo);
    assert_eq!((queue.active_len(), queue.queued_len()), (1, 1));
    assert_eq!(
        queue
            .durable_plan_admission_claim_with_state(&accepted, &adapter.state)
            .unwrap(),
        Some(exact_claim)
    );
    let local_index = adapter.local_validator_index().unwrap();
    let local_view = (0..u64::try_from(adapter.context.roster.len()).unwrap() * 2)
        .find(|view| adapter.context.leader(*view) == local_index)
        .expect("the current-height leader schedule eventually selects this validator");
    assert_eq!(
        adapter
            .reconcile_pending_queue_plan_admissions(local_view)
            .unwrap(),
        vec![certificate.clone()],
        "the current leader selects the exact retained admission for its next carrier"
    );
    assert_queue_plan_kura_source(&adapter, &certificate);
    assert_eq!(queue.fifo_snapshot_for_test(), before_fifo);
}

#[test]
fn queue_plan_handoff_retains_new_admission_while_worker_height_is_obsolete() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    prepare_queue_plan_test(&mut adapter, &keys);
    let old_worker_height = adapter.context.height;
    let successor_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"queue-plan-canonical-successor-before-worker-handoff",
    ));
    {
        let mut hashes = adapter.state.block_hashes.block();
        hashes.push_for_tests(successor_hash);
        hashes.commit_for_tests();
    }
    assert_eq!(adapter.state.committed_height() as u64, old_worker_height);
    let (_, certificate) = queue_plan_test_certificate_at_height(
        &adapter,
        &keys,
        0x4a,
        old_worker_height,
        Some(successor_hash),
    );
    adapter
        .kura
        .persist_pending_queue_plan_admission_certificate(&certificate)
        .expect("admission arrives after State commit and before worker handoff");
    let effects_before = adapter.effect_count();
    for view in [0, queue_plan_remote_leader_view(&adapter)] {
        assert!(
            adapter
                .reconcile_pending_queue_plan_admissions(view)
                .expect("an obsolete worker defers without attempting queue retirement")
                .is_empty()
        );
        assert_queue_plan_kura_source(&adapter, &certificate);
        assert_eq!(
            adapter.effect_count(),
            effects_before,
            "old workers cannot route the new certificate using their frozen leader"
        );
    }
    assert_eq!(
        adapter
            .state
            .classify_pending_queue_plan_admission(
                &certificate,
                old_worker_height.checked_add(1).expect("successor worker"),
            )
            .expect("successor carrier can consume the exact retained admission")
            .1,
        PendingQueuePlanAdmissionDisposition::EligibleAbsent
    );
}
