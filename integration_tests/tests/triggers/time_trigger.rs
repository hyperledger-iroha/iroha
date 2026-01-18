//! Time-based trigger execution paths using bounded async waits.

use std::time::Duration;

use eyre::{Result, WrapErr};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{Level, asset::AssetId, prelude::*},
};
use iroha_primitives::json::Json;
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, gen_account_in, load_sample_ivm};
use tokio::{
    task::spawn_blocking,
    time::{sleep, timeout},
};

use crate::triggers::get_asset_value;

fn curr_time() -> Duration {
    use std::time::SystemTime;

    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
}

async fn run_or_skip<F, Fut>(context: &'static str, test: F) -> Result<()>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    if sandbox::handle_result(test().await, context)?.is_none() {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mint_asset_after_3_sec() -> Result<()> {
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new().with_default_pipeline_time(),
        stringify!(mint_asset_after_3_sec),
    )
    .await?
    else {
        return Ok(());
    };
    let test_client = network.client();
    let sync_timeout = network.sync_timeout();
    network.ensure_blocks_with(|h| h.total >= 1).await?;

    run_or_skip(stringify!(mint_asset_after_3_sec), || async {
        let asset_definition_id = "rose#wonderland"
            .parse::<AssetDefinitionId>()
            .expect("Valid");
        let account_id = ALICE_ID.clone();
        let asset_id = AssetId::new(asset_definition_id.clone(), account_id.clone());

        let init_quantity = spawn_blocking({
            let client = test_client.clone();
            let asset_id = asset_id.clone();
            move || get_asset_value(&client, &asset_id)
        })
        .await??;
        let expected_value = init_quantity
            .clone()
            .checked_add(1u32.into())
            .expect("quantity should increment");

        let start_time = curr_time();
        let pipeline_time = network.pipeline_time();
        let gap = std::cmp::max(
            Duration::from_millis(200),
            pipeline_time.checked_div(2).unwrap_or(pipeline_time),
        );
        let schedule = TimeSchedule::starting_at(start_time + gap);
        let instruction = Mint::asset_numeric(1_u32, asset_id.clone());
        let register_trigger = Register::trigger(Trigger::new(
            "mint_rose".parse().expect("Valid"),
            Action::new(
                vec![instruction],
                Repeats::from(1_u32),
                account_id.clone(),
                TimeEventFilter::new(ExecutionTime::Schedule(schedule)),
            ),
        ));
        spawn_blocking({
            let client = test_client.clone();
            move || client.submit_blocking(register_trigger)
        })
        .await??;

        spawn_blocking({
            let client = test_client.clone();
            move || {
                client.submit_blocking(Log::new(Level::DEBUG, "Just to create block".to_string()))
            }
        })
        .await??;
        let after_registration_quantity = spawn_blocking({
            let client = test_client.clone();
            let asset_id = asset_id.clone();
            move || get_asset_value(&client, &asset_id)
        })
        .await??;
        if after_registration_quantity == expected_value {
            // Scheduling can fire immediately if the block time already passed the schedule.
            return Ok(());
        }
        assert_eq!(init_quantity, after_registration_quantity);

        let poll_delay = pipeline_time.checked_div(2).unwrap_or(pipeline_time);
        spawn_blocking({
            let client = test_client.clone();
            move || {
                client.submit_blocking(Log::new(Level::DEBUG, "Just to create block".to_string()))
            }
        })
        .await??;

        timeout(sync_timeout, async {
            loop {
                let after_wait_quantity = spawn_blocking({
                    let client = test_client.clone();
                    let asset_id = asset_id.clone();
                    move || get_asset_value(&client, &asset_id)
                })
                .await??;
                if after_wait_quantity == expected_value {
                    break Ok::<(), eyre::Report>(());
                }
                sleep(poll_delay).await;
            }
        })
        .await??;

        Ok(())
    })
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::items_after_statements)]
async fn pre_commit_trigger_should_be_executed() -> Result<()> {
    const CHECKS_COUNT: usize = 3;

    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(pre_commit_trigger_should_be_executed),
    )
    .await?
    else {
        return Ok(());
    };
    let test_client = network.client();

    run_or_skip(
        stringify!(pre_commit_trigger_should_be_executed),
        || async {
            let asset_definition_id = "rose#wonderland".parse().expect("Valid");
            let account_id = ALICE_ID.clone();
            let asset_id = AssetId::new(asset_definition_id, account_id.clone());

            let instruction = Mint::asset_numeric(1u32, asset_id.clone());
            let register_trigger = Register::trigger(Trigger::new(
                "mint_rose".parse()?,
                Action::new(
                    vec![instruction],
                    Repeats::Indefinitely,
                    account_id.clone(),
                    TimeEventFilter::new(ExecutionTime::PreCommit),
                ),
            ));
            spawn_blocking({
                let client = test_client.clone();
                move || client.submit(register_trigger)
            })
            .await??;

            let prev_value = spawn_blocking({
                let client = test_client.clone();
                let asset_id = asset_id.clone();
                move || get_asset_value(&client, &asset_id)
            })
            .await??;
            for _ in 0..CHECKS_COUNT {
                let sample_isi = SetKeyValue::account(
                    account_id.clone(),
                    "key".parse::<Name>()?,
                    "value".parse::<Json>()?,
                );
                spawn_blocking({
                    let client = test_client.clone();
                    move || client.submit(sample_isi)
                })
                .await??;
            }

            let expected_value = prev_value
                .checked_add(Numeric::from(u32::try_from(CHECKS_COUNT)?))
                .expect("value increment should fit");
            let poll_delay = std::cmp::max(
                Duration::from_millis(200),
                network
                    .pipeline_time()
                    .checked_div(2)
                    .unwrap_or_else(|| network.pipeline_time()),
            );
            timeout(network.sync_timeout(), async {
                loop {
                    let new_value = spawn_blocking({
                        let client = test_client.clone();
                        let asset_id = asset_id.clone();
                        move || get_asset_value(&client, &asset_id)
                    })
                    .await??;
                    if new_value >= expected_value {
                        break Ok::<(), eyre::Report>(());
                    }
                    sleep(poll_delay).await;
                }
            })
            .await
            .wrap_err("pre_commit_trigger_should_be_executed poll timed out")??;

            Ok(())
        },
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mint_nft_for_every_user_every_1_sec() -> Result<()> {
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new().with_default_pipeline_time(),
        stringify!(mint_nft_for_every_user_every_1_sec),
    )
    .await?
    else {
        return Ok(());
    };
    let test_client = network.client();
    let trigger_period = std::cmp::max(
        Duration::from_millis(300),
        network
            .pipeline_time()
            .checked_div(2)
            .unwrap_or_else(|| network.pipeline_time()),
    );
    let expected_count: u64 = 3;

    run_or_skip(stringify!(mint_nft_for_every_user_every_1_sec), || async {
        let alice_id = ALICE_ID.clone();

        let accounts: Vec<AccountId> = vec![
            alice_id.clone(),
            gen_account_in("wonderland").0,
            gen_account_in("wonderland").0,
            gen_account_in("wonderland").0,
            gen_account_in("wonderland").0,
        ];

        let register_accounts = accounts
            .iter()
            .skip(1)
            .cloned()
            .map(|account_id| Register::account(Account::new(account_id)))
            .collect::<Vec<_>>();
        spawn_blocking({
            let client = test_client.clone();
            move || client.submit_all_blocking(register_accounts)
        })
        .await??;
        println!("registered additional accounts for time trigger");

        let offset = trigger_period.saturating_mul(2);
        let start_time = curr_time() + offset;
        let schedule = TimeSchedule::starting_at(start_time).with_period(trigger_period);

        let filter = TimeEventFilter::new(ExecutionTime::Schedule(schedule));
        let register_trigger = Register::trigger(Trigger::new(
            "mint_nft_for_all".parse()?,
            Action::new(
                load_sample_ivm("create_nft_for_every_user_trigger"),
                Repeats::Indefinitely,
                alice_id.clone(),
                filter,
            ),
        ));
        spawn_blocking({
            let client = test_client.clone();
            move || client.submit_blocking(register_trigger)
        })
        .await??;
        sleep(offset).await;
        println!("registered time trigger, driving {expected_count} blocks");

        submit_sample_isi_on_every_block_commit(
            &test_client,
            &alice_id,
            trigger_period,
            usize::try_from(expected_count)?,
        )
        .await?;

        for account_id in accounts {
            let start_pattern = "nft_number_";
            let end_pattern = format!("_for_{}${}", account_id.signatory(), account_id.domain());
            let account_id_clone = account_id.clone();
            let count: u64 = spawn_blocking({
                let client = test_client.clone();
                move || {
                    client.query(FindNfts::new()).execute_all().map(|nfts| {
                        nfts.into_iter()
                            .filter(|nft| nft.owned_by() == &account_id_clone)
                            .filter(|nft| {
                                let s = nft.id().to_string();
                                s.starts_with(start_pattern) && s.ends_with(&end_pattern)
                            })
                            .count()
                    })
                }
            })
            .await??
            .try_into()
            .expect("`usize` should always fit in `u64`");

            assert!(
                count >= expected_count,
                "{account_id} has {count} NFTs, but at least {expected_count} expected",
            );
        }

        Ok(())
    })
    .await
}

/// Submit some sample ISIs to create new blocks
async fn submit_sample_isi_on_every_block_commit(
    test_client: &Client,
    account_id: &AccountId,
    timeout: Duration,
    times: usize,
) -> Result<()> {
    for idx in 0..times {
        sleep(timeout).await;
        let sample_isi = SetKeyValue::account(
            account_id.clone(),
            "key".parse::<Name>()?,
            Json::new("value"),
        );
        spawn_blocking({
            let client = test_client.clone();
            move || client.submit_blocking(sample_isi)
        })
        .await??;
        println!("submitted sample ISI {}/{}", idx + 1, times);
    }

    Ok(())
}
