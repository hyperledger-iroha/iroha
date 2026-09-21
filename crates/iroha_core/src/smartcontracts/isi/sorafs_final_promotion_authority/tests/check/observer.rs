//! Native observer permission separation and immutable original operation-account binding.
use super::*;

#[test]
fn receipt_check_only_observer_cannot_mutate_custody_or_operations() {
    let mut f = enrolled();
    let request = request(&f, 31);
    let row = reserve_exact(&mut f, request, 2_000);
    let check = check_instruction(&f, request, Subject::BeforeProvider(row.clone()));
    let actions = [
        Action::Configure(encode(&f.policy).unwrap()),
        Action::Enroll(snapshot(&f, None).control_record.enrollment.unwrap()),
        Action::Revoke(FinalPromotionRevocationV1 {
            signer: true,
            attester: false,
        }),
        Action::Reserve(FinalPromotionReserveV1 {
            intent: row.intent,
            custody: row.custody,
        }),
        Action::Complete(completion(&row)),
        Action::Expire(FinalPromotionExpireV1 {
            operation_id: row.intent.operation_id,
            reservation: row.reservation,
        }),
    ];
    transact(&mut f.state, 2_100, |tx| {
        assert_no_writes(tx, &check, &f.observer, true);
        for action in actions {
            let rightful = if matches!(
                &action,
                Action::Configure(_) | Action::Enroll(_) | Action::Revoke(_)
            ) {
                &f.manager
            } else {
                &f.operator
            };
            assert!(eligibility::authorized(
                tx.world(),
                rightful,
                DEPLOYMENT,
                &action
            ));
            assert!(!eligibility::authorized(
                tx.world(),
                &f.observer,
                DEPLOYMENT,
                &action
            ));
            let before = retained(tx);
            assert!(instruction(tx, action).execute(&f.observer, tx).is_err());
            assert_eq!(retained(tx), before);
        }
        assert_no_writes(tx, &check, &f.observer, true);
    });
}

#[test]
fn receipt_check_observer_and_operator_remain_distinct_even_with_both_grants() {
    let mut f = enrolled();
    let request = request(&f, 31);
    let base = check_instruction(
        &f,
        request,
        Subject::Current(snapshot(&f, None).operations.audit),
    );
    transact(&mut f.state, 2_000, |tx| {
        assert_no_writes(tx, &base, &f.observer, true);
        assert_no_writes(tx, &base, &f.operator, false);
        tx.world.account_permissions.insert(
            f.operator.clone(),
            Permissions::from_iter([
                CanCheckSorafsFinalPromotion {
                    deployment_id: DEPLOYMENT.into(),
                }
                .into(),
                CanOperateSorafsFinalPromotion {
                    deployment_id: DEPLOYMENT.into(),
                }
                .into(),
            ]),
        );
        assert_no_writes(tx, &base, &f.operator, false);
        tx.world.account_permissions.insert(
            f.observer.clone(),
            Permissions::from_iter([
                CanCheckSorafsFinalPromotion {
                    deployment_id: DEPLOYMENT.into(),
                }
                .into(),
                CanOperateSorafsFinalPromotion {
                    deployment_id: DEPLOYMENT.into(),
                }
                .into(),
            ]),
        );
        let mut self_check = base.clone();
        payload_mut(&mut self_check).expected_operator = f.observer.clone();
        assert_no_writes(tx, &self_check, &f.observer, false);
        assert_no_writes(tx, &base, &f.observer, true);
    });
}

#[test]
fn receipt_check_requires_both_exact_deployment_grants_and_original_row_operator() {
    let mut f = enrolled();
    let request = request(&f, 31);
    let row = reserve_exact(&mut f, request, 2_000);
    let check = check_instruction(&f, request, Subject::BeforeProvider(row.clone()));
    transact(&mut f.state, 2_100, |tx| {
        assert_no_writes(tx, &check, &f.observer, true);
        let observer_permissions = tx
            .world
            .account_permissions
            .get(&f.observer)
            .unwrap()
            .clone();
        tx.world.account_permissions.insert(
            f.observer.clone(),
            Permissions::from_iter([CanCheckSorafsFinalPromotion {
                deployment_id: "promotion-secondary".into(),
            }
            .into()]),
        );
        assert_no_writes(tx, &check, &f.observer, false);
        tx.world
            .account_permissions
            .insert(f.observer.clone(), observer_permissions);
        let mut changed = check.clone();
        payload_mut(&mut changed).expected_operator = f.other.clone();
        assert_no_writes(tx, &changed, &f.observer, false);
        tx.world.account_permissions.insert(
            f.other.clone(),
            Permissions::from_iter([CanOperateSorafsFinalPromotion {
                deployment_id: DEPLOYMENT.into(),
            }
            .into()]),
        );
        // The other operator now has the exact permission: full-row identity still forbids it.
        for subject in [
            Subject::BeforeProvider(row.clone()),
            Subject::AfterProvider(row.clone()),
            Subject::BeforeCommit(row.clone()),
        ] {
            payload_mut(&mut changed).subject = subject;
            assert_no_writes(tx, &changed, &f.observer, false);
        }
        payload_mut(&mut changed).subject = Subject::Current(row.intent.previous_audit);
        assert_no_writes(tx, &changed, &f.observer, true);
        instruction(tx, Action::Complete(completion(&row)))
            .execute(&f.operator, tx)
            .unwrap();
    });
    let completed = snapshot(&f, Some(request.operation_id)).operation.unwrap();
    transact(&mut f.state, 2_200, |tx| {
        let mut changed = check.clone();
        for subject in [
            Subject::AfterCommit(completed.clone()),
            Subject::BeforeRelease(completed.clone()),
        ] {
            payload_mut(&mut changed).subject = subject;
            payload_mut(&mut changed).expected_operator = f.operator.clone();
            assert_no_writes(tx, &changed, &f.observer, true);
            payload_mut(&mut changed).expected_operator = f.other.clone();
            assert_no_writes(tx, &changed, &f.observer, false);
        }
    });
}
