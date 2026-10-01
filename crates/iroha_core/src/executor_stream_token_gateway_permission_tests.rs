/// Gateway permission bootstrap and exact-scope administration use the ordinary executor.
mod stream_token_gateway_permission_tests {
    use super::*;
    use crate::executor::Executor;
    use crate::query::stream_token_gateway::storage;
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
    use iroha_data_model::{
        NetworkId,
        sorafs::{
            reputation::derive_stream_token_gateway_id_v1,
            stream_token_gateway::{
                StreamTokenGatewayAdmissionQualificationV1,
                native::{StreamTokenGatewayExecutionV1, StreamTokenGatewayPolicyV1},
            },
        },
    };
    use iroha_executor_data_model::permission::sorafs::{
        CanManageSorafsStreamTokenGateway, CanOperateSorafsStreamTokenGateway,
    };
    use std::sync::Arc;

    const NOW: u64 = 1_700_000_000_000;

    fn manage() -> Permission {
        CanManageSorafsStreamTokenGateway.into()
    }

    fn operate(gateway_id: [u8; 32]) -> Permission {
        CanOperateSorafsStreamTokenGateway { gateway_id }.into()
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("deterministic account")
                .public_key()
                .clone(),
        )
    }

    fn initial_world() -> World {
        World::with(
            [],
            [
                Account::new(ALICE_ID.clone()).build(&ALICE_ID),
                Account::new(BOB_ID.clone()).build(&BOB_ID),
                Account::new(account(73)).build(&ALICE_ID),
                Account::new(SAMPLE_GENESIS_ACCOUNT_ID.clone()).build(&ALICE_ID),
            ],
            [],
        )
    }

    fn before_genesis() -> State {
        State::new_for_testing(
            initial_world(),
            Kura::blank_kura_for_testing(),
            query::store::LiveQueryStore::start_test(),
        )
    }

    fn managed_state() -> Arc<State> {
        // The identity-only negative probe remains unauthenticated and must confer no authority.
        let pristine = before_genesis();
        let mut block = pristine.block(BlockHeader::new(nonzero!(1_u64), None, None, NOW - 1, 0));
        let transaction = block.transaction();
        for authority in [&*ALICE_ID, &*SAMPLE_GENESIS_ACCOUNT_ID] {
            assert!(
                !initial_permission_delegation_allowed(&transaction, authority, &manage())
                    .expect("registered account"),
                "identity alone confers no management capability before the signed grant"
            );
        }
        drop(transaction);
        drop(block);
        let mut config = TestChainConfig::new(initial_world(), NOW - 1);
        config.genesis_instructions.extend([
            Grant::account_permission(manage(), ALICE_ID.clone()).into(),
            Grant::account_permission(executor_permission::role::CanManageRoles, ALICE_ID.clone())
                .into(),
            Grant::account_permission(executor_permission::role::CanManageRoles, BOB_ID.clone())
                .into(),
        ]);
        let chain = CertifiedTestChain::start(config)
            .expect("original authenticated genesis seeds the unit policy administrator");
        assert!(
            authority_has_permission(chain.state().view().world(), &ALICE_ID, &manage()).unwrap()
        );
        Arc::clone(chain.state())
    }

    fn configure_policy(
        transaction: &mut StateTransaction<'_, '_>,
        label: &str,
        admission_enabled: bool,
        expired: bool,
    ) -> StreamTokenGatewayPolicyV1 {
        let network_id = *transaction.network_id();
        let mut policy = StreamTokenGatewayPolicyV1 {
            network_id,
            compliance_gateway_id: label.to_owned(),
            qualification: StreamTokenGatewayAdmissionQualificationV1 {
                gateway_id: derive_stream_token_gateway_id_v1(&network_id, label)
                    .expect("network-derived gateway"),
                revision: 1,
                policy_digest: [1; 32],
                max_pending: 64,
                max_tracked_tokens: 32,
                lease_ttl_ms: 120_000,
            },
            operators: BTreeSet::from([BOB_ID.clone()]),
            observers: BTreeSet::from([account(73)]),
            valid_from_unix_ms: NOW - 1_000,
            valid_until_unix_ms: if expired { NOW - 1 } else { NOW + 60_000 },
            max_observation_age_ms: 30_000,
            admission_enabled,
        };
        policy.qualification.policy_digest = policy.calculate_policy_digest().unwrap();
        let execution = StreamTokenGatewayExecutionV1 {
            height: transaction._curr_block.height().get(),
            transaction_hash: [8; 32],
            entry_index: 0,
            instruction_index: 0,
            recorded_at_unix_ms: transaction.block_unix_timestamp_ms(),
            authority: ALICE_ID.clone(),
        };
        // This fixture seeds the real row adapter. It does not stand in for native origin or
        // finality verification; the permission function reads these rows from its actual World.
        storage::configure(transaction, &policy, 0, [0; 32], &execution, [9; 32])
            .expect("configure fixture policy");
        policy
    }

    #[test]
    fn native_gateway_permission_payloads_are_exact_and_registered() {
        let state = managed_state();
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, NOW, 0));
        let mut transaction = block.transaction();
        let instruction = iroha_data_model::isi::sorafs::MutateSorafsStreamTokenGateway {
            request: iroha_data_model::sorafs::stream_token_gateway::native::StreamTokenGatewayRequestV1 {
                network_id: *transaction.network_id(),
                gateway_id: [1; 32],
                expected_policy_revision: 1,
                expected_policy_digest: [2; 32],
                action: iroha_data_model::sorafs::stream_token_gateway::native::StreamTokenGatewayActionV1::Expire { max_items: 1 },
            },
        }.into();
        assert!(
            initial_native_instruction_is_explicitly_admitted(&instruction),
            "registered CoreAuthorized handler owns the gateway action's runtime checks"
        );
        for permission in [manage(), operate([1; 32])] {
            assert!(is_builtin_initial_permission_name(permission.name()));
            assert!(!initial_permission_is_genesis_only(&permission));
            validate_initial_permission_payload_constraints(&permission).unwrap();
            assert_eq!(
                normalize_role_permission_for_initial_executor(&transaction, &permission)
                    .expect("canonical permission"),
                permission
            );
        }
        let invalid = [
            Permission::new(
                "CanManageSorafsStreamTokenGateway".to_owned(),
                Json::new(norito::json!({ "gateway_id": ([1_u8; 32]) })),
            ),
            operate([0; 32]),
            Permission::new(
                "CanOperateSorafsStreamTokenGateway".to_owned(),
                Json::new(()),
            ),
            Permission::new(
                "CanOperateSorafsStreamTokenGateway".to_owned(),
                Json::new(norito::json!({ "gateway_id": ([1_u8; 32]), "extra": true })),
            ),
        ];
        for permission in invalid {
            assert!(validate_initial_permission_payload_constraints(&permission).is_err());
            assert!(
                normalize_role_permission_for_initial_executor(&transaction, &permission).is_err()
            );
            for revoke in [false, true] {
                let instruction = if revoke {
                    Revoke::account_permission(permission.clone(), BOB_ID.clone()).into()
                } else {
                    Grant::account_permission(permission.clone(), BOB_ID.clone()).into()
                };
                assert!(
                    Executor::Initial
                        .execute_instruction(&mut transaction, &ALICE_ID, instruction)
                        .is_err()
                );
            }
        }
        let unknown = Permission::new("CanOperateUnknownGateway".to_owned(), Json::new(()));
        assert!(normalize_role_permission_for_initial_executor(&transaction, &unknown).is_err());
        assert!(!authority_has_permission(&transaction.world, &BOB_ID, &operate([0; 32])).unwrap());
    }

    #[test]
    fn native_gateway_signed_genesis_rejects_operate_account_and_role_bootstrap() {
        for role_carrier in [false, true] {
            let role: RoleId = "gateway_genesis_operator".parse().unwrap();
            let instruction: InstructionBox = if role_carrier {
                Register::role(
                    Role::new(role.clone(), ALICE_ID.clone()).add_permission(operate([1; 32])),
                )
                .into()
            } else {
                Grant::account_permission(operate([1; 32]), ALICE_ID.clone()).into()
            };
            let mut config = TestChainConfig::new(initial_world(), NOW);
            config.genesis_instructions.push(instruction);
            let failure = CertifiedTestChain::start(config)
                .expect_err("genesis cannot name its own network-derived gateway scope");
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
            assert!(!authority_has_permission(view.world(), &ALICE_ID, &operate([1; 32])).unwrap());
            assert!(view.world().roles().get(&role).is_none());
        }
        // Management is intentionally the seedable capability, without a permanent genesis-only
        // restriction on its later exact-holder or role-based delegation.
        let state = managed_state();
        let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, NOW, 0));
        let mut transaction = block.transaction();
        assert!(!is_initial_genesis_context(&transaction));
        assert!(
            Executor::Initial
                .execute_instruction(
                    &mut transaction,
                    &SAMPLE_GENESIS_ACCOUNT_ID,
                    Grant::account_permission(manage(), BOB_ID.clone()).into(),
                )
                .is_err(),
            "genesis identity and a height-one header cannot regain bootstrap authority"
        );
    }

    #[test]
    fn native_gateway_management_delegates_and_revokes_through_exact_roles() {
        let state = managed_state();
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, NOW, 0));
        let mut transaction = block.transaction();
        let role: RoleId = "gateway_policy_managers".parse().unwrap();
        Executor::Initial
            .execute_instruction(
                &mut transaction,
                &ALICE_ID,
                Register::role(Role::new(role.clone(), ALICE_ID.clone()).add_permission(manage()))
                    .into(),
            )
            .expect("manager seeds its exact capability into a role");
        Executor::Initial
            .execute_instruction(
                &mut transaction,
                &ALICE_ID,
                Revoke::account_permission(manage(), ALICE_ID.clone()).into(),
            )
            .expect("role holder can revoke its direct management permission");
        assert!(
            !authority_has_direct_permission(&transaction.world, &ALICE_ID, &manage()).unwrap()
        );
        assert!(authority_has_permission(&transaction.world, &ALICE_ID, &manage()).unwrap());
        let policy = configure_policy(&mut transaction, "gateway.role-manager", false, true);
        let scoped = operate(policy.qualification.gateway_id);
        Executor::Initial
            .execute_instruction(
                &mut transaction,
                &ALICE_ID,
                Grant::account_permission(scoped.clone(), BOB_ID.clone()).into(),
            )
            .expect("assigned management role roots an exact configured operation scope");
        Executor::Initial
            .execute_instruction(
                &mut transaction,
                &ALICE_ID,
                Revoke::account_permission(scoped, BOB_ID.clone()).into(),
            )
            .expect("assigned management role can revoke recovery authority");
        for revoke in [false, true] {
            let instruction = if revoke {
                Revoke::account_permission(manage(), BOB_ID.clone()).into()
            } else {
                Grant::account_permission(manage(), BOB_ID.clone()).into()
            };
            Executor::Initial
                .execute_instruction(&mut transaction, &ALICE_ID, instruction)
                .expect("management role delegates or revokes exact unit capability");
            assert_eq!(
                authority_has_permission(&transaction.world, &BOB_ID, &manage()).unwrap(),
                !revoke
            );
        }
        for revoke in [false, true] {
            let instruction = if revoke {
                Revoke::account_role(role.clone(), BOB_ID.clone()).into()
            } else {
                Grant::account_role(role.clone(), BOB_ID.clone()).into()
            };
            Executor::Initial
                .execute_instruction(&mut transaction, &ALICE_ID, instruction)
                .expect("management role assignment follows normal exact-holder rules");
            assert_eq!(
                authority_has_permission(&transaction.world, &BOB_ID, &manage()).unwrap(),
                !revoke
            );
        }
        let empty: RoleId = "gateway_unprivileged_role".parse().unwrap();
        Executor::Initial
            .execute_instruction(
                &mut transaction,
                &BOB_ID,
                Register::role(Role::new(empty.clone(), BOB_ID.clone())).into(),
            )
            .expect("unprivileged role fixture");
        for revoke in [false, true] {
            let instruction = if revoke {
                Revoke::role_permission(manage(), empty.clone()).into()
            } else {
                Grant::role_permission(manage(), empty.clone()).into()
            };
            assert!(
                Executor::Initial
                    .execute_instruction(&mut transaction, &BOB_ID, instruction)
                    .is_err(),
                "role ownership and CanManageRoles cannot manufacture gateway management"
            );
        }
        assert!(!authority_has_permission(&transaction.world, &BOB_ID, &manage()).unwrap());
        assert!(
            Executor::Initial
                .execute_instruction(
                    &mut transaction,
                    &BOB_ID,
                    Grant::account_permission(manage(), BOB_ID.clone()).into(),
                )
                .is_err(),
            "revoked former holder cannot restore its capability"
        );
    }

    #[test]
    fn native_gateway_manager_scope_requires_live_registered_same_network_policy() {
        let state = managed_state();
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, NOW, 0));
        let mut transaction = block.transaction();
        assert_eq!(
            initial_permission_capability_root_authority(
                &transaction,
                &ALICE_ID,
                &operate([1; 32])
            )
            .unwrap(),
            Some(false)
        );
        let policy = configure_policy(&mut transaction, "gateway.permissions", true, false);
        let permission = operate(policy.qualification.gateway_id);
        assert_eq!(
            initial_permission_capability_root_authority(&transaction, &ALICE_ID, &permission)
                .unwrap(),
            Some(true)
        );
        let unregistered = account(74);
        transaction
            .world
            .account_permissions
            .insert(unregistered.clone(), BTreeSet::from([manage()]));
        assert_eq!(
            initial_permission_capability_root_authority(&transaction, &unregistered, &permission)
                .unwrap(),
            Some(false)
        );
        assert!(
            Executor::Initial
                .execute_instruction(
                    &mut transaction,
                    &unregistered,
                    Grant::account_permission(permission.clone(), BOB_ID.clone()).into()
                )
                .is_err()
        );
        let original_network = transaction.network_id;
        transaction.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
            Hash::new(b"foreign-gateway-network"),
        ));
        assert!(
            initial_permission_capability_root_authority(&transaction, &ALICE_ID, &permission)
                .is_err(),
            "configured foreign network cannot root local permission issuance"
        );
        transaction.network_id = original_network;
        transaction
            .world
            .smart_contract_state
            .remove(storage::head_path(policy.qualification.gateway_id).unwrap());
        assert!(
            Executor::Initial
                .execute_instruction(
                    &mut transaction,
                    &ALICE_ID,
                    Grant::account_permission(permission, BOB_ID.clone()).into()
                )
                .is_err(),
            "partial policy state cannot root issuance"
        );
    }

    #[test]
    fn native_gateway_disabled_or_expired_policy_preserves_permission_drain() {
        for (enabled, expired) in [(false, false), (true, true)] {
            let state = managed_state();
            let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, NOW, 0));
            let mut transaction = block.transaction();
            let policy = configure_policy(&mut transaction, "gateway.drain", enabled, expired);
            assert!(!policy.allows_admission_at(NOW));
            let permission = operate(policy.qualification.gateway_id);
            for revoke in [false, true] {
                let instruction = if revoke {
                    Revoke::account_permission(permission.clone(), BOB_ID.clone()).into()
                } else {
                    Grant::account_permission(permission.clone(), BOB_ID.clone()).into()
                };
                Executor::Initial.execute_instruction(&mut transaction, &ALICE_ID, instruction)
                    .expect("configured manager administers recovery capability independently of admission interval");
                assert_eq!(
                    authority_has_permission(&transaction.world, &BOB_ID, &permission).unwrap(),
                    !revoke
                );
            }
        }
    }

    #[test]
    fn native_gateway_operate_holders_delegate_and_revoke_only_their_exact_scope() {
        let state = managed_state();
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, NOW, 0));
        let mut transaction = block.transaction();
        let first = configure_policy(&mut transaction, "gateway.first", true, false);
        let second = configure_policy(&mut transaction, "gateway.second", true, false);
        let exact = operate(first.qualification.gateway_id);
        let other = operate(second.qualification.gateway_id);
        Executor::Initial
            .execute_instruction(
                &mut transaction,
                &ALICE_ID,
                Grant::account_permission(exact.clone(), BOB_ID.clone()).into(),
            )
            .unwrap();
        let role: RoleId = "gateway_exact_operators".parse().unwrap();
        Executor::Initial
            .execute_instruction(
                &mut transaction,
                &BOB_ID,
                Register::role(
                    Role::new(role.clone(), BOB_ID.clone()).add_permission(exact.clone()),
                )
                .into(),
            )
            .unwrap();
        for revoke in [true, false] {
            let instruction = if revoke {
                Revoke::role_permission(exact.clone(), role.clone()).into()
            } else {
                Grant::role_permission(exact.clone(), role.clone()).into()
            };
            Executor::Initial
                .execute_instruction(&mut transaction, &BOB_ID, instruction)
                .expect("exact holder can remove and restore its role's exact scope");
            assert_eq!(
                transaction
                    .world
                    .roles()
                    .get(&role)
                    .unwrap()
                    .permissions()
                    .any(|permission| permission == &exact),
                !revoke
            );
        }
        for through_role in [false, true] {
            if through_role {
                Executor::Initial
                    .execute_instruction(
                        &mut transaction,
                        &BOB_ID,
                        Revoke::account_permission(exact.clone(), BOB_ID.clone()).into(),
                    )
                    .unwrap();
                assert!(
                    !authority_has_direct_permission(&transaction.world, &BOB_ID, &exact).unwrap()
                );
            }
            for revoke in [false, true] {
                let instruction = if revoke {
                    Revoke::account_permission(exact.clone(), account(73)).into()
                } else {
                    Grant::account_permission(exact.clone(), account(73)).into()
                };
                Executor::Initial
                    .execute_instruction(&mut transaction, &BOB_ID, instruction)
                    .expect("direct and role holders administer exact scope");
                assert_eq!(
                    authority_has_permission(&transaction.world, &account(73), &exact).unwrap(),
                    !revoke
                );
                for permission in [other.clone(), manage()] {
                    let account_instruction = if revoke {
                        Revoke::account_permission(permission.clone(), account(73)).into()
                    } else {
                        Grant::account_permission(permission.clone(), account(73)).into()
                    };
                    let role_instruction = if revoke {
                        Revoke::role_permission(permission, role.clone()).into()
                    } else {
                        Grant::role_permission(permission, role.clone()).into()
                    };
                    for instruction in [account_instruction, role_instruction] {
                        assert!(
                            Executor::Initial
                                .execute_instruction(&mut transaction, &BOB_ID, instruction)
                                .is_err(),
                            "operation scope cannot expand into another configured gateway or management"
                        );
                    }
                }
            }
        }
        for revoke in [false, true] {
            let instruction = if revoke {
                Revoke::account_role(role.clone(), account(73)).into()
            } else {
                Grant::account_role(role.clone(), account(73)).into()
            };
            Executor::Initial
                .execute_instruction(&mut transaction, &BOB_ID, instruction)
                .expect("exact operator role assignment");
            assert_eq!(
                authority_has_permission(&transaction.world, &account(73), &exact).unwrap(),
                !revoke
            );
        }
    }
    include!("executor_stream_token_gateway_check_permission_tests.rs");
}
