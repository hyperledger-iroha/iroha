// Same-scope Musubi replication-order regressions extracted to keep the parent source bounded.
#[test]
fn issue_replication_order_atomically_installs_musubi_archive_binding() {
    let state = make_state();
    let mut block = state.block(block_header());
    let mut stx = block.transaction();
    seed_test_call_hash(&mut stx);
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
