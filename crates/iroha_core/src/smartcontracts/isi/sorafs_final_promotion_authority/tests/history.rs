//! Immutable history, permanent identity indexes and transactional control rollback.
use super::*;

#[test]
fn orphan_key_indexes_forbid_reinitializing_deployment_custody() {
    for keep_signer in [false, true] {
        let mut f = fixture();
        configure(&mut f);
        let mut next = f.policy.clone();
        next.binding.public_key = key(8).public_key().clone();
        next.attester_public_key = key(9).public_key().clone();
        transact(&mut f.state, 2_000, |tx| {
            let old = read_control::<ReceiptPurpose>(tx.world(), DEPLOYMENT)
                .unwrap()
                .unwrap();
            for key in [
                control_head_key::<ReceiptPurpose>(DEPLOYMENT).unwrap(),
                control_record_key::<ReceiptPurpose>(DEPLOYMENT, 1).unwrap(),
                control_height_key::<ReceiptPurpose>(
                    DEPLOYMENT,
                    old.index.height,
                    old.index.ordinal,
                )
                .unwrap(),
                key_path::<ReceiptPurpose>(
                    DEPLOYMENT,
                    !keep_signer,
                    if keep_signer {
                        &f.policy.attester_public_key
                    } else {
                        &f.policy.binding.public_key
                    },
                )
                .unwrap(),
            ] {
                tx.world.smart_contract_state.remove(key);
            }
            assert!(read_control::<ReceiptPurpose>(tx.world(), DEPLOYMENT).is_err());
            let before = retained(tx);
            let fresh = MutateSorafsFinalPromotionAuthority {
                deployment_id: DEPLOYMENT.into(),
                expected_control_revision: 0,
                expected_control_digest: [0; 32],
                action: Action::Configure(encode(&next).unwrap()),
            };
            assert!(fresh.execute(&f.manager, tx).is_err());
            assert_eq!(retained(tx), before);
        });
    }
}

#[test]
fn completed_retry_after_later_audit_cannot_rewind_the_journal() {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    let first = reserve(&mut f, 31, 2_000);
    let first_request = completion(&first);
    transact(&mut f.state, 3_000, |tx| {
        instruction(tx, Action::Complete(first_request))
            .execute(&f.operator, tx)
            .unwrap()
    });
    let second = reserve(&mut f, 32, 4_000);
    let second_request = completion(&second);
    transact(&mut f.state, 5_000, |tx| {
        instruction(tx, Action::Complete(second_request))
            .execute(&f.operator, tx)
            .unwrap()
    });
    let latest = snapshot(&f, None).operations;
    assert_eq!(latest.audit.sequence, 2);
    transact(&mut f.state, 6_000, |tx| {
        let before = retained(tx);
        instruction(tx, Action::Complete(first_request))
            .execute(&f.operator, tx)
            .unwrap();
        assert_eq!(retained(tx), before);
    });
    assert_eq!(snapshot(&f, None).operations, latest);
    let historic =
        read_final_promotion_authority_at_v1(&f.state.view(), &f.policy.binding, 4, Some([32; 32]))
            .unwrap()
            .unwrap();
    assert_eq!(historic.operations.audit.sequence, 1);
    assert_eq!(historic.operation, None);
    let first = snapshot(&f, Some([31; 32])).operation.unwrap();
    assert_eq!(
        first.outcome,
        FinalPromotionOperationOutcomeV1::Completed(
            iroha_data_model::sorafs::final_promotion_authority::FinalPromotionCompletedV1 {
                commitment: first_request.commitment,
                signatures_digest: first_request.signatures_digest
            }
        )
    );
}

#[test]
fn rotation_preserves_fences_and_spent_ids_and_rejects_key_reuse() {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    let original_policy = f.policy.clone();
    let reserved = reserve(&mut f, 31, 2_000);
    f.policy.binding.public_key = key(8).public_key().clone();
    f.policy.binding.key_revision = 2;
    f.policy.binding.key_handle = "pkcs11:promotion/key-2".into();
    let bytes = encode(&f.policy).unwrap();
    transact(&mut f.state, 3_000, |tx| {
        instruction(tx, Action::Configure(bytes))
            .execute(&f.manager, tx)
            .unwrap()
    });
    let rotated = snapshot(&f, Some([31; 32]));
    assert!(rotated.control_record.enrollment.is_none());
    assert!(rotated.control.active_head.is_none());
    assert_eq!(rotated.control.next_sequence, 2);
    assert_eq!(rotated.operations.fence, 1);
    assert_eq!(
        rotated.operation.unwrap().outcome,
        FinalPromotionOperationOutcomeV1::Invalidated
    );
    let bytes = attest(&f, 3_500, 100_000);
    transact(&mut f.state, 3_500, |tx| {
        instruction(tx, Action::Enroll(bytes))
            .execute(&f.manager, tx)
            .unwrap()
    });
    let next = reserve(&mut f, 32, 4_000);
    assert_eq!(next.reservation.fence, reserved.reservation.fence + 1);
    let mut reused = f.policy.clone();
    reused.binding.public_key = original_policy.binding.public_key;
    reused.binding.key_revision = 3;
    let bytes = encode(&reused).unwrap();
    transact(&mut f.state, 4_500, |tx| {
        let before = retained(tx);
        assert!(
            instruction(tx, Action::Configure(bytes))
                .execute(&f.manager, tx)
                .is_err()
        );
        assert_eq!(retained(tx), before);
    });
}

#[test]
fn failed_slot_invalidation_publishes_no_control_or_key_changes() {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    reserve(&mut f, 31, 2_000);
    let mut policy = f.policy.clone();
    policy.binding.key_revision = 2;
    policy.binding.public_key = key(8).public_key().clone();
    let bytes = encode(&policy).unwrap();
    transact(&mut f.state, 3_000, |tx| {
        let key = operation_admission_key(DEPLOYMENT, [31; 32]).unwrap();
        let original = tx.world.smart_contract_state.get(&key).unwrap().clone();
        tx.world.smart_contract_state.remove(key.clone());
        // Head checks succeed; immutable control/key writes are prepared before invalidation
        // discovers the missing first-use index. None of those prepared writes may escape.
        assert!(read_operation_head(tx.world(), DEPLOYMENT).is_ok());
        let before = retained(tx);
        assert!(
            instruction(tx, Action::Configure(bytes))
                .execute(&f.manager, tx)
                .is_err()
        );
        assert_eq!(retained(tx), before);
        tx.world.smart_contract_state.insert(key, original);
    });
    assert_eq!(snapshot(&f, None).control_record.revision, 2);
}

#[test]
fn missing_operation_indexes_and_substituted_history_fail_closed() {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    let reserved = reserve(&mut f, 31, 2_000);
    transact(&mut f.state, 3_000, |tx| {
        instruction(tx, Action::Complete(completion(&reserved)))
            .execute(&f.operator, tx)
            .unwrap()
    });
    let retry = reserve_request(&f, 31);
    transact(&mut f.state, 4_000, |tx| {
        for key in [
            operation_admission_key(DEPLOYMENT, [31; 32]).unwrap(),
            operation_slot_key(DEPLOYMENT, [31; 32]).unwrap(),
        ] {
            let bytes = tx.world.smart_contract_state.get(&key).unwrap().clone();
            tx.world.smart_contract_state.remove(key.clone());
            assert!(read_operation_slot(tx.world(), DEPLOYMENT, [31; 32]).is_err());
            let before = retained(tx);
            assert!(
                instruction(tx, Action::Reserve(retry))
                    .execute(&f.operator, tx)
                    .is_err()
            );
            assert_eq!(retained(tx), before);
            tx.world.smart_contract_state.insert(key, bytes);
        }
        let head_key = operation_head_key(DEPLOYMENT).unwrap();
        let latest = tx
            .world
            .smart_contract_state
            .get(&head_key)
            .unwrap()
            .clone();
        let first = read_operation_record(tx.world(), DEPLOYMENT, 1).unwrap();
        tx.world
            .smart_contract_state
            .insert(head_key.clone(), encode(&first.index.head).unwrap());
        assert!(read_operation_head(tx.world(), DEPLOYMENT).is_err());
        tx.world.smart_contract_state.insert(head_key, latest);
        let key = operation_record_key(DEPLOYMENT, 2).unwrap();
        let bytes = tx.world.smart_contract_state.get(&key).unwrap().clone();
        let mut changed: FinalPromotionOperationRecordV1 = decode(&bytes).unwrap();
        changed.reserved.authority = f.manager.clone();
        tx.world
            .smart_contract_state
            .insert(key.clone(), encode(&changed).unwrap());
        assert!(read_operation_head(tx.world(), DEPLOYMENT).is_err());
        tx.world.smart_contract_state.insert(key, bytes);
    });
}

#[test]
fn native_reader_rejects_scope_height_and_retained_enrollment_substitution() {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    let before = snapshot(&f, None);
    for height in [0, 3] {
        assert_eq!(
            read_final_promotion_authority_at_v1(&f.state.view(), &f.policy.binding, height, None),
            Err(Error::HeightUnavailable)
        );
    }
    for field in 0..5 {
        let mut binding = f.policy.binding.clone();
        match field {
            0 => binding.chain_id = "other-chain".into(),
            1 => binding.network_id = [91; 32],
            2 => binding.role = SignerRoleV1::ReleaseManifest,
            3 => binding.public_key = key(8).public_key().clone(),
            _ => {
                binding.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
                    deployment_id: "promotion-secondary".into(),
                }
            }
        }
        let result = read_final_promotion_authority_at_v1(&f.state.view(), &binding, 2, None);
        if field == 4 {
            assert_eq!(result.unwrap(), None);
        } else {
            assert!(result.is_err());
        }
    }
    assert_eq!(snapshot(&f, None), before);
    transact(&mut f.state, 3_000, |tx| {
        let current = read_control::<ReceiptPurpose>(tx.world(), DEPLOYMENT)
            .unwrap()
            .unwrap();
        let key = control_record_key::<ReceiptPurpose>(DEPLOYMENT, current.index.revision).unwrap();
        let original = tx.world.smart_contract_state.get(&key).unwrap().clone();
        let mut changed = current.record;
        let mut enrollment: SignerCustodyRecordV1 =
            decode(changed.enrollment.as_ref().unwrap()).unwrap();
        enrollment.attestation[0] ^= 1;
        changed.enrollment = Some(encode(&enrollment).unwrap());
        tx.world
            .smart_contract_state
            .insert(key.clone(), encode(&changed).unwrap());
        assert!(read_control::<ReceiptPurpose>(tx.world(), DEPLOYMENT).is_err());
        tx.world.smart_contract_state.insert(key, original);
    });
}

#[test]
fn policy_generations_and_decoding_are_strict_before_publication() {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    transact(&mut f.state, 2_000, |tx| {
        for field in 0..6 {
            let mut policy = f.policy.clone();
            match field {
                0 => policy.binding.public_key = key(8).public_key().clone(),
                1 => policy.binding.key_revision += 1,
                2 => policy.binding.runtime_handle = "hsm://promotion/secondary".into(),
                3 => policy.attester_public_key = key(8).public_key().clone(),
                4 => policy.attester_authority.policy_digest = [9; 32],
                _ => policy.binding.role = SignerRoleV1::ReleaseManifest,
            }
            let before = retained(tx);
            assert!(
                instruction(tx, Action::Configure(encode(&policy).unwrap()))
                    .execute(&f.manager, tx)
                    .is_err(),
                "field {field}"
            );
            assert_eq!(retained(tx), before);
        }
        let before = retained(tx);
        let mut oversized = instruction(tx, Action::Configure(encode(&f.policy).unwrap()));
        oversized.deployment_id =
            "p".repeat(sorafs_manifest::signer::protocol::SIGNER_MAX_ID_BYTES_V1 + 1);
        assert!(oversized.execute(&f.manager, tx).is_err());
        assert_eq!(retained(tx), before);
        for bytes in [Vec::new(), vec![0; iroha_data_model::sorafs::final_promotion_authority::FINAL_PROMOTION_MAX_RECORD_BYTES_V1 + 1]] {
            assert!(instruction(tx, Action::Configure(bytes)).execute(&f.manager, tx).is_err());
            assert_eq!(retained(tx), before);
        }
    });
}
