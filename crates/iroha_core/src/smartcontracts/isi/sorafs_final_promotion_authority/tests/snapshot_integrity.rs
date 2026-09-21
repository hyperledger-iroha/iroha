//! Cross-journal replay rejection while preserving selected historical operation observations.
use super::*;

#[test]
fn canonical_operation_prefix_cannot_be_paired_with_a_newer_control() {
    for transition in ["configure", "enroll", "revoke"] {
        let mut f = fixture();
        configure(&mut f);
        enroll(&mut f);
        let reserved = reserve(&mut f, 31, 2_000);
        let historical = snapshot(&f, Some([31; 32]));
        let old_binding = f.policy.binding.clone();
        let mut next_policy = f.policy.clone();
        let action = match transition {
            "configure" => {
                next_policy.binding.key_revision += 1;
                next_policy.binding.public_key = key(8).public_key().clone();
                next_policy.binding.key_handle = "pkcs11:promotion/key-2".into();
                Action::Configure(encode(&next_policy).unwrap())
            }
            "enroll" => Action::Enroll(attest(&f, 3_000, 100_000)),
            _ => Action::Revoke(FinalPromotionRevocationV1 {
                signer: true,
                attester: false,
            }),
        };
        transact(&mut f.state, 3_000, |tx| {
            instruction(tx, action)
                .execute(&f.manager, tx)
                .expect("native control transition atomically invalidates the live slot");
        });
        f.policy = next_policy;
        let latest = snapshot(&f, Some([31; 32]));
        assert_eq!(latest.operations.active_operation, None, "{transition}");
        assert_eq!(
            latest.operation.as_ref().unwrap().outcome,
            FinalPromotionOperationOutcomeV1::Invalidated,
            "{transition}"
        );
        assert_eq!(
            read_final_promotion_authority_at_v1(&f.state.view(), &old_binding, 3, Some([31; 32]))
                .unwrap(),
            Some(historical.clone()),
            "a later terminal row must preserve the earlier live observation: {transition}"
        );
        transact(&mut f.state, 4_000, |tx| {
            // Only direct fixture corruption can replay one journal independently: native
            // publication writes the control and terminal transition in the same transaction.
            let admission_bytes = tx
                .world
                .smart_contract_state
                .get(&operation_admission_key(DEPLOYMENT, [31; 32]).unwrap())
                .unwrap()
                .clone();
            let admission: OperationIndexV1 = decode(&admission_bytes).unwrap();
            let terminal = latest.operation.as_ref().unwrap();
            tx.world
                .smart_contract_state
                .remove(operation_record_key(DEPLOYMENT, terminal.revision).unwrap());
            tx.world.smart_contract_state.remove(
                operation_height_key(
                    DEPLOYMENT,
                    terminal.execution.height,
                    terminal.execution.ordinal,
                )
                .unwrap(),
            );
            tx.world.smart_contract_state.insert(
                operation_head_key(DEPLOYMENT).unwrap(),
                encode(&admission.head).unwrap(),
            );
            tx.world.smart_contract_state.insert(
                operation_slot_key(DEPLOYMENT, [31; 32]).unwrap(),
                admission_bytes,
            );
            // Every remaining row/index is original canonical data, and each journal still
            // passes its independent validation. The selected cross-journal view must reject.
            assert_eq!(
                read_operation_head(tx.world(), DEPLOYMENT).unwrap(),
                historical.operations
            );
            assert_eq!(
                read_control::<ReceiptPurpose>(tx.world(), DEPLOYMENT)
                    .unwrap()
                    .unwrap()
                    .record,
                latest.control_record
            );
            let before = retained(tx);
            for requested in [None, Some([31; 32]), Some([32; 32])] {
                assert_eq!(
                    read_final_promotion_authority_at_v1(tx, &f.policy.binding, 4, requested),
                    Err(Error::CorruptHistory),
                    "{transition}"
                );
            }
            assert_eq!(
                read_final_promotion_authority_at_v1(tx, &old_binding, 3, Some([31; 32])).unwrap(),
                Some(historical.clone()),
                "the original historical pair remains valid: {transition}"
            );
            assert_eq!(retained(tx), before);
            // Even before the reader hardening, native completion rejected this stale custody;
            // this regression does not claim that ordinary transactions can create the replay.
            assert!(
                instruction(tx, Action::Complete(completion(&reserved)))
                    .execute(&f.operator, tx)
                    .is_err()
            );
            assert_eq!(retained(tx), before);
        });
    }
}
