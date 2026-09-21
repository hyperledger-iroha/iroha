/// All four account-custody actions use exact, independent deployment capabilities.
mod final_promotion_account_permission_tests {
    use super::*;
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

    fn permission(manage: bool, deployment: &str) -> PermissionObject {
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
    fn mutation(
        action: FinalPromotionAccountCustodyActionV1,
    ) -> MutateSorafsFinalPromotionAccountCustody {
        MutateSorafsFinalPromotionAccountCustody {
            deployment_id: "production-primary".to_owned(),
            expected_control_revision: 1,
            expected_control_digest: [9; 32],
            action,
        }
    }

    #[test]
    fn final_promotion_account_visitor_requires_exact_action_permission_even_at_genesis() {
        for (manage, action) in actions() {
            let instruction = mutation(action);
            for genesis in [false, true] {
                for permissions in [
                    vec![],
                    vec![permission(manage, "production-primary")],
                    vec![permission(!manage, "production-primary")],
                    vec![permission(manage, "production-secondary")],
                    vec![CanSetSorafsPricing.into()],
                    vec![iroha_executor_data_model::permission::sorafs::CanManageSorafsFinalPromotionCustody { deployment_id: "production-primary".into() }.into()],
                    vec![iroha_executor_data_model::permission::sorafs::CanOperateSorafsFinalPromotion { deployment_id: "production-primary".into() }.into()],
                    vec![iroha_executor_data_model::permission::sorafs::CanCheckSorafsFinalPromotion { deployment_id: "production-primary".into() }.into()],
                ] {
                    let allowed = permissions == vec![permission(manage, "production-primary")];
                    with_mock_permissions(permissions, || {
                        let mut executor = MockExecutor::new(genesis);
                        sorafs::visit_mutate_final_promotion_account_custody(&mut executor, &instruction);
                        assert_eq!(
                            executor.verdict().is_ok(),
                            allowed,
                            "{:?}, genesis={genesis}",
                            instruction.action
                        );
                    });
                }
            }
        }
    }

    #[test]
    fn final_promotion_account_box_dispatch_preserves_all_action_and_genesis_permission_gates() {
        for (manage, action) in actions() {
            let instruction: InstructionBox = mutation(action).into();
            for genesis in [false, true] {
                for permission in [
                    permission(manage, "production-primary"),
                    permission(!manage, "production-primary"),
                    permission(manage, "production-secondary"),
                ] {
                    let allowed = permission == self::permission(manage, "production-primary");
                    with_mock_permissions(vec![permission], || {
                        let mut executor = MockExecutor::new(genesis);
                        super::super::visit_instruction(&mut executor, &instruction);
                        assert_eq!(executor.verdict().is_ok(), allowed);
                    });
                }
            }
        }
    }

    #[test]
    fn custody_dispatch_declines_unrelated_instructions_without_changing_verdict() {
        let instruction: InstructionBox = Grant::account_permission(
            permission(true, "production-primary"),
            authority_account_id(),
        )
        .into();
        let mut executor = MockExecutor::new(false);
        assert!(!sorafs::visit_custody_instruction(
            &mut executor,
            &instruction
        ));
        assert!(executor.verdict().is_ok());
    }

    #[test]
    fn final_promotion_account_grant_revoke_visitors_reject_noncanonical_scopes_even_at_genesis() {
        for manage in [false, true] {
            let exact = permission(manage, "production-primary");
            let malformed = PermissionObject::new(
                exact.name().to_owned(),
                norito::json!({
                    "deployment_id": "production-primary", "extra": true
                }),
            );
            for genesis in [false, true] {
                for revoke in [false, true] {
                    let instruction: InstructionBox = if revoke {
                        Revoke::account_permission(malformed.clone(), authority_account_id()).into()
                    } else {
                        Grant::account_permission(malformed.clone(), authority_account_id()).into()
                    };
                    with_mock_permissions(vec![exact.clone()], || {
                        let mut executor = MockExecutor::new(genesis);
                        super::super::visit_instruction(&mut executor, &instruction);
                        assert!(executor.verdict().is_err());
                    });
                }
            }
        }
    }

    #[test]
    fn account_custody_observer_cannot_check_itself_even_with_both_capabilities() {
        let (_, action) = actions().into_iter().last().unwrap();
        let FinalPromotionAccountCustodyActionV1::Check(mut check) = action else {
            unreachable!()
        };
        assert_ne!(check.expected_account, authority_account_id());
        for genesis in [false, true] {
            for self_observation in [false, true] {
                if self_observation {
                    check.expected_account = authority_account_id();
                }
                let instruction =
                    mutation(FinalPromotionAccountCustodyActionV1::Check(check.clone())).into();
                with_mock_permissions(
                    vec![
                        permission(true, "production-primary"),
                        permission(false, "production-primary"),
                    ],
                    || {
                        let mut executor = MockExecutor::new(genesis);
                        super::super::visit_instruction(&mut executor, &instruction);
                        assert_eq!(executor.verdict().is_ok(), !self_observation);
                    },
                );
            }
            check.expected_account = AccountId::new(
                iroha_crypto::KeyPair::from_seed(vec![73; 32], iroha_crypto::Algorithm::Ed25519)
                    .public_key()
                    .clone(),
            );
        }
    }
}
