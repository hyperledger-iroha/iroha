//! No-write native Check, account separation and live permission revocation.
use super::*;
use iroha_data_model::{
    isi::{Register, Revoke, Unregister},
    role::{Role, RoleId},
};

#[test]
fn account_check_repeats_without_history_or_key_index_writes() {
    let mut f = enrolled();
    let base = check_instruction(&f);
    let before = snapshot(&f);
    assert!(
        applied(&f, &base, 1_700).is_err(),
        "applied cut must follow the floor"
    );
    transact(&mut f.state, 1_700, |tx| {
        for _ in 0..3 {
            assert_no_writes(tx, &base, &f.observer, true);
        }
        for account in [&f.manager, &f.target, &f.other] {
            assert_no_writes(tx, &base, account, false);
        }
        let mut fresh = base.clone();
        payload_mut(&mut fresh).challenge = [92; 32];
        assert_no_writes(tx, &fresh, &f.observer, true);
    });
    let after = applied(&f, &base, 1_700).unwrap();
    assert_eq!(after.control_record, before.control_record);
    assert_eq!(after.control, before.control);
    assert_eq!(
        after.custody_anchor.state_digest,
        before.custody_anchor.state_digest
    );
    let prefix = history::scope::<AccountPurpose>(DEPLOYMENT);
    assert!(
        f.state
            .view()
            .world()
            .smart_contract_state()
            .iter()
            .filter(|(key, _)| key.as_ref().starts_with(&prefix))
            .all(|(key, _)| !key.as_ref().contains("operation"))
    );
}

#[test]
fn account_check_rejects_missing_or_substituted_independent_coordinates() {
    let mut f = enrolled();
    let base = check_instruction(&f);
    transact(&mut f.state, 2_000, |tx| {
        assert_no_writes(tx, &base, &f.observer, true);
        for field in 0..10 {
            let mut changed = base.clone();
            match field {
                0 => payload_mut(&mut changed).challenge = [0; 32],
                1 => payload_mut(&mut changed).network_id = [0; 32],
                2 => payload_mut(&mut changed).network_id[0] ^= 1,
                3 => payload_mut(&mut changed).minimum_height = 0,
                4 => payload_mut(&mut changed).minimum_height += 1,
                5 => payload_mut(&mut changed).minimum_block_hash[0] ^= 1,
                6 => payload_mut(&mut changed).transaction_payload_digest = [0; 32],
                7 => payload_mut(&mut changed).expected_account = f.other.clone(),
                8 => changed.expected_control_revision += 1,
                9 => changed.expected_control_digest[0] ^= 1,
                _ => unreachable!(),
            }
            assert_no_writes(tx, &changed, &f.observer, false);
        }
    });
    assert!(applied(&f, &base, 2_000).is_ok());
}

#[test]
fn account_check_requires_observer_inequality_even_with_both_exact_permissions() {
    let mut f = enrolled();
    let base = check_instruction(&f);
    transact(&mut f.state, 2_000, |tx| {
        tx.world.account_permissions.insert(
            f.target.clone(),
            Permissions::from_iter([
                CanCheckSorafsFinalPromotionAccountCustody {
                    deployment_id: DEPLOYMENT.into(),
                }
                .into(),
                CanOperateSorafsFinalPromotion {
                    deployment_id: DEPLOYMENT.into(),
                }
                .into(),
            ]),
        );
        assert_no_writes(tx, &base, &f.target, false);
        assert_no_writes(tx, &base, &f.observer, true);
    });
    assert_eq!(
        check::check_applied_snapshot_v1(
            &f.state.view(),
            &base,
            &f.policy.binding,
            &f.target,
            2_000
        ),
        Err(HistoryError::BindingMismatch)
    );
}

#[test]
fn account_check_rechecks_both_account_roles_and_permissions_after_same_block_changes() {
    for target in [false, true] {
        for change in ["direct", "account", "role_permission", "assigned_role"] {
            let mut f = enrolled();
            let account = if target {
                f.target.clone()
            } else {
                f.observer.clone()
            };
            let permission: Permission = if target {
                CanOperateSorafsFinalPromotion {
                    deployment_id: DEPLOYMENT.into(),
                }
                .into()
            } else {
                CanCheckSorafsFinalPromotionAccountCustody {
                    deployment_id: DEPLOYMENT.into(),
                }
                .into()
            };
            let role: RoleId = "account_custody_check".parse().unwrap();
            if matches!(change, "role_permission" | "assigned_role") {
                transact(&mut f.state, 1_700, |tx| {
                    Register::role(
                        Role::new(role.clone(), account.clone()).add_permission(permission.clone()),
                    )
                    .execute(&account, tx)
                    .unwrap();
                    Revoke::account_permission(permission.clone(), account.clone())
                        .execute(&account, tx)
                        .unwrap();
                });
            }
            let check = check_instruction(&f);
            transact(&mut f.state, 2_000, |tx| {
                assert_no_writes(tx, &check, &f.observer, true);
                match change {
                    "direct" => Revoke::account_permission(permission.clone(), account.clone())
                        .execute(&account, tx)
                        .unwrap(),
                    "account" => Unregister::account(account.clone())
                        .execute(&account, tx)
                        .unwrap(),
                    "role_permission" => Revoke::role_permission(permission.clone(), role.clone())
                        .execute(&account, tx)
                        .unwrap(),
                    "assigned_role" => Revoke::account_role(role.clone(), account.clone())
                        .execute(&account, tx)
                        .unwrap(),
                    _ => unreachable!(),
                }
                assert_no_writes(tx, &check, &f.observer, false);
            });
            assert_eq!(
                applied(&f, &check, 2_000),
                Err(HistoryError::BindingMismatch),
                "target={target}, change={change}"
            );
        }
    }
}

#[test]
fn account_check_then_revoke_rejects_old_or_fresh_cas_at_current_cut() {
    for signer in [false, true] {
        let mut f = enrolled();
        let check = check_instruction(&f);
        transact(&mut f.state, 2_000, |tx| {
            assert_no_writes(tx, &check, &f.observer, true);
            instruction(
                tx,
                Action::Revoke(FinalPromotionAccountCustodyRevocationV1 {
                    signer,
                    attester: !signer,
                }),
            )
            .execute(&f.manager, tx)
            .unwrap();
            assert_no_writes(tx, &check, &f.observer, false);
            assert_no_writes(
                tx,
                &instruction(tx, check.action.clone()),
                &f.observer,
                false,
            );
        });
        assert!(applied(&f, &check, 2_000).is_err());
        let fresh = check_instruction(&f);
        transact(&mut f.state, 2_100, |tx| {
            assert_no_writes(tx, &fresh, &f.observer, false);
        });
        assert!(applied(&f, &fresh, 2_100).is_err());
    }
}

#[test]
fn account_check_time_bounds_and_same_snapshot_cas_cannot_be_substituted() {
    let mut f = enrolled();
    let check = check_instruction(&f);
    transact(&mut f.state, 2_000, |tx| {
        assert_no_writes(tx, &check, &f.observer, true);
    });
    let selected = applied(&f, &check, 2_000).unwrap();
    for now in [0, 1_499, 100_000, u64::MAX] {
        assert!(applied(&f, &check, now).is_err(), "invalid time {now}");
    }
    for field in 0..4 {
        let mut changed = selected.clone();
        match field {
            0 => changed.control_record.deployment_id.push('x'),
            1 => changed.control_record.revision += 1,
            2 => changed.custody_anchor.state_digest[0] ^= 1,
            3 => changed.control.policy.binding.public_key = key(99).public_key().clone(),
            _ => unreachable!(),
        }
        assert!(
            check::check_snapshot_eligibility_v1(
                &changed,
                &check,
                &f.policy.binding,
                &f.observer,
                2_000,
                2_000
            )
            .is_err()
        );
    }
    assert_eq!(applied(&f, &check, 2_000).unwrap(), selected);
}

#[test]
fn account_check_rejects_noncheck_binding_and_oversized_instruction_at_applied_cut() {
    let mut f = enrolled();
    let check = check_instruction(&f);
    transact(&mut f.state, 2_000, |tx| {
        assert_no_writes(tx, &check, &f.observer, true);
    });
    let mut noncheck = check.clone();
    noncheck.action = Action::Revoke(FinalPromotionAccountCustodyRevocationV1 {
        signer: true,
        attester: false,
    });
    assert_eq!(applied(&f, &noncheck, 2_000), Err(HistoryError::Invalid));
    for field in 0..5 {
        let mut binding = f.policy.binding.clone();
        match field {
            0 => binding.chain_id.push('x'), 1 => binding.network_id[0] ^= 1,
            2 => binding.role = sorafs_manifest::signer::protocol::SignerRoleV1::FinalPromotionProvenance,
            3 => binding.purpose = sorafs_manifest::signer::protocol::SignerPurposeBindingV1::FinalPromotionAccountTransaction { deployment_id: "promotion-secondary".into() },
            4 => binding.public_key = key(99).public_key().clone(), _ => unreachable!(),
        }
        assert!(
            check::check_applied_snapshot_v1(&f.state.view(), &check, &binding, &f.observer, 2_000)
                .is_err()
        );
    }
    let mut oversized = check.clone();
    oversized.deployment_id = "x".repeat(FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_RECORD_BYTES_V1);
    transact(&mut f.state, 2_100, |tx| {
        let before = retained(tx);
        assert_eq!(
            apply(oversized.clone(), &f.observer, tx),
            Err(HistoryError::Invalid)
        );
        assert_eq!(retained(tx), before);
    });
    assert_eq!(applied(&f, &oversized, 2_100), Err(HistoryError::Invalid));
}

#[test]
fn account_check_rejects_time_before_native_enrollment_even_after_signed_issuance() {
    let mut f = fixture();
    configure(&mut f);
    let bytes = attest(&f, 1_500, 100_000);
    transact(&mut f.state, 2_000, |tx| {
        instruction(tx, Action::Enroll(bytes))
            .execute(&f.manager, tx)
            .unwrap();
    });
    let check = check_instruction(&f);
    transact(&mut f.state, 2_500, |tx| {
        assert_no_writes(tx, &check, &f.observer, true);
    });
    for now in [1_500, 1_600, 1_999] {
        assert_eq!(applied(&f, &check, now), Err(HistoryError::Custody));
    }
    assert!(applied(&f, &check, 2_000).is_ok());
    assert!(applied(&f, &check, 2_500).is_ok());
}
