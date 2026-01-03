//! Integration tests for basic asset lifecycle operations.

use std::{
    sync::{Mutex, OnceLock},
    thread::sleep,
    time::{Duration, Instant},
};

use eyre::{Report, Result, eyre};
use integration_tests::{
    sandbox,
    sync::{get_status_with_retry, sync_after_submission},
};
use iroha::{
    client::{Client, Status},
    crypto::KeyPair,
    data_model::{
        ValidationFail,
        parameter::{Parameter, system::SumeragiParameter},
        prelude::*,
    },
    query::QueryError,
};
use iroha_data_model::query::error::{FindError, QueryExecutionFail};
use iroha_executor_data_model::permission::asset::CanTransferAsset;
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, gen_account_in};
use toml::Value as TomlValue;

static GENESIS_STATUS: OnceLock<std::result::Result<(), ()>> = OnceLock::new();
static START_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
const QUERY_RETRIES: usize = 240;
const QUERY_RETRY_DELAY: Duration = Duration::from_millis(500);
const NON_EMPTY_BLOCK_TIMEOUT: Duration = Duration::from_secs(300);

fn retry_query<T, F>(mut f: F) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    for attempt in 0..QUERY_RETRIES {
        match f() {
            Ok(value) => return Ok(value),
            Err(_err) if attempt + 1 < QUERY_RETRIES => sleep(QUERY_RETRY_DELAY),
            Err(err) => return Err(err),
        }
    }
    unreachable!()
}

fn wait_for_asset_definition_owner(
    client: &Client,
    asset_definition_id: &AssetDefinitionId,
    expected_owner: &AccountId,
    not_found_msg: &'static str,
    unexpected_owner_msg: &'static str,
    timeout_msg: &'static str,
) -> Result<()> {
    let deadline = Instant::now() + NON_EMPTY_BLOCK_TIMEOUT;
    loop {
        let last_err = match client.query(FindAssetsDefinitions::new()).execute_all() {
            Ok(definitions) => definitions
                .into_iter()
                .find(|asset_definition| asset_definition.id() == asset_definition_id)
                .map_or_else(
                    || Some(eyre!("{not_found_msg}")),
                    |asset_definition| {
                        if asset_definition.owned_by() == expected_owner {
                            None
                        } else {
                            Some(eyre!(
                                "{unexpected_owner_msg}: expected={expected_owner}, actual={}",
                                asset_definition.owned_by()
                            ))
                        }
                    },
                ),
            Err(err) => Some(Report::new(err)),
        };

        if last_err.is_none() {
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(last_err.unwrap_or_else(|| {
                eyre!("timed out waiting for {timeout_msg} after {NON_EMPTY_BLOCK_TIMEOUT:?}")
            }));
        }
        sleep(QUERY_RETRY_DELAY);
    }
}

/// Ensure noisy tracing is silenced before any network is spawned.
fn install_quiet_tracing() {
    static QUIET_TRACE: OnceLock<()> = OnceLock::new();
    QUIET_TRACE.get_or_init(|| {});
}

fn ivm_build_profile_exists() -> bool {
    use std::path::PathBuf;
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/ivm/target/prebuilt/build_config.toml")
        .exists()
}

fn quiet_network_builder() -> NetworkBuilder {
    install_quiet_tracing();
    init_instruction_registry();
    let mut sumeragi = toml::Table::new();
    sumeragi.insert("collectors_redundant_send_r".into(), TomlValue::Integer(3));
    sumeragi.insert("rbc_pending_ttl_ms".into(), TomlValue::Integer(120_000));
    sumeragi.insert("rbc_session_ttl_secs".into(), TomlValue::Integer(240));
    let mut layer = toml::Table::new();
    layer.insert("sumeragi".into(), TomlValue::Table(sumeragi));
    NetworkBuilder::new()
        .with_peers(4)
        .with_default_pipeline_time()
        // Make DA/RBC traffic more tolerant of dropped packets during local runs.
        .with_config_table(layer)
        // Keep on-chain parameters aligned with the overrides we pass via config layers so
        // consensus fingerprints and vote validation agree across peers.
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(3),
        )))
        .with_ivm_fuel(IvmFuelConfig::Unset)
}

fn submit_or_skip(
    client: &iroha::client::Client,
    instruction: impl Into<InstructionBox>,
    context: &str,
) -> Result<Option<()>> {
    sandbox::handle_result(client.submit_blocking(instruction).map(|_| ()), context)
}

fn submit_tx_or_skip(
    client: &iroha::client::Client,
    tx: &iroha::data_model::transaction::SignedTransaction,
    context: &str,
) -> Result<Option<()>> {
    match client.submit_transaction_blocking(tx) {
        Ok(_) => Ok(Some(())),
        Err(err) => {
            if is_tx_confirmation_timeout(&err) {
                eprintln!(
                    "warning: {context} confirmation timed out; continuing with state checks"
                );
                return Ok(Some(()));
            }
            sandbox::handle_result::<()>(Err(err), context)
        }
    }
}

fn submit_or_tolerate_timeout(
    client: &iroha::client::Client,
    instruction: impl Into<InstructionBox>,
    context: &str,
) -> Result<Option<()>> {
    match client.submit_blocking(instruction) {
        Ok(_) => Ok(Some(())),
        Err(err) => {
            if is_tx_confirmation_timeout(&err) {
                eprintln!(
                    "warning: {context} confirmation timed out; continuing with state checks"
                );
                return Ok(Some(()));
            }
            sandbox::handle_result::<()>(Err(err), context)
        }
    }
}

fn is_tx_confirmation_timeout(err: &Report) -> bool {
    const NEEDLES: [&str; 3] = [
        "haven't got tx confirmation within",
        "transaction queued for too long",
        "Connection dropped without `Committed/Applied` or `Rejected` event",
    ];
    err.chain().any(|cause| {
        let text = cause.to_string();
        NEEDLES.iter().any(|needle| text.contains(needle))
    })
}

fn start_test_network_with_builder(
    builder: NetworkBuilder,
) -> Option<(sandbox::SerializedNetwork, tokio::runtime::Runtime)> {
    if !ivm_build_profile_exists() {
        eprintln!("Skipping test: missing IVM build profile");
        return None;
    }

    if matches!(GENESIS_STATUS.get(), Some(Err(()))) {
        eprintln!("Skipping test: failed to start network (cached)");
        return None;
    }

    let serial_guard = sandbox::serial_guard();
    let guard = START_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();

    if matches!(GENESIS_STATUS.get(), Some(Err(()))) {
        eprintln!("Skipping test: failed to start network (cached)");
        drop(guard);
        return None;
    }

    let (network, runtime) = builder.with_min_peers(4).build_blocking();

    // Preflight binding check to avoid long hangs when the sandbox forbids sockets.
    if let Err(err) = network
        .peers()
        .iter()
        .try_for_each(|peer| -> std::io::Result<()> {
            std::net::TcpListener::bind(peer.p2p_address())?;
            std::net::TcpListener::bind(peer.api_address())?;
            Ok(())
        })
    {
        drop(guard);
        match sandbox::handle_result::<()>(Err(err.into()), "bind preflight") {
            Ok(None) => return None,
            Ok(Some(())) => unreachable!("sandbox handler should not return Some on Err"),
            Err(err) => panic!("failed during bind preflight: {err}"),
        }
    }

    if let Err(err) = runtime.block_on(async { network.start_all().await }) {
        drop(guard);
        match sandbox::handle_result::<()>(Err(err), "start test network") {
            Ok(None) => return None,
            Ok(Some(())) => unreachable!("sandbox handler should not return Some on Err"),
            Err(err) => {
                let _ = GENESIS_STATUS.set(Err(()));
                panic!("failed to start test network: {err}");
            }
        }
    }
    if let Err(err) = runtime.block_on(async { network.ensure_blocks(1).await }) {
        drop(guard);
        match sandbox::handle_result::<()>(Err(err), "reach block 1") {
            Ok(None) => return None,
            Ok(Some(())) => unreachable!("sandbox handler should not return Some on Err"),
            Err(err) => {
                let _ = GENESIS_STATUS.set(Err(()));
                panic!("failed to reach block 1: {err}");
            }
        }
    }

    let _ = GENESIS_STATUS.set(Ok(()));
    drop(guard);

    Some((
        sandbox::SerializedNetwork::new(network, serial_guard),
        runtime,
    ))
}

#[test]
// This test is also covered at the UI level in the iroha_cli tests
// in test_mint_assets.py
fn client_add_asset_quantity_to_existing_asset_should_increase_asset_amount() -> Result<()> {
    // Given
    let account_id = ALICE_ID.clone();
    let asset_definition_id = "xor#wonderland".parse::<AssetDefinitionId>()?;
    let create_asset =
        Register::asset_definition(AssetDefinition::numeric(asset_definition_id.clone()));

    let builder = quiet_network_builder();

    let Some((network, rt)) = start_test_network_with_builder(builder) else {
        return Ok(());
    };
    let test_client = network.client();

    if sandbox::handle_result(
        test_client.submit_blocking(create_asset),
        "register asset definition",
    )?
    .is_none()
    {
        return Ok(());
    }

    let metadata = iroha::data_model::metadata::Metadata::default();
    //When
    let quantity = numeric!(200);
    let mint = Mint::asset_numeric(
        quantity.clone(),
        AssetId::new(asset_definition_id.clone(), account_id.clone()),
    );
    let instructions: [InstructionBox; 1] = [mint.into()];
    let tx = test_client.build_transaction(instructions, metadata);
    if sandbox::handle_result(test_client.submit_transaction_blocking(&tx), "mint asset")?.is_none()
    {
        return Ok(());
    }
    if sandbox::handle_result(
        rt.block_on(async { network.ensure_blocks(2).await }),
        "wait for block with minted asset",
    )?
    .is_none()
    {
        return Ok(());
    }

    let asset = retry_query(|| {
        test_client
            .query(FindAssets::new())
            .execute_all()
            .map_err(eyre::Report::new)
    })?
    .into_iter()
    .filter(|asset| asset.id().account() == &account_id)
    .find(|asset| *asset.id().definition() == asset_definition_id)
    .ok_or_else(|| eyre::eyre!("expected asset to exist"))?;
    assert_eq!(*asset.value(), quantity);
    Ok(())
}

#[test]
fn client_add_big_asset_quantity_to_existing_asset_should_increase_asset_amount() -> Result<()> {
    // Given
    let account_id = ALICE_ID.clone();
    let asset_definition_id = "xor#wonderland".parse::<AssetDefinitionId>()?;
    let create_asset =
        Register::asset_definition(AssetDefinition::numeric(asset_definition_id.clone()));

    let builder = quiet_network_builder();

    let Some((network, rt)) = start_test_network_with_builder(builder) else {
        return Ok(());
    };
    let test_client = network.client();

    if sandbox::handle_result(
        test_client.submit_blocking(create_asset),
        "register asset definition",
    )?
    .is_none()
    {
        return Ok(());
    }

    let metadata = iroha::data_model::metadata::Metadata::default();
    // When
    let quantity = Numeric::new(2_u128.pow(65), 0);
    let mint = Mint::asset_numeric(
        quantity.clone(),
        AssetId::new(asset_definition_id.clone(), account_id.clone()),
    );
    let instructions: [InstructionBox; 1] = [mint.into()];
    let tx = test_client.build_transaction(instructions, metadata);
    if sandbox::handle_result(
        test_client.submit_transaction_blocking(&tx),
        "mint large asset",
    )?
    .is_none()
    {
        return Ok(());
    }
    if sandbox::handle_result(
        rt.block_on(async { network.ensure_blocks(2).await }),
        "wait for block with large minted asset",
    )?
    .is_none()
    {
        return Ok(());
    }

    let asset = retry_query(|| {
        test_client
            .query(FindAssets::new())
            .execute_all()
            .map_err(eyre::Report::new)
    })?
    .into_iter()
    .filter(|asset| asset.id().account() == &account_id)
    .find(|asset| *asset.id().definition() == asset_definition_id)
    .ok_or_else(|| eyre!("expected asset to exist"))?;
    assert_eq!(*asset.value(), quantity);
    Ok(())
}

#[test]
fn client_add_asset_with_decimal_should_increase_asset_amount() -> Result<()> {
    // Given
    let account_id = ALICE_ID.clone();
    let asset_definition_id = "xor#wonderland".parse::<AssetDefinitionId>()?;
    let asset_definition =
        AssetDefinition::new(asset_definition_id.clone(), NumericSpec::fractional(3));
    let create_asset = Register::asset_definition(asset_definition);

    let builder = quiet_network_builder();

    let Some((network, _rt)) = start_test_network_with_builder(builder) else {
        return Ok(());
    };
    let env_dir = network.env_dir().to_path_buf();
    let test_client = network.client();
    let torii = test_client.torii_url.clone();

    if submit_or_skip(&test_client, create_asset, "register asset definition")
        .map_err(|err| {
            err.wrap_err(format!(
                "register asset definition; torii={torii}, env_dir={}",
                env_dir.display()
            ))
        })?
        .is_none()
    {
        return Ok(());
    }

    let metadata = iroha::data_model::metadata::Metadata::default();

    //When
    let quantity = numeric!(123.456);
    let mint = Mint::asset_numeric(
        quantity.clone(),
        AssetId::new(asset_definition_id.clone(), account_id.clone()),
    );
    let instructions: [InstructionBox; 1] = [mint.into()];
    let tx = test_client.build_transaction(instructions, metadata);
    if submit_tx_or_skip(&test_client, &tx, "mint decimal asset")
        .map_err(|err| {
            err.wrap_err(format!(
                "mint decimal asset; torii={torii}, env_dir={}",
                env_dir.display()
            ))
        })?
        .is_none()
    {
        return Ok(());
    }

    let asset = retry_query(|| {
        test_client
            .query(FindAssets::new())
            .execute_all()
            .map_err(eyre::Report::new)
    })?
    .into_iter()
    .filter(|asset| asset.id().account() == &account_id)
    .find(|asset| *asset.id().definition() == asset_definition_id)
    .ok_or_else(|| eyre!("expected asset to exist"))?;
    assert_eq!(*asset.value(), quantity);

    // Add some fractional part
    let quantity2 = numeric!(0.55);
    let mint = Mint::asset_numeric(
        quantity2.clone(),
        AssetId::new(asset_definition_id.clone(), account_id.clone()),
    );
    // and check that it is added without errors
    let sum = quantity
        .checked_add(quantity2)
        .ok_or_else(|| eyre::eyre!("overflow"))?;
    if submit_or_skip(&test_client, mint, "mint fractional asset")
        .map_err(|err| {
            err.wrap_err(format!(
                "mint fractional asset; torii={torii}, env_dir={}",
                env_dir.display()
            ))
        })?
        .is_none()
    {
        return Ok(());
    }

    let asset = retry_query(|| {
        test_client
            .query(FindAssets::new())
            .execute_all()
            .map_err(eyre::Report::new)
    })?
    .into_iter()
    .filter(|asset| asset.id().account() == &account_id)
    .find(|asset| *asset.id().definition() == asset_definition_id)
    .unwrap();
    assert_eq!(*asset.value(), sum);

    Ok(())
}

#[allow(unused_must_use)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::expect_fun_call)]
#[test]
#[allow(clippy::unnecessary_wraps)]
fn find_rate_and_make_exchange_isi_should_succeed() -> Result<()> {
    let result: Result<()> = (|| {
        let (dex_id, _dex_keypair) = gen_account_in("exchange");
        let (seller_id, seller_keypair) = gen_account_in("company");
        let (buyer_id, buyer_keypair) = gen_account_in("company");
        let rate: AssetId = format!("btc/eth##{}", &dex_id)
            .parse()
            .expect("should be valid");
        let seller_btc: AssetId = format!("btc#crypto#{}", &seller_id)
            .parse()
            .expect("should be valid");
        let buyer_eth: AssetId = format!("eth#crypto#{}", &buyer_id)
            .parse()
            .expect("should be valid");

        let mut builder = quiet_network_builder();
        builder = builder
            .with_genesis_instruction(register::domain("exchange"))
            .with_genesis_instruction(register::domain("company"))
            .with_genesis_instruction(register::domain("crypto"))
            .with_genesis_instruction(register::account(dex_id.clone()))
            .with_genesis_instruction(register::account(seller_id.clone()))
            .with_genesis_instruction(register::account(buyer_id.clone()))
            .with_genesis_instruction(register::asset_definition_numeric("btc/eth#exchange"))
            .with_genesis_instruction(register::asset_definition_numeric("btc#crypto"))
            .with_genesis_instruction(register::asset_definition_numeric("eth#crypto"));

        let Some((network, rt)) = start_test_network_with_builder(builder) else {
            return Ok(());
        };
        let test_client = network.client();
        let mut status = match status_or_skip(test_client.get_status(), "initial status")? {
            Some(status) => status,
            None => return Ok(()),
        };
        let mut last_non_empty_height = status.blocks_non_empty;

        test_client.submit_all_blocking::<InstructionBox>([
            Mint::asset_numeric(20_u32, rate.clone()).into(),
            Mint::asset_numeric(10_u32, seller_btc.clone()).into(),
            Mint::asset_numeric(200_u32, buyer_eth.clone()).into(),
        ])?;
        status = match status_or_skip(
            sync_after_submission(
                &network,
                &rt,
                &test_client,
                last_non_empty_height,
                "seed exchange balances",
            ),
            "seed exchange balances",
        )? {
            Some(status) => status,
            None => return Ok(()),
        };
        last_non_empty_height = status.blocks_non_empty;

        let alice_id = ALICE_ID.clone();
        let mut alice_can_transfer_asset = |asset_id: AssetId,
                                            owner_key_pair: KeyPair|
         -> Result<()> {
            let permission = CanTransferAsset {
                asset: asset_id.clone(),
            };
            let instruction = Grant::account_permission(permission, alice_id.clone());
            let transaction = TransactionBuilder::new(
                ChainId::from("00000000-0000-0000-0000-000000000000"),
                asset_id.account().clone(),
            )
            .with_instructions([instruction])
            .sign(owner_key_pair.private_key());

            if submit_tx_or_skip(&test_client, &transaction, "grant transfer permission")?.is_none()
            {
                return Ok(());
            }
            status = match status_or_skip(
                sync_after_submission(
                    &network,
                    &rt,
                    &test_client,
                    last_non_empty_height,
                    "grant transfer permission",
                ),
                "grant transfer permission",
            )? {
                Some(status) => status,
                None => return Ok(()),
            };
            last_non_empty_height = status.blocks_non_empty;
            Ok::<(), eyre::Report>(())
        };
        alice_can_transfer_asset(seller_btc.clone(), seller_keypair)?;
        alice_can_transfer_asset(buyer_eth.clone(), buyer_keypair)?;

        let fetch_asset = |asset_id: &AssetId| -> Option<iroha_data_model::asset::Asset> {
            match test_client.query_single(FindAssetById::new(asset_id.clone())) {
                Ok(asset) => Some(asset),
                Err(QueryError::Validation(ValidationFail::QueryFailed(
                    QueryExecutionFail::Find(FindError::Asset(_)) | QueryExecutionFail::NotFound,
                ))) => None,
                Err(err) => panic!("query should succeed: {err:?}"),
            }
        };

        let assert_balance = |asset_id: AssetId, expected: Numeric| {
            let got = fetch_asset(&asset_id).expect("asset should exist");
            assert_eq!(*got.value(), expected);
        };
        // before: seller has $BTC10 and buyer has $ETH200
        assert_balance(seller_btc.clone(), numeric!(10));
        assert_balance(buyer_eth.clone(), numeric!(200));

        let rate_numeric = fetch_asset(&rate).expect("asset should exist");
        let rate: u32 = rate_numeric
            .value()
            .clone()
            .try_into()
            .expect("numeric should be u32 originally");
        test_client.submit_all_blocking([
            Transfer::asset_numeric(seller_btc.clone(), 10_u32, buyer_id.clone()),
            Transfer::asset_numeric(buyer_eth.clone(), 10_u32 * rate, seller_id.clone()),
        ])?;
        if let Some(_status) = status_or_skip(
            sync_after_submission(
                &network,
                &rt,
                &test_client,
                last_non_empty_height,
                "exchange transfers",
            ),
            "exchange transfers",
        )? {
        } else {
            return Ok(());
        }

        let assert_purged = |asset_id: AssetId| {
            let exists = fetch_asset(&asset_id).is_some();
            assert!(
                !exists,
                "query should fail, as zero assets are purged from accounts"
            );
        };
        let seller_eth: AssetId = format!("eth#crypto#{}", &seller_id)
            .parse()
            .expect("should be valid");
        let buyer_btc: AssetId = format!("btc#crypto#{}", &buyer_id)
            .parse()
            .expect("should be valid");
        // after: seller has $ETH200 and buyer has $BTC10
        assert_purged(seller_btc);
        assert_purged(buyer_eth);
        assert_balance(seller_eth, numeric!(200));
        assert_balance(buyer_btc, numeric!(10));

        Ok(())
    })();

    let _ = sandbox::handle_result(
        result,
        stringify!(find_rate_and_make_exchange_isi_should_succeed),
    )?;
    Ok(())
}

#[test]
#[allow(clippy::unnecessary_wraps)]
fn transfer_asset_definition() -> Result<()> {
    let alice_id = ALICE_ID.clone();
    // Create a destination account we can register (in a domain Alice can manage)
    let (new_owner_id, _kp) = gen_account_in("domain");
    let asset_definition_id: AssetDefinitionId = "asset#wonderland".parse().expect("Valid");

    let mut builder = quiet_network_builder();
    builder = builder
        .with_genesis_instruction(register::domain("domain"))
        .with_genesis_instruction(register::account(new_owner_id.clone()));

    let Some((network, _rt)) = start_test_network_with_builder(builder) else {
        return Ok(());
    };
    let test_client = network.client();

    if submit_or_tolerate_timeout(
        &test_client,
        Register::asset_definition(AssetDefinition::numeric(asset_definition_id.clone())),
        "register transferable asset definition",
    )?
    .is_none()
    {
        return Ok(());
    }

    wait_for_asset_definition_owner(
        &test_client,
        &asset_definition_id,
        &alice_id,
        "asset definition not found after registration",
        "unexpected asset definition owner",
        "asset definition registration",
    )?;

    if submit_or_tolerate_timeout(
        &test_client,
        Transfer::asset_definition(alice_id, asset_definition_id.clone(), new_owner_id.clone()),
        "transfer asset definition",
    )?
    .is_none()
    {
        return Ok(());
    }

    wait_for_asset_definition_owner(
        &test_client,
        &asset_definition_id,
        &new_owner_id,
        "asset definition not found after transfer",
        "unexpected asset definition owner after transfer",
        "asset definition transfer",
    )?;
    Ok(())
}

#[test]
#[allow(clippy::unnecessary_wraps)]
#[allow(clippy::too_many_lines)]
fn fail_if_dont_satisfy_spec() -> Result<()> {
    let result: Result<()> = (|| {
        let alice_id = ALICE_ID.clone();
        // Prepare a transferable destination account under a manageable domain
        let (dest_id, _kp) = gen_account_in("domain");

        let asset_definition_id: AssetDefinitionId = "asset#wonderland".parse().expect("Valid");
        let asset_id: AssetId = AssetId::new(asset_definition_id.clone(), alice_id.clone());
        // Create asset definition which accepts only integers
        let asset_definition =
            AssetDefinition::new(asset_definition_id.clone(), NumericSpec::integer());

        let mut builder = quiet_network_builder();
        builder = builder
            .with_genesis_instruction(register::domain("domain"))
            .with_genesis_instruction(register::account(dest_id.clone()));

        let Some((network, rt)) = start_test_network_with_builder(builder) else {
            return Ok(());
        };
        let env_dir = network.env_dir().to_path_buf();
        let test_client = network.client();
        let torii = test_client.torii_url.clone();
        let mut last_non_empty_height =
            match status_or_skip(get_status_with_retry(&test_client), "initial status").map_err(
                |err| {
                    err.wrap_err(format!(
                        "initial status; torii={torii}, env_dir={}",
                        env_dir.display()
                    ))
                },
            )? {
                Some(status) => status.blocks_non_empty,
                None => return Ok(()),
            };

        // Register and seed the asset definition under Alice's authority after genesis.
        sandbox::handle_result(
            test_client.submit_blocking(Register::asset_definition(asset_definition)),
            "register integer-only asset definition",
        )?;
        last_non_empty_height = match status_or_skip(
            sync_after_submission(
                &network,
                &rt,
                &test_client,
                last_non_empty_height,
                "register asset definition",
            ),
            "register asset definition",
        )
        .map_err(|err| {
            err.wrap_err(format!(
                "wait for non-empty block after register asset definition; torii={torii}, env_dir={}",
                env_dir.display()
            ))
        })? {
            Some(status) => status.blocks_non_empty,
            None => return Ok(()),
        };
        if submit_or_skip(
            &test_client,
            Mint::asset_numeric(numeric!(1), asset_id.clone()),
            "seed mint integer asset",
        )
        .map_err(|err| {
            err.wrap_err(format!(
                "seed mint integer asset; torii={torii}, env_dir={}",
                env_dir.display()
            ))
        })?
        .is_none()
        {
            return Ok(());
        }
        if status_or_skip(
            sync_after_submission(
                &network,
                &rt,
                &test_client,
                last_non_empty_height,
                "seed mint",
            ),
            "seed mint",
        )
        .map_err(|err| {
            err.wrap_err(format!(
                "sync after seed mint; torii={torii}, env_dir={}",
                env_dir.display()
            ))
        })?
        .is_none()
        {
            return Ok(());
        }
        let get_value = |id: &AssetId| -> Result<Numeric> {
            retry_query(|| {
                test_client
                    .query_single(FindAssetById { id: id.clone() })
                    .map(|asset| asset.value().clone())
                    .map_err(eyre::Report::new)
            })
        };
        let asset_exists = |id: &AssetId| -> Result<bool> {
            retry_query(
                || match test_client.query_single(FindAssetById { id: id.clone() }) {
                    Ok(_) => Ok(true),
                    Err(QueryError::Validation(ValidationFail::QueryFailed(
                        QueryExecutionFail::Find(FindError::Asset(_))
                        | QueryExecutionFail::NotFound,
                    ))) => Ok(false),
                    Err(err) => Err(eyre!(err)),
                },
            )
        };

        let isi = |value: Numeric| {
            [
                Mint::asset_numeric(value.clone(), asset_id.clone()).into(),
                Burn::asset_numeric(value.clone(), asset_id.clone()).into(),
                Transfer::asset_numeric(asset_id.clone(), value, dest_id.clone()).into(),
            ]
        };

        // Fail if submitting fractional value
        let fractional_value = numeric!(0.01);

        // No fractional operations should change the state
        let before = get_value(&asset_id)?;
        let dest_asset_id = AssetId::new(asset_definition_id.clone(), dest_id.clone());
        for op in isi(fractional_value) {
            match test_client.submit_blocking::<InstructionBox>(op) {
                Ok(_) => panic!("Should be rejected due to non integer value"),
                Err(err) => {
                    if let Some(reason) = sandbox::sandbox_reason(&eyre!(err.to_string())) {
                        return Err(eyre!(
                            "sandboxed network restriction detected during fractional rejection flow: {reason}"
                        ));
                    }
                }
            }

            // State unchanged for source and no asset created for destination
            let now = get_value(&asset_id)?;
            assert_eq!(now, before, "fractional op must not change balance");
            let dest_exists = asset_exists(&dest_asset_id)?;
            assert!(
                !dest_exists,
                "fractional transfer must not create destination asset"
            );
        }

        // Everything works fine when submitting proper integer value
        let mut sync_after = |context: &str| -> Result<Option<()>> {
            match status_or_skip(
                sync_after_submission(&network, &rt, &test_client, last_non_empty_height, context),
                context,
            )? {
                Some(status) => {
                    last_non_empty_height = status.blocks_non_empty;
                    Ok(Some(()))
                }
                None => Ok(None),
            }
        };
        let integer_value = numeric!(1);
        let expected_after_mint = before
            .clone()
            .checked_add(integer_value.clone())
            .ok_or_else(|| eyre!("integer mint overflow"))?;
        let expected_after_transfer = before
            .clone()
            .checked_sub(integer_value.clone())
            .ok_or_else(|| eyre!("integer transfer underflow"))?;

        if submit_or_tolerate_timeout(
            &test_client,
            Mint::asset_numeric(integer_value.clone(), asset_id.clone()),
            "integer mint",
        )?
        .is_none()
        {
            return Ok(());
        }
        if sync_after("integer mint")?.is_none() {
            return Ok(());
        }
        let after_mint = get_value(&asset_id)?;
        assert_eq!(after_mint, expected_after_mint);

        if submit_or_tolerate_timeout(
            &test_client,
            Burn::asset_numeric(integer_value.clone(), asset_id.clone()),
            "integer burn",
        )?
        .is_none()
        {
            return Ok(());
        }
        if sync_after("integer burn")?.is_none() {
            return Ok(());
        }
        let after_burn = get_value(&asset_id)?;
        assert_eq!(after_burn, before);

        if submit_or_tolerate_timeout(
            &test_client,
            Transfer::asset_numeric(asset_id.clone(), integer_value, dest_id.clone()),
            "integer transfer",
        )?
        .is_none()
        {
            return Ok(());
        }
        if sync_after("integer transfer")?.is_none() {
            return Ok(());
        }

        // After integer ops: asset moved to destination, zero at source (purged)
        let deadline = Instant::now() + NON_EMPTY_BLOCK_TIMEOUT;
        let mut last_err = None;
        loop {
            match (get_value(&dest_asset_id), asset_exists(&asset_id)) {
                (Ok(dest_val), Ok(source_exists))
                    if dest_val == numeric!(1)
                        && ((expected_after_transfer.is_zero() && !source_exists)
                            || (!expected_after_transfer.is_zero() && source_exists)) =>
                {
                    break;
                }
                (Ok(dest_val), Ok(source_exists)) => {
                    let expected_source = if expected_after_transfer.is_zero() {
                        "purged".to_string()
                    } else {
                        expected_after_transfer.to_string()
                    };
                    last_err = Some(eyre!(
                        "unexpected balances while waiting for integer ops: dest={dest_val}, source_exists={source_exists}, expected_source={expected_source}"
                    ));
                }
                (Err(err), _) | (_, Err(err)) => last_err = Some(err),
            }

            if Instant::now() >= deadline {
                return Err(last_err.unwrap_or_else(|| eyre!("timed out waiting for integer ops to settle after {NON_EMPTY_BLOCK_TIMEOUT:?}")).wrap_err(format!(
                    "final balance check; torii={torii}, env_dir={}",
                    env_dir.display()
                )));
            }
            sleep(Duration::from_millis(500));
        }

        drop(last_err);
        let dest_val = get_value(&dest_asset_id)?;
        assert_eq!(dest_val, numeric!(1));
        if expected_after_transfer.is_zero() {
            let source_exists = asset_exists(&asset_id)?;
            assert!(
                !source_exists,
                "zero assets are purged from accounts; source should be gone"
            );
        } else {
            let source_val = get_value(&asset_id)?;
            assert_eq!(source_val, expected_after_transfer);
        }

        Ok(())
    })();

    let _ = sandbox::handle_result(result, stringify!(fail_if_dont_satisfy_spec))?;
    Ok(())
}

fn status_or_skip(res: Result<Status>, context: &str) -> Result<Option<Status>> {
    sandbox::handle_result(res, context)
}

mod register {
    use super::*;

    pub fn domain(id: &str) -> Register<Domain> {
        Register::domain(Domain::new(id.parse().expect("should parse to DomainId")))
    }

    pub fn account(id: AccountId) -> Register<Account> {
        Register::account(Account::new(id))
    }

    pub fn asset_definition_numeric(id: &str) -> Register<AssetDefinition> {
        Register::asset_definition(AssetDefinition::numeric(
            id.parse().expect("should parse to AssetDefinitionId"),
        ))
    }
}
