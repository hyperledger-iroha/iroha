/// Native Check capabilities bootstrap after genesis and preserve exact observer scopes.
mod gateway_check_permission_tests {
    use super::*;
    use crate::executor::Executor;
    use iroha_executor_data_model::permission::sorafs::CanCheckSorafsStreamTokenGateway;

    fn check_permission(gateway_id: [u8; 32]) -> Permission {
        CanCheckSorafsStreamTokenGateway { gateway_id }.into()
    }

    #[test]
    fn gateway_check_permission_requires_exact_nonzero_payload_and_catalog_identity() {
        let state = managed_state();
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, NOW, 0));
        let mut tx = block.transaction();
        let exact = check_permission([1; 32]);
        assert!(is_builtin_initial_permission_name(exact.name()));
        assert!(!initial_permission_is_genesis_only(&exact));
        assert_eq!(
            normalize_role_permission_for_initial_executor(&tx, &exact).unwrap(),
            exact
        );
        assert_ne!(exact, operate([1; 32]));
        for malformed in [
            check_permission([0; 32]),
            Permission::new("CanCheckSorafsStreamTokenGateway".to_owned(), Json::new(())),
            Permission::new(
                "CanCheckSorafsStreamTokenGateway".to_owned(),
                Json::new(norito::json!({"gateway_id": ([1_u8; 32]), "extra": true})),
            ),
        ] {
            assert!(validate_initial_permission_payload_constraints(&malformed).is_err());
            assert!(normalize_role_permission_for_initial_executor(&tx, &malformed).is_err());
            assert!(
                Executor::Initial
                    .execute_instruction(
                        &mut tx,
                        &ALICE_ID,
                        Grant::account_permission(malformed, BOB_ID.clone()).into()
                    )
                    .is_err()
            );
        }
        assert_eq!(
            initial_permission_capability_root_authority(&tx, &ALICE_ID, &exact).unwrap(),
            Some(false),
            "unknown gateway cannot root Check issuance"
        );
    }

    #[test]
    fn gateway_check_permission_rejects_signed_genesis_account_and_role_scopes() {
        for role_carrier in [false, true] {
            let role: RoleId = "gateway_genesis_observer".parse().unwrap();
            let instruction: InstructionBox = if role_carrier {
                Register::role(
                    Role::new(role.clone(), ALICE_ID.clone())
                        .add_permission(check_permission([1; 32])),
                )
                .into()
            } else {
                Grant::account_permission(check_permission([1; 32]), ALICE_ID.clone()).into()
            };
            let mut config = TestChainConfig::new(initial_world(), NOW);
            config.genesis_instructions.push(instruction);
            let failure = CertifiedTestChain::start(config)
                .expect_err("Check scopes require a post-genesis configured gateway");
            // Preparation preserves the original typed execution rejection, including its
            // scoped permission cause, without publishing the rejected genesis overlay.
            let crate::sumeragi::test_chain::TestChainError::OriginalGenesisExecution(error) =
                &failure.error
            else {
                panic!("unexpected original genesis rejection: {:?}", failure.error);
            };
            let crate::block::BlockValidationError::InvalidGenesis(
                crate::block::InvalidGenesisError::RejectedOutput(rejection),
            ) = error.as_ref()
            else {
                panic!("expected rejected genesis output: {error:?}");
            };
            assert!(matches!(
                rejection.reason.as_ref(),
                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                    ValidationFail::NotPermitted(message)
                ) if message.contains("configured post-genesis scope")
            ));
            let view = failure.state.view();
            assert!(
                !authority_has_permission(view.world(), &ALICE_ID, &check_permission([1; 32]))
                    .unwrap()
            );
            assert!(view.world().roles().get(&role).is_none());
        }
    }

    #[test]
    fn gateway_check_permission_delegation_is_scoped_and_survives_disabled_policy() {
        let state = managed_state();
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, NOW, 0));
        let mut tx = block.transaction();
        let policy = configure_policy(&mut tx, "gateway.check-first", false, true);
        let other_policy = configure_policy(&mut tx, "gateway.check-second", true, false);
        assert!(!policy.allows_admission_at(NOW));
        let exact = check_permission(policy.qualification.gateway_id);
        let other = check_permission(other_policy.qualification.gateway_id);
        let operation = operate(policy.qualification.gateway_id);
        Executor::Initial
            .execute_instruction(
                &mut tx,
                &ALICE_ID,
                Grant::account_permission(exact.clone(), BOB_ID.clone()).into(),
            )
            .expect("configured manager can delegate an exact recovery Check capability");
        for via_role in [false, true] {
            if via_role {
                let role: RoleId = "gateway_delegated_observers".parse().unwrap();
                Executor::Initial
                    .execute_instruction(
                        &mut tx,
                        &BOB_ID,
                        Register::role(
                            Role::new(role, BOB_ID.clone()).add_permission(exact.clone()),
                        )
                        .into(),
                    )
                    .unwrap();
                Executor::Initial
                    .execute_instruction(
                        &mut tx,
                        &BOB_ID,
                        Revoke::account_permission(exact.clone(), BOB_ID.clone()).into(),
                    )
                    .unwrap();
                assert!(!authority_has_direct_permission(&tx.world, &BOB_ID, &exact).unwrap());
            }
            for revoke in [false, true] {
                let instruction = if revoke {
                    Revoke::account_permission(exact.clone(), account(73)).into()
                } else {
                    Grant::account_permission(exact.clone(), account(73)).into()
                };
                Executor::Initial
                    .execute_instruction(&mut tx, &BOB_ID, instruction)
                    .expect("exact Check holder can delegate and revoke");
                assert_eq!(
                    authority_has_permission(&tx.world, &account(73), &exact).unwrap(),
                    !revoke
                );
                for forbidden in [other.clone(), operation.clone(), manage()] {
                    let instruction = if revoke {
                        Revoke::account_permission(forbidden, account(73)).into()
                    } else {
                        Grant::account_permission(forbidden, account(73)).into()
                    };
                    assert!(
                        Executor::Initial
                            .execute_instruction(&mut tx, &BOB_ID, instruction)
                            .is_err(),
                        "Check scope never grants operation, management or another gateway"
                    );
                }
            }
        }
        // An exact operation holder has no implicit observer capability, even for its own gateway.
        Executor::Initial
            .execute_instruction(
                &mut tx,
                &ALICE_ID,
                Grant::account_permission(operation, account(73)).into(),
            )
            .unwrap();
        assert!(
            Executor::Initial
                .execute_instruction(
                    &mut tx,
                    &account(73),
                    Grant::account_permission(exact, account(73)).into()
                )
                .is_err()
        );
    }
}
