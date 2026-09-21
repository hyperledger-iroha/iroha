/// All seven native authority actions use exact, independent deployment capabilities.
mod final_promotion_permission_tests {
    use super::*;
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

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Capability {
        Manage,
        Operate,
        Check,
    }

    fn permission(capability: Capability, deployment: &str) -> PermissionObject {
        let deployment_id = deployment.to_owned();
        match capability {
            Capability::Manage => CanManageSorafsFinalPromotionCustody { deployment_id }.into(),
            Capability::Operate => CanOperateSorafsFinalPromotion { deployment_id }.into(),
            Capability::Check => CanCheckSorafsFinalPromotion { deployment_id }.into(),
        }
    }
    fn decode<T: norito::json::JsonDeserializeOwned>(value: norito::json::Value) -> T {
        norito::json::from_str(&norito::json::to_json(&value).expect("fixture JSON"))
            .expect("typed protocol field fixture")
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
        let intent = decode(norito::json!({
            "action": {"kind": "sign", "value": null},
            "operation_id": ([1_u8; 32]),
            "request_digest": ([2_u8; 32]),
            "previous_audit": {"sequence": 0, "digest": ([0_u8; 32])}
        }));
        let custody = decode(norito::json!({
            "record_digest": ([3_u8; 32]), "control_state_digest": ([4_u8; 32])
        }));
        let reservation = decode(norito::json!({
            "reservation_id": ([5_u8; 32]), "fence": 1, "expires_at_unix_ms": 60_001
        }));
        let commitment = decode(norito::json!({
            "audit": {"sequence": 1, "digest": ([6_u8; 32])}, "response_digest": ([7_u8; 32])
        }));
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
                    request: decode(norito::json!({
                        "operation_id": ([1_u8; 32]),
                        "binding_digest": ([13_u8; 32]),
                        "original_custody": custody,
                        "statement_digest": ([14_u8; 32]),
                        "statement_size": 128
                    })),
                    subject: FinalPromotionCheckSubjectV1::Current(intent.previous_audit),
                }),
            ),
        ]
    }
    fn mutation(action: FinalPromotionAuthorityActionV1) -> MutateSorafsFinalPromotionAuthority {
        MutateSorafsFinalPromotionAuthority {
            deployment_id: "production-primary".to_owned(),
            expected_control_revision: 1,
            expected_control_digest: [9; 32],
            action,
        }
    }

    #[test]
    fn final_promotion_visitor_requires_exact_action_permission_even_at_genesis() {
        for (required, action) in actions() {
            let instruction = mutation(action);
            for genesis in [false, true] {
                for permissions in [
                    vec![],
                    vec![permission(required, "production-primary")],
                    vec![permission(Capability::Manage, "production-primary")],
                    vec![permission(Capability::Operate, "production-primary")],
                    vec![permission(Capability::Check, "production-primary")],
                    vec![permission(required, "production-secondary")],
                    vec![CanSetSorafsPricing.into()],
                ] {
                    let allowed = permissions == vec![permission(required, "production-primary")];
                    with_mock_permissions(permissions, || {
                        let mut executor = MockExecutor::new(genesis);
                        sorafs::visit_mutate_final_promotion_authority(&mut executor, &instruction);
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
    fn final_promotion_box_dispatch_preserves_all_action_and_genesis_permission_gates() {
        for (required, action) in actions() {
            let instruction: InstructionBox = mutation(action).into();
            for genesis in [false, true] {
                for permission in [
                    permission(required, "production-primary"),
                    permission(Capability::Manage, "production-primary"),
                    permission(Capability::Operate, "production-primary"),
                    permission(Capability::Check, "production-primary"),
                    permission(required, "production-secondary"),
                ] {
                    let allowed = permission == self::permission(required, "production-primary");
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
            permission(Capability::Manage, "production-primary"),
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
    fn final_promotion_grant_revoke_visitors_reject_noncanonical_scopes_even_at_genesis() {
        for required in [Capability::Manage, Capability::Operate, Capability::Check] {
            let exact = permission(required, "production-primary");
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
    fn receipt_check_observer_cannot_check_itself_or_gain_mutation_authority() {
        let check_only = permission(Capability::Check, "production-primary");
        for (_, action) in actions() {
            let is_check = matches!(action, FinalPromotionAuthorityActionV1::Check(_));
            for genesis in [false, true] {
                with_mock_permissions(vec![check_only.clone()], || {
                    let mut executor = MockExecutor::new(genesis);
                    super::super::visit_instruction(
                        &mut executor,
                        &mutation(action.clone()).into(),
                    );
                    assert_eq!(executor.verdict().is_ok(), is_check);
                });
            }
        }
        let (_, FinalPromotionAuthorityActionV1::Check(mut check)) =
            actions().into_iter().last().unwrap()
        else {
            unreachable!()
        };
        assert_ne!(check.expected_operator, authority_account_id());
        for self_observation in [false, true] {
            check.expected_operator = if self_observation {
                authority_account_id()
            } else {
                operator()
            };
            for genesis in [false, true] {
                with_mock_permissions(
                    [Capability::Manage, Capability::Operate, Capability::Check]
                        .into_iter()
                        .map(|capability| permission(capability, "production-primary"))
                        .collect(),
                    || {
                        let mut executor = MockExecutor::new(genesis);
                        let instruction =
                            mutation(FinalPromotionAuthorityActionV1::Check(check.clone())).into();
                        super::super::visit_instruction(&mut executor, &instruction);
                        assert_eq!(executor.verdict().is_ok(), !self_observation);
                    },
                );
            }
        }
    }
}
