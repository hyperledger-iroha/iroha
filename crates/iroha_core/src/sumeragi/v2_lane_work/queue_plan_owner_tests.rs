fn queue_plan_owner_from_adapter(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    capacity: usize,
) -> QueuePlanAdmissionOwner {
    let (parent, receipt) = adapter
        .kura
        .v2_finality_artifact_with_receipt(adapter.context.height - 1)
        .expect("read original parent")
        .expect("durable parent");
    let pops = keys
        .iter()
        .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("validator PoP"))
        .collect();
    let verified = VerifiedHeightContext::successor(
        adapter.context.clone(),
        pops,
        &parent,
        &receipt,
        &parent.validator_set_pops,
    )
    .expect("original authenticated context");
    let queue = adapter.lane_drain_queue.clone().unwrap_or_else(|| {
        Arc::new(Queue::test(
            iroha_config::parameters::actual::Queue::default(),
            &iroha_primitives::time::TimeSource::new_system(),
        ))
    });
    let owner = QueuePlanAdmissionOwner::new(
        &verified,
        adapter.local_peer.clone(),
        adapter.voting_enabled,
        Arc::clone(&adapter.state),
        Arc::clone(&adapter.kura),
        queue,
        Arc::clone(&adapter.output_guard),
        NonZeroUsize::new(capacity).expect("positive capacity"),
    )
    .expect("construct real independent admission owner");
    owner
}

use super::super::v2_queue_plan_admission::QueuePlanAdmissionOwner;

pub(in crate::sumeragi) fn queue_plan_owner_fixture(
    capacity: usize,
) -> (
    V2LaneWorkAdapter,
    Vec<KeyPair>,
    QueuePlanAdmissionOwner,
    VerifiedHeightContext,
) {
    let (mut adapter, keys) = native_multilane_signing_fixture();
    let (parent, receipt) = adapter
        .kura
        .v2_finality_artifact_with_receipt(adapter.context.height - 1)
        .expect("read original parent")
        .expect("durable parent");
    let pops = keys
        .iter()
        .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("validator PoP"))
        .collect();
    let verified = VerifiedHeightContext::successor(
        adapter.context.clone(),
        pops,
        &parent,
        &receipt,
        &parent.validator_set_pops,
    )
    .expect("original authenticated context");
    let queue = Arc::new(Queue::test(
        iroha_config::parameters::actual::Queue::default(),
        &iroha_primitives::time::TimeSource::new_system(),
    ));
    adapter
        .install_lane_drain_queue(Arc::clone(&queue))
        .expect("original live queue");
    let owner = QueuePlanAdmissionOwner::new(
        &verified,
        adapter.local_peer.clone(),
        adapter.voting_enabled,
        Arc::clone(&adapter.state),
        Arc::clone(&adapter.kura),
        queue,
        Arc::clone(&adapter.output_guard),
        NonZeroUsize::new(capacity).expect("positive capacity"),
    )
    .expect("construct real independent admission owner");
    (adapter, keys, owner, verified)
}

#[test]
fn queue_plan_owner_retains_exact_outbound_until_original_acknowledgement() {
    let (adapter, keys, mut owner, _) = queue_plan_owner_fixture(2);
    let (_, bytes) = queue_plan_test_certificate(&adapter, &keys, 0x81);
    adapter
        .kura
        .persist_pending_queue_plan_admission_certificate(&bytes)
        .unwrap();
    let view = queue_plan_remote_leader_view(&adapter);
    assert!(owner.refresh(view).unwrap());
    let original = owner.next_effect().expect("original outbound");
    let repeated = owner.next_effect().expect("same retained outbound");
    let V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
        peer,
        view,
        certificate,
    } = &original
    else {
        panic!("wrong admission effect");
    };
    let V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
        certificate: repeated,
        ..
    } = repeated
    else {
        panic!("wrong repeated effect");
    };
    assert!(Arc::ptr_eq(certificate, &repeated));
    assert_eq!(certificate.as_slice(), bytes.as_slice());
    let reconstructed = V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
        peer: peer.clone(),
        view: *view,
        certificate: Arc::new(bytes.clone()),
    };
    assert!(!owner.acknowledge_effect(&reconstructed));
    assert_eq!(owner.effect_count(), 1);
    assert!(owner.acknowledge_effect(&original));
    assert!(!owner.acknowledge_effect(&original));
    assert!(owner.refresh(*view).unwrap());
    assert!(owner.next_effect().is_none());
    assert_queue_plan_kura_source(&adapter, &bytes);
}

#[test]
fn queue_plan_owner_capacity_retry_preserves_transferred_inventory() {
    let (adapter, keys, mut owner, _) = queue_plan_owner_fixture(1);
    for tag in [0x82, 0x83] {
        let (_, bytes) = queue_plan_test_certificate(&adapter, &keys, tag);
        adapter
            .kura
            .persist_pending_queue_plan_admission_certificate(&bytes)
            .unwrap();
    }
    let view = queue_plan_remote_leader_view(&adapter);
    assert!(!owner.refresh(view).unwrap());
    assert!(owner.needs_refresh(view).unwrap());
    let first = owner.next_effect().expect("first bounded occurrence");
    assert!(owner.rotate_next_effect());
    assert!(owner.acknowledge_effect(&first));
    assert!(owner.refresh(view).unwrap());
    let second = owner.next_effect().expect("remaining occurrence");
    assert!(!owner.acknowledge_effect(&first));
    assert!(owner.acknowledge_effect(&second));
    assert!(owner.refresh(view).unwrap());
    assert!(owner.next_effect().is_none());
    let (_, arrived) = queue_plan_test_certificate(&adapter, &keys, 0x84);
    adapter
        .kura
        .persist_pending_queue_plan_admission_certificate(&arrived)
        .unwrap();
    assert!(owner.refresh(view).unwrap());
    let V2LaneWorkEffect::PostQueuePlanAdmissionCertificate { certificate, .. } =
        owner.next_effect().expect("only new inventory")
    else {
        panic!("wrong effect");
    };
    assert_eq!(certificate.as_slice(), arrived.as_slice());
    assert_eq!(owner.effect_count(), 1);
}

#[test]
fn queue_plan_owner_view_change_rejects_old_occurrence_acknowledgement() {
    let (adapter, keys, mut owner, _) = queue_plan_owner_fixture(1);
    let (_, bytes) = queue_plan_test_certificate(&adapter, &keys, 0x85);
    adapter
        .kura
        .persist_pending_queue_plan_admission_certificate(&bytes)
        .unwrap();
    let first_view = queue_plan_remote_leader_view(&adapter);
    let next_view = (first_view + 1..first_view + 1 + 2 * adapter.context.roster.len() as u64)
        .find(|view| {
            adapter.context.leader(*view) != adapter.context.leader(first_view)
                && adapter.context.roster[adapter.context.leader(*view) as usize].validator
                    != adapter.local_peer
        })
        .expect("different remote leader");
    assert!(owner.refresh(first_view).unwrap());
    let old = owner.next_effect().unwrap();
    assert!(owner.needs_refresh(next_view).unwrap());
    assert!(owner.refresh(next_view).unwrap());
    let current = owner.next_effect().unwrap();
    assert!(!owner.acknowledge_effect(&old));
    assert_eq!(owner.effect_count(), 1);
    assert!(owner.acknowledge_effect(&current));
    assert_queue_plan_kura_source(&adapter, &bytes);
}

#[test]
fn queue_plan_owner_leader_uses_original_persistence_and_selection() {
    let (adapter, keys, mut owner, _) = queue_plan_owner_fixture(1);
    let (_, bytes) = queue_plan_test_certificate(&adapter, &keys, 0x86);
    let sender = PeerId::new(KeyPair::random().public_key().clone());
    assert_eq!(
        owner
            .accept_certificate(sender.clone(), Arc::new(bytes.clone()), 0)
            .unwrap(),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        owner
            .accept_certificate(sender.clone(), Arc::new(bytes.clone()), 0)
            .unwrap(),
        V2LaneIngressOutcome::Duplicate
    );
    assert_eq!(
        owner
            .accept_certificate(sender, Arc::new(vec![0xFF; 16]), 0)
            .unwrap(),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(owner.reconcile(0).unwrap(), vec![bytes.clone()]);
    assert!(owner.next_effect().is_none());
    assert_queue_plan_kura_source(&adapter, &bytes);
}

#[test]
fn queue_plan_owner_rejects_foreign_kura_without_replacing_original_sources() {
    let (adapter, _, owner, verified) = queue_plan_owner_fixture(1);
    let (foreign, _) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let result = QueuePlanAdmissionOwner::new(
        &verified,
        adapter.local_peer.clone(),
        true,
        Arc::clone(&adapter.state),
        Arc::clone(&foreign.kura),
        Arc::clone(adapter.lane_drain_queue.as_ref().unwrap()),
        Arc::clone(&adapter.output_guard),
        NonZeroUsize::new(1).unwrap(),
    );
    assert!(matches!(result, Err(V2LaneWorkError::InvalidContext(_))));
    assert_eq!(owner.effect_count(), 0);
    assert!(!adapter.output_guard.restart_required());
}

#[test]
fn queue_plan_owner_shared_fail_stop_guard_fences_output_and_ingress() {
    let (adapter, keys, mut owner, _) = queue_plan_owner_fixture(1);
    let (_, bytes) = queue_plan_test_certificate(&adapter, &keys, 0x87);
    adapter
        .kura
        .persist_pending_queue_plan_admission_certificate(&bytes)
        .unwrap();
    assert!(
        owner
            .refresh(queue_plan_remote_leader_view(&adapter))
            .unwrap()
    );
    let original = owner.next_effect().unwrap();
    adapter.output_guard.close_admission_for_restart();
    assert!(owner.next_effect().is_none());
    assert!(!owner.acknowledge_effect(&original));
    assert_eq!(owner.effect_count(), 1);
    assert!(matches!(
        owner.accept_certificate(adapter.local_peer.clone(), Arc::new(bytes.clone()), 0),
        Err(V2LaneWorkError::RestartRequired)
    ));
    assert_queue_plan_kura_source(&adapter, &bytes);
}

#[test]
fn queue_plan_owner_same_context_rollover_preserves_original_occurrence() {
    let (adapter, keys, mut owner, verified) = queue_plan_owner_fixture(1);
    let (_, bytes) = queue_plan_test_certificate(&adapter, &keys, 0x88);
    adapter
        .kura
        .persist_pending_queue_plan_admission_certificate(&bytes)
        .unwrap();
    let view = queue_plan_remote_leader_view(&adapter);
    assert!(owner.refresh(view).unwrap());
    let original = owner.next_effect().unwrap();
    owner
        .rollover(&verified)
        .expect("exact same authenticated authority");
    assert!(!owner.needs_refresh(view).unwrap());
    assert!(owner.acknowledge_effect(&original));
    assert_queue_plan_kura_source(&adapter, &bytes);
}

pub(in crate::sumeragi) fn queue_plan_owner_services_for_test(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
) -> ProductionV2Services {
    crate::sumeragi::v2_worker::tests::ordinary_dispatch_services_for_test(
        Arc::clone(&adapter.kura),
        adapter.context.clone(),
        keys,
        adapter.local_validator_index().expect("local validator"),
        Arc::clone(&adapter.state),
        Arc::clone(&adapter.output_guard),
        crate::sumeragi::v2_core::EventTag::new(
            adapter.context.height,
            0,
            crate::sumeragi::v2_core::Generation::INITIAL,
        ),
    )
}

#[test]
fn queue_plan_runner_dispatch_preserves_original_certificate_allocation() {
    let (adapter, keys, mut owner, _) = queue_plan_owner_fixture(2);
    let (_, bytes) = queue_plan_test_certificate(&adapter, &keys, 0x90);
    adapter
        .kura
        .persist_pending_queue_plan_admission_certificate(&bytes)
        .unwrap();
    let view = queue_plan_remote_leader_view(&adapter);
    assert!(owner.refresh(view).unwrap());
    let V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
        certificate: original,
        ..
    } = owner.next_effect().unwrap()
    else {
        panic!("expected QueuePlan effect")
    };
    let delivered = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&delivered);
    let mut service = queue_plan_owner_services_for_test(&adapter, &keys);
    service.set_exact_output_admission_hook(move |post, _ticket| {
        if let crate::NetworkMessage::QueuePlanAdmissionCertificate(certificate) = &post.data {
            captured.lock().unwrap().push(Arc::clone(certificate));
        }
        Ok(())
    });
    assert_eq!(
        crate::sumeragi::v2_runner::dispatch_queue_plan_admission_effects(&mut owner, &service, 0,)
            .unwrap(),
        0
    );
    assert_eq!(owner.effect_count(), 1);
    assert!(delivered.lock().unwrap().is_empty());
    assert_eq!(
        crate::sumeragi::v2_runner::dispatch_queue_plan_admission_effects(&mut owner, &service, 1,)
            .unwrap(),
        1
    );
    assert_eq!(owner.effect_count(), 0);
    let delivered = delivered.lock().unwrap();
    assert_eq!(delivered.len(), 1);
    assert!(Arc::ptr_eq(&delivered[0], &original));
    assert_queue_plan_kura_source(&adapter, &bytes);
}

#[test]
fn queue_plan_runner_dispatch_refusal_keeps_original_source() {
    let (adapter, keys, mut owner, _) = queue_plan_owner_fixture(1);
    let (_, bytes) = queue_plan_test_certificate(&adapter, &keys, 0x91);
    adapter
        .kura
        .persist_pending_queue_plan_admission_certificate(&bytes)
        .unwrap();
    let view = queue_plan_remote_leader_view(&adapter);
    assert!(owner.refresh(view).unwrap());
    let original = owner.next_effect().unwrap();
    adapter
        .kura
        .remove_pending_queue_plan_admission_certificate(Hash::new(&bytes))
        .unwrap();
    let service = queue_plan_owner_services_for_test(&adapter, &keys);
    assert!(
        crate::sumeragi::v2_runner::dispatch_queue_plan_admission_effects(&mut owner, &service, 1,)
            .is_err()
    );
    assert_eq!(owner.effect_count(), 1);
    assert!(owner.acknowledge_effect(&original));
}

#[test]
fn queue_plan_runner_relay_uses_global_owner_without_old_lane_admission() {
    let (adapter, keys, mut owner, _) = queue_plan_owner_fixture(1);
    let (_, bytes) = queue_plan_test_certificate(&adapter, &keys, 0x92);
    let sender = PeerId::new(KeyPair::random().public_key().clone());
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    send.try_send(LaneRelayMessage::QueuePlanAdmissionCertificate {
        sender,
        certificate: Arc::new(bytes.clone()),
    })
    .unwrap();
    let effects_before = adapter.effect_count();
    assert!(
        crate::sumeragi::v2_runner::drain_finalized_lane_relay_prefix_for_test(
            &receive,
            &mut owner,
            0,
            1,
        )
        .unwrap()
    );
    assert!(
        !crate::sumeragi::v2_runner::drain_finalized_lane_relay_prefix_for_test(
            &receive,
            &mut owner,
            0,
            1,
        )
        .unwrap()
    );
    assert_eq!(adapter.effect_count(), effects_before);
    assert_queue_plan_kura_source(&adapter, &bytes);
    assert_eq!(owner.reconcile(0).unwrap(), vec![bytes]);
}

#[test]
fn queue_plan_runner_dispatch_rejects_foreign_service_before_source_transfer() {
    let (adapter, keys, mut owner, _) = queue_plan_owner_fixture(1);
    let (_, bytes) = queue_plan_test_certificate(&adapter, &keys, 0x93);
    adapter
        .kura
        .persist_pending_queue_plan_admission_certificate(&bytes)
        .unwrap();
    assert!(
        owner
            .refresh(queue_plan_remote_leader_view(&adapter))
            .unwrap()
    );
    let original = owner.next_effect().unwrap();
    let (foreign, foreign_keys, _, _) = queue_plan_owner_fixture(1);
    let service = queue_plan_owner_services_for_test(&foreign, &foreign_keys);
    assert!(
        crate::sumeragi::v2_runner::dispatch_queue_plan_admission_effects(&mut owner, &service, 1,)
            .is_err()
    );
    assert!(foreign.output_guard.restart_required());
    assert!(!adapter.output_guard.restart_required());
    assert_eq!(owner.effect_count(), 1);
    assert!(owner.acknowledge_effect(&original));
    assert_queue_plan_kura_source(&adapter, &bytes);
}
