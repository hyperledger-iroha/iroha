// Same-scope Musubi replication-order regressions extracted to keep the parent source bounded.
#[test]
fn issue_replication_order_atomically_installs_musubi_archive_binding() {
    let state = make_state();
    let mut block = state.block(block_header());
    let mut stx = block.transaction_for_fastpq_testing(Hash::prehashed([0x51; Hash::LENGTH]));
    let pin = registry_grade_musubi_pin();
    let archive = musubi_archive_for_pin(&pin, 0x61);
    let archive_id = archive.archive_id;
    insert_pin_record_with_accounting(&mut stx, pin);
    stx.world
        .musubi_archives
        .insert(archive_id, archive.clone());
    let order_id = ReplicationOrderId::new([0x62; 32]);
    let providers = vec![
        ProviderId::new([0x63; 32]),
        ProviderId::new([0x64; 32]),
        ProviderId::new([0x65; 32]),
    ];
    seed_provider_owners(&mut stx, &providers, &alice());
    let order = replication_order_struct(order_id, default_digest(), &providers, 3);
    IssueReplicationOrder::new(
        order_id,
        encode_replication_order_for_epoch_window(order, 12, 32),
        12,
        32,
    )
    .for_musubi_archive(archive_id)
    .execute(&alice(), &mut stx)
    .expect("issue archive-bound replication order");
    assert!(stx.world.replication_orders.get(&order_id).is_some());
    let reference = stx
        .world
        .musubi_locations_by_replication_order
        .get(&order_id)
        .expect("pre-location archive binding stored");
    reference.validate().expect("stored binding validates");
    assert_eq!(reference.binding.archive_id, archive_id);
    assert_eq!(reference.binding.commitment, archive.commitment);
    assert_eq!(
        reference.lifecycle,
        MusubiReplicationOrderLocationLifecycleV1::PreLocation
    );
    let stored_order = stx
        .world
        .replication_orders
        .get(&order_id)
        .expect("archive-bound order stored");
    assert!(stored_order.provider_completions.is_empty());
    assert_eq!(stored_order.status, ReplicationOrderStatus::Pending);
}

#[test]
fn native_purpose_issuance_uses_the_paid_pin_schedule_and_requires_its_own_binding() {
    use crate::query::native_musubi_storage::{
        bound_purpose_order, native_musubi_replication_order_id_v1, plan_missing_purpose,
    };

    let state = make_state();
    let mut block = state.block(block_header());
    let mut stx = block.transaction_for_fastpq_testing(Hash::prehashed([0x51; Hash::LENGTH]));
    let pin = registry_grade_musubi_pin();
    let archive = musubi_archive_for_pin(&pin, 0x61);
    insert_pin_record_with_accounting(&mut stx, pin.clone());
    stx.world
        .musubi_archives
        .insert(archive.archive_id, archive.clone());
    let providers = [
        ProviderId::new([0x63; 32]),
        ProviderId::new([0x64; 32]),
        ProviderId::new([0x65; 32]),
    ];
    seed_provider_owners(&mut stx, &providers, &alice());
    let automatic = build_auto_replication_order(&pin, &alice(), 1, &providers).unwrap();
    let auto_id = automatic.order_id;
    assert!(auto_id.is_auto());
    stx.world
        .replication_orders
        .insert(auto_id, automatic.clone());
    let before = active_automatic_allocations(&stx).unwrap();

    assert!(
        bound_purpose_order(&stx.world, &archive, &pin)
            .unwrap()
            .is_none()
    );
    remove_permission(&mut stx, "CanIssueSorafsReplicationOrder");
    assert!(plan_missing_purpose(&stx.world, &archive, &pin, providers[0], 2, 2).is_err());
    grant_permission(&mut stx, "CanIssueSorafsReplicationOrder");
    let plan = plan_missing_purpose(&stx.world, &archive, &pin, providers[0], 2, 2).unwrap();
    let purpose = native_musubi_replication_order_id_v1(archive.archive_id, pin.digest);
    assert_eq!(plan.order_id, purpose);
    assert!(!purpose.is_auto());
    assert_ne!(
        purpose,
        native_musubi_replication_order_id_v1(
            musubi_archive_for_pin(&pin, 0x71).archive_id,
            pin.digest
        )
    );
    assert_ne!(
        purpose,
        native_musubi_replication_order_id_v1(archive.archive_id, second_digest())
    );
    assert_eq!(plan.musubi_archive, Some(archive.archive_id));
    assert_eq!(plan.issued_epoch, automatic.issued_epoch);
    assert_eq!(plan.deadline_epoch, automatic.deadline_epoch);
    assert!(
        plan_missing_purpose(
            &stx.world,
            &archive,
            &pin,
            ProviderId::new([0x66; 32]),
            2,
            2
        )
        .is_err()
    );
    assert!(
        plan_missing_purpose(
            &stx.world,
            &archive,
            &pin,
            providers[0],
            automatic.deadline_epoch + 1,
            2
        )
        .is_err()
    );

    plan.execute(&alice(), &mut stx).unwrap();
    remove_permission(&mut stx, "CanIssueSorafsReplicationOrder");
    let (issued, lifecycle) = bound_purpose_order(&stx.world, &archive, &pin)
        .unwrap()
        .unwrap();
    assert_eq!(issued.order_id, purpose);
    assert_eq!(
        *lifecycle,
        MusubiReplicationOrderLocationLifecycleV1::PreLocation
    );
    let decoded: ReplicationOrderV1 = norito::decode_canonical(&issued.canonical_order).unwrap();
    let mut expected: ReplicationOrderV1 =
        norito::decode_canonical(&automatic.canonical_order).unwrap();
    expected.order_id = *purpose.as_bytes();
    assert_eq!(decoded, expected);
    assert_eq!(active_automatic_allocations(&stx).unwrap(), before);

    let original = issued.clone();
    let mut substituted = original.clone();
    substituted.issued_by = AccountId::new(
        KeyPair::from_seed(vec![0x68; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    stx.world.replication_orders.insert(purpose, substituted);
    assert!(bound_purpose_order(&stx.world, &archive, &pin).is_err());
    stx.world.replication_orders.remove(purpose);
    assert!(bound_purpose_order(&stx.world, &archive, &pin).is_err());
    stx.world.replication_orders.insert(purpose, original);
    stx.world
        .musubi_locations_by_replication_order
        .remove(purpose);
    assert!(bound_purpose_order(&stx.world, &archive, &pin).is_err());
}
