//! Peer unregister flows under churn.

use std::time::{Duration, Instant};

use eyre::{Report, Result, WrapErr, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{isi::register::RegisterPeerWithPop, parameter::BlockParameter, prelude::*},
};
use iroha_test_network::{BlockHeight, Network, NetworkBuilder, NetworkPeer};
use iroha_test_samples::gen_account_in;
use nonzero_ext::nonzero;
use tokio::{
    task::spawn_blocking,
    time::{sleep, timeout},
};

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn network_stable_after_add_and_after_remove_peer() -> Result<()> {
    const PIPELINE_TIME: Duration = Duration::from_secs(2);

    // Given a network
    let builder = NetworkBuilder::new()
        .with_pipeline_time(PIPELINE_TIME)
        .with_peers(4)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )));
    let Some(mut network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(network_stable_after_add_and_after_remove_peer),
    )
    .await?
    else {
        return Ok(());
    };
    let sync_timeout = network.sync_timeout();
    let tx_timeout = sync_timeout;
    let client = network.client();

    let (account, _account_keypair) = gen_account_in("domain");
    let asset_def: AssetDefinitionId = "xor#domain".parse()?;
    run_blocking_with_timeout(
        {
            let client = client.clone();
            let account = account.clone();
            let asset_def = asset_def.clone();
            move || {
                client
                    .submit_all::<InstructionBox>([
                        Register::domain(Domain::new("domain".parse()?)).into(),
                        Register::account(Account::new(account)).into(),
                        Register::asset_definition(AssetDefinition::numeric(asset_def)).into(),
                    ])
                    .map(|_| ())
            }
        },
        tx_timeout,
        "network_stable_after_add_and_after_remove_peer register bootstrap",
    )
    .await?;

    // When assets are minted
    mint(&client, &asset_def, &account, numeric!(100), tx_timeout).await?;
    if ensure_blocks_or_skip(
        &network,
        3,
        sync_timeout,
        stringify!(network_stable_after_add_and_after_remove_peer),
    )
    .await?
    .is_none()
    {
        return Ok(());
    }
    // and a new peer is registered
    let new_peer = NetworkPeer::builder().build(network.env());
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
        "network_stable_after_add_and_after_remove_peer register_peer",
    )
    .await?;
    if ensure_blocks_or_skip(
        &network,
        4,
        sync_timeout,
        stringify!(network_stable_after_add_and_after_remove_peer),
    )
    .await?
    .is_none()
    {
        return Ok(());
    }
    let genesis = network.genesis();
    if let Err(err) = new_peer
        .start(
            network.config_layers_with_additional_peers([&new_peer]),
            Some(&genesis),
        )
        .await
    {
        if let Some(reason) = sandbox::sandbox_reason(&err) {
            return Err(err.wrap_err(format!(
                "sandboxed network restriction detected while starting new peer: {reason}"
            )));
        }
        return Err(err);
    }
    network.add_peer(&new_peer);
    let sync_result = timeout(
        sync_timeout,
        new_peer.once_block_with(BlockHeight::predicate_non_empty(4)),
    )
    .await
    .map_err(|_| {
        eyre!(
            "network_stable_after_add_and_after_remove_peer timed out waiting for new peer to sync"
        )
    });
    if sandbox::handle_result(
        sync_result,
        stringify!(network_stable_after_add_and_after_remove_peer),
    )?
    .is_none()
    {
        return Ok(());
    }
    // Then the new peer should already have the mint result.
    let new_peer_asset = match sandbox::handle_result(
        wait_for_asset(
            &new_peer_client,
            &account,
            &asset_def,
            sync_timeout,
            Some(numeric!(100)),
        )
        .await,
        stringify!(network_stable_after_add_and_after_remove_peer),
    )? {
        Some(value) => value,
        None => return Ok(()),
    };
    assert_eq!(new_peer_asset, numeric!(100));

    // When a peer is unregistered
    submit_or_skip(
        client.clone(),
        Unregister::peer(new_peer_id),
        tx_timeout,
        "network_stable_after_add_and_after_remove_peer unregister_peer",
    )
    .await?;
    if sandbox::handle_result(
        wait_for_peer_count(&client, 4, sync_timeout).await,
        stringify!(network_stable_after_add_and_after_remove_peer),
    )?
    .is_none()
    {
        return Ok(());
    }
    // blocks=6
    network.remove_peer(&new_peer);
    // We can mint without an error.
    mint(&client, &asset_def, &account, numeric!(200), tx_timeout).await?;
    // Assets are increased on the main network.
    if ensure_blocks_or_skip(
        &network,
        6,
        sync_timeout,
        stringify!(network_stable_after_add_and_after_remove_peer),
    )
    .await?
    .is_none()
    {
        return Ok(());
    }
    let main_asset = match sandbox::handle_result(
        wait_for_asset(
            &client,
            &account,
            &asset_def,
            sync_timeout,
            Some(numeric!(300)),
        )
        .await,
        stringify!(network_stable_after_add_and_after_remove_peer),
    )? {
        Some(value) => value,
        None => return Ok(()),
    };
    assert_eq!(main_asset, numeric!(300));
    // Removed peers should not observe new mints after unregister.
    sleep(PIPELINE_TIME * 5).await;
    let unregistered_asset = match sandbox::handle_result(
        wait_for_asset(
            &new_peer_client,
            &account,
            &asset_def,
            sync_timeout,
            Some(numeric!(100)),
        )
        .await,
        stringify!(network_stable_after_add_and_after_remove_peer),
    )? {
        Some(value) => value,
        None => return Ok(()),
    };
    assert_eq!(unregistered_asset, numeric!(100));

    Ok(())
}

async fn find_asset(
    client: &Client,
    account: &AccountId,
    asset_definition: &AssetDefinitionId,
) -> Result<Option<Numeric>> {
    let account_id = account.clone();
    let client = client.clone();
    let asset = spawn_blocking(move || client.query(FindAssets::new()).execute_all())
        .await??
        .into_iter()
        .filter(|asset| asset.id().account() == &account_id)
        .find(|asset| asset.id().definition() == asset_definition);

    Ok(asset.map(|asset| asset.value().clone()))
}

async fn wait_for_asset(
    client: &Client,
    account: &AccountId,
    asset_definition: &AssetDefinitionId,
    timeout_duration: Duration,
    expected: Option<Numeric>,
) -> Result<Numeric> {
    let deadline = Instant::now() + timeout_duration;
    loop {
        if let Some(value) = find_asset(client, account, asset_definition).await?
            && expected.as_ref().is_none_or(|expected| value == *expected)
        {
            return Ok(value);
        }
        if Instant::now() >= deadline {
            if expected.is_some() {
                return Err(eyre!("timed out waiting for asset to reach expected value"));
            }
            return Err(eyre!("timed out waiting for asset to appear"));
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn mint(
    client: &Client,
    asset_definition_id: &AssetDefinitionId,
    account_id: &AccountId,
    quantity: Numeric,
    timeout_duration: Duration,
) -> Result<()> {
    let mint_asset = Mint::asset_numeric(
        quantity,
        AssetId::new(asset_definition_id.clone(), account_id.clone()),
    );
    let client = client.clone();
    submit_or_skip(
        client,
        mint_asset,
        timeout_duration,
        "network_stable_after_add_and_after_remove_peer mint",
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
                .submit(instruction)
                .map(|_| ())
                .wrap_err_with(|| context_owned.clone())
        },
        timeout_duration,
        context,
    )
    .await
}

async fn ensure_blocks_or_skip(
    network: &Network,
    height: u64,
    timeout_duration: Duration,
    context: &str,
) -> Result<Option<()>> {
    let result = timeout(timeout_duration, network.ensure_blocks(height))
        .await
        .map_err(Report::new)?
        .map(|_| ());
    sandbox::handle_result(result, context)
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
    let join = spawn_blocking(task);
    let result = timeout(timeout_duration, join)
        .await
        .map_err(|_| eyre!("{context} timed out"))?;
    result.map_err(Report::new)?
}

async fn wait_for_peer_count(
    client: &Client,
    expected: u64,
    timeout_duration: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout_duration;
    loop {
        let status = spawn_blocking({
            let client = client.clone();
            move || client.get_status()
        })
        .await??;
        if status.peers == expected {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for peer count {expected}, last seen {}",
                status.peers
            ));
        }
        sleep(Duration::from_millis(200)).await;
    }
}
