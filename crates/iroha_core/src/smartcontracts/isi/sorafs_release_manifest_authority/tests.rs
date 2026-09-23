//! Role-13 permission isolation and mandatory closed execution before finalized native storage.
use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, StateReadOnly, World},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::Account,
    block::BlockHeader,
    permission::Permissions,
    sorafs::release_manifest_authority::{
        ReleaseManifestCheckPhaseV1, ReleaseManifestCheckV1, ReleaseManifestCompleteV1,
        ReleaseManifestExpireV1, ReleaseManifestFloorV1, ReleaseManifestReserveV1,
    },
};
use iroha_executor_data_model::permission::sorafs::CanOperateSorafsFinalPromotion;
use sorafs_manifest::signer::{
    protocol::{
        SignerOperationActionV1, SignerOperationAuditHeadV1, SignerOperationCommitmentV1,
        SignerOperationCustodyV1, SignerOperationIntentV1, SignerOperationReservationV1,
    },
    receipt::SignerReleaseManifestRequestV1,
};

const DEPLOYMENT: &str = "release-primary";

fn account(seed: u8) -> AccountId {
    let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture key");
    AccountId::new(key.public_key().clone())
}

struct Fixture {
    state: State,
    manager: AccountId,
    operator: AccountId,
    observer: AccountId,
    foreign: AccountId,
}
fn fixture() -> Fixture {
    let manager = account(1);
    let operator = account(2);
    let observer = account(3);
    let foreign = account(4);
    let mut world = World::new();
    for id in [&manager, &operator, &observer, &foreign] {
        let (key, value) = Account::new(id.clone()).build(&manager).into_key_value();
        world.accounts.insert(key, value);
    }
    for (id, token) in [
        (
            &manager,
            Permission::from(CanManageSorafsReleaseManifestCustody {
                deployment_id: DEPLOYMENT.into(),
            }),
        ),
        (
            &operator,
            Permission::from(CanOperateSorafsReleaseManifest {
                deployment_id: DEPLOYMENT.into(),
            }),
        ),
        (
            &observer,
            Permission::from(CanCheckSorafsReleaseManifest {
                deployment_id: DEPLOYMENT.into(),
            }),
        ),
        (
            &foreign,
            Permission::from(CanOperateSorafsFinalPromotion {
                deployment_id: DEPLOYMENT.into(),
            }),
        ),
    ] {
        let mut permissions = Permissions::new();
        permissions.insert(token);
        world.account_permissions.insert(id.clone(), permissions);
    }
    Fixture {
        state: State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ),
        manager,
        operator,
        observer,
        foreign,
    }
}

fn reviewed() -> ReleaseManifestReserveV1 {
    let request = SignerReleaseManifestRequestV1 {
        operation_id: [5; 32],
        binding_digest: [6; 32],
        original_custody: SignerOperationCustodyV1 {
            record_digest: [7; 32],
            control_state_digest: [8; 32],
        },
        manifest_digest: [9; 32],
        manifest_size: 1024,
    };
    ReleaseManifestReserveV1 {
        request,
        intent: SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: request.operation_id,
            request_digest: request.digest().expect("request digest"),
            previous_audit: SignerOperationAuditHeadV1 {
                sequence: 0,
                digest: [0; 32],
            },
        },
    }
}

fn instruction(action: Action) -> MutateSorafsReleaseManifestAuthority {
    MutateSorafsReleaseManifestAuthority {
        deployment_id: DEPLOYMENT.into(),
        expected_control_revision: 1,
        expected_control_digest: [8; 32],
        action,
    }
}

fn operation_actions() -> [Action; 3] {
    let reviewed = reviewed();
    let reservation = SignerOperationReservationV1 {
        reservation_id: [10; 32],
        fence: 1,
        expires_at_unix_ms: 60_000,
    };
    [
        Action::Reserve(reviewed),
        Action::Complete(ReleaseManifestCompleteV1 {
            reviewed,
            reservation,
            commitment: SignerOperationCommitmentV1 {
                audit: SignerOperationAuditHeadV1 {
                    sequence: 1,
                    digest: [11; 32],
                },
                response_digest: [12; 32],
            },
            signatures_digest: [13; 32],
            completed_at_unix_ms: 59_999,
        }),
        Action::Expire(ReleaseManifestExpireV1 {
            operation_id: reviewed.request.operation_id,
            reservation,
        }),
    ]
}

fn check(operator: &AccountId) -> Action {
    Action::Check(ReleaseManifestCheckV1 {
        challenge: [14; 32],
        network_id: [15; 32],
        expected_operator: operator.clone(),
        floor: ReleaseManifestFloorV1 {
            height: 1,
            block_hash: [16; 32],
        },
        reviewed: reviewed(),
        phase: ReleaseManifestCheckPhaseV1::Current(SignerOperationAuditHeadV1 {
            sequence: 0,
            digest: [0; 32],
        }),
    })
}

#[test]
fn role13_grants_match_only_their_action_and_independent_observer() {
    let f = fixture();
    let view = f.state.view();
    let world = view.world();
    for action in [
        Action::Configure(Vec::new()),
        Action::Enroll(Vec::new()),
        Action::Revoke {
            signer: true,
            attester: false,
        },
    ] {
        let request = instruction(action);
        assert!(authorized(world, &f.manager, &request));
        assert!(!authorized(world, &f.operator, &request));
        assert!(!authorized(world, &f.foreign, &request));
    }
    for action in operation_actions() {
        let request = instruction(action);
        assert!(authorized(world, &f.operator, &request));
        assert!(!authorized(world, &f.manager, &request));
        assert!(!authorized(world, &f.foreign, &request));
    }
    let request = instruction(check(&f.operator));
    assert!(authorized(world, &f.observer, &request));
    assert!(!authorized(world, &f.operator, &request));
    assert!(!authorized(world, &f.foreign, &request));
    let request = instruction(check(&account(20)));
    assert!(!authorized(world, &f.observer, &request));
}

#[test]
fn role13_permission_preflight_rejects_foreign_deployment_and_oversized_action() {
    let f = fixture();
    let view = f.state.view();
    let world = view.world();
    let mut request = instruction(Action::Configure(Vec::new()));
    request.deployment_id.clear();
    assert!(!authorized(world, &f.manager, &request));
    request.deployment_id = "x".repeat(SIGNER_MAX_ID_BYTES_V1 + 1);
    assert!(!authorized(world, &f.manager, &request));
    request.deployment_id = "release-secondary".into();
    assert!(!authorized(world, &f.manager, &request));
    request = instruction(Action::Configure(vec![
        0;
        RELEASE_MANIFEST_ACTION_MAX_BYTES_V1
    ]));
    assert!(!authorized(world, &f.manager, &request));
}

#[test]
fn every_role13_action_remains_closed_even_with_exact_permission() {
    let mut f = fixture();
    let header = BlockHeader::new(
        1.try_into().expect("positive height"),
        f.state.view().latest_block_hash(),
        None,
        1_000,
        0,
    );
    let mut block = f.state.block(header);
    let mut tx = block.transaction();
    let mut actions = vec![
        (Action::Configure(Vec::new()), &f.manager),
        (Action::Enroll(Vec::new()), &f.manager),
        (
            Action::Revoke {
                signer: true,
                attester: false,
            },
            &f.manager,
        ),
    ];
    actions.extend(
        operation_actions()
            .into_iter()
            .map(|action| (action, &f.operator)),
    );
    actions.push((check(&f.operator), &f.observer));
    for (action, actor) in actions {
        let failure = instruction(action)
            .execute(actor, &mut tx)
            .expect_err("role-13 native storage is not installed");
        assert!(matches!(
            failure,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(ref reason)
            ) if reason == CLOSED
        ));
    }
}
