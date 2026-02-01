//! Integration tests ensuring executor-enforced permission policies.

use std::{
    io::ErrorKind,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use eyre::Result;
use integration_tests::{metrics::MetricsReader, sandbox};
use iroha::{
    client::Client,
    crypto::KeyPair,
    data_model::{
        permission::Permission, prelude::*, role::RoleId,
        transaction::error::TransactionRejectionReason,
    },
};
use iroha_data_model::isi::{register::RegisterBox, transfer::TransferBox};
use iroha_executor_data_model::permission::{
    account::CanModifyAccountMetadata, asset::CanTransferAsset, domain::CanModifyDomainMetadata,
    nft::CanModifyNftMetadata,
};
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR, gen_account_in};
use tokio::{
    runtime::Runtime,
    time::{sleep, timeout},
};

fn start_network(context: &'static str) -> Option<(sandbox::SerializedNetwork, Runtime)> {
    sandbox::start_network_blocking_or_skip(NetworkBuilder::new(), context).unwrap()
}

fn poll_detached_metrics(rt: &Runtime, metrics_url: &reqwest::Url) -> Result<(f64, f64, f64)> {
    let http = reqwest::Client::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut prepared_seen: f64 = 0.0;
    let mut merged_seen: f64 = 0.0;
    let mut fallback_seen: f64 = 0.0;

    while Instant::now() < deadline {
        let snapshot = rt.block_on(async {
            let response = http.get(metrics_url.clone()).send().await?;
            response.error_for_status()?.text().await
        })?;
        let metrics = MetricsReader::new(&snapshot);
        prepared_seen = prepared_seen.max(metrics.get("pipeline_detached_prepared"));
        merged_seen = merged_seen.max(metrics.get("pipeline_detached_merged"));
        fallback_seen = fallback_seen.max(metrics.get("pipeline_detached_fallback"));
        if merged_seen > 0.0 || fallback_seen > 0.0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    Ok((prepared_seen, merged_seen, fallback_seen))
}

async fn read_peer_log_with_retry(
    peer: &NetworkPeer,
    getter: impl Fn(&NetworkPeer) -> Option<PathBuf>,
) -> String {
    const MAX_ATTEMPTS: usize = 20;
    const DELAY: Duration = Duration::from_millis(50);

    let mut last = String::new();
    for attempt in 0..MAX_ATTEMPTS {
        if let Some(path) = getter(peer) {
            match tokio::fs::read_to_string(&path).await {
                Ok(content) => {
                    if !content.is_empty() {
                        return content;
                    }
                    last = content;
                }
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => panic!("failed to read log {}: {err}", path.display()),
            }
        }
        if attempt + 1 < MAX_ATTEMPTS {
            sleep(DELAY).await;
        }
    }
    last
}

#[test]
#[ignore = "debug helper for inspecting genesis transactions"]
fn debug_print_genesis_transactions() {
    let asset_definition_id = "xor#wonderland".parse().expect("Valid");
    let invalid_instruction =
        Register::asset_definition(AssetDefinition::numeric(asset_definition_id));
    let Some(network) = sandbox::build_network_or_skip(
        NetworkBuilder::new()
            .with_config_layer(|layer| {
                layer.write(["confidential", "enabled"], true);
            })
            .with_genesis_instruction(invalid_instruction),
        stringify!(debug_print_genesis_transactions),
    ) else {
        return;
    };
    let genesis = network.genesis();
    for (tx_idx, tx) in genesis.0.transactions_vec().iter().enumerate() {
        println!("tx #{tx_idx}");
        if let Executable::Instructions(isi) = tx.instructions() {
            for (i, instr) in isi.iter().enumerate() {
                println!("  instr #{i}: {}", instr.id());
                println!("    type {}", std::any::type_name_of_val(&**instr));
                if let Some(reg_box) = instr.as_any().downcast_ref::<RegisterBox>() {
                    match reg_box {
                        RegisterBox::Peer(_) => println!("    register peer"),
                        RegisterBox::Domain(domain) => {
                            println!("    register domain {}", domain.object().id());
                        }
                        RegisterBox::Account(account) => {
                            println!("    register account {}", account.object().id());
                        }
                        RegisterBox::AssetDefinition(asset) => {
                            println!("    register asset definition {}", asset.object().id());
                        }
                        RegisterBox::Nft(nft) => {
                            println!("    register nft {}", nft.object().id());
                        }
                        RegisterBox::Role(role) => {
                            println!("    register role {}", role.object().id());
                        }
                        RegisterBox::Trigger(_) => println!("    register trigger"),
                    }
                }
                if let Some(TransferBox::Domain(transfer)) =
                    instr.as_any().downcast_ref::<TransferBox>()
                {
                    println!(
                        "    transfer domain {} -> {}",
                        transfer.object(),
                        transfer.destination()
                    );
                }
            }
        }
    }
    panic!("debug output above");
}

#[tokio::test]
async fn genesis_transactions_are_validated_by_executor() {
    // `wonderland` domain is owned by Alice,
    //  so the default executor will deny a genesis account to register asset definition.
    let asset_definition_id = "xor#wonderland".parse().expect("Valid");
    let invalid_instruction =
        Register::asset_definition(AssetDefinition::numeric(asset_definition_id));
    let Some(network) = sandbox::build_network_or_skip(
        NetworkBuilder::new()
            .with_config_layer(|layer| {
                layer.write(["confidential", "enabled"], true);
            })
            .with_genesis_instruction(invalid_instruction),
        stringify!(genesis_transactions_are_validated_by_executor),
    ) else {
        return;
    };
    // Build `irohad` ahead of the timeout so the initial binary compilation does not consume the limit.
    iroha_test_network::Program::Irohad
        .resolve()
        .expect("irohad binary should be buildable");
    let genesis = network.genesis();
    let peer = network.peer();

    let termination = Arc::new(Mutex::new(None));
    let termination_wait = {
        let capture = Arc::clone(&termination);
        peer.once(move |event| match event {
            PeerLifecycleEvent::Terminated { status } => {
                *capture.lock().expect("termination mutex poisoned") = Some(status);
                true
            }
            _ => false,
        })
    };

    // Start the peer; skip when sandbox restrictions prevent binding sockets.
    if let Err(err) = peer.start(network.config_layers(), Some(&genesis)).await {
        if let Some(reason) = integration_tests::sandbox::sandbox_reason(&err) {
            panic!(
                "sandboxed network restriction detected while starting genesis_transactions_are_validated_by_executor: {reason}"
            );
        }
        panic!("failed to start peer: {err:?}");
    }

    timeout(Duration::from_secs(5), termination_wait)
        .await
        .expect("peer should panic within timeout");

    let status = termination
        .lock()
        .expect("termination mutex poisoned")
        .expect("termination status captured");
    assert!(
        !status.success(),
        "expected invalid genesis to exit with non-zero status, got {status:?}"
    );
    if let Some(code) = status.code() {
        assert_eq!(
            code, 1,
            "invalid genesis should exit with status code 1, got {code}"
        );
    }

    let stderr_log = read_peer_log_with_retry(peer, NetworkPeer::latest_stderr_log_path).await;
    let stdout_log = read_peer_log_with_retry(peer, NetworkPeer::latest_stdout_log_path).await;
    let mentions_invalid_genesis = stderr_log.contains("Invalid genesis block")
        || stdout_log.contains("Invalid genesis block")
        || stderr_log.contains("genesis consensus metadata validation failed")
        || stdout_log.contains("genesis consensus metadata validation failed");
    assert!(
        mentions_invalid_genesis,
        "logs should mention genesis validation failure; stdout was:\n{stdout_log}\nstderr was:\n{stderr_log}"
    );
}

fn get_assets(iroha: &Client, id: &AccountId) -> Vec<Asset> {
    iroha
        .query(FindAssets::new())
        .execute_all()
        .expect("Failed to execute request.")
        .into_iter()
        .filter(|asset| asset.id().account() == id)
        .collect()
}

#[test]
fn permissions_disallow_asset_transfer() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: permissions negative test gated (#2851). Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

    let Some((network, _rt)) = start_network(stringify!(permissions_disallow_asset_transfer))
    else {
        return;
    };
    let iroha = network.client();

    // Given
    let alice_id = ALICE_ID.clone();
    let bob_id = BOB_ID.clone();
    let (mouse_id, _mouse_keypair) = gen_account_in("wonderland");
    let asset_definition_id: AssetDefinitionId = "xor#wonderland".parse().expect("Valid");
    let create_asset =
        Register::asset_definition(AssetDefinition::numeric(asset_definition_id.clone()));
    let mouse_keypair = KeyPair::random();

    let alice_start_assets = get_assets(&iroha, &alice_id);
    iroha
        .submit_blocking(create_asset)
        .expect("Failed to prepare state.");

    let quantity = numeric!(200);
    let mint_asset = Mint::asset_numeric(
        quantity.clone(),
        AssetId::new(asset_definition_id.clone(), bob_id.clone()),
    );
    iroha
        .submit_blocking(mint_asset)
        .expect("Failed to create asset.");

    //When
    let transfer_asset = Transfer::asset_numeric(
        AssetId::new(asset_definition_id, bob_id),
        quantity,
        alice_id.clone(),
    );
    let transfer_tx = TransactionBuilder::new(chain_id, mouse_id)
        .with_instructions([transfer_asset])
        .sign(mouse_keypair.private_key());
    let err = iroha
        .submit_transaction_blocking(&transfer_tx)
        .expect_err("Transaction was not rejected.");
    let rejection_reason = err
        .downcast_ref::<TransactionRejectionReason>()
        .expect("Error {err} is not TransactionRejectionReason");
    //Then
    assert!(matches!(
        rejection_reason,
        &TransactionRejectionReason::Validation(ValidationFail::NotPermitted(_))
    ));
    let alice_assets = get_assets(&iroha, &alice_id);
    assert_eq!(alice_assets, alice_start_assets);
}

#[test]
fn account_permission_revoke_then_grant_last_wins_detached() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_default_pipeline_time()
        .with_config_layer(|layer| {
            layer
                .write(["pipeline", "parallel_overlay"], true)
                .write(["pipeline", "parallel_apply"], true)
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full");
        });
    let Some((network, rt)) = sandbox::start_network_blocking_or_skip(
        builder,
        stringify!(account_permission_revoke_then_grant_last_wins_detached),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    let metrics_url = client.torii_url.join("/metrics")?;

    let (mouse_id, _mouse_keypair) = gen_account_in("wonderland");
    client.submit_blocking(Register::account(Account::new(mouse_id.clone())))?;

    let perm: Permission = CanModifyAccountMetadata {
        account: mouse_id.clone(),
    }
    .into();
    client.submit_blocking(Grant::account_permission(perm.clone(), ALICE_ID.clone()))?;

    let revoke = Revoke::account_permission(perm.clone(), ALICE_ID.clone());
    let grant = Grant::account_permission(perm.clone(), ALICE_ID.clone());
    let tx = TransactionBuilder::new(network.chain_id(), ALICE_ID.clone())
        .with_instructions([InstructionBox::from(revoke), InstructionBox::from(grant)])
        .sign(ALICE_KEYPAIR.private_key());
    client.submit_transaction_blocking(&tx)?;

    let (prepared_seen, merged_seen, fallback_seen) = poll_detached_metrics(&rt, &metrics_url)?;

    let permissions = client
        .query(FindPermissionsByAccountId::new(ALICE_ID.clone()))
        .execute_all()?;
    assert!(
        permissions.iter().any(|permission| permission == &perm),
        "last grant should keep permission on account"
    );

    assert!(
        prepared_seen > 0.0,
        "expected detached pipeline to prepare overlays"
    );
    assert!(
        merged_seen > 0.0,
        "expected detached merge to register in metrics"
    );
    assert_eq!(
        fallback_seen, 0.0,
        "detached fallback should not register for permission ops"
    );

    Ok(())
}

#[test]
fn permissions_disallow_asset_burn() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: permissions negative test gated (#2851). Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

    let Some((network, _rt)) = start_network(stringify!(permissions_disallow_asset_burn)) else {
        return;
    };
    let iroha = network.client();

    let alice_id = ALICE_ID.clone();
    let bob_id = BOB_ID.clone();
    let (mouse_id, _mouse_keypair) = gen_account_in("wonderland");
    let asset_definition_id = "xor#wonderland"
        .parse::<AssetDefinitionId>()
        .expect("Valid");
    let create_asset =
        Register::asset_definition(AssetDefinition::numeric(asset_definition_id.clone()));
    let mouse_keypair = KeyPair::random();

    let alice_start_assets = get_assets(&iroha, &alice_id);

    iroha
        .submit_blocking(create_asset)
        .expect("Failed to prepare state.");

    let quantity = numeric!(200);
    let mint_asset = Mint::asset_numeric(
        quantity.clone(),
        AssetId::new(asset_definition_id.clone(), bob_id),
    );
    iroha
        .submit_blocking(mint_asset)
        .expect("Failed to create asset.");
    let burn_asset = Burn::asset_numeric(
        quantity,
        AssetId::new(asset_definition_id, mouse_id.clone()),
    );
    let burn_tx = TransactionBuilder::new(chain_id, mouse_id)
        .with_instructions([burn_asset])
        .sign(mouse_keypair.private_key());

    let err = iroha
        .submit_transaction_blocking(&burn_tx)
        .expect_err("Transaction was not rejected.");
    let rejection_reason = err
        .downcast_ref::<TransactionRejectionReason>()
        .expect("Error {err} is not TransactionRejectionReason");

    assert!(matches!(
        rejection_reason,
        &TransactionRejectionReason::Validation(ValidationFail::NotPermitted(_))
    ));

    let alice_assets = get_assets(&iroha, &alice_id);
    assert_eq!(alice_assets, alice_start_assets);
}

#[test]
fn account_can_query_only_its_own_domain() -> Result<()> {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: permissions domain query test gated (#2851). Set IROHA_RUN_IGNORED=1 to run."
        );
        return Ok(());
    }
    let Some((network, _rt)) = start_network(stringify!(account_can_query_only_its_own_domain))
    else {
        return Ok(());
    };
    let client = network.client();

    // Given
    let domain_id: DomainId = "wonderland".parse()?;
    let new_domain_id: DomainId = "wonderland2".parse()?;
    let register_domain = Register::domain(Domain::new(new_domain_id.clone()));

    client.submit_blocking(register_domain)?;

    // Alice can query the domain in which her account exists.
    assert!(
        client
            .query(FindDomains::new())
            .execute_all()
            .unwrap()
            .into_iter()
            .any(|domain| domain.id() == &domain_id)
    );

    // Alice cannot query other domains.
    assert!(
        !client
            .query(FindDomains::new())
            .execute_all()
            .unwrap()
            .into_iter()
            .any(|domain| domain.id() == &new_domain_id)
    );
    Ok(())
}

#[test]
fn permissions_differ_not_only_by_names() {
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

    let Some((network, _rt)) = start_network(stringify!(permissions_differ_not_only_by_names))
    else {
        return;
    };
    let client = network.client();

    let submit_with_authority = |isi: InstructionBox,
                                 authority: &AccountId,
                                 authority_keypair: &KeyPair|
     -> Result<HashOf<SignedTransaction>> {
        let tx = TransactionBuilder::new(chain_id.clone(), authority.clone())
            .with_instructions([isi])
            .sign(authority_keypair.private_key());
        client.submit_transaction_blocking(&tx)
    };

    let alice_id = ALICE_ID.clone();
    let bob_id = BOB_ID.clone();
    let bob_keypair = BOB_KEYPAIR.clone();
    let (mouse_id, mouse_keypair) = gen_account_in("outfit");

    // Registering mouse
    let outfit_domain: DomainId = "outfit".parse().unwrap();
    let create_outfit_domain = Register::domain(Domain::new(outfit_domain.clone()));
    let register_mouse_account = Register::account(Account::new(mouse_id.clone()));
    client
        .submit_all_blocking::<InstructionBox>([
            create_outfit_domain.into(),
            register_mouse_account.into(),
        ])
        .expect("Failed to register mouse");

    // Registering NFT
    let hat_nft_id: NftId = "hat$outfit".parse().expect("Valid");
    let register_hat_nft = Register::nft(Nft::new(hat_nft_id.clone(), Metadata::default()));
    let transfer_shoes_domain = Transfer::domain(alice_id.clone(), outfit_domain, mouse_id.clone());
    let shoes_nft_id: NftId = "shoes$outfit".parse().expect("Valid");
    let register_shoes_nft = Register::nft(Nft::new(shoes_nft_id.clone(), Metadata::default()));
    client
        .submit_all_blocking::<InstructionBox>([
            register_hat_nft.into(),
            register_shoes_nft.into(),
            transfer_shoes_domain.into(),
        ])
        .expect("Failed to register new NFTs");

    // Granting permission to Bob to modify metadata in Mouse's hats
    let mouse_hat_permission = CanModifyNftMetadata {
        nft: hat_nft_id.clone(),
    };
    let allow_bob_to_set_key_value_in_hats =
        Grant::account_permission(mouse_hat_permission, bob_id.clone());

    submit_with_authority(
        allow_bob_to_set_key_value_in_hats.into(),
        &mouse_id,
        &mouse_keypair,
    )
    .expect("Failed grant permission to modify Mouse's hats");

    // Checking that Bob can modify Mouse's hats ...
    let set_hat_color = SetKeyValue::nft(
        hat_nft_id,
        "color".parse().expect("Valid"),
        "red".parse::<Json>().expect("Valid"),
    );
    submit_with_authority(set_hat_color.into(), &bob_id, &bob_keypair)
        .expect("Failed to modify Mouse's hats");

    // ... but not shoes
    let set_shoes_color = SetKeyValue::nft(
        shoes_nft_id.clone(),
        "color".parse().expect("Valid"),
        "yellow".parse::<Json>().expect("Valid"),
    );
    let _err = submit_with_authority(set_shoes_color.clone().into(), &bob_id, &bob_keypair)
        .expect_err("Expected Bob to fail to modify Mouse's shoes");

    let mouse_shoes_permission = CanModifyNftMetadata { nft: shoes_nft_id };
    let allow_bob_to_set_key_value_in_shoes =
        Grant::account_permission(mouse_shoes_permission, bob_id.clone());

    submit_with_authority(
        allow_bob_to_set_key_value_in_shoes.into(),
        &mouse_id,
        &mouse_keypair,
    )
    .expect("Failed grant permission to modify Mouse's shoes");

    // Checking that Bob can modify Mouse's shoes
    submit_with_authority(set_shoes_color.into(), &bob_id, &bob_keypair)
        .expect("Failed to modify Mouse's shoes");
}

#[test]
fn stored_vs_granted_permission_payload() {
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

    let Some((network, _rt)) = start_network(stringify!(stored_vs_granted_permission_payload))
    else {
        return;
    };
    let iroha = network.client();

    // Given
    let alice_id = ALICE_ID.clone();

    // Registering mouse and asset definition
    let asset_definition_id: AssetDefinitionId = "xor#wonderland".parse().expect("Valid");
    let create_asset =
        Register::asset_definition(AssetDefinition::numeric(asset_definition_id.clone()));
    let (mouse_id, mouse_keypair) = gen_account_in("wonderland");
    let register_mouse_account = Register::account(Account::new(mouse_id.clone()));
    iroha
        .submit_all_blocking::<InstructionBox>([register_mouse_account.into(), create_asset.into()])
        .expect("Failed to register mouse");

    // Allow alice to mint mouse asset and mint initial value
    let value_json = Json::from_string_unchecked(format!(
        // NOTE: Permissions is created explicitly as a json string to introduce additional whitespace
        // This way, if the executor compares permissions just as JSON strings, the test will fail
        r#"{{ "asset"   :   "xor#wonderland#{mouse_id}" }}"#
    ));

    let mouse_asset = AssetId::new(asset_definition_id, mouse_id.clone());
    let allow_alice_to_mint_mouse_asset = Grant::account_permission(
        Permission::new("CanMintAsset".parse().unwrap(), value_json),
        alice_id,
    );

    let transaction = TransactionBuilder::new(chain_id, mouse_id)
        .with_instructions([allow_alice_to_mint_mouse_asset])
        .sign(mouse_keypair.private_key());
    iroha
        .submit_transaction_blocking(&transaction)
        .expect("Failed to grant permission to alice.");

    // Check that alice can indeed mint mouse asset
    let mint_asset = Mint::asset_numeric(1_u32, mouse_asset);
    iroha
        .submit_blocking(mint_asset)
        .expect("Failed to mint asset for mouse.");
}

#[test]
fn permissions_are_unified() {
    let Some((network, _rt)) = start_network(stringify!(permissions_are_unified)) else {
        return;
    };
    let iroha = network.client();

    // Given
    let alice_id = ALICE_ID.clone();

    let permission1 = CanTransferAsset {
        asset: format!("rose#wonderland#{alice_id}").parse().unwrap(),
    };
    let allow_alice_to_transfer_rose_1 = Grant::account_permission(permission1, alice_id.clone());

    let permission2 = CanTransferAsset {
        asset: format!("rose##{alice_id}").parse().unwrap(),
    };
    let allow_alice_to_transfer_rose_2 = Grant::account_permission(permission2, alice_id);

    iroha
        .submit_blocking(allow_alice_to_transfer_rose_1)
        .expect("failed to grant permission");

    let _ = iroha
        .submit_blocking(allow_alice_to_transfer_rose_2)
        .expect_err("should reject due to duplication");
}

#[test]
fn associated_permissions_removed_on_unregister() {
    let Some((network, _rt)) =
        start_network(stringify!(associated_permissions_removed_on_unregister))
    else {
        return;
    };
    let iroha = network.client();

    let bob_id = BOB_ID.clone();
    let kingdom_id: DomainId = "kingdom".parse().expect("Valid");
    let kingdom = Domain::new(kingdom_id.clone());

    // register kingdom and give bob permissions in this domain
    let register_domain = Register::domain(kingdom);
    let bob_to_set_kv_in_domain = CanModifyDomainMetadata {
        domain: kingdom_id.clone(),
    };
    let allow_bob_to_set_kv_in_domain =
        Grant::account_permission(bob_to_set_kv_in_domain.clone(), bob_id.clone());

    iroha
        .submit_all_blocking::<InstructionBox>([
            register_domain.into(),
            allow_bob_to_set_kv_in_domain.into(),
        ])
        .expect("failed to register domain and grant permission");

    // check that bob indeed have granted permission
    assert!(
        iroha
            .query(FindPermissionsByAccountId::new(bob_id.clone()))
            .execute_all()
            .expect("failed to get permissions for bob")
            .into_iter()
            .any(|permission| {
                CanModifyDomainMetadata::try_from(&permission)
                    .is_ok_and(|permission| permission == bob_to_set_kv_in_domain)
            })
    );

    // unregister kingdom
    iroha
        .submit_blocking(Unregister::domain(kingdom_id))
        .expect("failed to unregister domain");

    // check that permission is removed from bob
    assert!(
        !iroha
            .query(FindPermissionsByAccountId::new(bob_id))
            .execute_all()
            .expect("failed to get permissions for bob")
            .into_iter()
            .any(|permission| {
                CanModifyDomainMetadata::try_from(&permission)
                    .is_ok_and(|permission| permission == bob_to_set_kv_in_domain)
            })
    );
}

#[test]
fn associated_permissions_removed_from_role_on_unregister() {
    let Some((network, _rt)) = start_network(stringify!(
        associated_permissions_removed_from_role_on_unregister
    )) else {
        return;
    };
    let iroha = network.client();

    let role_id: RoleId = "role".parse().expect("Valid");
    let kingdom_id: DomainId = "kingdom".parse().expect("Valid");
    let kingdom = Domain::new(kingdom_id.clone());

    // register kingdom and give bob permissions in this domain
    let register_domain = Register::domain(kingdom);
    let set_kv_in_domain = CanModifyDomainMetadata {
        domain: kingdom_id.clone(),
    };
    let register_role = Register::role(
        Role::new(role_id.clone(), ALICE_ID.clone()).add_permission(set_kv_in_domain.clone()),
    );

    iroha
        .submit_all_blocking::<InstructionBox>([register_domain.into(), register_role.into()])
        .expect("failed to register domain and grant permission");

    // check that role indeed have permission
    assert!(
        iroha
            .query(FindRoles::new())
            .execute_all()
            .unwrap()
            .into_iter()
            .find(|role| role.id() == &role_id)
            .expect("failed to get role")
            .permissions()
            .any(|permission| {
                CanModifyDomainMetadata::try_from(permission)
                    .is_ok_and(|permission| permission == set_kv_in_domain)
            })
    );

    // unregister kingdom
    iroha
        .submit_blocking(Unregister::domain(kingdom_id))
        .expect("failed to unregister domain");

    // check that permission is removed from role
    assert!(
        !iroha
            .query(FindRoles::new())
            .execute_all()
            .unwrap()
            .into_iter()
            .find(|role| role.id() == &role_id)
            .expect("failed to get role")
            .permissions()
            .any(|permission| {
                CanModifyDomainMetadata::try_from(permission)
                    .is_ok_and(|permission| permission == set_kv_in_domain)
            })
    );
}
