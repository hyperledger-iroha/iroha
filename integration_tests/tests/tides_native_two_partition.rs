//! Real four-validator Tides vertical slice: two logical state partitions in one
//! deployed IVM contract, native scheduled progression, and restart-safe atomic
//! transfer. This does not claim independent consensus roots, parallel execution,
//! a complete game kernel, live XOR qualification, or execution-proof support.

use super::contracts::{
    contract_probe_call_intent, contract_state_json_value, deploy_contract_locally_signed,
    verify_contract_finality_everywhere, wait_for_contract_applied_height, wait_for_tx_applied,
};
use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    blocking::Client,
    client::{AccountTransactionDraft, FeeQuoteRequest},
};
use iroha_crypto::HashOf;
use iroha_data_model::{
    events::time::{ExecutionTime, Schedule, TimeEventFilter},
    executor::ValidationFail,
    isi::{Grant, InstructionBox, Register, SetParameter},
    parameter::{CustomParameter, CustomParameterId, Parameter},
    permission::Permission,
    smart_contract::{ContractAddress, ContractAlias},
    transaction::{
        Executable, FeePaymentIntent, SignedTransaction, error::TransactionRejectionReason,
    },
    trigger::{
        Trigger,
        action::{Action, Repeats},
    },
};
use iroha_executor_data_model::permission::{
    account::{AccountAliasPermissionScope, CanManageAccountAlias},
    governance::CanEnactGovernance,
    smart_contract::{CanInvokeContractEntrypoint, CanManageSmartContractCode},
};
use iroha_model_base::{metadata::Metadata, topology::DataSpaceId};
use iroha_test_network::{NetworkBuilder, read_on_dedicated_thread};
use sha2::Digest as _;
use std::{
    num::NonZeroU64,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const CONTEXT: &str = "tides_two_partition_native_restart";
const GAS_LIMIT: u64 = 20_000_000;
const ADVANCES: u32 = 56;
const OBSERVATION_TIMEOUT: Duration = Duration::from_secs(300);
const SOURCE: &str = include_str!("../../crates/ivm/examples/tides_partition_fixture/contract.ko");

fn artifact() -> Result<Vec<u8>> {
    kotodama_lang::compiler::Compiler::new()
        .compile_source(SOURCE)
        .map_err(|error| eyre!("compile Tides fixture: {error}"))
}

fn block_gas_parameter() -> Parameter {
    Parameter::Custom(CustomParameter::new(
        CustomParameterId::new("ivm_gas_limit_per_block".parse().expect("static parameter")),
        iroha_primitives::json::Json::new(50_000_000_u64),
    ))
}

async fn submit_call(
    client: &Client,
    code: &[u8],
    address: &ContractAddress,
    alias: &ContractAlias,
    entrypoint: &str,
) -> Result<HashOf<SignedTransaction>> {
    let intent = contract_probe_call_intent(code, address, alias, entrypoint)?;
    let account = client.account_client();
    let mut payload = account.prepare_transaction(
        AccountTransactionDraft::new(
            Executable::ContractCall(intent.invocation),
            FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(GAS_LIMIT)),
            intent.metadata,
        )
        .with_time_to_live(Duration::from_secs(660)),
    )?;
    let quote = account
        .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })
        .await?;
    ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "fee quote changed payer or gas bound"
    );
    payload.fee_payment = quote.intent;
    let signed = account.sign_transaction(payload)?;
    read_on_dedicated_thread({
        let client = client.clone();
        move || client.submit_transaction_and_wait(&signed)
    })
    .await
}

async fn state_int(
    http: &reqwest::Client,
    client: &Client,
    address: &ContractAddress,
    field: &str,
) -> Result<i64> {
    let value = contract_state_json_value(http, client.client().endpoint(), address, field).await?;
    let text = value
        .as_str()
        .ok_or_else(|| eyre!("{field} is not a canonical integer string: {value:?}"))?;
    text.parse()
        .map_err(|error| eyre!("invalid integer {field}: {error}"))
}

async fn await_state(
    http: &reqwest::Client,
    client: &Client,
    address: &ContractAddress,
    field: &str,
    minimum: i64,
) -> Result<()> {
    let deadline = Instant::now() + OBSERVATION_TIMEOUT;
    loop {
        let value = state_int(http, client, address, field).await?;
        if value >= minimum {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "{field} did not reach {minimum}; last value {value}"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn scheduled_call(
    code: &[u8],
    address: &ContractAddress,
    alias: &ContractAlias,
    name: &str,
    entrypoint: &str,
    at: Duration,
    repeats: u32,
    period: Option<Duration>,
) -> Result<InstructionBox> {
    let invocation = contract_probe_call_intent(code, address, alias, entrypoint)?.invocation;
    let schedule = period.map_or_else(
        || Schedule::starting_at(at),
        |period| Schedule::starting_at(at).with_period(period),
    );
    let action = Action::new(
        Executable::ContractCall(invocation),
        Repeats::Exactly(repeats),
        iroha_test_samples::ALICE_ID.clone(),
        TimeEventFilter::new(ExecutionTime::Schedule(schedule)),
    )
    .map_err(|error| eyre!("invalid scheduled callback: {error}"))?;
    let mut metadata = Metadata::default();
    metadata.insert(
        "native_progress_attempts".parse()?,
        iroha_primitives::json::Json::new(repeats),
    );
    let action = action.with_metadata(metadata);
    Ok(Register::trigger(Trigger::new(name.parse()?, action)).into())
}

fn assert_contract_rejection(
    result: Result<HashOf<SignedTransaction>>,
    name: &str,
    code: u32,
) -> Result<()> {
    let error = result
        .err()
        .ok_or_else(|| eyre!("invalid transfer unexpectedly committed"))?;
    let Some(TransactionRejectionReason::Validation(ValidationFail::ContractRejected(rejection))) =
        error.downcast_ref::<TransactionRejectionReason>()
    else {
        return Err(eyre!(
            "transport, timeout, fee or unrelated refusal is not a declared contract rejection: {error:?}"
        ));
    };
    ensure!(
        rejection.contract.as_ref() == "TidesTwoPartitionFixture"
            && rejection.name == name
            && rejection.code == code,
        "wrong manifest-authenticated contract rejection: {rejection:?}"
    );
    Ok(())
}

#[test]
fn tides_fixture_has_bounded_real_ivm_entrypoints() -> Result<()> {
    let code = artifact()?;
    let verified = ivm::verify_contract_artifact(&code)?;
    ensure!(
        verified.metadata.max_cycles == 0,
        "a non-ZK fixture declares no trace bound; gas is its only budget"
    );
    for name in [
        "hajimari",
        "advance_a",
        "advance_b",
        "transfer_a_to_b",
        "reject_transfer_a_to_b",
    ] {
        let entry = verified
            .contract_interface
            .entrypoints
            .iter()
            .find(|entry| entry.name == name)
            .ok_or_else(|| eyre!("missing real entrypoint {name}"))?;
        ensure!(
            entry.params.is_empty(),
            "scheduled callbacks must have no fixed arguments"
        );
    }
    Ok(())
}

#[test]
fn tides_two_partition_native_restart() -> Result<()> {
    let worker = std::thread::Builder::new()
        .name(CONTEXT.into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .thread_stack_size(64 * 1024 * 1024)
                .enable_all()
                .build()
                .expect("native Tides runtime")
                .block_on(run())
        })?;
    match worker.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

async fn run() -> Result<()> {
    let alias = ContractAlias::from_components("tides_partition_fixture", None, "universal")?;
    let alias_permission = CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Alias(
            iroha_data_model::alias_setup::ResolvedAccountAliasV1::new(
                "tides_partition_fixture@universal".parse()?,
                DataSpaceId::UNIVERSAL,
            ),
        ),
    };
    let mut builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(Duration::from_secs(1))
        .with_npos_consensus()
        .with_genesis_instruction(SetParameter::new(block_gas_parameter()))
        .with_config_layer(|layer| {
            layer
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    1_073_741_824_i64,
                )
                .write(["logger", "level"], "INFO")
                .write(
                    ["logger", "filter"],
                    "info,irohad::sccp_keeper=debug,iroha_core::smartcontracts::isi::sccp=debug",
                )
                .write(
                    [
                        "network",
                        "soranet_handshake",
                        "pow",
                        "puzzle",
                        "memory_kib",
                    ],
                    i64::from(iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB),
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "time_cost"],
                    1_i64,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "lanes"],
                    1_i64,
                );
        });
    for permission in [
        Permission::from(CanManageSmartContractCode),
        Permission::from(CanEnactGovernance),
        Permission::from(alias_permission),
    ] {
        builder = builder.with_genesis_instruction(Grant::account_permission(
            permission,
            iroha_test_samples::ALICE_ID.clone(),
        ));
    }
    let network = sandbox::start_network_async_or_skip(builder, CONTEXT)
        .await?
        .ok_or_else(|| eyre!("required real four-validator Tides network was skipped"))?;
    ensure!(
        network.peers().len() == 4,
        "four actual validators required"
    );
    network.ensure_blocks(1).await?;
    let client =
        integration_tests::sync::rebind_blocking_client(&network.peers()[0].client(), |config| {
            config.transaction_status_timeout = OBSERVATION_TIMEOUT;
            config.transaction_ttl = Some(Duration::from_secs(660));
        });
    let code = artifact()?;
    println!(
        "TIDES_NATIVE_DEPLOYED_ARTIFACT bytes={} sha256={}",
        code.len(),
        hex::encode(sha2::Sha256::digest(&code))
    );
    let http = integration_tests::http::client_with_timeout(Duration::from_secs(10));
    let (address, _, _, deployment) = read_on_dedicated_thread({
        let client = client.clone();
        let code = code.clone();
        let alias = alias.clone();
        move || deploy_contract_locally_signed(&client, &code, alias)
    })
    .await?;
    let deployment_height = wait_for_tx_applied(
        &http,
        client.client().endpoint(),
        &hex::encode(deployment.as_ref()),
        OBSERVATION_TIMEOUT,
        "Tides deploy",
    )
    .await?;
    network.ensure_blocks(deployment_height).await?;
    read_on_dedicated_thread({
        let client = client.clone();
        let address = address.clone();
        move || {
            client.submit(
                Grant::account_permission(
                    Permission::from(CanInvokeContractEntrypoint {
                        contract: address,
                        entrypoint: "hajimari".into(),
                    }),
                    iroha_test_samples::ALICE_ID.clone(),
                ),
                FeePaymentIntent::authority(Vec::new(), None),
            )
        }
    })
    .await?;
    submit_call(&client, &code, &address, &alias, "hajimari").await?;
    assert_contract_rejection(
        submit_call(&client, &code, &address, &alias, "reject_transfer_a_to_b").await,
        "ForcedRollback",
        4,
    )
    .context("rollback probe")?;
    ensure!(
        state_int(&http, &client, &address, "a_unit_present").await? == 1,
        "failed transfer lost source unit"
    );
    ensure!(
        state_int(&http, &client, &address, "b_unit_present").await? == 0,
        "failed transfer created destination unit"
    );
    ensure!(
        state_int(&http, &client, &address, "transfer_nonce").await? == 0,
        "failed transfer consumed nonce"
    );

    // Finite periodic schedules have exact bounded liability. No client keeps
    // the clock moving and no execution prover is spawned in this fixture.
    let start = SystemTime::now().duration_since(UNIX_EPOCH)? + Duration::from_secs(60);
    // Native Time groups overdue callbacks by trigger ID, not by deadline.
    // Explicit finite phases make transfer ordering unambiguous even after a
    // long block gap: the 18 pre-calls and transfer fit the 64-output phase.
    // The two post schedules can continue until their finite successes finish.
    let mut registrations = Vec::new();
    for area in ["a", "b"] {
        registrations.push(scheduled_call(
            &code,
            &address,
            &alias,
            &format!("tides_0_pre_{area}"),
            &format!("advance_{area}"),
            start,
            9,
            Some(Duration::from_secs(2)),
        )?);
    }
    registrations.push(scheduled_call(
        &code,
        &address,
        &alias,
        "tides_1_unit_transfer",
        "transfer_a_to_b",
        start + Duration::from_secs(17),
        1,
        None,
    )?);
    for area in ["a", "b"] {
        registrations.push(scheduled_call(
            &code,
            &address,
            &alias,
            &format!("tides_2_post_{area}"),
            &format!("advance_{area}"),
            start + Duration::from_secs(20),
            47,
            Some(Duration::from_secs(2)),
        )?);
    }
    let registered = read_on_dedicated_thread({
        let client = client.clone();
        move || client.submit_all(registrations, FeePaymentIntent::authority(Vec::new(), None))
    })
    .await?;
    let registration_height = wait_for_tx_applied(
        &http,
        client.client().endpoint(),
        &hex::encode(registered.as_ref()),
        OBSERVATION_TIMEOUT,
        "Tides schedules",
    )
    .await?;
    network.ensure_blocks(registration_height).await?;
    let clients_off_at = Instant::now();
    println!(
        "TIDES_NATIVE_OFFLINE_START registration_height={registration_height} player_writes_after_start=0 prover_processes=0 logical_partitions=2 validator_processes=4"
    );
    await_state(&http, &client, &address, "a_tick", 2).await?;
    await_state(&http, &client, &address, "b_tick", 2).await?;
    ensure!(
        state_int(&http, &client, &address, "transfer_nonce").await? == 0,
        "transfer committed before planned restart outage"
    );
    let restart_peer = network.peers()[3].clone();
    let config_layers = network.config_layers().collect::<Vec<_>>();
    ensure!(
        restart_peer.shutdown_if_started().await,
        "selected validator was not alive"
    );
    let dark_before = state_int(&http, &client, &address, "a_tick").await?;
    println!(
        "TIDES_NATIVE_OBSERVER_DARK_BEGIN duration_ms=10000 a_tick={dark_before} player_writes=0"
    );
    // Deliberately send no test-client Torii request during this interval.
    tokio::time::sleep(Duration::from_secs(10)).await;
    let dark_after = state_int(&http, &client, &address, "a_tick").await?;
    println!(
        "TIDES_NATIVE_OBSERVER_DARK_END a_tick={dark_after} tick_delta={}",
        dark_after - dark_before
    );
    // Three remaining validators finalize the already scheduled transfer. There
    // are no player writes, bridge services or synthetic heartbeat transactions.
    await_state(&http, &client, &address, "transfer_nonce", 1).await?;
    let transfer_observed = clients_off_at.elapsed();
    ensure!(
        state_int(&http, &client, &address, "a_unit_present").await? == 0,
        "source still owns transferred unit"
    );
    ensure!(
        state_int(&http, &client, &address, "b_unit_present").await? == 1,
        "destination missing transferred unit"
    );
    tokio::time::timeout(
        network.peer_startup_timeout(),
        restart_peer.start_checked(config_layers.iter(), None),
    )
    .await
    .map_err(|_| eyre!("Tides validator restart timed out"))??;
    await_state(&http, &client, &address, "a_tick", i64::from(ADVANCES)).await?;
    await_state(&http, &client, &address, "b_tick", i64::from(ADVANCES)).await?;
    let height = client.client().status().get().await?.blocks;
    network.ensure_blocks(height).await?;
    let fields = [
        "a_tick",
        "b_tick",
        "a_budget",
        "b_budget",
        "a_stock",
        "b_stock",
        "a_deposit",
        "b_deposit",
        "a_unit_present",
        "b_unit_present",
        "transfer_nonce",
        "a_capture_paid",
        "b_capture_paid",
        "a_bond",
        "b_bond",
        "a_owner",
        "b_owner",
        "a_cargo",
        "b_cargo",
        "a_upkeep_spent",
        "b_upkeep_spent",
        "a_unit_hp",
        "b_unit_hp",
        "a_guard_hp",
        "b_guard_hp",
        "a_base_hp",
        "b_base_hp",
        "a_capture_hold",
        "b_capture_hold",
    ];
    for field in fields {
        let expected = state_int(&http, &client, &address, field).await?;
        for (index, peer) in network.peers().iter().enumerate() {
            wait_for_contract_applied_height(peer, height, OBSERVATION_TIMEOUT).await?;
            ensure!(
                state_int(&http, &peer.client(), &address, field).await? == expected,
                "validator {index} recovered divergent {field}"
            );
        }
    }
    ensure!(
        state_int(&http, &client, &address, "a_tick").await? == i64::from(ADVANCES),
        "extra area A tick"
    );
    ensure!(
        state_int(&http, &client, &address, "b_tick").await? == i64::from(ADVANCES),
        "extra area B tick"
    );
    ensure!(
        state_int(&http, &client, &address, "a_budget").await? == 128 - i64::from(ADVANCES),
        "area A work budget not debited exactly once"
    );
    ensure!(
        state_int(&http, &client, &address, "b_budget").await? == 128 - i64::from(ADVANCES),
        "area B work budget not debited exactly once"
    );
    // Independent closed-form oracle for this bounded fixture: fourteen full
    // worker trips deliver 56; each of 56 advances consumes one upkeep and one
    // work unit. B has ample post-transfer time to breach and hold for 60 s.
    for area in ["a", "b"] {
        for (suffix, expected) in [
            ("stock", 100),
            ("deposit", 944),
            ("cargo", 0),
            ("upkeep_spent", 56),
            ("worker_phase", 0),
        ] {
            ensure!(
                state_int(&http, &client, &address, &format!("{area}_{suffix}")).await? == expected,
                "{area} {suffix} disagrees with independent fixture oracle"
            );
        }
        let mut conserved = 0;
        for suffix in ["stock", "deposit", "cargo", "upkeep_spent"] {
            conserved += state_int(&http, &client, &address, &format!("{area}_{suffix}")).await?;
        }
        ensure!(
            conserved == 1100,
            "{area} worker resources were created or lost: {conserved}"
        );
        ensure!(
            state_int(&http, &client, &address, &format!("{area}_bond")).await?
                + state_int(&http, &client, &address, &format!("{area}_capture_paid")).await?
                == 100,
            "{area} capture collateral counter conservation failed"
        );
    }
    for (field, expected) in [
        ("a_owner", 0),
        ("a_bond", 100),
        ("a_capture_paid", 0),
        ("b_owner", 1),
        ("b_bond", 80),
        ("b_capture_paid", 20),
        ("b_guard_hp", 0),
        ("b_base_hp", 0),
        ("a_guard_hp", 4),
        ("a_base_hp", 16),
        ("a_unit_hp", 0),
        ("b_unit_hp", 95),
        ("a_capture_hold", 0),
        ("b_capture_hold", 60),
    ] {
        ensure!(
            state_int(&http, &client, &address, field).await? == expected,
            "{field} disagrees with native kill/capture oracle"
        );
    }
    verify_contract_finality_everywhere(&network, height, None).await?;
    println!(
        "TIDES_NATIVE_OFFLINE_END height={height} elapsed_ms={} transfer_observed_ms={} advances_a={ADVANCES} advances_b={ADVANCES} restart_recovered=true player_writes=0 prover_processes=0",
        clients_off_at.elapsed().as_millis(),
        transfer_observed.as_millis()
    );
    assert_contract_rejection(
        submit_call(&client, &code, &address, &alias, "transfer_a_to_b").await,
        "AlreadyTransferred",
        1,
    )
    .context("replay probe")?;
    ensure!(
        state_int(&http, &client, &address, "transfer_nonce").await? == 1,
        "replay changed nonce"
    );
    ensure!(
        state_int(&http, &client, &address, "a_unit_present").await?
            + state_int(&http, &client, &address, "b_unit_present").await?
            == 1,
        "replay violated unit conservation"
    );
    network.shutdown_and_release().await;
    Ok(())
}
