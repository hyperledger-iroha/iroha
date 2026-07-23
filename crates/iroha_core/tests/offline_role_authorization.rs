//! Public-boundary tests for Offline role-derived authorization.

use std::{collections::BTreeSet, num::NonZeroU64};

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::{
    isi::offline::{RegisterOfflineDeviceAttestation, SetOfflineDeviceAttestationPolicy},
    offline::{
        KagemushaDevicePublicKeyV2, OfflineDeviceAttestationPolicy,
        OfflineDeviceAttestationRegistration, OfflineDeviceAttestationTrustedRoot,
    },
    prelude::*,
};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::StorageReadOnly;

const POLICY_PERMISSION: &str = "CanManageOfflineDeviceAttestationPolicy";
const ESCROW_PERMISSION: &str = "CanManageOfflineEscrow";
const POLICY_STATE_KEY: &str = "offline_device_attestation_policy";
const TEST_TIME_MS: u64 = 1_800_000_000_000;

fn exact_permission(name: &str) -> Permission {
    Permission::new(name.to_owned(), Json::new(()))
}

fn permission_with_payload(name: &str, payload: Json) -> Permission {
    Permission::new(name.to_owned(), payload)
}

fn role_with_permission(name: &str, permission: Permission) -> (RoleId, Role) {
    let id: RoleId = name.parse().expect("valid test role id");
    let role = Role::new(id.clone(), ALICE_ID.clone())
        .add_permission(permission)
        .build(&ALICE_ID);
    (id, role)
}

fn base_world(roles: impl IntoIterator<Item = Role>) -> World {
    let alice = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let bob = Account::new(BOB_ID.clone()).build(&BOB_ID);
    World::with_assets_and_roles(
        std::iter::empty::<Domain>(),
        [alice, bob],
        std::iter::empty::<AssetDefinition>(),
        std::iter::empty::<Asset>(),
        std::iter::empty::<Nft>(),
        roles,
    )
}

fn world_with_direct(permission: Permission) -> World {
    let mut world = base_world([]);
    world
        .account_permissions_mut_for_testing()
        .insert(ALICE_ID.clone(), BTreeSet::from([permission]));
    world
}

fn world_with_role(
    permission: Permission,
    assigned_to: Option<AccountId>,
    retain_role_record: bool,
) -> World {
    let (role_id, role) = role_with_permission("offline_test_manager", permission);
    let mut world = if retain_role_record {
        base_world([role])
    } else {
        base_world([])
    };
    if let Some(account) = assigned_to {
        world.grant_role_for_tests(account, role_id);
    }
    world
}

fn state(world: World) -> State {
    State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn header() -> BlockHeader {
    BlockHeader::new(
        NonZeroU64::new(1).expect("nonzero block height"),
        None,
        None,
        None,
        TEST_TIME_MS,
        0,
    )
}

fn valid_policy() -> OfflineDeviceAttestationPolicy {
    OfflineDeviceAttestationPolicy {
        version: 1,
        trusted_roots: vec![OfflineDeviceAttestationTrustedRoot {
            platform: "ios-appattest".to_owned(),
            der: include_bytes!("../../../certs/apple_app_attestation_root.der").to_vec(),
            not_before_ms: None,
            not_after_ms: None,
        }],
        revoked_certificate_sha256: Vec::new(),
        ios_apps: Vec::new(),
        android_apps: Vec::new(),
        require_ios_app_policy: false,
        require_android_app_policy: false,
    }
}

fn assert_rejection_contains(
    result: Result<(), iroha_data_model::isi::error::InstructionExecutionError>,
    expected_reason: &str,
    context: &str,
) {
    let error = result.expect_err("instruction must be rejected");
    assert!(
        error.to_string().contains(expected_reason),
        "{context}: expected {expected_reason}, got {error}"
    );
}

#[test]
fn exact_direct_and_assigned_role_grants_update_policy() {
    let worlds = [
        world_with_direct(exact_permission(POLICY_PERMISSION)),
        world_with_role(
            exact_permission(POLICY_PERMISSION),
            Some(ALICE_ID.clone()),
            true,
        ),
    ];

    for world in worlds {
        let expected = valid_policy();
        let state = state(world);
        let mut block = state.block(header());
        let mut transaction = block.transaction();
        SetOfflineDeviceAttestationPolicy::new(expected.clone())
            .execute(&ALICE_ID, &mut transaction)
            .expect("an exact direct or assigned-role grant must authorize policy updates");

        let key: Name = POLICY_STATE_KEY.parse().expect("valid policy state key");
        let stored = transaction
            .world()
            .smart_contract_state()
            .get(&key)
            .expect("authorized policy update must write state");
        let actual: OfflineDeviceAttestationPolicy =
            norito::decode_from_bytes(stored).expect("stored policy must decode");
        assert_eq!(actual, expected);
    }
}

#[test]
fn stale_unassigned_and_same_name_payload_grants_fail_closed() {
    let cases = [
        ("no grant", base_world([])),
        (
            "unassigned exact role",
            world_with_role(exact_permission(POLICY_PERMISSION), None, true),
        ),
        (
            "role assigned to another account",
            world_with_role(
                exact_permission(POLICY_PERMISSION),
                Some(BOB_ID.clone()),
                true,
            ),
        ),
        (
            "dangling role assignment",
            world_with_role(
                exact_permission(POLICY_PERMISSION),
                Some(ALICE_ID.clone()),
                false,
            ),
        ),
        (
            "same-name role with boolean payload",
            world_with_role(
                permission_with_payload(POLICY_PERMISSION, Json::new(true)),
                Some(ALICE_ID.clone()),
                true,
            ),
        ),
        (
            "same-name direct grant with string payload",
            world_with_direct(permission_with_payload(
                POLICY_PERMISSION,
                Json::new("forged-scope"),
            )),
        ),
        (
            "different permission name",
            world_with_role(
                exact_permission("CanManageOfflineDeviceAttestationPolicyExtra"),
                Some(ALICE_ID.clone()),
                true,
            ),
        ),
    ];

    for (context, world) in cases {
        let state = state(world);
        let mut block = state.block(header());
        let mut transaction = block.transaction();
        assert_rejection_contains(
            SetOfflineDeviceAttestationPolicy::new(valid_policy())
                .execute(&ALICE_ID, &mut transaction),
            "offline_reason::unauthorized_controller",
            context,
        );

        let key: Name = POLICY_STATE_KEY.parse().expect("valid policy state key");
        assert!(
            transaction
                .world()
                .smart_contract_state()
                .get(&key)
                .is_none(),
            "{context}: rejected policy update must not mutate state"
        );
    }
}

fn invalid_registration(account_id: AccountId) -> OfflineDeviceAttestationRegistration {
    let public_key = KagemushaDevicePublicKeyV2::from_sec1_bytes(&[
        0x04, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4,
        0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45, 0xd8,
        0x98, 0xc2, 0x96, 0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e, 0xe7, 0xeb, 0x4a,
        0x7c, 0x0f, 0x9e, 0x16, 0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce, 0xcb, 0xb6, 0x40,
        0x68, 0x37, 0xbf, 0x51, 0xf5,
    ])
    .expect("canonical P-256 generator point");
    let report = b"deliberately-invalid-report".to_vec();
    let evidence = b"deliberately-invalid-evidence".to_vec();

    OfflineDeviceAttestationRegistration {
        version: 0,
        platform: "ios-appattest".to_owned(),
        key_id: "invalid-test-key".to_owned(),
        device_id: "invalid-test-device".to_owned(),
        account_id,
        asset_definition_id: None,
        ios_team_id: None,
        ios_bundle_id: None,
        ios_environment: None,
        android_package_name: None,
        android_signing_certificate_sha256: None,
        public_key,
        assertion_scheme: "ios-appattest".to_owned(),
        assertion_key_algorithm: "ecdsa-p256-sha256".to_owned(),
        assertion_public_key: vec![1],
        assertion_usage_count_limit: Some(1),
        one_use: true,
        challenge_hash: Hash::new(b"invalid-challenge"),
        attestation_report_hash: Hash::new(&report),
        attestation_report: report,
        evidence_hash: Hash::new(&evidence),
        evidence,
        recent_block_height: 1,
        recent_block_hash: Hash::new(b"invalid-block"),
        expires_at_ms: TEST_TIME_MS + 1,
    }
}

#[test]
fn delegated_registration_honors_only_exact_assigned_escrow_grants() {
    let authorized_worlds = [
        world_with_direct(exact_permission(ESCROW_PERMISSION)),
        world_with_role(
            exact_permission(ESCROW_PERMISSION),
            Some(ALICE_ID.clone()),
            true,
        ),
    ];
    for world in authorized_worlds {
        let state = state(world);
        let mut block = state.block(header());
        let mut transaction = block.transaction();
        assert_rejection_contains(
            RegisterOfflineDeviceAttestation::new(invalid_registration(BOB_ID.clone()))
                .execute(&ALICE_ID, &mut transaction),
            "offline_reason::invalid_attestation",
            "exact delegated escrow grant",
        );
    }

    let rejected_worlds = [
        world_with_role(exact_permission(ESCROW_PERMISSION), None, true),
        world_with_role(
            exact_permission(ESCROW_PERMISSION),
            Some(BOB_ID.clone()),
            true,
        ),
        world_with_role(
            permission_with_payload(ESCROW_PERMISSION, Json::new(vec![1_u8, 2_u8])),
            Some(ALICE_ID.clone()),
            true,
        ),
    ];
    for world in rejected_worlds {
        let state = state(world);
        let mut block = state.block(header());
        let mut transaction = block.transaction();
        assert_rejection_contains(
            RegisterOfflineDeviceAttestation::new(invalid_registration(BOB_ID.clone()))
                .execute(&ALICE_ID, &mut transaction),
            "offline_reason::unauthorized_controller",
            "inexact or unassigned delegated escrow grant",
        );
    }

    let state = state(base_world([]));
    let mut block = state.block(header());
    let mut transaction = block.transaction();
    assert_rejection_contains(
        RegisterOfflineDeviceAttestation::new(invalid_registration(ALICE_ID.clone()))
            .execute(&ALICE_ID, &mut transaction),
        "offline_reason::invalid_attestation",
        "self-submission",
    );
}
