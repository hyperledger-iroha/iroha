//! Permission, custody, reservation and timing rejection without partial native writes.
use super::*;

#[test]
fn native_permissions_and_control_cas_are_exact_and_independent() {
    let mut f = fixture();
    let policy = encode(&f.policy).unwrap();
    transact(&mut f.state, 1_000, |tx| {
        for actor in [&f.operator, &f.other] {
            let before = retained(tx);
            assert!(
                instruction(tx, Action::Configure(policy.clone()))
                    .execute(actor, tx)
                    .is_err()
            );
            assert_eq!(retained(tx), before);
        }
        instruction(tx, Action::Configure(policy))
            .execute(&f.manager, tx)
            .unwrap();
        let before = retained(tx);
        let mut stale = instruction(
            tx,
            Action::Revoke(FinalPromotionRevocationV1 {
                signer: true,
                attester: false,
            }),
        );
        stale.expected_control_digest = [0xFF; 32];
        assert!(stale.execute(&f.manager, tx).is_err());
        assert_eq!(retained(tx), before);
        configure_account_control(tx, &f.manager, &f.account_policy);
    });
    enroll(&mut f);
    let request = reserve_request(&f, 31);
    transact(&mut f.state, 2_000, |tx| {
        for actor in [&f.manager, &f.other] {
            let before = retained(tx);
            assert!(
                instruction(tx, Action::Reserve(request))
                    .execute(actor, tx)
                    .is_err()
            );
            assert_eq!(retained(tx), before);
        }
        let absent = AccountId::new(key(99).public_key().clone());
        let permission = CanOperateSorafsFinalPromotion {
            deployment_id: DEPLOYMENT.into(),
        }
        .into();
        tx.world
            .account_permissions
            .insert(absent.clone(), Permissions::from_iter([permission]));
        assert!(
            instruction(tx, Action::Reserve(request))
                .execute(&absent, tx)
                .is_err()
        );
        instruction(tx, Action::Reserve(request))
            .execute(&f.operator, tx)
            .unwrap();
    });
}

#[test]
fn reserve_rejects_drift_invalid_actions_and_competing_operations() {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    let request = reserve_request(&f, 31);
    transact(&mut f.state, 2_000, |tx| {
        for field in 0..7 {
            let mut changed = request;
            match field {
                0 => changed.intent.action = SignerOperationActionV1::Qualify,
                1 => changed.intent.operation_id = [0; 32],
                2 => changed.intent.request_digest = [0; 32],
                3 => changed.intent.previous_audit.sequence = 1,
                4 => changed.intent.previous_audit.digest = [1; 32],
                5 => changed.custody.record_digest = [2; 32],
                _ => changed.custody.control_state_digest = [3; 32],
            }
            let before = retained(tx);
            assert!(
                instruction(tx, Action::Reserve(changed))
                    .execute(&f.operator, tx)
                    .is_err(),
                "field {field}"
            );
            assert_eq!(retained(tx), before);
        }
        let exact = instruction(tx, Action::Reserve(request));
        exact.clone().execute(&f.operator, tx).unwrap();
        let reserved = retained(tx);
        assert!(exact.execute(&f.operator, tx).is_err());
        assert_eq!(
            retained(tx),
            reserved,
            "the consumed direct source cannot be reused for another success"
        );
        let mut changed = request;
        changed.intent.operation_id = [32; 32];
        assert!(
            instruction(tx, Action::Reserve(changed))
                .execute(&f.operator, tx)
                .is_err()
        );
        assert_eq!(retained(tx), reserved);
        let active = read_operation_slot(tx.world(), DEPLOYMENT, [31; 32])
            .unwrap()
            .unwrap()
            .record;
        assert!(
            instruction(tx, Action::Complete(completion(&active)))
                .execute(&f.operator, tx)
                .is_err(),
            "reservation must first commit in an earlier block"
        );
        assert_eq!(retained(tx), reserved);
    });
}

#[test]
fn completion_rejects_substituted_original_coordinates_and_nonowner() {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    let reserved = reserve(&mut f, 31, 2_000);
    let request = completion(&reserved);
    transact(&mut f.state, 3_000, |tx| {
        for field in 0..11 {
            let mut changed = request;
            match field {
                0 => changed.intent.operation_id = [32; 32],
                1 => changed.intent.request_digest[0] ^= 1,
                2 => changed.custody.record_digest = [32; 32],
                3 => changed.custody.control_state_digest = [32; 32],
                4 => changed.reservation.reservation_id = [32; 32],
                5 => changed.reservation.fence += 1,
                6 => changed.reservation.expires_at_unix_ms += 1,
                7 => changed.commitment.audit.sequence += 1,
                8 => changed.commitment.audit.digest = [0; 32],
                9 => changed.commitment.response_digest = [0; 32],
                _ => changed.signatures_digest = [0; 32],
            }
            assert_ne!(
                changed, request,
                "field {field} must change the exact request"
            );
            let before = retained(tx);
            assert!(
                instruction(tx, Action::Complete(changed))
                    .execute(&f.operator, tx)
                    .is_err(),
                "field {field}"
            );
            assert_eq!(retained(tx), before);
        }
        tx.world.account_permissions.insert(
            f.other.clone(),
            Permissions::from_iter([CanOperateSorafsFinalPromotion {
                deployment_id: DEPLOYMENT.into(),
            }
            .into()]),
        );
        let before = retained(tx);
        assert!(
            instruction(tx, Action::Complete(request))
                .execute(&f.other, tx)
                .is_err()
        );
        assert_eq!(retained(tx), before);
        instruction(tx, Action::Complete(request))
            .execute(&f.operator, tx)
            .unwrap();
        let mut next = FinalPromotionReserveV1 {
            intent: request.intent,
            custody: request.custody,
        };
        next.intent.operation_id = [33; 32];
        next.intent.previous_audit = request.commitment.audit;
        let before = retained(tx);
        assert!(
            instruction(tx, Action::Reserve(next))
                .execute(&f.operator, tx)
                .is_err(),
            "new audit predecessor must first commit in a prior block"
        );
        assert_eq!(retained(tx), before);
    });
}

#[test]
fn exclusive_expiry_is_terminal_and_ids_cannot_be_reused() {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    let reserved = reserve(&mut f, 31, 2_000);
    let expire = FinalPromotionExpireV1 {
        operation_id: [31; 32],
        reservation: reserved.reservation,
    };
    transact(&mut f.state, 61_999, |tx| {
        let before = retained(tx);
        assert!(
            instruction(tx, Action::Expire(expire))
                .execute(&f.operator, tx)
                .is_err()
        );
        assert_eq!(retained(tx), before);
    });
    transact(&mut f.state, 62_000, |tx| {
        let before = retained(tx);
        assert!(
            instruction(tx, Action::Complete(completion(&reserved)))
                .execute(&f.operator, tx)
                .is_err()
        );
        assert_eq!(retained(tx), before);
        instruction(tx, Action::Expire(expire))
            .execute(&f.operator, tx)
            .unwrap();
    });
    let current = snapshot(&f, Some([31; 32]));
    assert_eq!(
        current.operation.unwrap().outcome,
        FinalPromotionOperationOutcomeV1::Expired
    );
    assert_eq!(current.operations.audit.sequence, 0);
    let retry = reserve_request(&f, 31);
    transact(&mut f.state, 62_001, |tx| {
        let before = retained(tx);
        assert!(
            instruction(tx, Action::Reserve(retry))
                .execute(&f.operator, tx)
                .is_err()
        );
        assert_eq!(retained(tx), before);
    });
    let next = reserve(&mut f, 32, 63_000);
    assert_eq!(next.reservation.fence, 2);
    assert_eq!(snapshot(&f, None).operations.total_admissions, 2);
}

#[test]
fn reservation_expiry_is_clipped_to_independent_custody_lifetime() {
    let mut f = fixture();
    configure(&mut f);
    let bytes = attest(&f, 1_500, 5_000);
    let account_enrollment = account_enrollment(&f, 5_000);
    transact(&mut f.state, 1_500, |tx| {
        instruction(tx, Action::Enroll(bytes))
            .execute(&f.manager, tx)
            .unwrap();
        account_enrollment.execute(&f.manager, tx).unwrap();
    });
    let reserved = reserve(&mut f, 31, 2_000);
    assert_eq!(reserved.reservation.expires_at_unix_ms, 5_000);
    transact(&mut f.state, 5_000, |tx| {
        let before = retained(tx);
        assert!(
            instruction(tx, Action::Complete(completion(&reserved)))
                .execute(&f.operator, tx)
                .is_err()
        );
        assert_eq!(retained(tx), before);
        instruction(
            tx,
            Action::Expire(FinalPromotionExpireV1 {
                operation_id: [31; 32],
                reservation: reserved.reservation,
            }),
        )
        .execute(&f.operator, tx)
        .unwrap();
    });
    assert_eq!(snapshot(&f, None).operations.active_operation, None);
}

#[test]
fn governance_revocation_atomically_invalidates_the_original_slot() {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    let reserved = reserve(&mut f, 31, 2_000);
    let old_control = snapshot(&f, None);
    transact(&mut f.state, 3_000, |tx| {
        instruction(
            tx,
            Action::Revoke(FinalPromotionRevocationV1 {
                signer: true,
                attester: false,
            }),
        )
        .execute(&f.manager, tx)
        .unwrap()
    });
    let current = snapshot(&f, Some([31; 32]));
    assert!(current.control.signer_revoked);
    assert_eq!(
        current.control_record.enrollment,
        old_control.control_record.enrollment
    );
    assert_eq!(current.operations.audit, old_control.operations.audit);
    assert_eq!(current.operations.active_operation, None);
    let invalidated = current.operation.unwrap();
    assert_eq!(
        invalidated.outcome,
        FinalPromotionOperationOutcomeV1::Invalidated
    );
    assert_eq!(invalidated.reserved, reserved.reserved);
    assert_eq!(invalidated.execution.authority, f.manager);
    transact(&mut f.state, 4_000, |tx| {
        let before = retained(tx);
        let stale = MutateSorafsFinalPromotionAuthority {
            deployment_id: DEPLOYMENT.into(),
            expected_control_revision: old_control.control_record.revision,
            expected_control_digest: old_control.custody_anchor.state_digest,
            action: Action::Complete(completion(&reserved)),
        };
        assert!(stale.execute(&f.operator, tx).is_err());
        assert!(
            instruction(tx, Action::Complete(completion(&reserved)))
                .execute(&f.operator, tx)
                .is_err()
        );
        assert_eq!(retained(tx), before);
    });
}
