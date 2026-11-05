//! Integration tests for role registration and assignment flows.
use crate::permissions::{account_permissions, account_role_ids, stored_role};
use executor_custom_data_model::permissions::CanControlDomainLives;
use eyre::Result;
use integration_tests::sandbox;
use iroha::{
    blocking::Client,
    data_model::{prelude::*, transaction::error::TransactionRejectionReason},
};
use iroha_executor_data_model::permission::{
    account::CanModifyAccountMetadata, role::CanManageRoles,
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::name::Name;
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, gen_account_in};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
fn start_network(context: &'static str) -> Option<(sandbox::SerializedNetwork, Runtime)> {
    sandbox::start_network_blocking_or_skip(NetworkBuilder::new(), context).unwrap()
}
fn err_chain_contains(err: &eyre::Report, needle: &str) -> bool {
    err.chain().any(|cause| cause.to_string().contains(needle))
}
fn wait_for_role(client: &Client, role_id: &RoleId, context: &str) -> Role {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);
    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "roles were not queried".to_owned();
    while Instant::now() < deadline {
        match stored_role(client, role_id) {
            Ok(role) => {
                last_observed = format!("role_present={}", role.is_some());
                if let Some(role) = role {
                    return role;
                }
            }
            Err(err) => {
                last_observed = format!("query failed: {err}");
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    panic!(
        "timed out waiting for role after {context}; role_id={role_id}; last_observed={last_observed}"
    );
}
#[test]
fn register_empty_role() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(register_empty_role)) else {
        return Ok(());
    };
    let test_client = network.client();
    let role_id = "root".parse().expect("Valid");
    let register_role = Register::role(Role::new(role_id, ALICE_ID.clone()));
    test_client.submit(
        register_role,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    Ok(())
}
/// Test meant to mirror the test of the same name in the Iroha Kotlin SDK. This doesn't actually
/// test the functionality of the role granted, merely that the role can be constructed and
/// registered. Once @appetrosyan (me) is onboarded into the Kotlin SDK, I'll update both tests to
/// actually verify functionality. We now ensure metadata edits are rejected prior to the grant so
/// the permission check is exercised.
///
/// @s8sato added: This test represents #2081 case.
#[test]
fn register_and_grant_role_for_metadata_access() -> Result<()> {
    let (mouse_id, mouse_keypair) = gen_account_in("wonderland");
    // Role management is a genesis-only capability. Author Mouse's account and
    // that capability in the signed fixture; Mouse still delegates only its own
    // account-metadata permission when it registers the nonempty role below.
    let builder = NetworkBuilder::new()
        .with_genesis_instruction(Register::account(Account::new(mouse_id.clone())))
        .with_genesis_instruction(Grant::account_permission(CanManageRoles, mouse_id.clone()));
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        builder,
        stringify!(register_and_grant_role_for_metadata_access),
    )?
    else {
        return Ok(());
    };
    let test_client = network.client();
    let alice_id = ALICE_ID.clone();
    let fund_mouse = Transfer::asset_quantity(
        AssetId::new(
            iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?,
            alice_id.clone(),
        ),
        100_u32,
        mouse_id.clone(),
    );
    test_client.submit(
        fund_mouse,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let mouse_client =
        network.peers()[0].client_for(&mouse_id, mouse_keypair.private_key().clone());
    // Registering role
    let role_id = "ACCESS_TO_MOUSE_METADATA".parse::<RoleId>()?;
    let role =
        Role::new(role_id.clone(), mouse_id.clone()).add_permission(CanModifyAccountMetadata {
            account: mouse_id.clone(),
        });
    let register_role = Register::role(role);
    // Only Mouse may delegate its account-metadata permission into the role.
    mouse_client.submit(
        register_role,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // Preserve the fixture's domain transfer; account-metadata rights remain
    // bound to Mouse's universal account rather than the domain owner.
    let wonderland: DomainId = DomainId::try_new("wonderland", "universal")?;
    let transfer_domain = Transfer::domain(alice_id.clone(), wonderland, mouse_id.clone());
    test_client.submit(
        transfer_domain,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // Metadata edits must fail before the permission is granted.
    let metadata_key = "key".parse::<Name>()?;
    let metadata_value = Json::from("value");
    let err = test_client
        .submit(
            SetKeyValue::account(
                mouse_id.clone(),
                metadata_key.clone(),
                metadata_value.clone(),
            ),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("metadata update without permission should be rejected");
    assert!(
        matches!(
            err.downcast_ref::<TransactionRejectionReason>(),
            Some(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(reason)))
                if reason == "authority cannot modify this metadata"
        ),
        "expected the account-metadata authorization rejection, got: {err:?}"
    );
    // Mouse grants role to Alice
    let grant_role = Grant::account_role(role_id.clone(), alice_id.clone());
    mouse_client.submit(
        grant_role,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // Alice modifies Mouse's metadata
    let set_key_value = SetKeyValue::account(mouse_id, metadata_key, metadata_value);
    test_client.submit(
        set_key_value,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // Making request to find Alice's roles
    let found_role_ids = account_role_ids(&test_client, &alice_id)?;
    assert!(found_role_ids.contains(&role_id));
    Ok(())
}
#[test]
fn unregistered_role_removed_from_account() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(unregistered_role_removed_from_account))
    else {
        return Ok(());
    };
    let test_client = network.client();
    let role_id: RoleId = "root".parse().expect("Valid");
    let alice_id = ALICE_ID.clone();
    let (mouse_id, _mouse_keypair) = gen_account_in("wonderland");
    // Registering Mouse
    let register_mouse = Register::account(Account::new(mouse_id.clone()));
    test_client.submit(
        register_mouse,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // Register root role
    let register_role = Register::role(
        Role::new(role_id.clone(), alice_id.clone())
            .add_permission(CanModifyAccountMetadata { account: alice_id }),
    );
    test_client.submit(
        register_role,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // Grant root role to Mouse
    let grant_role = Grant::account_role(role_id.clone(), mouse_id.clone());
    test_client.submit(
        grant_role,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // Check that Mouse has root role
    let found_mouse_roles = account_role_ids(&test_client, &mouse_id)?;
    assert!(found_mouse_roles.contains(&role_id));
    // Unregister root role
    let unregister_role = Unregister::role(role_id.clone());
    test_client.submit(
        unregister_role,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // Check that Mouse doesn't have the root role
    let found_mouse_roles = account_role_ids(&test_client, &mouse_id)?;
    assert!(!found_mouse_roles.contains(&role_id));
    Ok(())
}
#[test]
fn role_with_invalid_permissions_is_not_accepted() -> Result<()> {
    let Some((network, _rt)) =
        start_network(stringify!(role_with_invalid_permissions_is_not_accepted))
    else {
        return Ok(());
    };
    let test_client = network.client();
    let role_id = "ACCESS_TO_ACCOUNT_METADATA".parse()?;
    let role = Role::new(role_id, ALICE_ID.clone()).add_permission(CanControlDomainLives);
    let err = test_client
        .submit(
            Register::role(role),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("Submitting role with non-existing permission should fail");
    if let Some(rejection_reason) = err.downcast_ref::<TransactionRejectionReason>() {
        assert!(matches!(
            rejection_reason,
            &TransactionRejectionReason::Validation(ValidationFail::NotPermitted(_))
        ));
    } else {
        assert!(
            err_chain_contains(&err, "Unknown permission")
                || err_chain_contains(&err, "NotPermitted"),
            "expected unknown permission rejection, got: {err:?}"
        );
    }
    Ok(())
}
#[test]
// The two independently constructed permissions have the same canonical V1 payload and must be
// deduplicated by the executor.
fn role_permissions_are_deduplicated() {
    let Some((network, _rt)) = start_network(stringify!(role_permissions_are_deduplicated)) else {
        return;
    };
    let test_client = network.client();
    let wonderland: DomainId =
        DomainId::try_new("wonderland", "universal").expect("wonderland domain");
    let rose_definition = AssetDefinitionId::derive_from_components(
        wonderland.clone(),
        "rose".parse().expect("valid rose name"),
    );
    let rose_asset = AssetId::new(rose_definition, ALICE_ID.clone());
    let rose_asset_lower = rose_asset.canonical_literal();
    let allow_alice_to_transfer_rose_1 = Permission::new(
        "CanTransferAsset".parse().unwrap(),
        iroha_primitives::json::Json::new(
            norito::json::object([(
                "asset",
                norito::json::to_value(&rose_asset_lower).expect("serialize asset"),
            )])
            .expect("serialize permission payload"),
        ),
    );
    // Independently construct the same canonical payload through the strict text boundary.
    let allow_alice_to_transfer_rose_2 = Permission::new(
        "CanTransferAsset".parse().unwrap(),
        iroha_primitives::json::Json::from_raw_json(format!(r#"{{"asset":"{rose_asset_lower}"}}"#))
            .expect("valid permission JSON fixture"),
    );
    let role_id: RoleId = "role_id".parse().expect("Valid");
    let role = Role::new(role_id.clone(), ALICE_ID.clone())
        .add_permission(allow_alice_to_transfer_rose_1)
        .add_permission(allow_alice_to_transfer_rose_2);
    test_client
        .submit(
            Register::role(role),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("failed to register role");
    let role = wait_for_role(&test_client, &role_id, "role registration");
    // Permissions are unified so only one is left
    assert_eq!(
        role.permissions().len(),
        1,
        "permissions for role aren't deduplicated"
    );
}
#[test]
fn grant_revoke_role_permissions() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(grant_revoke_role_permissions)) else {
        return Ok(());
    };
    let test_client = network.client();
    let alice_id = ALICE_ID.clone();
    let (mouse_id, mouse_keypair) = gen_account_in("wonderland");
    // Registering Mouse
    let register_mouse = Register::account(Account::new(mouse_id.clone()));
    let fund_mouse = Transfer::asset_quantity(
        AssetId::new(
            iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?,
            alice_id.clone(),
        ),
        100_u32,
        mouse_id.clone(),
    );
    test_client.submit_all::<InstructionBox>(
        [register_mouse.into(), fund_mouse.into()],
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let mouse_client =
        network.peers()[0].client_for(&mouse_id, mouse_keypair.private_key().clone());
    // Registering role
    let role_id = "ACCESS_TO_MOUSE_METADATA".parse::<RoleId>()?;
    let role = Role::new(role_id.clone(), mouse_id.clone());
    let register_role = Register::role(role);
    test_client.submit(
        register_role,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // Transfer domain ownership to Mouse
    let domain_id = DomainId::try_new("wonderland", "universal")?;
    let transfer_domain = Transfer::domain(alice_id.clone(), domain_id, mouse_id.clone());
    test_client.submit(
        transfer_domain,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // Mouse grants role to Alice
    let grant_role = Grant::account_role(role_id.clone(), alice_id.clone());
    mouse_client.submit(
        grant_role,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let set_key_value = SetKeyValue::account(mouse_id.clone(), "key".parse()?, Json::from("value"));
    let can_set_key_value_in_mouse = CanModifyAccountMetadata {
        account: mouse_id.clone(),
    };
    let grant_role_permission =
        Grant::role_permission(can_set_key_value_in_mouse.clone(), role_id.clone());
    let revoke_role_permission =
        Revoke::role_permission(can_set_key_value_in_mouse.clone(), role_id.clone());
    // Alice can't modify Mouse's metadata without proper permission
    assert!(
        !account_permissions(&test_client, &alice_id)?
            .iter()
            .any(|permission| {
                CanModifyAccountMetadata::try_from(permission)
                    .is_ok_and(|permission| permission == can_set_key_value_in_mouse)
            })
    );
    let _ = test_client
        .submit(
            set_key_value.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("shouldn't be able to modify metadata");
    // Alice can modify Mouse's metadata after permission is granted to role
    mouse_client.submit(
        grant_role_permission,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    assert!(account_role_ids(&test_client, &alice_id)?.contains(&role_id));
    test_client.submit(
        set_key_value.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    // Alice can't modify Mouse's metadata after permission is removed from role
    mouse_client.submit(
        revoke_role_permission,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    assert!(
        !account_permissions(&test_client, &alice_id)?
            .iter()
            .any(|permission| {
                CanModifyAccountMetadata::try_from(permission)
                    .is_ok_and(|permission| permission == can_set_key_value_in_mouse)
            })
    );
    let _ = test_client
        .submit(
            set_key_value,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("shouldn't be able to modify metadata");
    Ok(())
}
#[test]
fn role_permission_revoke_then_grant_last_wins_canonical_output() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_default_block_cadence()
        .with_config_layer(|layer| {
            layer
                .write(["pipeline", "parallel_overlay"], true)
                .write(["pipeline", "parallel_apply"], true)
                .write("telemetry_profile", "full");
        });
    let Some((network, rt)) = sandbox::start_network_blocking_or_skip(
        builder,
        stringify!(role_permission_revoke_then_grant_last_wins_canonical_output),
    )?
    else {
        return Ok(());
    };
    let test_client = network.client();
    let alice_id = ALICE_ID.clone();
    let role_id: RoleId = "PERM_LAST_WINS".parse()?;
    test_client.submit(
        Register::role(Role::new(role_id.clone(), alice_id.clone())),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let perm: Permission = CanModifyAccountMetadata {
        account: alice_id.clone(),
    }
    .into();
    test_client.submit(
        Grant::role_permission(perm.clone(), role_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let expected_permissions = stored_role(&test_client, &role_id)?
        .expect("seeded role must exist")
        .permissions()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    assert!(expected_permissions.contains(&perm));
    crate::permissions::canonical_permission_execution::assert_no_detached_activity(&network, &rt)?;
    let instructions: Vec<InstructionBox> = vec![
        Revoke::role_permission(perm.clone(), role_id.clone()).into(),
        Grant::role_permission(perm.clone(), role_id.clone()).into(),
    ];
    let submitted = test_client.submit_all(
        instructions.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    crate::permissions::canonical_permission_execution::assert_canonical_output(
        &network,
        submitted,
        &alice_id,
        &instructions,
    )?;
    for peer in network.validators() {
        let role = stored_role(&peer.client(), &role_id)?.expect("role should exist");
        assert_eq!(
            role.permissions()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            expected_permissions,
            "last grant must retain the exact role permissions on every voter"
        );
    }
    crate::permissions::canonical_permission_execution::assert_no_detached_activity(&network, &rt)?;
    Ok(())
}
#[path = "support/invalid_genesis.rs"]
mod invalid_genesis;

#[tokio::test]
async fn grant_unexisting_role_in_genesis_fail() -> Result<()> {
    let placeholder: InstructionBox = Log::new(
        iroha_data_model::Level::INFO,
        "unknown-role genesis placeholder".to_owned(),
    )
    .into();
    let Some(network) = sandbox::build_network_or_skip(
        NetworkBuilder::new()
            .with_peers(4)
            .with_genesis_instruction(placeholder.clone()),
        stringify!(grant_unexisting_role_in_genesis_fail),
    ) else {
        return Ok(());
    };
    let grant = Grant::account_role("UNEXISTING".parse::<RoleId>()?, ALICE_ID.clone());
    invalid_genesis::assert_rejected_by_all_peers(
        &network,
        &placeholder,
        grant.into(),
        "cannot delegate an unknown role",
        Duration::from_secs(10),
    )
    .await
}

#[test]
fn bounded_role_collections_preserve_assignments_and_authorization() -> Result<()> {
    use iroha::collections::{Collection, FilterExpr, ListQuery, field};
    use std::collections::BTreeSet;

    let Some((network, _rt)) = start_network(stringify!(
        bounded_role_collections_preserve_assignments_and_authorization
    )) else {
        return Ok(());
    };
    let client = network.client();
    let (mouse, keys) = gen_account_in("wonderland");
    let ids: Vec<RoleId> = (0..7)
        .map(|i| format!("bounded_role_{i}").parse())
        .collect::<Result<_, _>>()?;
    let permission: Permission = CanModifyAccountMetadata {
        account: ALICE_ID.clone(),
    }
    .into();
    let mut setup: Vec<InstructionBox> =
        vec![Register::account(Account::new(mouse.clone())).into()];
    setup.extend(ids.iter().map(|id| {
        Register::role(Role::new(id.clone(), ALICE_ID.clone()).add_permission(permission.clone()))
            .into()
    }));
    setup.push(Grant::account_role(ids[3].clone(), mouse.clone()).into());
    client.submit_all(
        setup,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let filter = FilterExpr::Or(
        ids.iter()
            .map(|id| field("id").eq(id.to_string()))
            .collect(),
    );
    for collection in [
        Collection::Roles,
        Collection::AccountRoles(ALICE_ID.clone()),
    ] {
        let mut query = ListQuery::new().filter(filter.clone()).limit(2);
        let mut found = Vec::new();
        let mut cursors = BTreeSet::new();
        for _ in 0..8 {
            let page = client.list_page(&collection, &query)?;
            assert!(page.items.len() <= 2);
            assert!(page.total.is_none());
            for row in page.items {
                let id: RoleId = row
                    .get("id")
                    .and_then(norito::json::Value::as_str)
                    .expect("role identity")
                    .parse()?;
                if matches!(collection, Collection::Roles) {
                    let role: Role = norito::json::from_value(row)?;
                    assert_eq!(role.permissions().collect::<Vec<_>>(), vec![&permission]);
                }
                found.push(id);
            }
            let Some(cursor) = page.next_cursor else {
                break;
            };
            assert!(
                cursors.insert(cursor.clone()),
                "role page repeated its cursor"
            );
            query = query.cursor(cursor);
        }
        assert_eq!(
            found, ids,
            "native pages must preserve every ordered assignment"
        );
        assert_eq!(
            cursors.len(),
            3,
            "seven rows at limit two require four pages"
        );
    }
    let mouse_client = network.peers()[0].client_for(&mouse, keys.private_key().clone());
    assert_eq!(
        account_role_ids(&mouse_client, &mouse)?,
        vec![ids[3].clone()]
    );
    let denied = mouse_client
        .list_page(
            &Collection::AccountRoles(ALICE_ID.clone()),
            &ListQuery::new(),
        )
        .expect_err("self authority must not read another account's roles");
    assert_eq!(denied.http_status(), Some(403));
    assert_eq!(denied.code(), Some("role_read_denied"));
    let denied = mouse_client
        .list_page(&Collection::Roles, &ListQuery::new())
        .expect_err("role membership must not grant whole-ledger role reads");
    assert_eq!(denied.http_status(), Some(403));
    let grant = iroha_executor_data_model::permission::query::CanReadAccountData {
        account: ALICE_ID.clone(),
    };
    client.submit(
        Grant::account_permission(grant.clone(), mouse.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let permitted = mouse_client.list_page(
        &Collection::AccountRoles(ALICE_ID.clone()),
        &ListQuery::new().filter(filter).limit(64),
    )?;
    assert_eq!(permitted.items.len(), ids.len());
    client.submit(
        Revoke::account_permission(grant, mouse),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let denied = mouse_client
        .list_page(
            &Collection::AccountRoles(ALICE_ID.clone()),
            &ListQuery::new(),
        )
        .expect_err("revoked account-read grant must stop subsequent pages");
    assert_eq!(denied.http_status(), Some(403));
    assert_eq!(denied.code(), Some("role_read_denied"));
    Ok(())
}
