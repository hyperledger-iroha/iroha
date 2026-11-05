//! Exercise seeded concurrent mint bursts and balance convergence across all peers.
use eyre::{Result, eyre};
use integration_tests::{
    sandbox,
    sync::{get_status_with_retry_async, run_on_dedicated_thread},
};
use iroha::{
    data_model::{
        parameter::BlockParameter,
        prelude::*,
        query::error::{FindError, QueryExecutionFail},
    },
    query::QueryError,
};
use iroha_model_base::domain::DomainId;
use iroha_test_network::*;
use iroha_test_samples::gen_account_in;
use rand::{SeedableRng, prelude::IteratorRandom};
use rand_chacha::ChaCha8Rng;
use std::{
    num::NonZero,
    time::{Duration, Instant},
};
use tokio::time::sleep;
/// Send seeded concurrent mint bursts through the selected ingress and require
/// every peer to converge to the exact accumulated amount.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn multiple_blocks_created() -> Result<()> {
    const N_ROUNDS: u64 = 5;
    const N_MAX_TXS_PER_BLOCK: u64 = 10;
    // Given
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_block_cadence(std::time::Duration::from_secs(2))
        .with_config_layer(|layer| {
            // All burst submissions use one selected ingress. Its finite local
            // compute capacity must admit the declared maximum burst of ten.
            layer.write(
                [
                    "torii",
                    "transaction_ingress",
                    "max_concurrent_compute_jobs",
                ],
                i64::try_from(N_MAX_TXS_PER_BLOCK).expect("fixture concurrency fits i64"),
            );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(NonZero::new(N_MAX_TXS_PER_BLOCK).expect("valid")),
        )));
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(multiple_blocks_created)).await?
    else {
        return Ok(());
    };
    let sync_timeout = network.sync_timeout().max(Duration::from_secs(600));
    let leader = network
        .peers()
        .iter()
        .min_by_key(|peer| peer.public_key().to_string())
        .expect("at least one peer");
    let submit_client =
        integration_tests::sync::rebind_blocking_client(&leader.client(), |client| {
            client.transaction_status_timeout = sync_timeout;
            client.transaction_ttl = Some(sync_timeout + Duration::from_secs(5));
        });
    // The default genesis already owns `domain` and its XOR definition.
    let domain_id: DomainId = DomainId::try_new("multipleblocks", "universal")?;
    let create_domain = domain_setup_instruction(&domain_id, submit_client.client().account())?;
    let (account_id, _account_keypair) = gen_account_in(domain_id.name());
    let create_account = Register::account(Account::new(account_id.clone()));
    let asset_definition_id: AssetDefinitionId =
        AssetDefinitionId::derive_from_components(domain_id.clone(), "xor".parse()?);
    let create_asset = Register::asset_definition({
        let __asset_definition_id = asset_definition_id.clone();
        AssetDefinition::numeric(
            __asset_definition_id.clone(),
            "xor".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(domain_id.clone()),
        )
    });
    {
        let client = submit_client.clone();
        let submit_res: eyre::Result<()> = run_on_dedicated_thread(move || {
            client
                .submit_all::<InstructionBox>(
                    [
                        create_domain,
                        InstructionBox::from(create_account),
                        InstructionBox::from(create_asset),
                    ],
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .map(|_| ())
        })
        .await?;
        if sandbox::handle_result(submit_res, stringify!(multiple_blocks_created))?.is_none() {
            return Ok(());
        }
    }
    if sandbox::handle_result(
        network.ensure_blocks(2).await,
        stringify!(multiple_blocks_created),
    )?
    .is_none()
    {
        return Ok(());
    }
    let mut last_non_empty = match sandbox::handle_result(
        get_status_with_retry_async(&submit_client).await,
        stringify!(multiple_blocks_created),
    )? {
        Some(status) => status.blocks_non_empty,
        None => return Ok(()),
    };
    // When
    let mut rng = ChaCha8Rng::seed_from_u64(0x4d55_4c54);
    let mut total: u128 = 0;
    for _ in 1..=N_ROUNDS {
        let txs = (1..=N_MAX_TXS_PER_BLOCK)
            .choose(&mut rng)
            .expect("there is a room to choose from");
        println!("submitting {txs} concurrent transactions through the selected ingress");
        let mut submit_handles = Vec::new();
        for _ in 0..txs {
            let value = (0..999_999)
                .choose(&mut rng)
                .expect("there is quite a room to choose from");
            total += value;
            let client = submit_client.clone();
            let mint = Mint::asset_quantity(
                u64::try_from(value).expect("generated quantity fits u64"),
                AssetId::new(asset_definition_id.clone(), account_id.clone()),
            );
            // Keep every burst concurrent while each synchronous SDK operation
            // obtains its fee quote outside Tokio's runtime context.
            submit_handles.push(tokio::spawn(async move {
                run_on_dedicated_thread(move || {
                    client
                        .submit(
                            mint,
                            iroha_data_model::transaction::FeePaymentIntent::authority(
                                Vec::new(),
                                None,
                            ),
                        )
                        .map(|_| ())
                })
                .await?
            }));
        }
        for handle in submit_handles {
            let submit_res: eyre::Result<()> = handle.await.map_err(eyre::Report::from)?;
            if sandbox::handle_result(submit_res, stringify!(multiple_blocks_created))?.is_none() {
                return Ok(());
            }
        }
        if sandbox::handle_result(
            network
                .ensure_blocks_with(|height| height.non_empty > last_non_empty)
                .await,
            stringify!(multiple_blocks_created),
        )?
        .is_none()
        {
            return Ok(());
        }
        match sandbox::handle_result(
            get_status_with_retry_async(&submit_client).await,
            stringify!(multiple_blocks_created),
        )? {
            Some(status) => last_non_empty = status.blocks_non_empty,
            None => return Ok(()),
        }
    }
    // ensuring all have the same total
    println!("all peers should have total={total}");
    let expected_value = Quantity::from(total);
    // Give the network a chance to flush any straggling transactions before asserting.
    let deadline = Instant::now() + sync_timeout;
    loop {
        let mut all_ok = true;
        for peer in network.peers() {
            let client = peer.client();
            let asset_id = AssetId::new(asset_definition_id.clone(), account_id.clone());
            let found = match client
                .account_client()
                .query_single(FindAssetById::new(asset_id.clone()))
                .await
            {
                Ok(asset) => {
                    assert_eq!(asset.id(), &asset_id);
                    Ok(Some(asset))
                }
                Err(QueryError::Validation(ValidationFail::QueryFailed(
                    QueryExecutionFail::Find(FindError::Asset(id)),
                ))) if *id == asset_id => Ok(None),
                Err(error) => Err(eyre::Report::from(error)),
            };
            let asset = match sandbox::handle_result(found, stringify!(multiple_blocks_created))? {
                Some(v) => v,
                None => return Ok(()),
            };
            if let Some(asset) = asset {
                if *asset.value() != expected_value {
                    all_ok = false;
                    break;
                }
            } else {
                all_ok = false;
                break;
            }
        }
        if all_ok {
            break;
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "asset totals did not converge to expected value {expected_value}"
            ));
        }
        sleep(Duration::from_millis(200)).await;
    }
    Ok(())
}
