//! Independent observer identity never replaces the operation owner or its live permission.
use super::*;
use iroha_data_model::{
    isi::{Register, Unregister},
    permission::Permission,
    role::{Role, RoleId},
};
use iroha_executor_data_model::permission::sorafs::CanCheckSorafsFinalPromotion;

#[test]
fn receipt_check_retains_distinct_observer_and_operator_through_real_finality() {
    let mut f = Fixture::new();
    let prepared = f.prepared();
    let observer = AccountId::new(key(3).public_key().clone());
    let operator = AccountId::new(key(2).public_key().clone());
    assert_ne!(observer, operator);
    assert_eq!(prepared.observer(), &observer);
    assert_eq!(prepared.expected_operator(), &operator);
    let signed = f.sign(prepared_instruction(&prepared), 3, NOW);
    let pending = prepared.bind_signed_transaction(signed).unwrap();
    assert_eq!(
        f.commit(NOW, vec![pending.signed_transaction().clone()], true, true),
        [true]
    );
    let verified = pending
        .verify_finalized(FinalPromotionCheckSourceV1::Current, || {
            Ok(interval(NOW, NOW))
        })
        .unwrap();
    assert_eq!(verified.observer(), &observer);
    assert_eq!(verified.expected_operator(), &operator);
    let FinalPromotionAuthorityActionV1::Check(check) = &verified.instruction().action else {
        panic!("Check")
    };
    assert_eq!(check.expected_operator, operator);
    assert_eq!(verified.check_height(), 3);
    assert_eq!(verified.snapshot(), &f.snapshot());
}

#[test]
fn receipt_check_rejects_self_observation_and_substituted_operator_or_observer() {
    let f = Fixture::new();
    let mut expected = f.expected();
    expected.observer = expected.expected_operator.clone();
    assert_eq!(
        begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
            .err(),
        Some(Error::Invalid)
    );
    for operator in [
        AccountId::new(key(1).public_key().clone()),
        AccountId::new(key(3).public_key().clone()),
    ] {
        let prepared = f.prepared();
        let mut changed = prepared.instruction().clone();
        let FinalPromotionAuthorityActionV1::Check(check) = &mut changed.action else {
            panic!("Check")
        };
        check.expected_operator = operator;
        assert_ne!(&changed, prepared.instruction());
        let signed = f.sign(changed.into(), 3, NOW);
        assert_eq!(
            prepared.bind_signed_transaction(signed).err(),
            Some(Error::Transaction)
        );
    }
    for signer in [1, 2, 9] {
        let prepared = f.prepared();
        let signed = f.sign(prepared_instruction(&prepared), signer, NOW);
        assert_eq!(
            prepared.bind_signed_transaction(signed).err(),
            Some(Error::Transaction)
        );
    }
    assert_eq!(f.state.view().height(), 2);
}

#[test]
fn receipt_current_check_requires_the_pinned_operator_registered_and_authorized() {
    for seed in [1, 9] {
        let mut f = Fixture::new();
        let mut expected = f.expected();
        expected.expected_operator = AccountId::new(key(seed).public_key().clone());
        let prepared =
            begin_final_promotion_check_v1(Arc::clone(&f.state), expected, Duration::from_secs(60))
                .unwrap();
        let signed = f.sign(prepared_instruction(&prepared), 3, NOW);
        let pending = prepared.bind_signed_transaction(signed).unwrap();
        assert_eq!(
            f.commit(NOW, vec![pending.signed_transaction().clone()], true, true),
            [false]
        );
        assert_eq!(
            pending
                .verify_finalized(FinalPromotionCheckSourceV1::Current, || panic!(
                    "failed native operator check precedes clock"
                ))
                .err(),
            Some(Error::Execution)
        );
    }
}

#[test]
fn receipt_observer_role_permission_and_account_removal_are_rechecked_at_applied_cut() {
    for change in ["role_permission", "assigned_role", "account"] {
        for same_block in [true, false] {
            let mut f = Fixture::with_observer_permissions([
                iroha_executor_data_model::permission::role::CanManageRoles.into(),
            ]);
            let observer = AccountId::new(key(3).public_key().clone());
            let permission: Permission = CanCheckSorafsFinalPromotion {
                deployment_id: DEPLOYMENT.into(),
            }
            .into();
            let role: RoleId = "receipt_observer".parse().unwrap();
            let register: InstructionBox = Register::role(
                Role::new(role.clone(), observer.clone()).add_permission(permission.clone()),
            )
            .into();
            let remove_direct: InstructionBox =
                Revoke::account_permission(permission.clone(), observer.clone()).into();
            assert_eq!(
                f.commit(
                    NOW,
                    vec![f.sign(register, 3, NOW), f.sign(remove_direct, 3, NOW)],
                    true,
                    true
                ),
                [true, true]
            );
            let pending = f.pending();
            let revoke: InstructionBox = match change {
                "role_permission" => Revoke::role_permission(permission, role).into(),
                "assigned_role" => Revoke::account_role(role, observer).into(),
                "account" => Unregister::account(observer).into(),
                _ => unreachable!(),
            };
            let revoke = f.sign(revoke, 3, NOW + 1);
            let mut entries = vec![pending.signed_transaction().clone()];
            if same_block {
                entries.push(revoke.clone());
            }
            assert!(
                f.commit(NOW + 1, entries, true, true)
                    .into_iter()
                    .all(|success| success)
            );
            if !same_block {
                assert_eq!(f.commit(NOW + 2, vec![revoke], true, true), [true]);
            }
            assert_eq!(
                pending
                    .verify_finalized(FinalPromotionCheckSourceV1::Current, || Ok(interval(
                        NOW + 2,
                        NOW + 2
                    )))
                    .err(),
                Some(Error::Authority),
                "{change}, same_block={same_block}"
            );
        }
    }
}

#[test]
fn receipt_observer_permission_revoked_before_execution_cannot_supply_a_success() {
    let mut f = Fixture::new();
    let pending = f.pending();
    let revoke = Revoke::account_permission(
        CanCheckSorafsFinalPromotion {
            deployment_id: DEPLOYMENT.into(),
        },
        AccountId::new(key(3).public_key().clone()),
    );
    assert_eq!(
        f.commit(
            NOW,
            vec![
                f.sign(revoke.into(), 3, NOW),
                pending.signed_transaction().clone()
            ],
            true,
            true
        ),
        [true, false]
    );
    assert_eq!(
        pending
            .verify_finalized(FinalPromotionCheckSourceV1::Current, || panic!(
                "rejected Check result precedes clock"
            ))
            .err(),
        Some(Error::Execution)
    );
}
