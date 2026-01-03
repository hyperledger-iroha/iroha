//! Integration tests for role registration and assignment flows.
use std::time::Duration;

use executor_custom_data_model::permissions::CanControlDomainLives;
use eyre::Result;
use futures_util::future::join_all;
use integration_tests::sandbox;
use iroha::data_model::{prelude::*, transaction::error::TransactionRejectionReason};
use iroha_executor_data_model::permission::account::CanModifyAccountMetadata;
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, gen_account_in};
use tokio::{fs, runtime::Runtime, time::timeout};

fn start_network(context: &'static str) -> Option<(sandbox::SerializedNetwork, Runtime)> {
    sandbox::start_network_blocking_or_skip(NetworkBuilder::new(), context).unwrap()
}

#[test]
fn register_empty_role() -> Result<()> {
    let Some((network, _rt)) = start_network(stringify!(register_empty_role)) else {
        return Ok(());
    };
    let test_client = network.client();

    let role_id = "root".parse().expect("Valid");
    let register_role = Register::role(Role::new(role_id, ALICE_ID.clone()));

    test_client.submit(register_role)?;
    Ok(())
}

/// Test meant to mirror the test of the same name in the Iroha Kotlin
/// SDK. This doesn't actually test the functionality of the role
/// granted, merely that the role can be constructed and
/// registered. Once @appetrosyan (me) is onboarded into the Kotlin
/// SDK, I'll update both tests to actually verify functionality. We now ensure
/// metadata edits are rejected prior to the grant so the permission check is exercised.
///
/// @s8sato added: This test represents #2081 case.
#[test]
fn register_and_grant_role_for_metadata_access() -> Result<()> {
    let Some((network, _rt)) =
        start_network(stringify!(register_and_grant_role_for_metadata_access))
    else {
        return Ok(());
    };
    let test_client = network.client();

    let alice_id = ALICE_ID.clone();
    let (mouse_id, mouse_keypair) = gen_account_in("wonderland");

    // Registering Mouse
    let register_mouse = Register::account(Account::new(mouse_id.clone()));
    test_client.submit_blocking(register_mouse)?;

    // Registering role
    let role_id = "ACCESS_TO_MOUSE_METADATA".parse::<RoleId>()?;
    let role =
        Role::new(role_id.clone(), mouse_id.clone()).add_permission(CanModifyAccountMetadata {
            account: mouse_id.clone(),
        });
    let register_role = Register::role(role);
    test_client.submit_blocking(register_role)?;

    // Metadata edits must fail before the permission is granted.
    let metadata_key = "key".parse::<Name>()?;
    let metadata_value = "value".parse::<Json>()?;
    let err = test_client
        .submit_blocking(SetKeyValue::account(
            mouse_id.clone(),
            metadata_key.clone(),
            metadata_value.clone(),
        ))
        .expect_err("metadata update without permission should be rejected");
    assert!(
        err.to_string().contains("Not permitted"),
        "expected a Not permitted validation error, got: {err:?}"
    );

    // Mouse grants role to Alice
    let grant_role = Grant::account_role(role_id.clone(), alice_id.clone());
    let grant_role_tx = TransactionBuilder::new(network.chain_id(), mouse_id.clone())
        .with_instructions([grant_role])
        .sign(mouse_keypair.private_key());
    test_client.submit_transaction_blocking(&grant_role_tx)?;

    // Alice modifies Mouse's metadata
    let set_key_value = SetKeyValue::account(mouse_id, metadata_key, metadata_value);
    test_client.submit_blocking(set_key_value)?;

    // Making request to find Alice's roles
    let found_role_ids = test_client
        .query(FindRolesByAccountId::new(alice_id))
        .execute_all()?;
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
    test_client.submit_blocking(register_mouse)?;

    // Register root role
    let register_role = Register::role(
        Role::new(role_id.clone(), alice_id.clone())
            .add_permission(CanModifyAccountMetadata { account: alice_id }),
    );
    test_client.submit_blocking(register_role)?;

    // Grant root role to Mouse
    let grant_role = Grant::account_role(role_id.clone(), mouse_id.clone());
    test_client.submit_blocking(grant_role)?;

    // Check that Mouse has root role
    let found_mouse_roles = test_client
        .query(FindRolesByAccountId::new(mouse_id.clone()))
        .execute_all()?;
    assert!(found_mouse_roles.contains(&role_id));

    // Unregister root role
    let unregister_role = Unregister::role(role_id.clone());
    test_client.submit_blocking(unregister_role)?;

    // Check that Mouse doesn't have the root role
    let found_mouse_roles = test_client
        .query(FindRolesByAccountId::new(mouse_id.clone()))
        .execute_all()?;
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
        .submit_blocking(Register::role(role))
        .expect_err("Submitting role with non-existing permission should fail");

    let rejection_reason = err
        .downcast_ref::<TransactionRejectionReason>()
        .unwrap_or_else(|| panic!("Error {err} is not TransactionRejectionReason"));

    assert!(matches!(
        rejection_reason,
        &TransactionRejectionReason::Validation(ValidationFail::NotPermitted(_))
    ));

    Ok(())
}

#[test]
// NOTE: Permissions in this test are created explicitly as json strings
// so that they don't get deduplicated eagerly but rather in the executor
// This way, if the executor compares permissions just as JSON strings, the test will fail
fn role_permissions_are_deduplicated() {
    let Some((network, _rt)) = start_network(stringify!(role_permissions_are_deduplicated)) else {
        return;
    };
    let test_client = network.client();

    let allow_alice_to_transfer_rose_1 = Permission::new(
        "CanTransferAsset".parse().unwrap(),
        iroha_primitives::json::Json::new(
            norito::json::object([
                (
                    "asset",
                    norito::json::to_value(&"rose#wonderland#ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland")
                        .expect("serialize asset"),
                ),
            ])
            .expect("serialize permission payload"),
        ),
    );

    // Different content, but same meaning
    let allow_alice_to_transfer_rose_2 = Permission::new(
        "CanTransferAsset".parse().unwrap(),
        iroha_primitives::json::Json::new(
            norito::json::object([
                (
                    "asset",
                    norito::json::to_value(&"rose##ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland")
                        .expect("serialize asset"),
                ),
            ])
            .expect("serialize permission payload"),
        ),
    );

    let role_id: RoleId = "role_id".parse().expect("Valid");
    let role = Role::new(role_id.clone(), ALICE_ID.clone())
        .add_permission(allow_alice_to_transfer_rose_1)
        .add_permission(allow_alice_to_transfer_rose_2);

    test_client
        .submit_blocking(Register::role(role))
        .expect("failed to register role");

    let role = test_client
        .query(FindRoles::new())
        .execute_all()
        .expect("failed to find role")
        .into_iter()
        .find(|role| role.id() == &role_id)
        .expect("failed to find role");

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
    test_client.submit_blocking(register_mouse)?;

    // Registering role
    let role_id = "ACCESS_TO_MOUSE_METADATA".parse::<RoleId>()?;
    let role = Role::new(role_id.clone(), mouse_id.clone());
    let register_role = Register::role(role);
    test_client.submit_blocking(register_role)?;

    // Transfer domain ownership to Mouse
    let domain_id = "wonderland".parse::<DomainId>()?;
    let transfer_domain = Transfer::domain(alice_id.clone(), domain_id, mouse_id.clone());
    test_client.submit_blocking(transfer_domain)?;

    // Mouse grants role to Alice
    let grant_role = Grant::account_role(role_id.clone(), alice_id.clone());
    let grant_role_tx = TransactionBuilder::new(network.chain_id(), mouse_id.clone())
        .with_instructions([grant_role])
        .sign(mouse_keypair.private_key());
    test_client.submit_transaction_blocking(&grant_role_tx)?;

    let set_key_value =
        SetKeyValue::account(mouse_id.clone(), "key".parse()?, "value".parse::<Json>()?);
    let can_set_key_value_in_mouse = CanModifyAccountMetadata {
        account: mouse_id.clone(),
    };
    let grant_role_permission =
        Grant::role_permission(can_set_key_value_in_mouse.clone(), role_id.clone());
    let revoke_role_permission =
        Revoke::role_permission(can_set_key_value_in_mouse.clone(), role_id.clone());

    // Alice can't modify Mouse's metadata without proper permission
    assert!(
        !test_client
            .query(FindPermissionsByAccountId::new(alice_id.clone()))
            .execute_all()?
            .iter()
            .any(|permission| {
                CanModifyAccountMetadata::try_from(permission)
                    .is_ok_and(|permission| permission == can_set_key_value_in_mouse)
            })
    );
    let _ = test_client
        .submit_blocking(set_key_value.clone())
        .expect_err("shouldn't be able to modify metadata");

    // Alice can modify Mouse's metadata after permission is granted to role
    let grant_role_permission_tx = TransactionBuilder::new(network.chain_id(), mouse_id.clone())
        .with_instructions([grant_role_permission])
        .sign(mouse_keypair.private_key());
    test_client.submit_transaction_blocking(&grant_role_permission_tx)?;
    assert!(
        test_client
            .query(FindRolesByAccountId::new(alice_id.clone()))
            .execute_all()?
            .contains(&role_id)
    );
    test_client.submit_blocking(set_key_value.clone())?;

    // Alice can't modify Mouse's metadata after permission is removed from role
    let revoke_role_permission_tx = TransactionBuilder::new(network.chain_id(), mouse_id)
        .with_instructions([revoke_role_permission])
        .sign(mouse_keypair.private_key());
    test_client.submit_transaction_blocking(&revoke_role_permission_tx)?;
    assert!(
        !test_client
            .query(FindPermissionsByAccountId::new(alice_id.clone()))
            .execute_all()?
            .iter()
            .any(|permission| {
                CanModifyAccountMetadata::try_from(permission)
                    .is_ok_and(|permission| permission == can_set_key_value_in_mouse)
            })
    );
    let _ = test_client
        .submit_blocking(set_key_value)
        .expect_err("shouldn't be able to modify metadata");

    Ok(())
}

#[tokio::test]
async fn grant_unexisting_role_in_genesis_fail() {
    // Grant Alice UNEXISTING role
    let alice_id = ALICE_ID.clone();
    let role_id = "UNEXISTING".parse::<RoleId>().unwrap();
    let grant_genesis_role = Grant::account_role(role_id, alice_id);

    let Some(network) = sandbox::build_network_or_skip(
        NetworkBuilder::new()
            .with_min_peers(4)
            .with_genesis_instruction(grant_genesis_role),
        stringify!(grant_unexisting_role_in_genesis_fail),
    ) else {
        return;
    };
    let peer = network.peer();

    // Sanity-check stderr to ensure CI surfaces the expected genesis failure diagnostics (see #5423).
    let terminated_futs = network
        .peers()
        .iter()
        .map(|peer| peer.once(|e| matches!(e, PeerLifecycleEvent::Terminated { .. })))
        .collect::<Vec<_>>();
    for peer in network.peers() {
        if let Err(err) = peer
            .start(network.config_layers(), Some(&network.genesis()))
            .await
        {
            if let Some(reason) = integration_tests::sandbox::sandbox_reason(&err) {
                panic!(
                    "sandboxed network restriction detected while starting grant_unexisting_role_in_genesis_fail: {reason}"
                );
            }
            panic!("failed to start peer: {err:?}");
        }
    }
    timeout(Duration::from_secs(10), join_all(terminated_futs))
        .await
        .expect("must terminate immediately");

    let stderr_path = peer
        .latest_stderr_log_path()
        .expect("stderr log should be captured for failed genesis run");
    let stderr = fs::read_to_string(&stderr_path)
        .await
        .unwrap_or_else(|err| panic!("failed to read {stderr_path:?}: {err}"));
    let markers = [
        "confidential.enabled = true",
        "Failed IO operation",
        "Block confidential feature digest mismatch.",
        "Genesis transactions must not contain errors",
        "Genesis block execution failed",
        "Block contained no committed overlays",
    ];
    assert!(
        markers.iter().any(|needle| stderr.contains(needle)),
        "stderr should surface the genesis startup failure cause:\n{stderr}"
    );
}
