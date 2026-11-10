//! Peer registry removal and explicit permissioned P2P trust revocation under churn.
use super::connected_peers::{
    assert_consensus_roles, assert_peers_status, authenticated_genesis_voters, online_peer_count,
    wait_for_peer_registry,
};
use eyre::{Report, Result, WrapErr, ensure, eyre};
use integration_tests::{sandbox, sync::run_on_dedicated_thread};
use iroha::{
    blocking::Client,
    data_model::{isi::register::RegisterPeerWithPop, parameter::BlockParameter, prelude::*},
};
use iroha_config::base::toml::WriteExt as _;
use iroha_model_base::domain::DomainId;
use iroha_primitives::nonzero;
use iroha_test_network::{
    Network, NetworkBuilder, NetworkPeer, ObserverP2pBootstrap, domain_setup_instruction,
};
use iroha_test_samples::gen_account_in;
use std::{
    borrow::Cow,
    future::Future,
    time::{Duration, Instant},
};
use tokio::time::{sleep, timeout};
use toml::Table;
#[path = "unregister_peer/retained_asset.rs"]
mod retained_asset;

// The default genesis already registers domain.universal and its xor definition.
fn observer_asset_setup(
    owner: &AccountId,
    holder: &AccountId,
) -> Result<(AssetDefinitionId, [InstructionBox; 3])> {
    let domain = DomainId::try_new("peer-isolation", "universal")?;
    let definition = AssetDefinitionId::derive_from_components(domain.clone(), "xor".parse()?);
    let setup = [
        domain_setup_instruction(&domain, owner)?,
        Register::account(Account::new(holder.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(
            definition.clone(),
            "xor".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(domain),
        ))
        .into(),
    ];
    Ok((definition, setup))
}

#[test]
fn observer_asset_setup_claims_a_new_signer_owned_domain() -> Result<()> {
    use iroha::data_model::{
        alias_setup::AliasIntentV1, isi::alias_setup::EnsureAlias, transaction::Executable,
    };
    use iroha_model_base::topology::DataSpaceId;
    use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID;
    use std::collections::{BTreeMap, BTreeSet};

    let network = NetworkBuilder::new().with_peers(4).build();
    let genesis = network.genesis();
    let client = network.client();
    let owner = client.client().account();
    let holder = gen_account_in("peer-isolation").0;
    let (definition, [claim, _, registration]) = observer_asset_setup(owner, &holder)?;
    let ensure = claim.as_any().downcast_ref::<EnsureAlias>().unwrap();
    let AliasIntentV1::Domain(intent) = &ensure.intent else {
        panic!("isolation asset must claim its own domain");
    };
    let domain = &intent.domain.canonical_name;
    assert_eq!(intent.domain.dataspace_id, DataSpaceId::UNIVERSAL);
    assert_eq!(&intent.owner, owner);
    assert_eq!(
        definition,
        AssetDefinitionId::derive_from_components(domain.clone(), "xor".parse()?)
    );
    let Some(RegisterBox::AssetDefinition(asset)) =
        registration.as_any().downcast_ref::<RegisterBox>()
    else {
        panic!("isolation fixture must register its own asset definition");
    };
    assert_eq!(asset.object().id(), &definition);
    assert_eq!(asset.object().owning_domain.as_ref(), Some(domain));

    let mut domain_registrars = BTreeMap::new();
    let mut definitions = BTreeSet::new();
    for transaction in genesis.0.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            continue;
        };
        for instruction in instructions {
            match instruction.as_any().downcast_ref::<RegisterBox>() {
                Some(RegisterBox::Domain(registration)) => {
                    domain_registrars.insert(
                        registration.object().id().clone(),
                        transaction.authority().clone(),
                    );
                }
                Some(RegisterBox::AssetDefinition(registration)) => {
                    definitions.insert(registration.object().id().clone());
                }
                _ => {}
            }
        }
    }
    let seeded_domain = DomainId::try_new("domain", "universal")?;
    assert_eq!(
        domain_registrars.get(&seeded_domain),
        Some(&*SAMPLE_GENESIS_ACCOUNT_ID)
    );
    assert!(
        definitions.contains(&AssetDefinitionId::derive_from_components(
            seeded_domain,
            "xor".parse()?
        ))
    );
    assert!(!domain_registrars.contains_key(domain));
    assert!(!definitions.contains(&definition));
    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn network_stable_after_peer_registry_and_trust_removal() -> Result<()> {
    const PIPELINE_TIME: Duration = Duration::from_secs(2);
    // Given a network
    let builder = NetworkBuilder::new()
        .with_block_cadence(PIPELINE_TIME)
        .with_peers(4)
        .with_permissioned_consensus()
        .with_observer_p2p_bootstrap(ObserverP2pBootstrap::new(1)?)?
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )));
    let Some(network) = sandbox::build_network_or_skip(
        builder,
        stringify!(network_stable_after_peer_registry_and_trust_removal),
    ) else {
        return Ok(());
    };
    // Retain the original replay/churn budget. The four authenticated voters
    // never change; registry membership and operator P2P trust are separate.
    let sync_timeout = network.sync_timeout().saturating_mul(3);
    let tx_timeout = sync_timeout;
    let voters = authenticated_genesis_voters(&network, 4)?;
    let genesis = network.genesis();
    let genesis_hash = genesis.0.hash();
    futures_util::future::try_join_all(network.validators().iter().map(|validator| {
        validator.start_checked(network.config_layers_for_peer(validator), Some(&genesis))
    }))
    .await?;
    assert_peers_status(network.validators(), 1, 3, sync_timeout).await?;
    let client = network.client();
    let account = gen_account_in("peer-isolation").0;
    let (asset_def, setup) = observer_asset_setup(client.client().account(), &account)?;
    // Register a new peer early to keep the pre-join block history short.
    let new_peer = &network.observers()[0];
    assert!(!new_peer.is_running());
    let new_peer_id = new_peer.id();
    let new_peer_client = new_peer.client();
    let register_peer = RegisterPeerWithPop::new(
        new_peer_id.clone(),
        new_peer
            .bls_pop()
            .expect("network peer should have BLS PoP")
            .to_vec(),
    );
    submit_or_skip(
        client.clone(),
        register_peer,
        tx_timeout,
        "network_stable_after_peer_registry_and_trust_removal register_peer",
    )
    .await?;
    if sandbox::handle_result(
        wait_for_peer_count(&client, 5, sync_timeout).await,
        stringify!(network_stable_after_peer_registry_and_trust_removal),
    )?
    .is_none()
    {
        return Ok(());
    }
    let mut registered = voters.clone();
    registered.insert(new_peer.id());
    wait_for_peer_registry(network.validators(), &registered, sync_timeout).await?;
    let mut expected_height =
        status_height_until(tokio::time::Instant::now() + tx_timeout, |deadline| {
            let client = client.client().with_request_deadline(deadline.into_std());
            async move { Ok(client.status().get().await?.blocks) }
        })
        .await?;
    if let Err(err) = new_peer
        .start(network.config_layers_for_peer(new_peer), Some(&genesis))
        .await
    {
        if let Some(reason) = sandbox::sandbox_reason(&err) {
            return Err(err.wrap_err(format!(
                "sandboxed network restriction detected while starting new peer: {reason}"
            )));
        }
        return Err(err);
    }
    if sandbox::handle_result(
        timeout(sync_timeout, new_peer.once_block(expected_height))
            .await
            .wrap_err_with(|| {
                format!("new peer did not sync to height {expected_height} before timeout")
            }),
        stringify!(network_stable_after_peer_registry_and_trust_removal),
    )?
    .is_none()
    {
        return Ok(());
    }
    assert_peers_status(network.all_peers(), expected_height, 4, sync_timeout).await?;
    wait_for_peer_registry(network.all_peers(), &registered, sync_timeout).await?;
    assert_consensus_roles(&network, true, sync_timeout).await?;
    run_blocking_with_timeout(
        {
            let client = client.clone();
            move || {
                client
                    .submit_all::<InstructionBox>(
                        setup,
                        iroha::data_model::transaction::FeePaymentIntent::authority(
                            Vec::new(),
                            None,
                        ),
                    )
                    .map(|_| ())
            }
        },
        tx_timeout,
        "network_stable_after_peer_registry_and_trust_removal register bootstrap",
    )
    .await?;
    expected_height = expected_height.saturating_add(1);
    if sandbox::handle_result(
        network.ensure_blocks(expected_height).await,
        stringify!(network_stable_after_peer_registry_and_trust_removal),
    )?
    .is_none()
    {
        return Ok(());
    }
    // When assets are minted
    mint(&client, &asset_def, &account, 100_u32.into(), tx_timeout).await?;
    expected_height = expected_height.saturating_add(1);
    if sandbox::handle_result(
        network.ensure_blocks(expected_height).await,
        stringify!(network_stable_after_peer_registry_and_trust_removal),
    )?
    .is_none()
    {
        return Ok(());
    }
    let main_bootstrap_asset = match sandbox::handle_result(
        wait_for_asset(
            &client,
            &account,
            &asset_def,
            sync_timeout,
            Some(100_u32.into()),
        )
        .await,
        stringify!(network_stable_after_peer_registry_and_trust_removal),
    )? {
        Some(value) => value,
        None => return Ok(()),
    };
    assert_eq!(main_bootstrap_asset, Quantity::from(100_u32));
    // The new peer should already have the mint result.
    let new_peer_asset = match sandbox::handle_result(
        wait_for_asset(
            &new_peer_client,
            &account,
            &asset_def,
            sync_timeout,
            Some(100_u32.into()),
        )
        .await,
        stringify!(network_stable_after_peer_registry_and_trust_removal),
    )? {
        Some(value) => value,
        None => return Ok(()),
    };
    assert_eq!(new_peer_asset, Quantity::from(100_u32));
    // When a peer is unregistered
    submit_or_skip(
        client.clone(),
        Unregister::peer(new_peer_id),
        tx_timeout,
        "network_stable_after_peer_registry_and_trust_removal unregister_peer",
    )
    .await?;
    if sandbox::handle_result(
        wait_for_peer_count(&client, 4, sync_timeout).await,
        stringify!(network_stable_after_peer_registry_and_trust_removal),
    )?
    .is_none()
    {
        return Ok(());
    }
    expected_height = status_height_until(tokio::time::Instant::now() + tx_timeout, |deadline| {
        let client = client.client().with_request_deadline(deadline.into_std());
        async move { Ok(client.status().get().await?.blocks) }
    })
    .await?;
    wait_for_peer_registry(network.all_peers(), &voters, sync_timeout).await?;
    assert_peers_status(network.all_peers(), expected_height, 4, sync_timeout).await?;
    // Registry removal does not revoke configured P2P trust. Restart only the
    // four voters with an explicit validator-only trust layer, retaining their
    // Kura history and the exact signed genesis. The excluded observer stays
    // alive: its isolation must come from the production permissioned trust gate.
    let trust_layer = validator_only_trust_layer(&network);
    futures_util::future::join_all(network.validators().iter().map(NetworkPeer::shutdown)).await;
    assert!(new_peer.is_running());
    timeout(
        sync_timeout,
        futures_util::future::try_join_all(network.validators().iter().map(|validator| {
            validator.start_checked(
                network
                    .config_layers_for_peer(validator)
                    .chain(std::iter::once(Cow::Owned(trust_layer.clone()))),
                Some(&genesis),
            )
        })),
    )
    .await
    .wrap_err("retained validator trust-removal restart timed out")??;
    assert_eq!(network.genesis().0.hash(), genesis_hash);
    assert_peers_status(network.validators(), expected_height, 3, sync_timeout).await?;
    wait_for_peer_registry(network.validators(), &voters, sync_timeout).await?;
    assert_consensus_roles(&network, true, sync_timeout).await?;
    let isolated_height = expected_height;
    // The retained validators can still mint without an error.
    mint(&client, &asset_def, &account, 200_u32.into(), tx_timeout).await?;
    expected_height = expected_height.saturating_add(1);
    if sandbox::handle_result(
        assert_peers_status(network.validators(), expected_height, 3, sync_timeout).await,
        stringify!(network_stable_after_peer_registry_and_trust_removal),
    )?
    .is_none()
    {
        return Ok(());
    }
    // Assets are increased on the main network.
    let main_asset = match sandbox::handle_result(
        wait_for_asset(
            &client,
            &account,
            &asset_def,
            sync_timeout,
            Some(300_u32.into()),
        )
        .await,
        stringify!(network_stable_after_peer_registry_and_trust_removal),
    )? {
        Some(value) => value,
        None => return Ok(()),
    };
    assert_eq!(main_asset, Quantity::from(300_u32));
    // Keep the observer alive through the original isolation window. Its HTTP
    // asset query can legitimately forward to the global committee, so it is
    // not a local-State assertion. Read local diagnostic height, then replay a
    // copy of its retained originals only after this live observation ends.
    sleep(PIPELINE_TIME * 5).await;
    assert!(new_peer.is_running());
    wait_for_isolated_observer(new_peer, isolated_height, sync_timeout).await?;
    wait_for_peer_registry(network.validators(), &voters, sync_timeout).await?;
    assert_consensus_roles(&network, true, sync_timeout).await?;
    let config_path = new_peer
        .latest_stdout_log_path()
        .ok_or_else(|| eyre!("observer start did not record its run paths"))?
        .with_file_name("run-1-config.toml");
    let store = new_peer.kura_store_dir();
    // Shutdown is solely for the store-root inspection lock, after live
    // isolation and continued four-voter progress have both been established.
    new_peer.shutdown().await;
    let asset = AssetId::new(asset_def.clone(), account.clone());
    let chain = network.chain_id();
    let unregistered_asset = run_blocking_with_timeout(
        move || {
            retained_asset::replay_asset(
                &config_path,
                &store,
                &genesis,
                &chain,
                &voters,
                &asset,
                isolated_height,
            )
        },
        sync_timeout,
        "isolated observer authenticated retained replay",
    )
    .await?;
    assert_eq!(unregistered_asset, Quantity::from(100_u32));
    Ok(())
}
fn isolated_status_matches(blocks: u64, peers: u64, target: u64) -> Result<bool> {
    ensure!(
        blocks <= target,
        "isolated observer applied unexpected block {blocks} above {target}"
    );
    Ok(blocks == target && peers == 0)
}

// Retry only the typed temporary status publication refusal. Requests, delays,
// and successful completion all share the caller's existing absolute deadline.
async fn status_height_until<F, Fut>(deadline: tokio::time::Instant, mut request: F) -> Result<u64>
where
    F: FnMut(tokio::time::Instant) -> Fut,
    Fut: Future<Output = Result<u64>>,
{
    let expired = |last: Option<Report>| match last {
        Some(error) => error.wrap_err("status height read exceeded its original deadline"),
        None => eyre!("status height read exceeded its original deadline"),
    };
    let mut last_busy = None;
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(expired(last_busy));
        }
        let result = match tokio::time::timeout_at(deadline, request(deadline)).await {
            Ok(result) => result,
            Err(_) => return Err(expired(last_busy)),
        };
        if tokio::time::Instant::now() >= deadline {
            return Err(expired(result.err().or(last_busy)));
        }
        match result {
            Ok(height) => return Ok(height),
            Err(error) => {
                let Some(iroha::Error::StatusUnavailable {
                    reason: Some(iroha::StatusFailureReason::StateBusy),
                    retry_after,
                }) = error.downcast_ref::<iroha::Error>()
                else {
                    return Err(error);
                };
                let delay = Duration::from_millis(200).max(retry_after.unwrap_or_default());
                last_busy = Some(error);
                tokio::time::sleep_until(
                    tokio::time::Instant::now()
                        .checked_add(delay)
                        .unwrap_or(deadline)
                        .min(deadline),
                )
                .await;
            }
        }
    }
}

async fn wait_for_isolated_observer(
    peer: &NetworkPeer,
    target: u64,
    bound: Duration,
) -> Result<()> {
    let deadline = tokio::time::Instant::now() + bound;
    let http = reqwest::Client::builder()
        .timeout(integration_tests::http::request_timeout())
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut last = None;
    loop {
        ensure!(
            tokio::time::Instant::now() < deadline,
            "isolated observer deadline: {last:?}"
        );
        let blocks =
            status_height_until(deadline, |_| async { Ok(peer.status().await?.blocks) }).await?;
        ensure!(
            tokio::time::Instant::now() < deadline,
            "isolated observer deadline: {last:?}"
        );
        // Classified status.peers is deliberately zero for an unregistered
        // observer, even while connected. Query the actual local transport count.
        let online = tokio::time::timeout_at(deadline, online_peer_count(&http, peer))
            .await
            .wrap_err_with(|| format!("isolated observer transport deadline: {last:?}"))??;
        last = Some((blocks, online));
        let matched = isolated_status_matches(blocks, online, target)?;
        ensure!(
            tokio::time::Instant::now() < deadline,
            "isolated observer deadline: {last:?}"
        );
        if matched {
            return Ok(());
        }
        tokio::time::sleep_until(
            tokio::time::Instant::now()
                .checked_add(Duration::from_millis(200))
                .unwrap_or(deadline)
                .min(deadline),
        )
        .await;
    }
}

#[tokio::test(start_paused = true)]
async fn status_height_retries_only_typed_busy_within_original_deadline() -> Result<()> {
    let start = tokio::time::Instant::now();
    let deadline = start + Duration::from_secs(5);
    let mut attempts = Vec::new();
    let height = status_height_until(deadline, |received_deadline| {
        assert_eq!(received_deadline, deadline);
        attempts.push(tokio::time::Instant::now().duration_since(start));
        let result = if attempts.len() == 1 {
            Err(Report::from(iroha::Error::StatusUnavailable {
                reason: Some(iroha::StatusFailureReason::StateBusy),
                retry_after: Some(Duration::from_secs(1)),
            }))
        } else {
            Ok(6)
        };
        async move { result }
    })
    .await?;
    assert_eq!(height, 6);
    assert_eq!(attempts, [Duration::ZERO, Duration::from_secs(1)]);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn status_height_deadline_retains_busy_cause_and_refuses_late_success() {
    let mut attempts = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let error = status_height_until(deadline, |_| {
        attempts += 1;
        async {
            Err(Report::from(iroha::Error::StatusUnavailable {
                reason: Some(iroha::StatusFailureReason::StateBusy),
                retry_after: Some(Duration::from_secs(20)),
            }))
        }
    })
    .await
    .unwrap_err();
    assert_eq!(attempts, 1);
    assert_eq!(tokio::time::Instant::now(), deadline);
    assert!(matches!(
        error.downcast_ref::<iroha::Error>(),
        Some(iroha::Error::StatusUnavailable {
            reason: Some(iroha::StatusFailureReason::StateBusy),
            ..
        })
    ));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    assert!(
        status_height_until(deadline, |_| async {
            tokio::time::advance(Duration::from_secs(2)).await;
            Ok(6)
        })
        .await
        .is_err()
    );
}

#[tokio::test(start_paused = true)]
async fn status_height_preserves_permanent_refusals_and_bounds_stalled_requests() {
    for refusal in [
        Report::from(iroha::Error::StatusUnavailable {
            reason: None,
            retry_after: None,
        }),
        Report::from(iroha::Error::Http {
            operation: "diagnostic.status",
            status: 503,
            retry_after: None,
            body: b"status_state_busy".to_vec(),
        }),
        eyre!("status_state_busy"),
    ] {
        let expected = refusal.to_string();
        let mut response = Some(refusal);
        let mut attempts = 0;
        let start = tokio::time::Instant::now();
        let error = status_height_until(start + Duration::from_secs(2), |_| {
            attempts += 1;
            let error = response.take().expect("permanent refusal never retried");
            async move { Err(error) }
        })
        .await
        .unwrap_err();
        assert_eq!(attempts, 1);
        assert_eq!(tokio::time::Instant::now(), start);
        assert_eq!(error.to_string(), expected);
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let error = status_height_until(deadline, |_| std::future::pending::<Result<u64>>())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("original deadline"));
    assert_eq!(tokio::time::Instant::now(), deadline);
}

#[test]
fn isolated_observer_requires_exact_applied_height_and_no_connections() -> Result<()> {
    assert!(isolated_status_matches(5, 0, 5)?);
    assert!(!isolated_status_matches(4, 0, 5)?);
    assert!(!isolated_status_matches(5, 1, 5)?);
    assert!(isolated_status_matches(6, 0, 5).is_err());
    Ok(())
}

fn validator_only_trust_layer(network: &Network) -> Table {
    Table::new().write(
        ["trusted_peers"],
        network
            .validators()
            .iter()
            .map(|peer| {
                iroha_model_base::peer::Peer::new(peer.p2p_address(), peer.id()).to_string()
            })
            .collect::<Vec<_>>(),
    )
}

#[test]
fn trust_removal_preserves_all_voter_endpoints_and_consensus_pops() -> Result<()> {
    let network = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_observer_p2p_bootstrap(ObserverP2pBootstrap::new(1)?)?
        .build();
    let voters = authenticated_genesis_voters(&network, 4)?;
    let layer = validator_only_trust_layer(&network);
    assert_eq!(layer.len(), 1, "only operator P2P trust changes");
    let actual = layer["trusted_peers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    let expected = network
        .validators()
        .iter()
        .map(|peer| iroha_model_base::peer::Peer::new(peer.p2p_address(), peer.id()).to_string())
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 4);
    assert!(
        !actual.contains(
            &iroha_model_base::peer::Peer::new(
                network.observers()[0].p2p_address(),
                network.observers()[0].id()
            )
            .to_string()
        )
    );
    let pop_roster = |layers: Vec<Table>| {
        layers
            .into_iter()
            .filter_map(|layer| layer.get("trusted_peers_pop").cloned())
            .next_back()
            .unwrap()
    };
    let original_layers = network
        .config_layers()
        .map(Cow::into_owned)
        .collect::<Vec<_>>();
    let original_pops = pop_roster(original_layers.clone());
    let mut amended_layers = original_layers;
    amended_layers.push(layer);
    assert_eq!(pop_roster(amended_layers), original_pops);
    let pop_keys = original_pops
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["public_key"].as_str().unwrap().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        pop_keys,
        voters
            .iter()
            .map(|peer| peer.public_key().to_string())
            .collect()
    );
    Ok(())
}

async fn find_asset(
    client: &Client,
    account: &AccountId,
    asset_definition: &AssetDefinitionId,
) -> Result<Option<Quantity>> {
    let id = AssetId::new(asset_definition.clone(), account.clone());
    match client
        .account_client()
        .query_single(FindAssetById::new(id.clone()))
        .await
    {
        Ok(asset) => {
            assert_eq!(asset.id(), &id);
            Ok(Some(asset.value().clone()))
        }
        Err(iroha::query::QueryError::Validation(ValidationFail::QueryFailed(
            iroha::data_model::query::error::QueryExecutionFail::Find(
                iroha::data_model::query::error::FindError::Asset(missing),
            ),
        ))) if missing.as_ref() == &id => Ok(None),
        Err(error) => Err(error.into()),
    }
}
async fn wait_for_asset(
    client: &Client,
    account: &AccountId,
    asset_definition: &AssetDefinitionId,
    timeout_duration: Duration,
    expected: Option<Quantity>,
) -> Result<Quantity> {
    let deadline = Instant::now() + timeout_duration;
    let mut last_err: Option<Report> = None;
    loop {
        match find_asset(client, account, asset_definition).await {
            Ok(Some(value)) if expected.as_ref().is_none_or(|expected| value == *expected) => {
                return Ok(value);
            }
            Ok(_) => {}
            Err(err) => {
                last_err = Some(err);
            }
        }
        if Instant::now() >= deadline {
            if expected.is_some() {
                return Err(eyre!(
                    "timed out waiting for asset to reach expected value; last_err={last_err:?}"
                ));
            }
            return Err(eyre!(
                "timed out waiting for asset to appear; last_err={last_err:?}"
            ));
        }
        sleep(Duration::from_millis(200)).await;
    }
}
async fn mint(
    client: &Client,
    asset_definition_id: &AssetDefinitionId,
    account_id: &AccountId,
    quantity: Quantity,
    timeout_duration: Duration,
) -> Result<()> {
    let mint_asset = Mint::asset_quantity(
        quantity,
        AssetId::new(asset_definition_id.clone(), account_id.clone()),
    );
    let client = client.clone();
    submit_or_skip(
        client,
        mint_asset,
        timeout_duration,
        "network_stable_after_peer_registry_and_trust_removal mint",
    )
    .await
}
async fn submit_or_skip<I>(
    client: Client,
    instruction: I,
    timeout_duration: Duration,
    context: &str,
) -> Result<()>
where
    I: Into<InstructionBox> + Send + 'static,
{
    let context_owned = context.to_string();
    run_blocking_with_timeout(
        move || {
            client
                .submit(
                    instruction,
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .map(|_| ())
                .wrap_err_with(|| context_owned.clone())
        },
        timeout_duration,
        context,
    )
    .await
}
async fn run_blocking_with_timeout<T, F>(
    task: F,
    timeout_duration: Duration,
    context: &str,
) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let join = run_on_dedicated_thread(task);
    let result = timeout(timeout_duration, join)
        .await
        .map_err(|_| eyre!("{context} timed out"))?;
    result?
}
async fn wait_for_peer_count(
    client: &Client,
    expected: usize,
    timeout_duration: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout_duration;
    loop {
        let count = run_on_dedicated_thread({
            let client = client.clone();
            move || {
                client
                    .client()
                    .query(FindPeers)
                    .execute_all()
                    .map(|peers| peers.len())
            }
        })
        .await??;
        if count == expected {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for peer count {expected}, last seen {count}"
            ));
        }
        sleep(Duration::from_millis(200)).await;
    }
}
