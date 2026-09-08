// Replication admission, completion, and replay preserve canonical pin history.

#[test]
fn issue_replication_order_rejects_duplicates() {
    let state = make_state();
    let mut block = state.block(block_header());
    let mut stx = block.transaction();
    seed_test_call_hash(&mut stx);
    insert_manifest_with_status_at_epoch(
        &mut stx,
        default_digest(),
        default_chunk_digest(),
        None,
        PinStatus::Approved(1),
        1,
    );
    let order_id = ReplicationOrderId::new([0x55; 32]);
    let providers = vec![
        ProviderId::new([0x21; 32]),
        ProviderId::new([0x22; 32]),
        ProviderId::new([0x23; 32]),
    ];
    seed_provider_owners(&mut stx, &providers, &alice());
    let order_struct = replication_order_struct(order_id, default_digest(), &providers, 3);
    let payload = encode_replication_order_for_epoch_window(order_struct, 1, 10);
    let issue = IssueReplicationOrder {
        order_id,
        order_payload: payload.clone(),
        issued_epoch: 1,
        deadline_epoch: 10,
        musubi_archive: None,
    };
    issue
        .execute(&alice(), &mut stx)
        .expect("issue replication order");
    let retained = stx.world.replication_orders.get(&order_id).unwrap().clone();
    let duplicate = IssueReplicationOrder {
        order_id,
        order_payload: payload,
        issued_epoch: 1,
        deadline_epoch: 10,
        musubi_archive: None,
    };
    let err = duplicate
        .execute(&alice(), &mut stx)
        .expect_err("duplicate order must fail");
    assert!(
        matches!(
            &err,
            InstructionExecutionError::InvariantViolation(message)
                if message.as_ref() == format!("replication order {} already exists", hex::encode(order_id.as_bytes()))
        ),
        "unexpected duplicate rejection: {err:?}"
    );
    assert_eq!(stx.world.replication_orders.get(&order_id), Some(&retained));
}

#[test]
fn complete_replication_order_updates_status() {
    let state = make_state_with_completion_anchor();
    let mut block = state.block(block_header());
    let mut stx = block.transaction();
    seed_test_call_hash(&mut stx);
    insert_manifest_with_status_at_epoch(
        &mut stx,
        default_digest(),
        default_chunk_digest(),
        None,
        PinStatus::Approved(1),
        1,
    );
    let order_id = ReplicationOrderId::new([0x77; 32]);
    let providers = vec![
        ProviderId::new([0x31; 32]),
        ProviderId::new([0x32; 32]),
        ProviderId::new([0x33; 32]),
        ProviderId::new([0x34; 32]),
    ];
    seed_provider_owners(&mut stx, &providers, &alice());
    let order_struct = replication_order_struct(order_id, default_digest(), &providers, 3);
    let payload = encode_replication_order_for_epoch_window(order_struct, 1, 10);
    IssueReplicationOrder {
        order_id,
        order_payload: payload,
        issued_epoch: 1,
        deadline_epoch: 10,
        musubi_archive: None,
    }
    .execute(&alice(), &mut stx)
    .expect("issue replication order");
    let complete = completion_instruction(order_id, providers[0], 2, &alice());
    complete
        .execute(&alice(), &mut stx)
        .expect("complete replication order");
    SetProviderIngestCompletionAuthority::new(
        providers[0],
        Some(completion_authority(&alice(), 1)),
        completion_authority(&alice(), 2),
    )
    .execute(&alice(), &mut stx)
    .expect("rotate completion authority after the retained completion");
    completion_instruction(order_id, providers[0], 2, &alice())
        .execute(&alice(), &mut stx)
        .expect("exact retained completion replay remains idempotent after policy rotation");
    let conflicting_replay = completion_instruction(order_id, providers[0], 3, &alice())
        .execute(&alice(), &mut stx)
        .expect_err("completion replay at a different epoch must fail");
    assert!(matches!(
        conflicting_replay,
        InstructionExecutionError::InvariantViolation(message)
            if message.contains("different retained completion context")
    ));
    let partial_record = stx
        .world
        .replication_orders
        .get(&order_id)
        .expect("order stored");
    assert_eq!(partial_record.provider_completions.len(), 1);
    assert_eq!(partial_record.status, ReplicationOrderStatus::Pending);
    completion_instruction(order_id, providers[1], 3, &alice())
        .execute(&alice(), &mut stx)
        .expect("second provider completion");
    assert_eq!(
        stx.world
            .replication_orders
            .get(&order_id)
            .expect("order stored")
            .status,
        ReplicationOrderStatus::Pending
    );
    completion_instruction(order_id, providers[2], 4, &alice())
        .execute(&alice(), &mut stx)
        .expect("target provider completion");
    let surplus_completion = completion_instruction(order_id, providers[3], 5, &alice())
        .execute(&alice(), &mut stx)
        .expect_err("completed redundancy target must reject surplus completion");
    assert!(matches!(
        surplus_completion,
        InstructionExecutionError::InvariantViolation(message)
            if message.contains("reached its redundancy target at epoch 4")
    ));
    let record = stx
        .world
        .replication_orders
        .get(&order_id)
        .expect("order stored");
    assert!(matches!(
        record.status,
        ReplicationOrderStatus::Completed(epoch) if epoch == 4
    ));
    assert_eq!(record.provider_completions.len(), 3);
}

#[test]
fn future_dated_completion_fails_without_mutating_the_order() {
    let state = make_state_with_completion_anchor();
    let mut block = state.block(block_header());
    let mut stx = block.transaction();
    seed_test_call_hash(&mut stx);
    insert_manifest_with_status_at_epoch(
        &mut stx,
        default_digest(),
        default_chunk_digest(),
        None,
        PinStatus::Approved(1),
        1,
    );
    let order_id = ReplicationOrderId::new([0x72; 32]);
    let providers = vec![
        ProviderId::new([0x27; 32]),
        ProviderId::new([0x28; 32]),
        ProviderId::new([0x29; 32]),
    ];
    seed_provider_owners(&mut stx, &providers, &alice());
    let payload = encode_replication_order_for_epoch_window(
        replication_order_struct(order_id, default_digest(), &providers, 3),
        1,
        10,
    );
    IssueReplicationOrder {
        order_id,
        order_payload: payload,
        issued_epoch: 1,
        deadline_epoch: 10,
        musubi_archive: None,
    }
    .execute(&alice(), &mut stx)
    .expect("issue order");
    let retained = stx.world.replication_orders.get(&order_id).unwrap().clone();
    let error = completion_instruction(order_id, providers[0], 6, &alice())
        .execute(&alice(), &mut stx)
        .expect_err("a completion cannot claim a future consensus second");
    assert!(matches!(
        error,
        InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(message)
        ) if message.contains("completion_epoch 6 is later than current consensus epoch 5")
    ));
    let record = stx
        .world
        .replication_orders
        .get(&order_id)
        .expect("order remains");
    assert!(record.provider_completions.is_empty());
    assert_eq!(record.status, ReplicationOrderStatus::Pending);
    assert_eq!(record, &retained);
}
