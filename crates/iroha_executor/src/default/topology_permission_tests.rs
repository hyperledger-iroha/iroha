/// Exact deployment and action separation for role-16 topology authority.
mod topology_permission_tests {
    use super::*;
    use iroha_data_model::{
        isi::sorafs::MutateSorafsTopologyAuthority,
        sorafs::topology_authority::{
            TopologyActionV1, TopologyCheckPhaseV1, TopologyCheckV1, TopologyCompleteV1,
            TopologyExpireV1, TopologyFloorClaimV1, TopologyHeadV1, TopologyReserveV1,
            TopologyRevocationV1, TopologyTransitionV1,
        },
    };
    use iroha_executor_data_model::permission::sorafs::{
        CanCheckSorafsTopologyApproval, CanManageSorafsTopologyCustody,
        CanOperateSorafsTopologyApproval,
    };
    use sorafs_manifest::signer::{
        protocol::{
            SignerOperationActionV1, SignerOperationAuditHeadV1, SignerOperationCommitmentV1,
            SignerOperationCustodyV1, SignerOperationIntentV1, SignerOperationReservationV1,
        },
        topology::{SignerTopologyRequestV1, subject::TopologyApprovalSubjectV1},
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Capability {
        Manage,
        Operate,
        Check,
    }

    fn token(capability: Capability, deployment: &str) -> PermissionObject {
        let deployment_id = deployment.to_owned();
        match capability {
            Capability::Manage => CanManageSorafsTopologyCustody { deployment_id }.into(),
            Capability::Operate => CanOperateSorafsTopologyApproval { deployment_id }.into(),
            Capability::Check => CanCheckSorafsTopologyApproval { deployment_id }.into(),
        }
    }

    fn instruction(action: TopologyActionV1) -> MutateSorafsTopologyAuthority {
        MutateSorafsTopologyAuthority {
            transition: TopologyTransitionV1 {
                deployment_id: "production-primary".into(),
                control: TopologyHeadV1::EMPTY,
                operations: TopologyHeadV1::EMPTY,
                action,
            },
        }
    }

    fn action_cases() -> [(Capability, TopologyActionV1); 7] {
        let audit = SignerOperationAuditHeadV1 {
            sequence: 0,
            digest: [0; 32],
        };
        let request = SignerTopologyRequestV1 {
            operation_id: [1; 32],
            binding_digest: [2; 32],
            original_custody: SignerOperationCustodyV1 {
                record_digest: [3; 32],
                control_state_digest: [4; 32],
            },
            subject_digest: [5; 32],
        };
        let intent = SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: request.operation_id,
            request_digest: [6; 32],
            previous_audit: audit,
        };
        let reviewed = TopologyReserveV1 {
            subject: TopologyApprovalSubjectV1 {
                deployment_id: "production-primary".into(),
                network_id: [7; 32],
                chain_id: "topology-chain".into(),
                chain_discriminant: 369,
                release_manifest_sha256: [8; 32],
                qualification_summary_sha256: [9; 32],
                manifest_sha256: [10; 32],
                canonical_manifest_sha256: [11; 32],
                validator_ids_sha256: [12; 32],
                reviewed_at_unix_ms: 1_000,
                expires_at_unix_ms: 10_000,
            },
            request,
            intent,
        };
        let reservation = SignerOperationReservationV1 {
            reservation_id: [13; 32],
            fence: 1,
            expires_at_unix_ms: 9_000,
        };
        [
            (
                Capability::Manage,
                TopologyActionV1::Configure(vec![1, 2, 3]),
            ),
            (Capability::Manage, TopologyActionV1::Enroll(vec![4, 5, 6])),
            (
                Capability::Manage,
                TopologyActionV1::Revoke(TopologyRevocationV1 {
                    signer: true,
                    attester: false,
                }),
            ),
            (
                Capability::Operate,
                TopologyActionV1::Reserve(Box::new(reviewed.clone())),
            ),
            (
                Capability::Operate,
                TopologyActionV1::Complete(Box::new(TopologyCompleteV1 {
                    request,
                    intent,
                    reservation,
                    commitment: SignerOperationCommitmentV1 {
                        audit: SignerOperationAuditHeadV1 {
                            sequence: 1,
                            digest: [14; 32],
                        },
                        response_digest: [15; 32],
                    },
                    signatures_digest: [16; 32],
                })),
            ),
            (
                Capability::Operate,
                TopologyActionV1::Expire(TopologyExpireV1 {
                    operation_id: request.operation_id,
                    reservation,
                }),
            ),
            (
                Capability::Check,
                TopologyActionV1::Check(Box::new(TopologyCheckV1 {
                    challenge: [17; 32],
                    network_id: [7; 32],
                    floor: TopologyFloorClaimV1 {
                        height: 1,
                        block_hash: [18; 32],
                    },
                    expected_operator: owner_account_id(),
                    reviewed,
                    phase: TopologyCheckPhaseV1::Current(Box::new(audit)),
                })),
            ),
        ]
    }

    #[test]
    fn topology_permissions_have_exact_closed_deployment_scope_and_distinct_roles() {
        for capability in [Capability::Manage, Capability::Operate, Capability::Check] {
            let exact = token(capability, "production-primary");
            let parsed = AnyPermission::try_from(&exact).expect("registered topology capability");
            assert_eq!(PermissionObject::from(parsed), exact);
            assert_ne!(exact, token(capability, "production-secondary"));
            for other in [Capability::Manage, Capability::Operate, Capability::Check] {
                assert_eq!(
                    exact == token(other, "production-primary"),
                    capability == other
                );
            }
            for payload in [
                Json::new(()),
                Json::new(norito::json!({})),
                Json::new(norito::json!({"deployment_id": 1})),
                Json::new(norito::json!({"deploymentId": "production-primary"})),
                Json::new(norito::json!({"deployment_id": "production-primary", "extra": true})),
            ] {
                assert!(
                    AnyPermission::try_from(&PermissionObject::new(
                        exact.name().to_owned(),
                        payload,
                    ))
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn topology_visitor_and_box_dispatch_require_exact_action_permission_even_at_genesis() {
        for (required, action) in action_cases() {
            let mutation = instruction(action);
            let boxed: InstructionBox = mutation.clone().into();
            for genesis in [false, true] {
                for held in [
                    token(required, "production-primary"),
                    token(Capability::Manage, "production-primary"),
                    token(Capability::Operate, "production-primary"),
                    token(Capability::Check, "production-primary"),
                    token(required, "production-secondary"),
                    iroha_executor_data_model::permission::sorafs::CanManageSorafsFinalPromotionCustody {
                        deployment_id: "production-primary".into(),
                    }.into(),
                ] {
                    let allowed = held == token(required, "production-primary");
                    with_mock_permissions(vec![held], || {
                        let mut direct = MockExecutor::new(genesis);
                        sorafs::visit_mutate_topology_authority(&mut direct, &mutation);
                        assert_eq!(direct.verdict().is_ok(), allowed);
                        let mut dispatched = MockExecutor::new(genesis);
                        super::super::visit_instruction(&mut dispatched, &boxed);
                        assert_eq!(dispatched.verdict().is_ok(), allowed);
                    });
                }
            }
        }
    }

    #[test]
    fn topology_check_refuses_a_self_observer_even_with_check_permission() {
        let (_, TopologyActionV1::Check(mut check)) = action_cases().into_iter().last().unwrap()
        else {
            unreachable!("last action is Check")
        };
        check.expected_operator = authority_account_id();
        let mutation = instruction(TopologyActionV1::Check(check));
        with_mock_permissions(vec![token(Capability::Check, "production-primary")], || {
            let mut direct = MockExecutor::new(false);
            sorafs::visit_mutate_topology_authority(&mut direct, &mutation);
            assert!(direct.verdict().is_err());
            let mut dispatched = MockExecutor::new(false);
            super::super::visit_instruction(&mut dispatched, &mutation.into());
            assert!(dispatched.verdict().is_err());
        });
    }
}
