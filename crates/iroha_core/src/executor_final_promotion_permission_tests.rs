/// Native final-promotion authority has exact, independent deployment capabilities.
mod final_promotion_permission_tests {
    use super::*;
    use crate::executor::Executor;
    use iroha_data_model::{
        isi::sorafs::MutateSorafsFinalPromotionAuthority,
        sorafs::final_promotion_authority::{
            FinalPromotionAuthorityActionV1, FinalPromotionCheckSubjectV1, FinalPromotionCheckV1,
            FinalPromotionCompleteV1, FinalPromotionExpireV1, FinalPromotionReserveV1,
            FinalPromotionRevocationV1,
        },
    };
    use iroha_executor_data_model::permission::sorafs::{
        CanCheckSorafsFinalPromotion, CanManageSorafsFinalPromotionCustody,
        CanOperateSorafsFinalPromotion,
    };
    use sorafs_manifest::signer::final_promotion::SignerFinalPromotionRequestV1;
    use sorafs_manifest::signer::protocol::{
        SignerOperationActionV1, SignerOperationAuditHeadV1, SignerOperationCommitmentV1,
        SignerOperationCustodyV1, SignerOperationIntentV1, SignerOperationReservationV1,
    };

    #[derive(Clone, Copy, std::fmt::Debug, PartialEq, Eq)]
    enum Capability {
        Manage,
        Operate,
        Check,
    }

    fn permission(capability: Capability, deployment: &str) -> Permission {
        let deployment_id = deployment.to_owned();
        match capability {
            Capability::Manage => CanManageSorafsFinalPromotionCustody { deployment_id }.into(),
            Capability::Operate => CanOperateSorafsFinalPromotion { deployment_id }.into(),
            Capability::Check => CanCheckSorafsFinalPromotion { deployment_id }.into(),
        }
    }
    fn operator() -> AccountId {
        AccountId::new(
            iroha_crypto::KeyPair::try_from_seed(vec![73; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("independent operator fixture")
                .public_key()
                .clone(),
        )
    }
    fn actions() -> [(Capability, FinalPromotionAuthorityActionV1); 7] {
        let intent = SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: [1; 32],
            request_digest: [2; 32],
            previous_audit: SignerOperationAuditHeadV1 {
                sequence: 0,
                digest: [0; 32],
            },
        };
        let custody = SignerOperationCustodyV1 {
            record_digest: [3; 32],
            control_state_digest: [4; 32],
        };
        let reservation = SignerOperationReservationV1 {
            reservation_id: [5; 32],
            fence: 1,
            expires_at_unix_ms: 60_001,
        };
        let commitment = SignerOperationCommitmentV1 {
            audit: SignerOperationAuditHeadV1 {
                sequence: 1,
                digest: [6; 32],
            },
            response_digest: [7; 32],
        };
        [
            (
                Capability::Manage,
                FinalPromotionAuthorityActionV1::Configure(vec![1, 2, 3]),
            ),
            (
                Capability::Manage,
                FinalPromotionAuthorityActionV1::Enroll(vec![4, 5, 6]),
            ),
            (
                Capability::Manage,
                FinalPromotionAuthorityActionV1::Revoke(FinalPromotionRevocationV1 {
                    signer: true,
                    attester: false,
                }),
            ),
            (
                Capability::Operate,
                FinalPromotionAuthorityActionV1::Reserve(FinalPromotionReserveV1 {
                    intent,
                    custody,
                }),
            ),
            (
                Capability::Operate,
                FinalPromotionAuthorityActionV1::Complete(FinalPromotionCompleteV1 {
                    intent,
                    custody,
                    reservation,
                    commitment,
                    signatures_digest: [8; 32],
                }),
            ),
            (
                Capability::Operate,
                FinalPromotionAuthorityActionV1::Expire(FinalPromotionExpireV1 {
                    operation_id: [1; 32],
                    reservation,
                }),
            ),
            (
                Capability::Check,
                FinalPromotionAuthorityActionV1::Check(FinalPromotionCheckV1 {
                    challenge: [10; 32],
                    network_id: [11; 32],
                    expected_operator: operator(),
                    minimum_height: 1,
                    minimum_block_hash: [12; 32],
                    request: SignerFinalPromotionRequestV1 {
                        operation_id: intent.operation_id,
                        binding_digest: [13; 32],
                        original_custody: custody,
                        statement_digest: [14; 32],
                        statement_size: 128,
                    },
                    subject: FinalPromotionCheckSubjectV1::Current(intent.previous_audit),
                }),
            ),
        ]
    }
    fn mutation(deployment: &str, action: FinalPromotionAuthorityActionV1) -> InstructionBox {
        MutateSorafsFinalPromotionAuthority {
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
                Account::new(operator()).build(&ALICE_ID),
            ],
            [],
        );
        world
            .account_permissions
            .insert(ALICE_ID.clone(), alice.into_iter().collect());
        world
            .account_permissions
            .insert(BOB_ID.clone(), bob.into_iter().collect());
        world.account_permissions.insert(
            operator(),
            [permission(Capability::Operate, "production-primary")]
                .into_iter()
                .collect(),
        );
        state_after_genesis(world)
    }
    fn assert_delegated_action_matrix(
        transaction: &StateTransaction<'_, '_>,
        authority: &AccountId,
        capability: Capability,
        has_permission: bool,
    ) {
        for (required, action) in actions() {
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
                            && capability == required
                            && deployment == "production-primary",
                        "{action:?}, {deployment}, genesis={genesis}"
                    );
                }
            }
        }
    }

    #[test]
    fn native_final_promotion_requires_exact_action_and_deployment_even_at_genesis() {
        for capability in [Capability::Manage, Capability::Operate, Capability::Check] {
            let state = fixture(
                vec![permission(capability, "production-primary")],
                vec![executor_permission::sorafs::CanSetSorafsPricing.into()],
            );
            let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
            let transaction = block.transaction();
            for name in [
                "CanManageSorafsFinalPromotionCustody",
                "CanOperateSorafsFinalPromotion",
                "CanCheckSorafsFinalPromotion",
            ] {
                assert!(is_builtin_initial_permission_name(name));
            }
            for (required, action) in actions() {
                for deployment in ["production-primary", "production-secondary"] {
                    let instruction = mutation(deployment, action.clone());
                    assert!(initial_native_instruction_is_explicitly_admitted(
                        &instruction
                    ));
                    for genesis in [false, true] {
                        for authority in [&*ALICE_ID, &*BOB_ID] {
                            assert_eq!(
                                validate_initial_native_instruction_authority(
                                    &transaction,
                                    authority,
                                    &instruction,
                                    genesis
                                )
                                .is_ok(),
                                authority == &*ALICE_ID
                                    && deployment == "production-primary"
                                    && capability == required,
                                "all actions require their exact capability; unrelated permission and foreign deployment cannot substitute"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn native_final_promotion_genesis_bootstraps_grants_but_never_infers_mutation_authority() {
        let state = fixture(vec![], vec![]);
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
        let transaction = block.transaction();
        for (required, action) in actions() {
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
            let grant: InstructionBox = Grant::account_permission(
                permission(required, "production-primary"),
                BOB_ID.clone(),
            )
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
    fn native_final_promotion_rejects_noncanonical_permission_payloads_before_genesis_grants() {
        let state = fixture(vec![], vec![]);
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
        let transaction = block.transaction();
        for required in [Capability::Manage, Capability::Operate, Capability::Check] {
            let exact = permission(required, "production-primary");
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
    fn native_final_promotion_direct_and_role_delegation_cannot_expand_action_or_deployment() {
        for required in [Capability::Manage, Capability::Operate, Capability::Check] {
            let exact = permission(required, "production-primary");
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
            assert_delegated_action_matrix(&transaction, &ALICE_ID, required, true);
            assert_delegated_action_matrix(&transaction, &BOB_ID, required, false);
            for revoke in [false, true] {
                let change: InstructionBox = if revoke {
                    Revoke::account_permission(exact.clone(), BOB_ID.clone()).into()
                } else {
                    Grant::account_permission(exact.clone(), BOB_ID.clone()).into()
                };
                Executor::Initial
                    .execute_instruction(&mut transaction, &ALICE_ID, change)
                    .expect("exact assigned role holder delegates or revokes");
                assert_delegated_action_matrix(&transaction, &BOB_ID, required, !revoke);
                for other in [Capability::Manage, Capability::Operate, Capability::Check]
                    .into_iter()
                    .filter(|capability| *capability != required)
                    .map(|capability| permission(capability, "production-primary"))
                    .chain([permission(required, "production-secondary")])
                {
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
                assert_delegated_action_matrix(&transaction, &BOB_ID, required, !revoke);
            }
        }
    }

    #[test]
    fn native_receipt_check_observer_permission_never_grants_mutation_or_self_check() {
        let state = fixture(
            vec![permission(Capability::Check, "production-primary")],
            vec![],
        );
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
        let mut transaction = block.transaction();
        for (_, action) in actions() {
            let is_check = matches!(action, FinalPromotionAuthorityActionV1::Check(_));
            let instruction = mutation("production-primary", action);
            for genesis in [false, true] {
                assert_eq!(
                    validate_initial_native_instruction_authority(
                        &transaction,
                        &ALICE_ID,
                        &instruction,
                        genesis
                    )
                    .is_ok(),
                    is_check
                );
            }
        }
        transaction.world.account_permissions.insert(
            ALICE_ID.clone(),
            [Capability::Manage, Capability::Operate, Capability::Check]
                .into_iter()
                .map(|capability| permission(capability, "production-primary"))
                .collect(),
        );
        let (_, FinalPromotionAuthorityActionV1::Check(mut check)) =
            actions().into_iter().last().unwrap()
        else {
            unreachable!()
        };
        assert_ne!(check.expected_operator, *ALICE_ID);
        for self_observation in [false, true] {
            check.expected_operator = if self_observation {
                ALICE_ID.clone()
            } else {
                operator()
            };
            let instruction = mutation(
                "production-primary",
                FinalPromotionAuthorityActionV1::Check(check.clone()),
            );
            for genesis in [false, true] {
                assert_eq!(
                    validate_initial_native_instruction_authority(
                        &transaction,
                        &ALICE_ID,
                        &instruction,
                        genesis
                    )
                    .is_ok(),
                    !self_observation
                );
            }
        }
    }
}
