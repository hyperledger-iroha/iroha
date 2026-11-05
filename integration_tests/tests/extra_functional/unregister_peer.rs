//! Candidate registration/removal preserves the committee and explicit observer sync trust.
use eyre::{Report, Result, WrapErr, eyre};
use integration_tests::{sandbox, sync::spawn_blocking};
use iroha::{
    blocking::Client,
    data_model::{isi::register::RegisterPeerWithPop, parameter::BlockParameter, prelude::*},
};
use iroha_model_base::domain::DomainId;
use iroha_primitives::nonzero;
use iroha_test_network::{NetworkBuilder, domain_setup_instruction};
use iroha_test_samples::gen_account_in;
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};
use tokio::time::{sleep, timeout};
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn candidate_registry_removal_preserves_four_voters_and_observer_sync() -> Result<()> {
    const PIPELINE_TIME: Duration = Duration::from_secs(2);
    // Given a network
    let builder = NetworkBuilder::new()
        .with_block_cadence(PIPELINE_TIME)
        .with_peers(4)
        .with_max_validator_capacity(5)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )));
    let Some(network) = super::connected_peers::start_validators_with_offline_observer(
        builder,
        stringify!(candidate_registry_removal_preserves_four_voters_and_observer_sync),
    )
    .await?
    else {
        return Ok(());
    };
    // Preserve the existing bounded history-sync budget. Candidate registry churn
    // does not change the authenticated four-validator committee.
    let sync_timeout = network.sync_timeout().saturating_mul(3);
    let tx_timeout = sync_timeout;
    let client = network.client();
    let (account, _account_keypair) = gen_account_in("domain");
    let domain_id: DomainId = DomainId::try_new("peer-churn", "universal")?;
    let asset_def: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("peer-churn", "universal")?,
        "xor".parse()?,
    );
    // The offline replica has explicit P2P trust, independently of this candidate
    // key registration. It remains outside the four signed voting positions.
    let new_peer = &network.observers()[0];
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
        "candidate_registry_removal_preserves_four_voters_and_observer_sync register_peer",
    )
    .await?;
    if sandbox::handle_result(
        wait_for_peer_count(&client, 5, sync_timeout).await,
        stringify!(candidate_registry_removal_preserves_four_voters_and_observer_sync),
    )?
    .is_none()
    {
        return Ok(());
    }
    let mut expected_height = timeout(tx_timeout, client.client().status().get())
        .await
        .map_err(|_| {
            eyre!("candidate_registry_removal_preserves_four_voters_and_observer_sync fetch status timed out")
        })??
        .blocks;
    let genesis = network.genesis();
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
        stringify!(candidate_registry_removal_preserves_four_voters_and_observer_sync),
    )?
    .is_none()
    {
        return Ok(());
    }
    let setup_domain = domain_setup_instruction(&domain_id, client.client().account())?;
    run_blocking_with_timeout(
        {
            let client = client.clone();
            let account = account.clone();
            let asset_def = asset_def.clone();
            let owning_domain = domain_id.clone();
            let setup_domain = setup_domain.clone();
            move || {
                client
                    .submit_all::<InstructionBox>(
                        [
                            setup_domain,
                            Register::account(Account::new(account.clone())).into(),
                            Register::asset_definition({
                                let __asset_definition_id = asset_def;
                                AssetDefinition::numeric(
                                    __asset_definition_id.clone(),
                                    "xor".to_owned(),
                                    iroha_data_model::asset::AssetBalancePolicy::Global,
                                    Some(owning_domain),
                                )
                            })
                            .into(),
                        ],
                        iroha::data_model::transaction::FeePaymentIntent::authority(
                            Vec::new(),
                            None,
                        ),
                    )
                    .map(|_| ())
            }
        },
        tx_timeout,
        "candidate_registry_removal_preserves_four_voters_and_observer_sync register bootstrap",
    )
    .await?;
    expected_height = expected_height.saturating_add(1);
    if sandbox::handle_result(
        network.ensure_blocks(expected_height).await,
        stringify!(candidate_registry_removal_preserves_four_voters_and_observer_sync),
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
        stringify!(candidate_registry_removal_preserves_four_voters_and_observer_sync),
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
        stringify!(candidate_registry_removal_preserves_four_voters_and_observer_sync),
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
        stringify!(candidate_registry_removal_preserves_four_voters_and_observer_sync),
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
        "candidate_registry_removal_preserves_four_voters_and_observer_sync unregister_peer",
    )
    .await?;
    if sandbox::handle_result(
        wait_for_peer_count(&client, 4, sync_timeout).await,
        stringify!(candidate_registry_removal_preserves_four_voters_and_observer_sync),
    )?
    .is_none()
    {
        return Ok(());
    }
    let roster_client = client.clone();
    let retained_voters =
        spawn_blocking(move || roster_client.client().query(FindPeers).execute_all())
            .await??
            .into_iter()
            .collect::<BTreeSet<_>>();
    assert_eq!(
        retained_voters,
        network
            .validators()
            .iter()
            .map(iroha_test_network::NetworkPeer::id)
            .collect(),
        "candidate removal must retain exactly the original four voter identities"
    );
    expected_height = timeout(tx_timeout, client.client().status().get())
        .await
        .map_err(|_| eyre!("candidate_registry_removal_preserves_four_voters_and_observer_sync refresh status after unregister timed out"))??
        .blocks;
    // We can mint without an error.
    mint(&client, &asset_def, &account, 200_u32.into(), tx_timeout).await?;
    expected_height = expected_height.saturating_add(1);
    if sandbox::handle_result(
        network.ensure_blocks(expected_height).await,
        stringify!(candidate_registry_removal_preserves_four_voters_and_observer_sync),
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
        stringify!(candidate_registry_removal_preserves_four_voters_and_observer_sync),
    )? {
        Some(value) => value,
        None => return Ok(()),
    };
    assert_eq!(main_asset, Quantity::from(300_u32));
    // Removing a candidate key does not revoke immutable observer networking
    // trust. The still-authorized replica must continue to sync real execution.
    let unregistered_asset = match sandbox::handle_result(
        wait_for_asset(
            &new_peer_client,
            &account,
            &asset_def,
            sync_timeout,
            Some(300_u32.into()),
        )
        .await,
        stringify!(candidate_registry_removal_preserves_four_voters_and_observer_sync),
    )? {
        Some(value) => value,
        None => return Ok(()),
    };
    assert_eq!(unregistered_asset, Quantity::from(300_u32));
    Ok(())
}
async fn find_asset(
    client: &Client,
    account: &AccountId,
    asset_definition: &AssetDefinitionId,
) -> Result<Option<Quantity>> {
    let asset_id = AssetId::new(asset_definition.clone(), account.clone());
    let client = client.clone();
    let asset = spawn_blocking(move || super::find_asset(&client, &asset_id)).await??;
    Ok(asset.map(|asset| asset.value().clone()))
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
        "candidate_registry_removal_preserves_four_voters_and_observer_sync mint",
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
    let join = spawn_blocking(task);
    let result = timeout(timeout_duration, join)
        .await
        .map_err(|_| eyre!("{context} timed out"))?;
    result.map_err(Report::new)?
}
async fn wait_for_peer_count(
    client: &Client,
    expected: usize,
    timeout_duration: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout_duration;
    loop {
        let count = spawn_blocking({
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
