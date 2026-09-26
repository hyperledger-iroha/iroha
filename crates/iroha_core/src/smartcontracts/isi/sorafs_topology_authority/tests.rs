//! Registered role-16 instructions remain closed without a topology State owner.
use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    role::RoleIdWithOwner,
    smartcontracts::isi::{
        InitialNativeInstructionAdmission, execute_borrowed_instruction,
        registered_native_instruction_initial_admission,
    },
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::Account,
    block::BlockHeader,
    isi::InstructionBox,
    permission::Permission,
    role::{Role, RoleId},
    sorafs::topology_authority::{
        TopologyActionV1, TopologyCheckPhaseV1, TopologyCheckV1, TopologyCompleteV1,
        TopologyExpireV1, TopologyFloorClaimV1, TopologyHeadV1, TopologyReserveV1,
        TopologyRevocationV1, TopologyTransitionV1,
    },
};
use mv::storage::StorageReadOnly;
use sorafs_manifest::signer::{
    protocol::{
        SignerOperationActionV1, SignerOperationAuditHeadV1, SignerOperationCommitmentV1,
        SignerOperationCustodyV1, SignerOperationIntentV1, SignerOperationReservationV1,
    },
    topology::{SignerTopologyRequestV1, subject::TopologyApprovalSubjectV1},
};

const DEPLOYMENT: &str = "production-primary";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Capability {
    Manage,
    Operate,
    Check,
}

fn account(seed: u8) -> AccountId {
    let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("deterministic test authority");
    AccountId::new(key.public_key().clone())
}

fn seeded_world(actor: &AccountId, operator: &AccountId) -> World {
    let mut world = World::new();
    for id in [actor, operator] {
        let (id, value) = Account::new(id.clone()).build(actor).into_key_value();
        world.accounts.insert(id, value);
    }
    world
}

fn state_for(world: World) -> State {
    State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn header(state: &State) -> BlockHeader {
    BlockHeader::new(
        1.try_into().expect("positive height"),
        state.view().latest_block_hash(),
        None,
        1_000,
        0,
    )
}

fn authorized_in_world(
    world: World,
    authority: &AccountId,
    transition: &TopologyTransitionV1,
) -> bool {
    let state = state_for(world);
    authorized(state.view().world(), authority, transition)
}

fn permission(capability: Capability, deployment: &str) -> Permission {
    let deployment_id = deployment.to_owned();
    match capability {
        Capability::Manage => CanManageSorafsTopologyCustody { deployment_id }.into(),
        Capability::Operate => CanOperateSorafsTopologyApproval { deployment_id }.into(),
        Capability::Check => CanCheckSorafsTopologyApproval { deployment_id }.into(),
    }
}

fn grant_direct(
    world: &mut World,
    actor: &AccountId,
    grants: impl IntoIterator<Item = Permission>,
) {
    world
        .account_permissions
        .insert(actor.clone(), grants.into_iter().collect());
}

fn transition(deployment: &str, action: TopologyActionV1) -> TopologyTransitionV1 {
    TopologyTransitionV1 {
        deployment_id: deployment.into(),
        control: TopologyHeadV1::EMPTY,
        operations: TopologyHeadV1::EMPTY,
        action,
    }
}

fn action_cases(operator: AccountId) -> [(Capability, TopologyActionV1); 7] {
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
            deployment_id: DEPLOYMENT.into(),
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
                expected_operator: operator,
                reviewed,
                phase: TopologyCheckPhaseV1::Current(Box::new(audit)),
            })),
        ),
    ]
}

#[test]
fn current_world_permission_preflight_separates_all_actions_scopes_and_revocations() {
    let actor = account(17);
    let operator = account(18);
    for (required, action) in action_cases(operator.clone()) {
        for through_role in [false, true] {
            let mut world = seeded_world(&actor, &operator);
            let exact = permission(required, DEPLOYMENT);
            let role_id: RoleId = "topology_actor".parse().expect("role id");
            let role_key = RoleIdWithOwner::new(actor.clone(), role_id.clone());
            if through_role {
                world.roles.insert(
                    role_id.clone(),
                    Role::new(role_id.clone(), actor.clone())
                        .add_permission(exact.clone())
                        .build(&actor),
                );
                world.account_roles.insert(role_key.clone(), ());
            } else {
                grant_direct(&mut world, &actor, [exact.clone()]);
            }
            grant_direct(
                &mut world,
                &operator,
                [permission(Capability::Operate, DEPLOYMENT)],
            );
            let state = state_for(world);
            let mut block = state.block(header(&state));
            let mut tx = block.transaction();
            let exact_action = transition(DEPLOYMENT, action.clone());
            assert!(
                authorized(tx.world(), &actor, &exact_action),
                "{required:?}, role={through_role}"
            );
            assert!(!authorized(
                tx.world(),
                &actor,
                &transition("production-other", action.clone())
            ));
            for other in [Capability::Manage, Capability::Operate, Capability::Check] {
                if other == required {
                    continue;
                }
                let mut wrong = seeded_world(&actor, &operator);
                grant_direct(&mut wrong, &actor, [permission(other, DEPLOYMENT)]);
                grant_direct(
                    &mut wrong,
                    &operator,
                    [permission(Capability::Operate, DEPLOYMENT)],
                );
                assert!(!authorized_in_world(wrong, &actor, &exact_action));
            }
            if through_role {
                tx.world.account_roles.remove(role_key.clone());
                assert!(!authorized(tx.world(), &actor, &exact_action));
                tx.world.account_roles.insert(role_key, ());
                tx.world.roles.remove(role_id);
            } else {
                tx.world.account_permissions.remove(actor.clone());
            }
            assert!(!authorized(tx.world(), &actor, &exact_action));
            let mut stale_grant = seeded_world(&actor, &operator);
            grant_direct(&mut stale_grant, &actor, [exact]);
            grant_direct(
                &mut stale_grant,
                &operator,
                [permission(Capability::Operate, DEPLOYMENT)],
            );
            let stale_state = state_for(stale_grant);
            let mut stale_block = stale_state.block(header(&stale_state));
            stale_block.world.accounts.remove(actor.clone());
            assert!(!authorized(&stale_block.world, &actor, &exact_action));
        }
    }
}

#[test]
fn check_requires_registered_current_operator_and_distinct_observer() {
    let observer = account(19);
    let operator = account(20);
    let (_, TopologyActionV1::Check(check)) = action_cases(operator.clone())
        .into_iter()
        .last()
        .expect("seven action cases")
    else {
        unreachable!("last action is Check")
    };
    let action = transition(DEPLOYMENT, TopologyActionV1::Check(check.clone()));
    let mut world = seeded_world(&observer, &operator);
    grant_direct(
        &mut world,
        &observer,
        [permission(Capability::Check, DEPLOYMENT)],
    );
    grant_direct(
        &mut world,
        &operator,
        [permission(Capability::Operate, DEPLOYMENT)],
    );
    let state = state_for(world);
    let mut block = state.block(header(&state));
    let mut tx = block.transaction();
    assert!(authorized(tx.world(), &observer, &action));
    assert!(!authorized(tx.world(), &operator, &action));

    tx.world.account_permissions.remove(operator.clone());
    assert!(!authorized(tx.world(), &observer, &action));
    let operator_role: RoleId = "topology_operator".parse().expect("role id");
    tx.world.roles.insert(
        operator_role.clone(),
        Role::new(operator_role.clone(), operator.clone())
            .add_permission(permission(Capability::Operate, DEPLOYMENT))
            .build(&operator),
    );
    let operator_role_key = RoleIdWithOwner::new(operator.clone(), operator_role);
    tx.world.account_roles.insert(operator_role_key.clone(), ());
    assert!(authorized(tx.world(), &observer, &action));
    tx.world.account_roles.remove(operator_role_key);
    assert!(!authorized(tx.world(), &observer, &action));
    tx.world.account_permissions.insert(
        operator.clone(),
        [permission(Capability::Operate, "production-other")]
            .into_iter()
            .collect(),
    );
    assert!(!authorized(tx.world(), &observer, &action));
    tx.world.account_permissions.insert(
        operator.clone(),
        [permission(Capability::Operate, DEPLOYMENT)]
            .into_iter()
            .collect(),
    );
    tx.world.accounts.remove(operator.clone());
    assert!(!authorized(tx.world(), &observer, &action));

    let mut self_check = check;
    self_check.expected_operator = observer.clone();
    tx.world.account_permissions.insert(
        observer.clone(),
        [
            permission(Capability::Check, DEPLOYMENT),
            permission(Capability::Operate, DEPLOYMENT),
        ]
        .into_iter()
        .collect(),
    );
    assert!(!authorized(
        tx.world(),
        &observer,
        &transition(DEPLOYMENT, TopologyActionV1::Check(self_check))
    ));
}

#[test]
fn every_registered_topology_action_stays_closed_without_state_mutation() {
    let authority = account(21);
    let operator = account(22);
    let mut world = seeded_world(&authority, &operator);
    grant_direct(
        &mut world,
        &authority,
        [
            permission(Capability::Manage, DEPLOYMENT),
            permission(Capability::Operate, DEPLOYMENT),
            permission(Capability::Check, DEPLOYMENT),
        ],
    );
    grant_direct(
        &mut world,
        &operator,
        [permission(Capability::Operate, DEPLOYMENT)],
    );
    let state = state_for(world);
    let mut block = state.block(header(&state));
    let mut tx = block.transaction();
    let before = tx
        .world()
        .smart_contract_state()
        .iter()
        .map(|(path, value)| (path.clone(), value.clone()))
        .collect::<Vec<_>>();
    for (_, action) in action_cases(operator.clone()) {
        let mutation = MutateSorafsTopologyAuthority {
            transition: transition(DEPLOYMENT, action),
        };
        assert!(authorized(tx.world(), &authority, &mutation.transition));
        let instruction: InstructionBox = mutation.into();
        assert_eq!(
            registered_native_instruction_initial_admission(&instruction),
            Some(InitialNativeInstructionAdmission::Closed)
        );
        let failure = execute_borrowed_instruction(&instruction, &authority, &mut tx)
            .expect_err("topology execution must remain closed");
        assert!(matches!(
            failure,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                ref message
            )) if message == INITIAL_NATIVE_INSTRUCTION_CLOSED_REASON
        ));
        let after = tx
            .world()
            .smart_contract_state()
            .iter()
            .map(|(path, value)| (path.clone(), value.clone()))
            .collect::<Vec<_>>();
        assert_eq!(after, before);
    }
    let denied: InstructionBox = MutateSorafsTopologyAuthority {
        transition: transition(DEPLOYMENT, TopologyActionV1::Configure(vec![1])),
    }
    .into();
    let failure = execute_borrowed_instruction(&denied, &operator, &mut tx)
        .expect_err("an operator cannot configure topology custody");
    assert!(matches!(
        failure,
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            ref message
        )) if message == TOPOLOGY_PERMISSION_REQUIRED_REASON
    ));
    let after = tx
        .world()
        .smart_contract_state()
        .iter()
        .map(|(path, value)| (path.clone(), value.clone()))
        .collect::<Vec<_>>();
    assert_eq!(after, before);
}
