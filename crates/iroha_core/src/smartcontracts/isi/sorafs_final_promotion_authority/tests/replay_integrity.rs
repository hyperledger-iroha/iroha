//! Independent replay-index corruption and staged-publication collision regressions.
use super::*;

#[test]
fn older_operation_replay_indexes_reject_corruption_and_canonical_rollback_without_writes() {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    let first = reserve(&mut f, 31, 2_000);
    let completed = completion(&first);
    transact(&mut f.state, 3_000, |tx| {
        instruction(tx, Action::Complete(completed))
            .execute(&f.operator, tx)
            .expect("complete the original operation");
    });
    let second = reserve(&mut f, 32, 4_000);
    transact(&mut f.state, 5_000, |tx| {
        instruction(tx, Action::Complete(completion(&second)))
            .execute(&f.operator, tx)
            .expect("advance the global audit past the original operation");
    });
    let original = snapshot(&f, Some([31; 32]));
    let retry = reserve_request(&f, 31);
    let old_admission = operation_admission_key(DEPLOYMENT, [31; 32]).unwrap();
    let old_slot = operation_slot_key(DEPLOYMENT, [31; 32]).unwrap();
    let new_admission = operation_admission_key(DEPLOYMENT, [32; 32]).unwrap();
    let new_slot = operation_slot_key(DEPLOYMENT, [32; 32]).unwrap();
    transact(&mut f.state, 6_000, |tx| {
        let first_admission = tx
            .world
            .smart_contract_state
            .get(&old_admission)
            .unwrap()
            .clone();
        let first_slot = tx
            .world
            .smart_contract_state
            .get(&old_slot)
            .unwrap()
            .clone();
        for (key, other_key, substituted_phase) in [
            (old_slot.clone(), new_slot, first_admission.clone()),
            (old_admission.clone(), new_admission, first_slot.clone()),
        ] {
            let canonical = tx.world.smart_contract_state.get(&key).unwrap().clone();
            let other_operation = tx
                .world
                .smart_contract_state
                .get(&other_key)
                .unwrap()
                .clone();
            let mut wrong_digest: OperationIndexV1 = decode(&canonical).unwrap();
            wrong_digest.head.digest[0] ^= 1;
            for (label, corrupt) in [
                ("invalid canonical frame", vec![0]),
                ("another operation's canonical index", other_operation),
                ("substituted digest", encode(&wrong_digest).unwrap()),
                ("this operation's wrong canonical phase", substituted_phase),
            ] {
                tx.world.smart_contract_state.insert(key.clone(), corrupt);
                // The global head belongs to the newer operation and remains independently valid.
                // Rejection must inspect the requested older ID, not merely the global tail.
                assert_eq!(
                    read_operation_head(tx.world(), DEPLOYMENT).unwrap(),
                    original.operations,
                    "{label}"
                );
                assert!(
                    matches!(
                        read_operation_slot(tx.world(), DEPLOYMENT, [31; 32]),
                        Err(Error::CorruptHistory)
                    ),
                    "{label}"
                );
                assert_eq!(
                    read_final_promotion_authority_at_v1(tx, &f.policy.binding, 6, Some([31; 32])),
                    Err(Error::CorruptHistory),
                    "{label}"
                );
                let before = retained(tx);
                for action in [Action::Reserve(retry), Action::Complete(completed)] {
                    assert!(
                        instruction(tx, action).execute(&f.operator, tx).is_err(),
                        "{label}"
                    );
                    assert_eq!(retained(tx), before, "{label}");
                }
                tx.world
                    .smart_contract_state
                    .insert(key.clone(), canonical.clone());
            }
        }
    });
    let after = snapshot(&f, Some([31; 32]));
    assert_eq!(after.control_record, original.control_record);
    assert_eq!(after.operations, original.operations);
    assert_eq!(after.operation, original.operation);
}

#[test]
fn terminal_record_and_height_collisions_never_publish_staged_custody_or_key_changes() {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    reserve(&mut f, 31, 2_000);
    let original = snapshot(&f, Some([31; 32]));
    let mut next_policy = f.policy.clone();
    next_policy.binding.key_revision += 1;
    next_policy.binding.public_key = key(8).public_key().clone();
    next_policy.binding.key_handle = "pkcs11:promotion/key-2".into();
    let policy_bytes = encode(&next_policy).unwrap();
    transact(&mut f.state, 3_000, |tx| {
        let current = read_control::<ReceiptPurpose>(tx.world(), DEPLOYMENT)
            .unwrap()
            .unwrap();
        let head = read_operation_head(tx.world(), DEPLOYMENT).unwrap();
        let mutation = instruction(tx, Action::Configure(policy_bytes));
        let request = final_promotion_authority_request_digest_v1(&mutation, &f.manager).unwrap();
        let next_height = tx._curr_block.height().get();
        let new_key =
            key_path::<ReceiptPurpose>(DEPLOYMENT, true, &next_policy.binding.public_key).unwrap();
        let control_head = control_head_key::<ReceiptPurpose>(DEPLOYMENT).unwrap();
        for collision in [
            operation_record_key(DEPLOYMENT, head.revision + 1).unwrap(),
            operation_height_key(DEPLOYMENT, next_height, 0).unwrap(),
        ] {
            assert!(tx.world.smart_contract_state.get(&collision).is_none());
            let pristine = retained(tx);
            // Exercise the late staging boundary directly with an already claimed terminal key.
            // Control/key writes are prepared first, but a failed batch must remain unpublished.
            let mut writes = vec![(collision.clone(), vec![0])];
            assert_eq!(
                control::prepare(
                    &mutation,
                    &f.manager,
                    tx,
                    Some(&current),
                    head,
                    request,
                    &mut writes
                ),
                Err(Error::CorruptHistory)
            );
            assert!(writes.iter().any(|(key, _)| key == &control_head));
            assert!(writes.iter().any(|(key, _)| key == &new_key));
            assert_eq!(retained(tx), pristine);
            // A retained conflicting terminal row/index is also rejected by the complete native
            // entry point. Its early history check must not publish any custody or operation data.
            tx.world
                .smart_contract_state
                .insert(collision.clone(), vec![0]);
            let before = retained(tx);
            assert!(mutation.clone().execute(&f.manager, tx).is_err());
            assert_eq!(retained(tx), before);
            tx.world.smart_contract_state.remove(collision);
            assert_eq!(retained(tx), pristine);
        }
    });
    let after = snapshot(&f, Some([31; 32]));
    assert_eq!(after.control_record, original.control_record);
    assert_eq!(after.operations, original.operations);
    assert_eq!(after.operation, original.operation);
    // The same reviewed policy is otherwise valid and really invalidates the pending operation.
    let bytes = encode(&next_policy).unwrap();
    transact(&mut f.state, 4_000, |tx| {
        instruction(tx, Action::Configure(bytes))
            .execute(&f.manager, tx)
            .expect("collision-free rotation publishes one atomic control/terminal batch");
    });
    f.policy = next_policy;
    let rotated = snapshot(&f, Some([31; 32]));
    assert_eq!(rotated.control.policy, f.policy);
    assert_eq!(rotated.operations.active_operation, None);
    assert_eq!(rotated.operations.audit, original.operations.audit);
    assert_eq!(
        rotated.operation.unwrap().outcome,
        FinalPromotionOperationOutcomeV1::Invalidated
    );
}
