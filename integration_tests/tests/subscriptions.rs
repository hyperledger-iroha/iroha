#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Subscription trigger integration tests.

use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime},
};

use eyre::{Result, WrapErr, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{Level, asset::AssetId, prelude::*},
};
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, BOB_ID, BOB_KEYPAIR};
use tokio::{
    task::spawn_blocking,
    time::{sleep, timeout},
};

use ivm::{ProgramMetadata, encoding, instruction, syscalls};

use iroha::data_model::subscription::{
    SUBSCRIPTION_INVOICE_METADATA_KEY, SUBSCRIPTION_METADATA_KEY, SUBSCRIPTION_PLAN_METADATA_KEY,
    SUBSCRIPTION_TRIGGER_REF_METADATA_KEY,
};

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

fn current_time() -> Duration {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time should be after unix epoch")
}

fn schedule_start(network: &sandbox::SerializedNetwork) -> (Duration, u64) {
    let now = current_time();
    let pipeline_time = network.pipeline_time();
    let gap = std::cmp::max(
        Duration::from_millis(200),
        pipeline_time.checked_div(2).unwrap_or(pipeline_time),
    );
    let start = now + gap;
    let start_ms = u64::try_from(start.as_millis()).expect("timestamp should fit in u64");
    (start, start_ms)
}

fn ivm_syscall_program(syscall: u32) -> IvmBytecode {
    let opcode = u8::try_from(syscall).expect("syscall should fit in u8");
    let mut code = Vec::new();
    code.extend_from_slice(
        &encoding::wide::encode_sys(instruction::wide::system::SCALL, opcode).to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut blob = ProgramMetadata::default().encode();
    blob.extend_from_slice(&code);
    IvmBytecode::from_compiled(blob)
}

fn asset_value(client: &Client, asset_id: &AssetId) -> Result<Numeric> {
    let assets = client.query(FindAssets::new()).execute_all()?;
    let asset = assets
        .into_iter()
        .find(|asset| asset.id() == asset_id)
        .ok_or_else(|| eyre!("asset {asset_id} not found"))?;
    Ok(asset.value().clone())
}

fn nft_metadata_value(client: &Client, nft_id: &NftId, key: &Name) -> Result<Option<Json>> {
    let nft = client
        .query(FindNfts::new())
        .execute_all()?
        .into_iter()
        .find(|nft| nft.id() == nft_id)
        .ok_or_else(|| eyre!("nft {nft_id} not found"))?;
    Ok(nft.content().get(key).cloned())
}

fn subscription_state_for_nft(client: &Client, nft_id: &NftId) -> Result<SubscriptionState> {
    let key: Name = SUBSCRIPTION_METADATA_KEY.parse()?;
    let Some(value) = nft_metadata_value(client, nft_id, &key)? else {
        return Err(eyre!("subscription metadata missing for {nft_id}"));
    };
    Ok(value.try_into_any_norito::<SubscriptionState>()?)
}

fn subscription_invoice_for_nft(
    client: &Client,
    nft_id: &NftId,
) -> Result<Option<SubscriptionInvoice>> {
    let key: Name = SUBSCRIPTION_INVOICE_METADATA_KEY.parse()?;
    let Some(value) = nft_metadata_value(client, nft_id, &key)? else {
        return Ok(None);
    };
    Ok(Some(value.try_into_any_norito::<SubscriptionInvoice>()?))
}

async fn tick_block(client: &Client) -> Result<()> {
    let client = client.clone();
    spawn_blocking(move || {
        client.submit_blocking(Log::new(
            Level::DEBUG,
            "subscription trigger tick".to_string(),
        ))
    })
    .await??;
    Ok(())
}

async fn wait_for_invoice_status(
    client: &Client,
    nft_id: &NftId,
    status: SubscriptionInvoiceStatus,
    sync_timeout: Duration,
    poll_delay: Duration,
) -> Result<SubscriptionInvoice> {
    timeout(sync_timeout, async {
        loop {
            tick_block(client).await?;
            let invoice = spawn_blocking({
                let client = client.clone();
                let nft_id = nft_id.clone();
                move || subscription_invoice_for_nft(&client, &nft_id)
            })
            .await??;
            if let Some(invoice) = invoice
                && invoice.status == status
            {
                return Ok(invoice);
            }
            sleep(poll_delay).await;
        }
    })
    .await
    .wrap_err("timed out waiting for subscription invoice")?
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines)]
async fn subscription_scenarios() -> Result<()> {
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new()
            .with_peers(4)
            .with_pipeline_time(std::time::Duration::from_secs(2)),
        stringify!(subscription_scenarios),
    )
    .await?
    else {
        return Ok(());
    };
    let client = network.client();
    network.ensure_blocks_with(|h| h.total >= 1).await?;

    subscription_usage_arrears_billing_charges_usage_scenario(&network, &client).await?;
    subscription_fixed_advance_billing_charges_future_period_scenario(&network, &client).await?;
    subscription_retry_grace_failure_marks_past_due_scenario(&network, &client).await?;

    Ok(())
}

#[allow(clippy::too_many_lines)]
async fn subscription_usage_arrears_billing_charges_usage_scenario(
    network: &sandbox::SerializedNetwork,
    client: &Client,
) -> Result<()> {
    let client = client.clone();
    run_or_skip(
        stringify!(subscription_usage_arrears_billing_charges_usage),
        || async {
            let provider = ALICE_ID.clone();
            let subscriber = BOB_ID.clone();
            let charge_def_id: AssetDefinitionId = "usd#wonderland".parse()?;
            let plan_id: AssetDefinitionId = "usage_plan#wonderland".parse()?;
            let billing_trigger_id: TriggerId = "usage_billing".parse()?;
            let usage_trigger_id: TriggerId = "usage_record".parse()?;
            let nft_id: NftId = "subscription_usage$wonderland".parse()?;
            let unit_key: Name = "requests".parse()?;
            let period_ms = 2_000_u64;
            let retry_backoff_ms = 500_u64;
            let (start_time, start_ms) = schedule_start(&network);

            let unit_price = Numeric::new(2_u32, 0);
            let usage_delta = Numeric::new(3_u32, 0);
            let amount = Numeric::new(6_u32, 0);
            let initial_balance = Numeric::new(100_u32, 0);

            spawn_blocking({
                let client = client.clone();
                let charge_def_id = charge_def_id.clone();
                move || {
                    client.submit_blocking(Register::asset_definition(AssetDefinition::numeric(
                        charge_def_id,
                    )))
                }
            })
            .await??;
            spawn_blocking({
                let client = client.clone();
                let plan_id = plan_id.clone();
                move || {
                    client.submit_blocking(Register::asset_definition(AssetDefinition::numeric(
                        plan_id,
                    )))
                }
            })
            .await??;

            let plan = SubscriptionPlan {
                provider: provider.clone(),
                billing: SubscriptionBilling {
                    cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
                        period_ms,
                    }),
                    bill_for: SubscriptionBillFor::PreviousPeriod,
                    retry_backoff_ms,
                    max_failures: 3,
                    grace_ms: 0,
                },
                pricing: SubscriptionPricing::Usage(SubscriptionUsagePricing {
                    unit_price: unit_price.clone(),
                    unit_key: unit_key.clone(),
                    asset_definition: charge_def_id.clone(),
                }),
            };
            let plan_key: Name = SUBSCRIPTION_PLAN_METADATA_KEY.parse()?;
            spawn_blocking({
                let client = client.clone();
                let plan_id = plan_id.clone();
                let plan_key = plan_key.clone();
                let plan = plan.clone();
                move || {
                    client.submit_blocking(SetKeyValue::asset_definition(
                        plan_id,
                        plan_key,
                        Json::new(plan),
                    ))
                }
            })
            .await??;

            let asset_id = AssetId::new(charge_def_id.clone(), subscriber.clone());
            spawn_blocking({
                let client = client.clone();
                let asset_id = asset_id.clone();
                let amount = initial_balance.clone();
                move || client.submit_blocking(Mint::asset_numeric(amount, asset_id))
            })
            .await??;
            let bob_client = network
                .peer()
                .client_for(&subscriber, BOB_KEYPAIR.private_key().clone());
            let transfer_permission: Permission =
                iroha_executor_data_model::permission::asset::CanTransferAsset {
                    asset: asset_id.clone(),
                }
                .into();
            spawn_blocking({
                let client = bob_client.clone();
                let provider = provider.clone();
                move || {
                    client.submit_blocking(Grant::account_permission(transfer_permission, provider))
                }
            })
            .await??;

            let subscription_state = SubscriptionState {
                plan_id: plan_id.clone(),
                provider: provider.clone(),
                subscriber: subscriber.clone(),
                status: SubscriptionStatus::Active,
                current_period_start_ms: start_ms - period_ms,
                current_period_end_ms: start_ms,
                next_charge_ms: start_ms,
                cancel_at_period_end: false,
                cancel_at_ms: None,
                failure_count: 0,
                usage_accumulated: BTreeMap::new(),
                billing_trigger_id: billing_trigger_id.clone(),
            };
            let mut metadata = Metadata::default();
            let subscription_key: Name = SUBSCRIPTION_METADATA_KEY.parse()?;
            metadata.insert(subscription_key, Json::new(subscription_state.clone()));
            spawn_blocking({
                let client = client.clone();
                let nft_id = nft_id.clone();
                let metadata = metadata.clone();
                move || client.submit_blocking(Register::nft(Nft::new(nft_id, metadata)))
            })
            .await??;
            spawn_blocking({
                let client = client.clone();
                let nft_id = nft_id.clone();
                let provider = provider.clone();
                let subscriber = subscriber.clone();
                move || client.submit_blocking(Transfer::nft(provider, nft_id, subscriber))
            })
            .await??;

            let usage_program = ivm_syscall_program(syscalls::SYSCALL_SUBSCRIPTION_RECORD_USAGE);
            let usage_action = Action::new(
                Executable::Ivm(usage_program),
                Repeats::Indefinitely,
                provider.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(usage_trigger_id.clone())
                    .under_authority(provider.clone()),
            );
            spawn_blocking({
                let client = client.clone();
                let usage_trigger_id = usage_trigger_id.clone();
                let usage_action = usage_action.clone();
                move || {
                    client.submit_blocking(Register::trigger(Trigger::new(
                        usage_trigger_id,
                        usage_action,
                    )))
                }
            })
            .await??;

            let usage_args = SubscriptionUsageDelta {
                subscription_nft_id: nft_id.clone(),
                unit_key: unit_key.clone(),
                delta: usage_delta.clone(),
            };
            spawn_blocking({
                let client = client.clone();
                let usage_trigger_id = usage_trigger_id.clone();
                move || {
                    client.submit_blocking(
                        ExecuteTrigger::new(usage_trigger_id).with_args(usage_args),
                    )
                }
            })
            .await??;

            let usage_state = spawn_blocking({
                let client = client.clone();
                let nft_id = nft_id.clone();
                move || subscription_state_for_nft(&client, &nft_id)
            })
            .await??;
            assert_eq!(
                usage_state.usage_accumulated.get(&unit_key),
                Some(&usage_delta)
            );

            let billing_program = ivm_syscall_program(syscalls::SYSCALL_SUBSCRIPTION_BILL);
            let mut trigger_metadata = Metadata::default();
            let trigger_ref_key: Name = SUBSCRIPTION_TRIGGER_REF_METADATA_KEY.parse()?;
            trigger_metadata.insert(
                trigger_ref_key,
                Json::new(SubscriptionTriggerRef {
                    subscription_nft_id: nft_id.clone(),
                }),
            );
            let schedule = TimeSchedule::starting_at(start_time);
            let billing_action = Action::new(
                Executable::Ivm(billing_program),
                Repeats::Exactly(1),
                provider.clone(),
                TimeEventFilter::new(ExecutionTime::Schedule(schedule)),
            )
            .with_metadata(trigger_metadata);
            spawn_blocking({
                let client = client.clone();
                let billing_trigger_id = billing_trigger_id.clone();
                let billing_action = billing_action.clone();
                move || {
                    client.submit_blocking(Register::trigger(Trigger::new(
                        billing_trigger_id,
                        billing_action,
                    )))
                }
            })
            .await??;

            let poll_delay = std::cmp::max(
                Duration::from_millis(200),
                network
                    .pipeline_time()
                    .checked_div(2)
                    .unwrap_or_else(|| network.pipeline_time()),
            );
            let invoice = wait_for_invoice_status(
                &client,
                &nft_id,
                SubscriptionInvoiceStatus::Paid,
                network.sync_timeout(),
                poll_delay,
            )
            .await?;

            assert_eq!(invoice.subscription_nft_id, nft_id);
            assert_eq!(invoice.amount, amount);
            assert_eq!(invoice.asset_definition, charge_def_id);
            assert_eq!(invoice.period_start_ms, start_ms - period_ms);
            assert_eq!(invoice.period_end_ms, start_ms);

            let billed_state = spawn_blocking({
                let client = client.clone();
                let nft_id = nft_id.clone();
                move || subscription_state_for_nft(&client, &nft_id)
            })
            .await??;
            assert_eq!(billed_state.status, SubscriptionStatus::Active);
            assert_eq!(billed_state.failure_count, 0);
            assert_eq!(billed_state.next_charge_ms, start_ms + period_ms);
            assert_eq!(billed_state.current_period_start_ms, start_ms);
            assert_eq!(billed_state.current_period_end_ms, start_ms + period_ms);
            assert!(!billed_state.usage_accumulated.contains_key(&unit_key));

            let final_balance = spawn_blocking({
                let client = client.clone();
                let asset_id = asset_id.clone();
                move || asset_value(&client, &asset_id)
            })
            .await??;
            let expected_balance = initial_balance
                .checked_sub(amount)
                .expect("balance should cover usage bill");
            assert_eq!(final_balance, expected_balance);

            Ok(())
        },
    )
    .await
}

#[allow(clippy::too_many_lines)]
async fn subscription_fixed_advance_billing_charges_future_period_scenario(
    network: &sandbox::SerializedNetwork,
    client: &Client,
) -> Result<()> {
    let client = client.clone();
    run_or_skip(
        stringify!(subscription_fixed_advance_billing_charges_future_period),
        || async {
            let provider = ALICE_ID.clone();
            let subscriber = BOB_ID.clone();
            let charge_def_id: AssetDefinitionId = "usd_fixed#wonderland".parse()?;
            let plan_id: AssetDefinitionId = "fixed_plan#wonderland".parse()?;
            let billing_trigger_id: TriggerId = "fixed_billing".parse()?;
            let nft_id: NftId = "subscription_fixed$wonderland".parse()?;
            let period_ms = 3_000_u64;
            let (start_time, start_ms) = schedule_start(&network);

            let amount = Numeric::new(120_u32, 0);
            let initial_balance = Numeric::new(300_u32, 0);

            spawn_blocking({
                let client = client.clone();
                let charge_def_id = charge_def_id.clone();
                move || {
                    client.submit_blocking(Register::asset_definition(AssetDefinition::numeric(
                        charge_def_id,
                    )))
                }
            })
            .await??;
            spawn_blocking({
                let client = client.clone();
                let plan_id = plan_id.clone();
                move || {
                    client.submit_blocking(Register::asset_definition(AssetDefinition::numeric(
                        plan_id,
                    )))
                }
            })
            .await??;

            let plan = SubscriptionPlan {
                provider: provider.clone(),
                billing: SubscriptionBilling {
                    cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
                        period_ms,
                    }),
                    bill_for: SubscriptionBillFor::NextPeriod,
                    retry_backoff_ms: 1_000,
                    max_failures: 3,
                    grace_ms: 0,
                },
                pricing: SubscriptionPricing::Fixed(SubscriptionFixedPricing {
                    amount: amount.clone(),
                    asset_definition: charge_def_id.clone(),
                }),
            };
            let plan_key: Name = SUBSCRIPTION_PLAN_METADATA_KEY.parse()?;
            spawn_blocking({
                let client = client.clone();
                let plan_id = plan_id.clone();
                let plan_key = plan_key.clone();
                let plan = plan.clone();
                move || {
                    client.submit_blocking(SetKeyValue::asset_definition(
                        plan_id,
                        plan_key,
                        Json::new(plan),
                    ))
                }
            })
            .await??;

            let asset_id = AssetId::new(charge_def_id.clone(), subscriber.clone());
            spawn_blocking({
                let client = client.clone();
                let asset_id = asset_id.clone();
                let amount = initial_balance.clone();
                move || client.submit_blocking(Mint::asset_numeric(amount, asset_id))
            })
            .await??;
            let bob_client = network
                .peer()
                .client_for(&subscriber, BOB_KEYPAIR.private_key().clone());
            let transfer_permission: Permission =
                iroha_executor_data_model::permission::asset::CanTransferAsset {
                    asset: asset_id.clone(),
                }
                .into();
            spawn_blocking({
                let client = bob_client.clone();
                let provider = provider.clone();
                move || {
                    client.submit_blocking(Grant::account_permission(transfer_permission, provider))
                }
            })
            .await??;

            let subscription_state = SubscriptionState {
                plan_id: plan_id.clone(),
                provider: provider.clone(),
                subscriber: subscriber.clone(),
                status: SubscriptionStatus::Active,
                current_period_start_ms: start_ms,
                current_period_end_ms: start_ms + period_ms,
                next_charge_ms: start_ms,
                cancel_at_period_end: false,
                cancel_at_ms: None,
                failure_count: 0,
                usage_accumulated: BTreeMap::new(),
                billing_trigger_id: billing_trigger_id.clone(),
            };
            let mut metadata = Metadata::default();
            let subscription_key: Name = SUBSCRIPTION_METADATA_KEY.parse()?;
            metadata.insert(subscription_key, Json::new(subscription_state.clone()));
            spawn_blocking({
                let client = client.clone();
                let nft_id = nft_id.clone();
                let metadata = metadata.clone();
                move || client.submit_blocking(Register::nft(Nft::new(nft_id, metadata)))
            })
            .await??;
            spawn_blocking({
                let client = client.clone();
                let nft_id = nft_id.clone();
                let provider = provider.clone();
                let subscriber = subscriber.clone();
                move || client.submit_blocking(Transfer::nft(provider, nft_id, subscriber))
            })
            .await??;

            let billing_program = ivm_syscall_program(syscalls::SYSCALL_SUBSCRIPTION_BILL);
            let mut trigger_metadata = Metadata::default();
            let trigger_ref_key: Name = SUBSCRIPTION_TRIGGER_REF_METADATA_KEY.parse()?;
            trigger_metadata.insert(
                trigger_ref_key,
                Json::new(SubscriptionTriggerRef {
                    subscription_nft_id: nft_id.clone(),
                }),
            );
            let schedule = TimeSchedule::starting_at(start_time);
            let billing_action = Action::new(
                Executable::Ivm(billing_program),
                Repeats::Exactly(1),
                provider.clone(),
                TimeEventFilter::new(ExecutionTime::Schedule(schedule)),
            )
            .with_metadata(trigger_metadata);
            spawn_blocking({
                let client = client.clone();
                let billing_trigger_id = billing_trigger_id.clone();
                let billing_action = billing_action.clone();
                move || {
                    client.submit_blocking(Register::trigger(Trigger::new(
                        billing_trigger_id,
                        billing_action,
                    )))
                }
            })
            .await??;

            let poll_delay = std::cmp::max(
                Duration::from_millis(200),
                network
                    .pipeline_time()
                    .checked_div(2)
                    .unwrap_or_else(|| network.pipeline_time()),
            );
            let invoice = wait_for_invoice_status(
                &client,
                &nft_id,
                SubscriptionInvoiceStatus::Paid,
                network.sync_timeout(),
                poll_delay,
            )
            .await?;

            assert_eq!(invoice.amount, amount);
            assert_eq!(invoice.asset_definition, charge_def_id);
            assert_eq!(invoice.period_start_ms, start_ms);
            assert_eq!(invoice.period_end_ms, start_ms + period_ms);

            let billed_state = spawn_blocking({
                let client = client.clone();
                let nft_id = nft_id.clone();
                move || subscription_state_for_nft(&client, &nft_id)
            })
            .await??;
            assert_eq!(billed_state.status, SubscriptionStatus::Active);
            assert_eq!(billed_state.current_period_start_ms, start_ms);
            assert_eq!(billed_state.current_period_end_ms, start_ms + period_ms);
            assert_eq!(billed_state.next_charge_ms, start_ms + period_ms);

            let final_balance = spawn_blocking({
                let client = client.clone();
                let asset_id = asset_id.clone();
                move || asset_value(&client, &asset_id)
            })
            .await??;
            let expected_balance = initial_balance
                .checked_sub(amount)
                .expect("balance should cover fixed bill");
            assert_eq!(final_balance, expected_balance);

            Ok(())
        },
    )
    .await
}

#[allow(clippy::too_many_lines)]
async fn subscription_retry_grace_failure_marks_past_due_scenario(
    network: &sandbox::SerializedNetwork,
    client: &Client,
) -> Result<()> {
    let client = client.clone();
    run_or_skip(
        stringify!(subscription_retry_grace_failure_marks_past_due),
        || async {
            let provider = ALICE_ID.clone();
            let subscriber = BOB_ID.clone();
            let charge_def_id: AssetDefinitionId = "usd_retry#wonderland".parse()?;
            let plan_id: AssetDefinitionId = "retry_plan#wonderland".parse()?;
            let billing_trigger_id: TriggerId = "retry_billing".parse()?;
            let nft_id: NftId = "subscription_retry$wonderland".parse()?;
            let period_ms = 1_500_u64;
            let retry_backoff_ms = 750_u64;
            let (start_time, start_ms) = schedule_start(&network);

            let amount = Numeric::new(120_u32, 0);
            let initial_balance = Numeric::new(50_u32, 0);

            spawn_blocking({
                let client = client.clone();
                let charge_def_id = charge_def_id.clone();
                move || {
                    client.submit_blocking(Register::asset_definition(AssetDefinition::numeric(
                        charge_def_id,
                    )))
                }
            })
            .await??;
            spawn_blocking({
                let client = client.clone();
                let plan_id = plan_id.clone();
                move || {
                    client.submit_blocking(Register::asset_definition(AssetDefinition::numeric(
                        plan_id,
                    )))
                }
            })
            .await??;

            let plan = SubscriptionPlan {
                provider: provider.clone(),
                billing: SubscriptionBilling {
                    cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
                        period_ms,
                    }),
                    bill_for: SubscriptionBillFor::PreviousPeriod,
                    retry_backoff_ms,
                    max_failures: 3,
                    grace_ms: 0,
                },
                pricing: SubscriptionPricing::Fixed(SubscriptionFixedPricing {
                    amount: amount.clone(),
                    asset_definition: charge_def_id.clone(),
                }),
            };
            let plan_key: Name = SUBSCRIPTION_PLAN_METADATA_KEY.parse()?;
            spawn_blocking({
                let client = client.clone();
                let plan_id = plan_id.clone();
                let plan_key = plan_key.clone();
                let plan = plan.clone();
                move || {
                    client.submit_blocking(SetKeyValue::asset_definition(
                        plan_id,
                        plan_key,
                        Json::new(plan),
                    ))
                }
            })
            .await??;

            let asset_id = AssetId::new(charge_def_id.clone(), subscriber.clone());
            spawn_blocking({
                let client = client.clone();
                let asset_id = asset_id.clone();
                let amount = initial_balance.clone();
                move || client.submit_blocking(Mint::asset_numeric(amount, asset_id))
            })
            .await??;

            let subscription_state = SubscriptionState {
                plan_id: plan_id.clone(),
                provider: provider.clone(),
                subscriber: subscriber.clone(),
                status: SubscriptionStatus::Active,
                current_period_start_ms: start_ms - period_ms,
                current_period_end_ms: start_ms,
                next_charge_ms: start_ms,
                cancel_at_period_end: false,
                cancel_at_ms: None,
                failure_count: 0,
                usage_accumulated: BTreeMap::new(),
                billing_trigger_id: billing_trigger_id.clone(),
            };
            let mut metadata = Metadata::default();
            let subscription_key: Name = SUBSCRIPTION_METADATA_KEY.parse()?;
            metadata.insert(subscription_key, Json::new(subscription_state.clone()));
            spawn_blocking({
                let client = client.clone();
                let nft_id = nft_id.clone();
                let metadata = metadata.clone();
                move || client.submit_blocking(Register::nft(Nft::new(nft_id, metadata)))
            })
            .await??;
            spawn_blocking({
                let client = client.clone();
                let nft_id = nft_id.clone();
                let provider = provider.clone();
                let subscriber = subscriber.clone();
                move || client.submit_blocking(Transfer::nft(provider, nft_id, subscriber))
            })
            .await??;

            let billing_program = ivm_syscall_program(syscalls::SYSCALL_SUBSCRIPTION_BILL);
            let mut trigger_metadata = Metadata::default();
            let trigger_ref_key: Name = SUBSCRIPTION_TRIGGER_REF_METADATA_KEY.parse()?;
            trigger_metadata.insert(
                trigger_ref_key,
                Json::new(SubscriptionTriggerRef {
                    subscription_nft_id: nft_id.clone(),
                }),
            );
            let schedule = TimeSchedule::starting_at(start_time);
            let billing_action = Action::new(
                Executable::Ivm(billing_program),
                Repeats::Exactly(1),
                provider.clone(),
                TimeEventFilter::new(ExecutionTime::Schedule(schedule)),
            )
            .with_metadata(trigger_metadata);
            spawn_blocking({
                let client = client.clone();
                let billing_trigger_id = billing_trigger_id.clone();
                let billing_action = billing_action.clone();
                move || {
                    client.submit_blocking(Register::trigger(Trigger::new(
                        billing_trigger_id,
                        billing_action,
                    )))
                }
            })
            .await??;

            let poll_delay = std::cmp::max(
                Duration::from_millis(200),
                network
                    .pipeline_time()
                    .checked_div(2)
                    .unwrap_or_else(|| network.pipeline_time()),
            );
            let invoice = wait_for_invoice_status(
                &client,
                &nft_id,
                SubscriptionInvoiceStatus::Failed,
                network.sync_timeout(),
                poll_delay,
            )
            .await?;

            assert_eq!(invoice.subscription_nft_id, nft_id);
            assert_eq!(invoice.amount, amount);
            assert_eq!(invoice.asset_definition, charge_def_id);
            assert_eq!(invoice.period_start_ms, start_ms - period_ms);
            assert_eq!(invoice.period_end_ms, start_ms);

            let billed_state = spawn_blocking({
                let client = client.clone();
                let nft_id = nft_id.clone();
                move || subscription_state_for_nft(&client, &nft_id)
            })
            .await??;
            assert_eq!(billed_state.status, SubscriptionStatus::PastDue);
            assert_eq!(billed_state.failure_count, 1);
            assert_eq!(
                billed_state.next_charge_ms,
                invoice.attempted_at_ms.saturating_add(retry_backoff_ms)
            );

            let final_balance = spawn_blocking({
                let client = client.clone();
                let asset_id = asset_id.clone();
                move || asset_value(&client, &asset_id)
            })
            .await??;
            assert_eq!(final_balance, initial_balance);

            Ok(())
        },
    )
    .await
}
