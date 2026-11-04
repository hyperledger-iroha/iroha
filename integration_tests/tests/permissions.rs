//! Integration tests ensuring executor-enforced permission policies.
use eyre::Result;
use integration_tests::{
    collections, metrics::MetricsReader, sandbox, sync::sync_after_submission,
};
use iroha::{
    blocking::Client,
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
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_test_network::*;
use iroha_test_samples::{
    ALICE_ID, BOB_ID, BOB_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_ID, gen_account_in,
};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
fn start_network(context: &'static str) -> Option<(sandbox::SerializedNetwork, Runtime)> {
    sandbox::start_network_blocking_or_skip(NetworkBuilder::new(), context).unwrap()
}
fn start_network_with_builder(
    builder: NetworkBuilder,
    context: &'static str,
) -> Option<(sandbox::SerializedNetwork, Runtime)> {
    sandbox::start_network_blocking_or_skip(builder, context).unwrap()
}
fn poll_detached_metrics(rt: &Runtime, metrics_url: &reqwest::Url) -> Result<(f64, f64, f64)> {
    let http = integration_tests::http::client();
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
fn wait_for_role_permission_state(
    client: &Client,
    role_id: &RoleId,
    permission: &Permission,
    expected_present: bool,
    context: &str,
) {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);
    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "role was not queried".to_owned();
    while Instant::now() < deadline {
        match collections::roles(client) {
            Ok(roles) => {
                if let Some(role) = roles.into_iter().find(|role| role.id() == role_id) {
                    let present = role
                        .permissions()
                        .any(|role_permission| role_permission == permission);
                    last_observed = format!("role exists; permission_present={present}");
                    if present == expected_present {
                        return;
                    }
                } else {
                    "role not found".clone_into(&mut last_observed);
                }
            }
            Err(err) => {
                last_observed = format!("query failed: {err}");
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    panic!(
        "timed out waiting for role permission state after {context}; expected_present={expected_present}; last_observed={last_observed}"
    );
}
fn wait_for_account_permission_state(
    client: &Client,
    account_id: &AccountId,
    permission: &Permission,
    expected_present: bool,
    context: &str,
) {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);
    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "account permissions were not queried".to_owned();
    while Instant::now() < deadline {
        match collections::permissions(client, account_id) {
            Ok(permissions) => {
                let present = permissions
                    .into_iter()
                    .any(|account_permission| &account_permission == permission);
                last_observed = format!("permission_present={present}");
                if present == expected_present {
                    return;
                }
            }
            Err(err) => {
                last_observed = format!("query failed: {err}");
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    panic!(
        "timed out waiting for account permission state after {context}; expected_present={expected_present}; last_observed={last_observed}"
    );
}
#[test]
#[ignore = "debug helper for inspecting genesis transactions"]
fn debug_print_genesis_transactions() {
    let kingdom_id: DomainId = DomainId::try_new("kingdom", "universal").expect("Valid");
    let register_domain = Register::domain(Domain::new(kingdom_id.clone()));
    let duplicate_domain = Register::domain(Domain::new(kingdom_id.clone()));
    let Some(network) = sandbox::build_network_or_skip(
        NetworkBuilder::new()
            .with_config_layer(|layer| {
                layer.write(["confidential", "enabled"], true);
            })
            .with_genesis_instruction(register_domain)
            .with_genesis_instruction(duplicate_domain),
        stringify!(debug_print_genesis_transactions),
    ) else {
        return;
    };
    let genesis = network.genesis();
    for (tx_idx, tx) in genesis.0.external_transactions().enumerate() {
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
    // Preparation now executes genesis before any peer can start. Inspect the
    // native typed refusal at that owner, then prepare the one-registration control.
    let kingdom_id = DomainId::try_new("kingdom", "universal").expect("fixture domain");
    let builder = NetworkBuilder::new().with_peers(4).with_genesis_block(
        move |topology, topology_entries| {
            let register = InstructionBox::from(Register::domain(Domain::new(kingdom_id.clone())));
            let error = try_genesis_factory(
                vec![vec![register.clone(), register.clone()]],
                topology.clone(),
                topology_entries.clone(),
            )
            .expect_err("the native executor must reject duplicate domain registration");
            let rejection = error
                .chain()
                .find_map(|cause| cause.downcast_ref::<iroha_core::block::GenesisOutputRejection>())
                .unwrap_or_else(|| panic!("expected a native genesis output rejection: {error:?}"));
            let expected =
                TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
                    iroha_data_model::isi::error::InstructionExecutionError::Repetition(
                        iroha_data_model::isi::error::RepetitionError {
                            instruction: iroha_data_model::isi::InstructionType::Register,
                            id: kingdom_id.clone().into(),
                        },
                    ),
                ));
            assert_eq!(rejection.reason.as_ref(), &expected);
            try_genesis_factory(vec![vec![register]], topology, topology_entries)
                .expect("one registration must prepare through the same native validator")
        },
    );
    let Some(network) = sandbox::build_network_or_skip(
        builder,
        stringify!(genesis_transactions_are_validated_by_executor),
    ) else {
        return;
    };
    assert!(network.genesis().0.has_results());
}

fn get_assets(client: &Client, id: &AccountId) -> Vec<norito::json::Value> {
    let rows = client
        .list(
            iroha::collections::Collection::AccountAssets(id.clone()),
            iroha::collections::ListQuery::new(),
        )
        .collect::<iroha::Result<Vec<_>>>()
        .expect("Failed to execute asset collection request");
    assert!(
        rows.iter()
            .all(|row| collections::field::<AccountId>(row, "account_id").unwrap() == *id)
    );
    rows
}
#[test]
fn permissions_disallow_asset_transfer() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: permissions negative test gated (#2851). Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let Some((network, _rt)) = start_network(stringify!(permissions_disallow_asset_transfer))
    else {
        return;
    };
    let network_id = network.network_id();
    let iroha = network.client();
    // Given
    let alice_id = ALICE_ID.clone();
    let bob_id = BOB_ID.clone();
    let (mouse_id, _mouse_keypair) = gen_account_in("wonderland");
    let asset_definition_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("Valid"),
        "xor".parse().expect("Valid"),
    );
    let create_asset = Register::asset_definition(AssetDefinition::numeric(
        asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    ));
    let mouse_keypair = KeyPair::random();
    let alice_start_assets = get_assets(&iroha, &alice_id);
    iroha
        .submit(
            create_asset,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("Failed to prepare state.");
    let quantity = Quantity::from(200_u32);
    let mint_asset = Mint::asset_quantity(
        quantity.clone(),
        AssetId::new(asset_definition_id.clone(), bob_id.clone()),
    );
    iroha
        .submit(
            mint_asset,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("Failed to create asset.");
    //When
    let transfer_asset = Transfer::asset_quantity(
        AssetId::new(asset_definition_id, bob_id),
        quantity,
        alice_id.clone(),
    );
    let transfer_tx = TransactionBuilder::new(
        network_id,
        mouse_id,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([transfer_asset])
    .sign(mouse_keypair.private_key());
    let err = iroha
        .submit_transaction_and_wait(&transfer_tx)
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
        .with_default_block_cadence()
        .with_config_layer(|layer| {
            layer
                .write(["pipeline", "parallel_overlay"], true)
                .write(["pipeline", "parallel_apply"], true)
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
    let metrics_url = client.client().endpoint().join("/metrics")?;
    let mut status = client.status().get()?;
    let mut last_non_empty_height = status.blocks_non_empty;
    let (mouse_id, _mouse_keypair) = gen_account_in("wonderland");
    client.submit(
        Register::account(Account::new(mouse_id.clone())),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    status = sync_after_submission(
        &network,
        &rt,
        &client,
        last_non_empty_height,
        "register detached permission target account",
    )?;
    last_non_empty_height = status.blocks_non_empty;
    let perm: Permission = CanModifyAccountMetadata {
        account: ALICE_ID.clone(),
    }
    .into();
    client.submit(
        Grant::account_permission(perm.clone(), mouse_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let _status = sync_after_submission(
        &network,
        &rt,
        &client,
        last_non_empty_height,
        "seed detached account permission",
    )?;
    let revoke = Revoke::account_permission(perm.clone(), mouse_id.clone());
    let grant = Grant::account_permission(perm.clone(), mouse_id.clone());
    client.submit_all(
        [InstructionBox::from(revoke), InstructionBox::from(grant)],
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let (prepared_seen, merged_seen, fallback_seen) = poll_detached_metrics(&rt, &metrics_url)?;
    let permissions = collections::permissions(&client, &mouse_id)?;
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
    let Some((network, _rt)) = start_network(stringify!(permissions_disallow_asset_burn)) else {
        return;
    };
    let network_id = network.network_id();
    let iroha = network.client();
    let alice_id = ALICE_ID.clone();
    let bob_id = BOB_ID.clone();
    let (mouse_id, _mouse_keypair) = gen_account_in("wonderland");
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("Valid"),
        "xor".parse().expect("Valid"),
    );
    let create_asset = Register::asset_definition(AssetDefinition::numeric(
        asset_definition_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    ));
    let mouse_keypair = KeyPair::random();
    let alice_start_assets = get_assets(&iroha, &alice_id);
    iroha
        .submit(
            create_asset,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("Failed to prepare state.");
    let quantity = Quantity::from(200_u32);
    let mint_asset = Mint::asset_quantity(
        quantity.clone(),
        AssetId::new(asset_definition_id.clone(), bob_id),
    );
    iroha
        .submit(
            mint_asset,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("Failed to create asset.");
    let burn_asset = Burn::asset_quantity(
        quantity,
        AssetId::new(asset_definition_id, mouse_id.clone()),
    );
    let burn_tx = TransactionBuilder::new(
        network_id,
        mouse_id,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([burn_asset])
    .sign(mouse_keypair.private_key());
    let err = iroha
        .submit_transaction_and_wait(&burn_tx)
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
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal")?;
    let new_domain_id: DomainId = DomainId::try_new("wonderland2", "universal")?;
    let Some((network, _rt)) = start_network_with_builder(
        NetworkBuilder::new()
            .with_genesis_instruction(Register::domain(Domain::new(new_domain_id.clone()))),
        stringify!(account_can_query_only_its_own_domain),
    ) else {
        return Ok(());
    };
    let client = network.client();
    // Alice can query the domain in which her account exists.
    assert!(
        client
            .client()
            .query(FindDomains::new())
            .execute_all()
            .unwrap()
            .into_iter()
            .any(|domain| domain.id() == &domain_id)
    );
    // Alice cannot query other domains.
    assert!(
        !client
            .client()
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
    let outfit_domain: DomainId = DomainId::try_new("outfit", "universal").expect("Valid");
    let Some((network, _rt)) = start_network_with_builder(
        NetworkBuilder::new(),
        stringify!(permissions_differ_not_only_by_names),
    ) else {
        return;
    };
    let client = network.client();
    submit_ensure_domain_for_network(&network, &client, Domain::new(outfit_domain.clone()))
        .expect("Failed to register outfit domain");
    let submit_with_authority = |isi: InstructionBox,
                                 authority: &AccountId,
                                 authority_keypair: &KeyPair|
     -> Result<HashOf<SignedTransaction>> {
        let authority_client = integration_tests::sync::rebind_blocking_client(&client, |client| {
            client.account = authority.clone();
            client.key_pair = authority_keypair.clone();
        });
        authority_client.submit(
            isi,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
    };
    let alice_id = ALICE_ID.clone();
    let bob_id = BOB_ID.clone();
    let bob_keypair = BOB_KEYPAIR.clone();
    let (mouse_id, mouse_keypair) = gen_account_in("outfit");
    // Registering mouse
    let register_mouse_account = Register::account(Account::new(mouse_id.clone()));
    let fee_asset_definition = iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
        .parse::<AssetDefinitionId>()
        .expect("canonical fixture fee asset");
    let fund_mouse = Transfer::asset_quantity(
        AssetId::new(fee_asset_definition, ALICE_ID.clone()),
        100u32,
        mouse_id.clone(),
    );
    client
        .submit_all::<InstructionBox>(
            [register_mouse_account.into(), fund_mouse.into()],
            iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("Failed to register mouse");
    // Registering NFT
    let hat_nft_id = NftId::new(
        DomainId::try_new("outfit", "universal").expect("Valid"),
        "hat".parse().expect("Valid"),
    );
    let register_hat_nft = Register::nft(Nft::new(hat_nft_id.clone(), Metadata::default()));
    let transfer_shoes_domain = Transfer::domain(alice_id.clone(), outfit_domain, mouse_id.clone());
    let shoes_nft_id = NftId::new(
        DomainId::try_new("outfit", "universal").expect("Valid"),
        "shoes".parse().expect("Valid"),
    );
    let register_shoes_nft = Register::nft(Nft::new(shoes_nft_id.clone(), Metadata::default()));
    client
        .submit_all::<InstructionBox>(
            [
                register_hat_nft.into(),
                register_shoes_nft.into(),
                transfer_shoes_domain.into(),
            ],
            iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
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
        Json::from("red"),
    );
    submit_with_authority(set_hat_color.into(), &bob_id, &bob_keypair)
        .expect("Failed to modify Mouse's hats");
    // ... but not shoes
    let set_shoes_color = SetKeyValue::nft(
        shoes_nft_id.clone(),
        "color".parse().expect("Valid"),
        Json::from("yellow"),
    );
    let err = submit_with_authority(set_shoes_color.clone().into(), &bob_id, &bob_keypair)
        .expect_err("Expected Bob to fail to modify Mouse's shoes");
    assert!(
        matches!(
            err.downcast_ref::<TransactionRejectionReason>(),
            Some(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(_)
            ))
        ),
        "the distinct NFT permission must cause the refusal: {err:?}"
    );
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
    let Some((network, _rt)) = start_network(stringify!(stored_vs_granted_permission_payload))
    else {
        return;
    };
    let iroha = network.client();
    // Given
    let alice_id = ALICE_ID.clone();
    // Registering mouse and asset definition
    let asset_definition_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("Valid"),
        "xor".parse().expect("Valid"),
    );
    let create_asset = Register::asset_definition({
        let __asset_definition_id = asset_definition_id.clone();
        AssetDefinition::numeric(
            __asset_definition_id.clone(),
            "xor".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(DomainId::try_new("wonderland", "universal").expect("fixture domain")),
        )
    });
    let (mouse_id, mouse_keypair) = gen_account_in("wonderland");
    let register_mouse_account = Register::account(Account::new(mouse_id.clone()));
    let fee_asset_definition = iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
        .parse::<AssetDefinitionId>()
        .expect("canonical fixture fee asset");
    let fund_mouse = Transfer::asset_quantity(
        AssetId::new(fee_asset_definition, ALICE_ID.clone()),
        100u32,
        mouse_id.clone(),
    );
    iroha
        .submit_all::<InstructionBox>(
            [
                register_mouse_account.into(),
                create_asset.into(),
                fund_mouse.into(),
            ],
            iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("Failed to register mouse");
    let mouse_asset = AssetId::new(asset_definition_id.clone(), mouse_id.clone());
    // The exact mint permission is rooted in the asset-definition authority, not in the
    // destination account that happens to hold the asset instance.
    let value_json = Json::from_raw_json(format!(
        r#"{{"account":"{mouse_id}","asset_definition":"{asset_definition_id}"}}"#
    ))
    .expect("valid permission JSON fixture");
    let allow_alice_to_mint_mouse_asset = Grant::account_permission(
        Permission::new("CanMintAssetToAccount".parse().unwrap(), value_json.clone()),
        alice_id.clone(),
    );
    let holder_client = integration_tests::sync::rebind_blocking_client(&iroha, |client| {
        client.account = mouse_id;
        client.key_pair = mouse_keypair;
    });
    let err = holder_client
        .submit(
            allow_alice_to_mint_mouse_asset.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("an asset holder must not manufacture mint authority for another definition");
    assert!(
        matches!(
            err.downcast_ref::<TransactionRejectionReason>(),
            Some(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(_)
            ))
        ),
        "the holder's lack of mint authority must cause the refusal: {err:?}"
    );
    iroha
        .submit(
            allow_alice_to_mint_mouse_asset,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("asset-definition owner should grant the exact mint permission");
    // Check that alice can indeed mint mouse asset
    let mint_asset = Mint::asset_quantity(1_u32, mouse_asset);
    iroha
        .submit(
            mint_asset,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
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
    let wonderland_domain: DomainId =
        DomainId::try_new("wonderland", "universal").expect("wonderland domain");
    let rose_definition = AssetDefinitionId::derive_from_components(
        wonderland_domain.clone(),
        "rose".parse().expect("valid rose name"),
    );
    let rose_asset = AssetId::new(rose_definition, alice_id.clone());
    let permission1 = CanTransferAsset {
        asset: rose_asset.clone(),
    };
    let allow_alice_to_transfer_rose_1 = Grant::account_permission(permission1, alice_id.clone());
    let encoded_uppercase = rose_asset.to_string();
    let permission2 = CanTransferAsset {
        asset: encoded_uppercase.parse().expect("valid asset id"),
    };
    let allow_alice_to_transfer_rose_2 = Grant::account_permission(permission2, alice_id);
    iroha
        .submit(
            allow_alice_to_transfer_rose_1,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("failed to grant permission");
    let _ = iroha
        .submit(
            allow_alice_to_transfer_rose_2,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("should reject due to duplication");
}
#[test]
fn associated_permissions_removed_on_unregister() {
    let kingdom_id: DomainId = DomainId::try_new("kingdom", "universal").expect("Valid");
    let Some((network, rt)) = start_network_with_builder(
        NetworkBuilder::new()
            .with_genesis_instruction(Register::domain(Domain::new(kingdom_id.clone())))
            .next_genesis_transaction()
            .with_genesis_instruction(Transfer::domain(
                SAMPLE_GENESIS_ACCOUNT_ID.clone(),
                kingdom_id.clone(),
                ALICE_ID.clone(),
            )),
        stringify!(associated_permissions_removed_on_unregister),
    ) else {
        return;
    };
    let iroha = network.client();
    let mut status = iroha.status().get().expect("failed to read initial status");
    let mut last_non_empty_height = status.blocks_non_empty;
    let bob_id = BOB_ID.clone();
    let bob_to_set_kv_in_domain = CanModifyDomainMetadata {
        domain: kingdom_id.clone(),
    };
    let bob_to_set_kv_permission: Permission = bob_to_set_kv_in_domain.clone().into();
    let allow_bob_to_set_kv_in_domain =
        Grant::account_permission(bob_to_set_kv_in_domain.clone(), bob_id.clone());
    iroha
        .submit_all::<InstructionBox>(
            [allow_bob_to_set_kv_in_domain.into()],
            iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("failed to grant permission");
    status = sync_after_submission(
        &network,
        &rt,
        &iroha,
        last_non_empty_height,
        "grant account-associated permission",
    )
    .expect("failed to synchronize after account permission grant");
    last_non_empty_height = status.blocks_non_empty;
    // check that bob indeed have granted permission
    wait_for_account_permission_state(
        &iroha,
        &bob_id,
        &bob_to_set_kv_permission,
        true,
        "account permission grant",
    );
    // unregister kingdom
    iroha
        .submit(
            Unregister::domain(kingdom_id),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("failed to unregister domain");
    let _status = sync_after_submission(
        &network,
        &rt,
        &iroha,
        last_non_empty_height,
        "unregister domain with account-associated permission",
    )
    .expect("failed to synchronize after domain unregister");
    // check that permission is removed from bob
    wait_for_account_permission_state(
        &iroha,
        &bob_id,
        &bob_to_set_kv_permission,
        false,
        "domain unregister",
    );
}
#[test]
fn associated_permissions_removed_from_role_on_unregister() {
    let kingdom_id: DomainId = DomainId::try_new("kingdom", "universal").expect("Valid");
    let Some((network, rt)) = start_network_with_builder(
        NetworkBuilder::new()
            .with_genesis_instruction(Register::domain(Domain::new(kingdom_id.clone())))
            .next_genesis_transaction()
            .with_genesis_instruction(Transfer::domain(
                SAMPLE_GENESIS_ACCOUNT_ID.clone(),
                kingdom_id.clone(),
                ALICE_ID.clone(),
            )),
        stringify!(associated_permissions_removed_from_role_on_unregister),
    ) else {
        return;
    };
    let iroha = network.client();
    let mut status = iroha.status().get().expect("failed to read initial status");
    let mut last_non_empty_height = status.blocks_non_empty;
    let role_id: RoleId = "role".parse().expect("Valid");
    let set_kv_in_domain = CanModifyDomainMetadata {
        domain: kingdom_id.clone(),
    };
    let set_kv_permission: Permission = set_kv_in_domain.clone().into();
    let register_role = Register::role(
        Role::new(role_id.clone(), ALICE_ID.clone()).add_permission(set_kv_in_domain.clone()),
    );
    iroha
        .submit_all::<InstructionBox>(
            [register_role.into()],
            iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("failed to register role");
    status = sync_after_submission(
        &network,
        &rt,
        &iroha,
        last_non_empty_height,
        "register role with associated permission",
    )
    .expect("failed to synchronize after role registration");
    last_non_empty_height = status.blocks_non_empty;
    wait_for_role_permission_state(
        &iroha,
        &role_id,
        &set_kv_permission,
        true,
        "role registration",
    );
    // unregister kingdom
    iroha
        .submit(
            Unregister::domain(kingdom_id),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect("failed to unregister domain");
    let _status = sync_after_submission(
        &network,
        &rt,
        &iroha,
        last_non_empty_height,
        "unregister domain with role-associated permission",
    )
    .expect("failed to synchronize after domain unregister");
    wait_for_role_permission_state(
        &iroha,
        &role_id,
        &set_kv_permission,
        false,
        "domain unregister",
    );
}
