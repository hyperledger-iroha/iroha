/// Native account custody has exact, independent deployment capabilities.
mod final_promotion_account_permission_tests {
    use super::*;
    use crate::executor::Executor;
    use iroha_data_model::{
        isi::sorafs::MutateSorafsFinalPromotionAccountCustody,
        sorafs::final_promotion_account_custody::{
            FinalPromotionAccountCustodyActionV1, FinalPromotionAccountCustodyCheckV1,
            FinalPromotionAccountCustodyRevocationV1,
        },
    };
    use iroha_executor_data_model::permission::sorafs::{
        CanCheckSorafsFinalPromotionAccountCustody, CanManageSorafsFinalPromotionAccountCustody,
    };
    fn permission(manage: bool, deployment: &str) -> Permission {
        if manage {
            CanManageSorafsFinalPromotionAccountCustody {
                deployment_id: deployment.to_owned(),
            }
            .into()
        } else {
            CanCheckSorafsFinalPromotionAccountCustody {
                deployment_id: deployment.to_owned(),
            }
            .into()
        }
    }
    fn actions() -> [(bool, FinalPromotionAccountCustodyActionV1); 4] {
        [
            (
                true,
                FinalPromotionAccountCustodyActionV1::Configure(vec![1, 2, 3]),
            ),
            (
                true,
                FinalPromotionAccountCustodyActionV1::Enroll(vec![4, 5, 6]),
            ),
            (
                true,
                FinalPromotionAccountCustodyActionV1::Revoke(
                    FinalPromotionAccountCustodyRevocationV1 {
                        signer: true,
                        attester: false,
                    },
                ),
            ),
            (
                false,
                FinalPromotionAccountCustodyActionV1::Check(FinalPromotionAccountCustodyCheckV1 {
                    challenge: [10; 32],
                    network_id: [11; 32],
                    minimum_height: 1,
                    minimum_block_hash: [12; 32],
                    expected_account: AccountId::new(
                        iroha_crypto::KeyPair::from_seed(
                            vec![73; 32],
                            iroha_crypto::Algorithm::Ed25519,
                        )
                        .public_key()
                        .clone(),
                    ),
                    transaction_payload_digest: [14; 32],
                }),
            ),
        ]
    }
    fn mutation(deployment: &str, action: FinalPromotionAccountCustodyActionV1) -> InstructionBox {
        MutateSorafsFinalPromotionAccountCustody {
            deployment_id: deployment.to_owned(),
            expected_control_revision: 1,
            expected_control_digest: [9; 32],
            action,
        }
        .into()
    }
    fn fixture(alice: Vec<Permission>, bob: Vec<Permission>) -> State {
        let mut world = World::with(
            [],
            [
                Account::new(ALICE_ID.clone()).build(&ALICE_ID),
                Account::new(BOB_ID.clone()).build(&BOB_ID),
            ],
            [],
        );
        world
            .account_permissions
            .insert(ALICE_ID.clone(), alice.into_iter().collect());
        world
            .account_permissions
            .insert(BOB_ID.clone(), bob.into_iter().collect());
        state_after_genesis(world)
    }
    fn assert_delegated_action_matrix(
        transaction: &StateTransaction<'_, '_>,
        authority: &AccountId,
        manages: bool,
        has_permission: bool,
    ) {
        for (requires_management, action) in actions() {
            for deployment in ["production-primary", "production-secondary"] {
                let instruction = mutation(deployment, action.clone());
                for genesis in [false, true] {
                    assert_eq!(
                        validate_initial_native_instruction_authority(
                            transaction,
                            authority,
                            &instruction,
                            genesis,
                        )
                        .is_ok(),
                        has_permission
                            && manages == requires_management
                            && deployment == "production-primary",
                        "{action:?}, {deployment}, genesis={genesis}"
                    );
                }
            }
        }
    }

    #[test]
    fn native_account_custody_requires_exact_action_and_deployment_even_at_genesis() {
        let state = fixture(
            vec![permission(true, "production-primary")],
            vec![
                permission(false, "production-primary"),
                executor_permission::sorafs::CanSetSorafsPricing.into(),
            ],
        );
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
        let transaction = block.transaction();
        for name in [
            "CanManageSorafsFinalPromotionAccountCustody",
            "CanCheckSorafsFinalPromotionAccountCustody",
        ] {
            assert!(is_builtin_initial_permission_name(name));
        }
        for (manage, action) in actions() {
            for deployment in ["production-primary", "production-secondary"] {
                let instruction = mutation(deployment, action.clone());
                assert!(initial_native_instruction_is_explicitly_admitted(
                    &instruction
                ));
                for genesis in [false, true] {
                    for (authority, manages) in [(&*ALICE_ID, true), (&*BOB_ID, false)] {
                        assert_eq!(
                            validate_initial_native_instruction_authority(
                                &transaction,
                                authority,
                                &instruction,
                                genesis
                            )
                            .is_ok(),
                            deployment == "production-primary" && manage == manages,
                            "all actions require their exact permission without domain, provider, or role prerequisites"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn native_account_custody_genesis_bootstraps_grants_but_never_infers_mutation_authority() {
        let state = fixture(vec![], vec![]);
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
        let transaction = block.transaction();
        for (manage, action) in actions() {
            let instruction = mutation("production-primary", action);
            for genesis in [false, true] {
                assert!(
                    validate_initial_native_instruction_authority(
                        &transaction,
                        &ALICE_ID,
                        &instruction,
                        genesis
                    )
                    .is_err()
                );
            }
            let grant: InstructionBox =
                Grant::account_permission(permission(manage, "production-primary"), BOB_ID.clone())
                    .into();
            assert!(
                validate_initial_permission_or_role_mutation(
                    &transaction,
                    &ALICE_ID,
                    &grant,
                    true,
                    None
                )
                .is_ok()
            );
            assert!(
                validate_initial_permission_or_role_mutation(
                    &transaction,
                    &ALICE_ID,
                    &grant,
                    false,
                    None
                )
                .is_err()
            );
        }
    }

    #[test]
    fn native_account_custody_rejects_noncanonical_permission_payloads_before_genesis_grants() {
        let state = fixture(vec![], vec![]);
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
        let transaction = block.transaction();
        for manage in [false, true] {
            let exact = permission(manage, "production-primary");
            assert!(validate_initial_permission_payload_constraints(&exact).is_ok());
            assert_eq!(
                normalize_role_permission_for_initial_executor(&transaction, &exact)
                    .expect("known canonical token"),
                exact
            );
            for payload in [
                Json::new(()),
                Json::new(norito::json!({})),
                Json::new(norito::json!({"deployment_id": 1})),
                Json::new(norito::json!({"deploymentId": "production-primary"})),
                Json::new(norito::json!({"deployment_id": "production-primary", "extra": true})),
            ] {
                let malformed = Permission::new(exact.name().to_owned(), payload);
                assert!(validate_initial_permission_payload_constraints(&malformed).is_err());
                assert!(
                    normalize_role_permission_for_initial_executor(&transaction, &malformed)
                        .is_err()
                );
                for genesis in [false, true] {
                    for revoke in [false, true] {
                        let mutation: InstructionBox = if revoke {
                            Revoke::account_permission(malformed.clone(), BOB_ID.clone()).into()
                        } else {
                            Grant::account_permission(malformed.clone(), BOB_ID.clone()).into()
                        };
                        assert!(
                            validate_initial_permission_or_role_mutation(
                                &transaction,
                                &ALICE_ID,
                                &mutation,
                                genesis,
                                None
                            )
                            .is_err()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn native_account_custody_direct_and_role_delegation_cannot_expand_action_or_deployment() {
        for manage in [false, true] {
            let exact = permission(manage, "production-primary");
            let state = fixture(vec![exact.clone()], vec![]);
            let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
            let mut transaction = block.transaction();
            let role: RoleId = "final_promotion_operator".parse().expect("role id");
            Register::role(Role::new(role.clone(), ALICE_ID.clone()).add_permission(exact.clone()))
                .execute(&ALICE_ID, &mut transaction)
                .expect("seed exact role");
            transaction
                .world
                .account_permissions
                .insert(ALICE_ID.clone(), BTreeSet::new());
            assert!(authority_has_role(&transaction.world, &ALICE_ID, &role));
            assert_delegated_action_matrix(&transaction, &ALICE_ID, manage, true);
            assert_delegated_action_matrix(&transaction, &BOB_ID, manage, false);
            for revoke in [false, true] {
                let change: InstructionBox = if revoke {
                    Revoke::account_permission(exact.clone(), BOB_ID.clone()).into()
                } else {
                    Grant::account_permission(exact.clone(), BOB_ID.clone()).into()
                };
                Executor::Initial
                    .execute_instruction(&mut transaction, &ALICE_ID, change)
                    .expect("exact assigned role holder delegates or revokes");
                assert_delegated_action_matrix(&transaction, &BOB_ID, manage, !revoke);
                for other in [
                    permission(!manage, "production-primary"),
                    permission(manage, "production-secondary"),
                ] {
                    let expansion: InstructionBox = if revoke {
                        Revoke::role_permission(other, role.clone()).into()
                    } else {
                        Grant::role_permission(other, role.clone()).into()
                    };
                    assert!(
                        validate_initial_permission_or_role_mutation(
                            &transaction,
                            &ALICE_ID,
                            &expansion,
                            false,
                            None
                        )
                        .is_err()
                    );
                }
            }
            for revoke in [false, true] {
                let change: InstructionBox = if revoke {
                    Revoke::account_role(role.clone(), BOB_ID.clone()).into()
                } else {
                    Grant::account_role(role.clone(), BOB_ID.clone()).into()
                };
                Executor::Initial
                    .execute_instruction(&mut transaction, &ALICE_ID, change)
                    .expect("exact assigned role is delegable and revocable");
                assert_delegated_action_matrix(&transaction, &BOB_ID, manage, !revoke);
            }
        }
    }

    #[test]
    fn account_custody_self_observation_and_receipt_permissions_fail_closed() {
        let state = fixture(
            vec![
                permission(true, "production-primary"),
                permission(false, "production-primary"),
            ],
            vec![
                executor_permission::sorafs::CanManageSorafsFinalPromotionCustody {
                    deployment_id: "production-primary".into(),
                }
                .into(),
                executor_permission::sorafs::CanOperateSorafsFinalPromotion {
                    deployment_id: "production-primary".into(),
                }
                .into(),
                executor_permission::sorafs::CanCheckSorafsFinalPromotion {
                    deployment_id: "production-primary".into(),
                }
                .into(),
            ],
        );
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
        let transaction = block.transaction();
        for (_, action) in actions() {
            let instruction = mutation("production-primary", action);
            for genesis in [false, true] {
                assert!(
                    validate_initial_native_instruction_authority(
                        &transaction,
                        &ALICE_ID,
                        &instruction,
                        genesis
                    )
                    .is_ok()
                );
                assert!(
                    validate_initial_native_instruction_authority(
                        &transaction,
                        &BOB_ID,
                        &instruction,
                        genesis
                    )
                    .is_err()
                );
            }
        }
        let (_, FinalPromotionAccountCustodyActionV1::Check(mut check)) =
            actions().into_iter().last().unwrap()
        else {
            unreachable!()
        };
        assert_ne!(check.expected_account, *ALICE_ID);
        check.expected_account = ALICE_ID.clone();
        let instruction = mutation(
            "production-primary",
            FinalPromotionAccountCustodyActionV1::Check(check),
        );
        for genesis in [false, true] {
            assert!(
                validate_initial_native_instruction_authority(
                    &transaction,
                    &ALICE_ID,
                    &instruction,
                    genesis
                )
                .is_err()
            );
        }
    }
}
