//! Integration tests for basic asset lifecycle operations.

use std::{
    sync::OnceLock,
    thread::sleep,
    time::{Duration, Instant},
};

use eyre::{Report, Result, WrapErr, eyre};
use integration_tests::{
    sandbox,
    sync::{get_status_with_retry_or_storage, sync_after_submission},
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
use iroha_executor_data_model::permission::{
    asset::CanTransferAsset, asset_definition::CanRegisterAssetDefinition,
};
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, gen_account_in};
use toml::Value as TomlValue;

static GENESIS_STATUS: OnceLock<std::result::Result<(), ()>> = OnceLock::new();
static SERIAL_NETWORK_GUARD: OnceLock<sandbox::NetworkParallelismGuard> = OnceLock::new();
const QUERY_RETRIES: usize = 1_200;
const QUERY_RETRY_DELAY: Duration = Duration::from_millis(100);
const NON_EMPTY_BLOCK_TIMEOUT: Duration = Duration::from_secs(600);
// DA-enabled consensus needs a wider pipeline in local test runs to avoid view-change stalls.
const FAST_PIPELINE_TIME: Duration = Duration::from_secs(6);

struct ClientPool {
    clients: Vec<Client>,
    index: usize,
}

impl ClientPool {
    fn new(network: &Network) -> Self {
        let clients = network
            .peers()
            .iter()
            .map(NetworkPeer::client)
            .collect::<Vec<_>>();
        let clients = if clients.is_empty() {
            vec![network.client()]
        } else {
            clients
        };
        Self { clients, index: 0 }
    }

    fn len(&self) -> usize {
        self.clients.len()
    }

    fn current(&self) -> &Client {
        &self.clients[self.index]
    }

    fn next(&mut self) -> &Client {
        let idx = self.index;
        self.index = (self.index + 1) % self.clients.len();
        &self.clients[idx]
    }
}

fn is_transient_client_error(err: &Report) -> bool {
    const NEEDLES: [&str; 6] = [
        "Failed to send http",
        "error sending request for url",
        "operation timed out",
        "Connection refused",
        "connection closed",
        "connection reset",
    ];
    err.chain().any(|cause| {
        let text = cause.to_string();
        NEEDLES.iter().any(|needle| text.contains(needle))
    })
}

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

fn asset_value(clients: &mut ClientPool, asset_id: &AssetId) -> Result<Numeric> {
    retry_query(|| {
        let client = clients.next();
        client
            .query_single(FindAssetById {
                id: asset_id.clone(),
            })
            .map(|asset| asset.value().clone())
            .map_err(Report::new)
    })
}

fn asset_exists(clients: &mut ClientPool, asset_id: &AssetId) -> Result<bool> {
    retry_query(|| {
        let client = clients.next();
        match client.query_single(FindAssetById {
            id: asset_id.clone(),
        }) {
            Ok(_) => Ok(true),
            Err(QueryError::Validation(ValidationFail::QueryFailed(
                QueryExecutionFail::Find(FindError::Asset(_)) | QueryExecutionFail::NotFound,
            ))) => Ok(false),
            Err(err) => Err(eyre!(err)),
        }
    })
}

fn wait_for_asset_definition_owner(
    clients: &mut ClientPool,
    asset_definition_id: &AssetDefinitionId,
    expected_owner: &AccountId,
    not_found_msg: &'static str,
    unexpected_owner_msg: &'static str,
    timeout_msg: &'static str,
) -> Result<()> {
    let deadline = Instant::now() + NON_EMPTY_BLOCK_TIMEOUT;
    loop {
        let client = clients.next();
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

fn wait_for_asset_value(
    clients: &mut ClientPool,
    asset_id: &AssetId,
    expected: &Numeric,
    context: &'static str,
) -> Result<()> {
    let deadline = Instant::now() + NON_EMPTY_BLOCK_TIMEOUT;
    loop {
        let client = clients.next();
        if let Ok(asset) = client.query_single(FindAssetById::new(asset_id.clone()))
            && asset.value() == expected
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for {context} after {NON_EMPTY_BLOCK_TIMEOUT:?}"
            ));
        }
        sleep(QUERY_RETRY_DELAY);
    }
}

fn wait_for_asset_absent(
    clients: &mut ClientPool,
    asset_id: &AssetId,
    context: &'static str,
) -> Result<()> {
    let deadline = Instant::now() + NON_EMPTY_BLOCK_TIMEOUT;
    loop {
        let client = clients.next();
        if let Err(QueryError::Validation(ValidationFail::QueryFailed(
            QueryExecutionFail::Find(FindError::Asset(_)) | QueryExecutionFail::NotFound,
        ))) = client.query_single(FindAssetById::new(asset_id.clone()))
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for {context} after {NON_EMPTY_BLOCK_TIMEOUT:?}"
            ));
        }
        sleep(QUERY_RETRY_DELAY);
    }
}

/// Ensure noisy tracing is silenced before any network is spawned.
#[allow(unsafe_code)]
fn install_quiet_tracing() {
    static QUIET_TRACE: OnceLock<()> = OnceLock::new();
    QUIET_TRACE.get_or_init(|| {
        if std::env::var_os("IROHA_TEST_SERIALIZE_NETWORKS").is_none() {
            // Safety: asset tests serialize env mutation via QUIET_TRACE.
            unsafe {
                std::env::set_var("IROHA_TEST_SERIALIZE_NETWORKS", "1");
            }
        }
    });
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
    let mut collectors = toml::Table::new();
    collectors.insert("redundant_send_r".into(), TomlValue::Integer(3));
    sumeragi.insert("collectors".into(), TomlValue::Table(collectors));

    let mut rbc = toml::Table::new();
    rbc.insert("pending_ttl_ms".into(), TomlValue::Integer(120_000));
    rbc.insert("session_ttl_ms".into(), TomlValue::Integer(240_000));
    sumeragi.insert("rbc".into(), TomlValue::Table(rbc));

    // Increase DA quorum/availability timeouts to tolerate slower CI and local hosts.
    let mut da = toml::Table::new();
    da.insert("quorum_timeout_multiplier".into(), TomlValue::Integer(6));
    da.insert(
        "availability_timeout_multiplier".into(),
        TomlValue::Integer(3),
    );
    sumeragi.insert("da".into(), TomlValue::Table(da));
    let mut nexus = toml::Table::new();
    nexus.insert("enabled".into(), TomlValue::Boolean(false));
    let mut layer = toml::Table::new();
    layer.insert("sumeragi".into(), TomlValue::Table(sumeragi));
    layer.insert("nexus".into(), TomlValue::Table(nexus));
    NetworkBuilder::new()
        .with_peers(4)
        .with_pipeline_time(FAST_PIPELINE_TIME)
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
    clients: &mut ClientPool,
    instruction: impl Into<InstructionBox>,
    context: &str,
) -> Result<Option<()>> {
    submit_or_tolerate_timeout(clients, instruction, context)
}

fn submit_tx_or_skip(
    clients: &mut ClientPool,
    tx: &iroha::data_model::transaction::SignedTransaction,
    context: &str,
) -> Result<Option<()>> {
    let mut last_err = None;
    let mut accepted = false;
    for _ in 0..clients.len() {
        let client = clients.next();
        match client.submit_transaction(tx) {
            Ok(_) => {
                accepted = true;
            }
            Err(err) => {
                if is_duplicate_tx_error(&err) {
                    accepted = true;
                    continue;
                }
                if is_transient_client_error(&err) {
                    last_err = Some(err);
                    continue;
                }
                return sandbox::handle_result::<()>(Err(err), context);
            }
        }
    }
    if accepted {
        Ok(Some(()))
    } else {
        sandbox::handle_result::<()>(
            Err(last_err.unwrap_or_else(|| eyre!("all peers unreachable"))),
            context,
        )
    }
}

fn submit_or_tolerate_timeout(
    clients: &mut ClientPool,
    instruction: impl Into<InstructionBox>,
    context: &str,
) -> Result<Option<()>> {
    let instruction = instruction.into();
    let tx = clients
        .current()
        .build_transaction([instruction.clone()], Metadata::default());
    submit_tx_or_skip(clients, &tx, context)
}

fn sync_after_optional(
    network: &Network,
    rt: &tokio::runtime::Runtime,
    clients: &mut ClientPool,
    last_non_empty_height: &mut u64,
    context: &str,
) -> Result<Option<()>> {
    match status_or_skip(
        sync_after_submission(network, rt, clients.next(), *last_non_empty_height, context),
        context,
    )? {
        Some(status) => {
            *last_non_empty_height = status.blocks_non_empty;
            Ok(Some(()))
        }
        None => Ok(None),
    }
}

fn is_tx_confirmation_timeout(err: &Report) -> bool {
    const NEEDLES: [&str; 4] = [
        "haven't got tx confirmation within",
        "transaction queued for too long",
        "Connection dropped without `Committed/Applied` or `Rejected` event",
        "fallback status check failed",
    ];
    err.chain().any(|cause| {
        let text = cause.to_string();
        NEEDLES.iter().any(|needle| text.contains(needle))
    })
}

fn is_duplicate_tx_error(err: &Report) -> bool {
    const NEEDLES: [&str; 6] = [
        "PRTRY:ALREADY_COMMITTED",
        "PRTRY:ALREADY_ENQUEUED",
        "already_committed",
        "already_enqueued",
        "transaction already committed",
        "transaction already present in the queue",
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
    SERIAL_NETWORK_GUARD.get_or_init(|| sandbox::override_network_parallelism(Some(true), None));

    if matches!(GENESIS_STATUS.get(), Some(Err(()))) {
        eprintln!("Skipping test: failed to start network (cached)");
        return None;
    }

    if let Err(err) = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)) {
        let report = Report::new(err);
        if let Some(reason) = sandbox::sandbox_reason(&report) {
            eprintln!(
                "sandboxed network restriction detected while running asset tests; skipping ({reason})"
            );
            return None;
        }
        panic!("failed during bind preflight: {report}");
    }

    let serial_guard = sandbox::serial_guard();
    if matches!(GENESIS_STATUS.get(), Some(Err(()))) {
        eprintln!("Skipping test: failed to start network (cached)");
        return None;
    }

    let (network, runtime) = builder
        .with_auto_populated_trusted_peers()
        .with_min_peers(4)
        .build_blocking();

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
        match sandbox::handle_result::<()>(Err(err.into()), "bind preflight") {
            Ok(None) => return None,
            Ok(Some(())) => unreachable!("sandbox handler should not return Some on Err"),
            Err(err) => panic!("failed during bind preflight: {err}"),
        }
    }

    if let Err(err) = runtime.block_on(async { network.start_all().await }) {
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
    Some((
        sandbox::SerializedNetwork::new(network, serial_guard),
        runtime,
    ))
}

#[test]
// This test is also covered at the UI level in the iroha_cli tests
// in test_mint_assets.py
#[allow(clippy::too_many_lines)]
fn client_add_asset_quantities_should_increase_asset_amounts() -> Result<()> {
    // Given
    let account_id = ALICE_ID.clone();
    let asset_definition_id = "xor#wonderland".parse::<AssetDefinitionId>()?;
    let big_asset_definition_id = "xorbig#wonderland".parse::<AssetDefinitionId>()?;
    let decimal_definition_id = "xorfrac#wonderland".parse::<AssetDefinitionId>()?;
    let asset_definition = AssetDefinition::numeric(asset_definition_id.clone());
    let big_asset_definition = AssetDefinition::numeric(big_asset_definition_id.clone());
    let decimal_definition =
        AssetDefinition::new(decimal_definition_id.clone(), NumericSpec::fractional(3));

    let builder = quiet_network_builder();

    let Some((network, rt)) = start_test_network_with_builder(builder) else {
        return Ok(());
    };
    let env_dir = network.env_dir().to_path_buf();
    let mut clients = ClientPool::new(&network);
    let torii = clients.current().torii_url.clone();
    let mut last_non_empty_height = match status_or_skip(
        get_status_with_retry_or_storage(&network, clients.next(), "initial status"),
        "initial status",
    )? {
        Some(status) => status.blocks_non_empty,
        None => return Ok(()),
    };

    let register_instructions: [InstructionBox; 3] = [
        Register::asset_definition(asset_definition).into(),
        Register::asset_definition(big_asset_definition).into(),
        Register::asset_definition(decimal_definition).into(),
    ];
    let register_tx = clients
        .current()
        .build_transaction(register_instructions, Metadata::default());
    if submit_tx_or_skip(&mut clients, &register_tx, "register asset definitions")
        .map_err(|err| {
            err.wrap_err(format!(
                "register asset definitions; torii={torii}, env_dir={}",
                env_dir.display()
            ))
        })?
        .is_none()
    {
        return Ok(());
    }
    if sync_after_optional(
        &network,
        &rt,
        &mut clients,
        &mut last_non_empty_height,
        "register asset definitions",
    )?
    .is_none()
    {
        return Ok(());
    }
    for asset_definition_id in [
        &asset_definition_id,
        &big_asset_definition_id,
        &decimal_definition_id,
    ] {
        wait_for_asset_definition_owner(
            &mut clients,
            asset_definition_id,
            &account_id,
            "asset definition not found after registration",
            "unexpected asset definition owner after registration",
            "asset definition registration",
        )
        .map_err(|err| {
            err.wrap_err(format!(
                "asset definition registration wait; torii={torii}, env_dir={}",
                env_dir.display()
            ))
        })?;
    }

    // When: mint integer asset quantity
    let quantity = numeric!(200);
    let asset_id = AssetId::new(asset_definition_id.clone(), account_id.clone());
    let mint = Mint::asset_numeric(quantity.clone(), asset_id.clone());
    let instructions: [InstructionBox; 1] = [mint.into()];
    let tx = clients
        .current()
        .build_transaction(instructions, Metadata::default());
    if submit_tx_or_skip(&mut clients, &tx, "mint asset")?.is_none() {
        return Ok(());
    }
    if sync_after_optional(
        &network,
        &rt,
        &mut clients,
        &mut last_non_empty_height,
        "mint asset",
    )?
    .is_none()
    {
        return Ok(());
    }
    wait_for_asset_value(&mut clients, &asset_id, &quantity, "mint asset")?;

    // And: mint large integer asset quantity
    let big_quantity = Numeric::new(2_u128.pow(65), 0);
    let big_asset_id = AssetId::new(big_asset_definition_id.clone(), account_id.clone());
    let mint = Mint::asset_numeric(big_quantity.clone(), big_asset_id.clone());
    let instructions: [InstructionBox; 1] = [mint.into()];
    let tx = clients
        .current()
        .build_transaction(instructions, Metadata::default());
    if submit_tx_or_skip(&mut clients, &tx, "mint large asset")?.is_none() {
        return Ok(());
    }
    if sync_after_optional(
        &network,
        &rt,
        &mut clients,
        &mut last_non_empty_height,
        "mint large asset",
    )?
    .is_none()
    {
        return Ok(());
    }
    wait_for_asset_value(
        &mut clients,
        &big_asset_id,
        &big_quantity,
        "mint large asset",
    )?;

    // And: mint decimal asset quantity
    let decimal_quantity = numeric!(123.456);
    let decimal_asset_id = AssetId::new(decimal_definition_id.clone(), account_id.clone());
    let mint = Mint::asset_numeric(decimal_quantity.clone(), decimal_asset_id.clone());
    let instructions: [InstructionBox; 1] = [mint.into()];
    let tx = clients
        .current()
        .build_transaction(instructions, Metadata::default());
    if submit_tx_or_skip(&mut clients, &tx, "mint decimal asset")
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
    if sync_after_optional(
        &network,
        &rt,
        &mut clients,
        &mut last_non_empty_height,
        "mint decimal asset",
    )?
    .is_none()
    {
        return Ok(());
    }
    wait_for_asset_value(
        &mut clients,
        &decimal_asset_id,
        &decimal_quantity,
        "mint decimal asset",
    )?;

    // Add some fractional part
    let quantity2 = numeric!(0.55);
    let mint = Mint::asset_numeric(quantity2.clone(), decimal_asset_id.clone());
    // and check that it is added without errors
    let sum = decimal_quantity
        .checked_add(quantity2)
        .ok_or_else(|| eyre::eyre!("overflow"))?;
    if submit_or_skip(&mut clients, mint, "mint fractional asset")
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
    if sync_after_optional(
        &network,
        &rt,
        &mut clients,
        &mut last_non_empty_height,
        "mint fractional asset",
    )?
    .is_none()
    {
        return Ok(());
    }
    wait_for_asset_value(
        &mut clients,
        &decimal_asset_id,
        &sum,
        "mint fractional asset",
    )?;

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
        let exchange_domain: DomainId = "exchange".parse().expect("domain should be valid");
        let crypto_domain: DomainId = "crypto".parse().expect("domain should be valid");
        let rate_def: AssetDefinitionId = "btc/eth#exchange"
            .parse()
            .expect("asset definition should be valid");
        let btc_def: AssetDefinitionId = "btc#crypto"
            .parse()
            .expect("asset definition should be valid");
        let eth_def: AssetDefinitionId = "eth#crypto"
            .parse()
            .expect("asset definition should be valid");
        let rate = AssetId::new(rate_def.clone(), dex_id.clone());
        let seller_btc = AssetId::new(btc_def.clone(), seller_id.clone());
        let buyer_eth = AssetId::new(eth_def.clone(), buyer_id.clone());

        let mut builder = quiet_network_builder();
        builder = builder
            .with_genesis_instruction(register::domain("exchange"))
            .with_genesis_instruction(register::domain("company"))
            .with_genesis_instruction(register::domain("crypto"))
            .with_genesis_instruction(register::account(dex_id.clone()))
            .with_genesis_instruction(register::account(seller_id.clone()))
            .with_genesis_instruction(register::account(buyer_id.clone()))
            .with_genesis_instruction(Grant::account_permission(
                CanRegisterAssetDefinition {
                    domain: exchange_domain.clone(),
                },
                ALICE_ID.clone(),
            ))
            .with_genesis_instruction(Grant::account_permission(
                CanRegisterAssetDefinition {
                    domain: crypto_domain.clone(),
                },
                ALICE_ID.clone(),
            ));

        let Some((network, rt)) = start_test_network_with_builder(builder) else {
            return Ok(());
        };
        let mut clients = ClientPool::new(&network);
        let mut status = match status_or_skip(
            get_status_with_retry_or_storage(&network, clients.next(), "initial status"),
            "initial status",
        )? {
            Some(status) => status,
            None => return Ok(()),
        };
        let mut last_non_empty_height = status.blocks_non_empty;

        let register_instructions: [InstructionBox; 3] = [
            Register::asset_definition(AssetDefinition::numeric(rate_def.clone())).into(),
            Register::asset_definition(AssetDefinition::numeric(btc_def.clone())).into(),
            Register::asset_definition(AssetDefinition::numeric(eth_def.clone())).into(),
        ];
        let register_tx = clients
            .current()
            .build_transaction(register_instructions, Metadata::default());
        if submit_tx_or_skip(&mut clients, &register_tx, "register exchange assets")?.is_none() {
            return Ok(());
        }
        status = match status_or_skip(
            sync_after_submission(
                &network,
                &rt,
                clients.next(),
                last_non_empty_height,
                "register exchange assets",
            ),
            "register exchange assets",
        )? {
            Some(status) => status,
            None => return Ok(()),
        };
        last_non_empty_height = status.blocks_non_empty;

        let seed_instructions: [InstructionBox; 3] = [
            Mint::asset_numeric(20_u32, rate.clone()).into(),
            Mint::asset_numeric(10_u32, seller_btc.clone()).into(),
            Mint::asset_numeric(200_u32, buyer_eth.clone()).into(),
        ];
        let seed_tx = clients
            .current()
            .build_transaction(seed_instructions, Metadata::default());
        if submit_tx_or_skip(&mut clients, &seed_tx, "seed exchange balances")?.is_none() {
            return Ok(());
        }
        status = match status_or_skip(
            sync_after_submission(
                &network,
                &rt,
                clients.next(),
                last_non_empty_height,
                "seed exchange balances",
            ),
            "seed exchange balances",
        )? {
            Some(status) => status,
            None => return Ok(()),
        };
        last_non_empty_height = status.blocks_non_empty;

        wait_for_asset_value(&mut clients, &rate, &numeric!(20), "seed rate asset")?;
        wait_for_asset_value(&mut clients, &seller_btc, &numeric!(10), "seed seller btc")?;
        wait_for_asset_value(&mut clients, &buyer_eth, &numeric!(200), "seed buyer eth")?;

        let alice_id = ALICE_ID.clone();
        {
            let mut alice_can_transfer_asset =
                |asset_id: AssetId, owner_key_pair: KeyPair| -> Result<()> {
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

                    if submit_tx_or_skip(&mut clients, &transaction, "grant transfer permission")?
                        .is_none()
                    {
                        return Ok(());
                    }
                    status = match status_or_skip(
                        sync_after_submission(
                            &network,
                            &rt,
                            clients.next(),
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
        }

        let rate: u32 = asset_value(&mut clients, &rate)?
            .try_into()
            .expect("numeric should be u32 originally");
        let transfer_instructions: [InstructionBox; 2] = [
            Transfer::asset_numeric(seller_btc.clone(), 10_u32, buyer_id.clone()).into(),
            Transfer::asset_numeric(buyer_eth.clone(), 10_u32 * rate, seller_id.clone()).into(),
        ];
        let transfer_tx = clients
            .current()
            .build_transaction(transfer_instructions, Metadata::default());
        if submit_tx_or_skip(&mut clients, &transfer_tx, "exchange transfers")?.is_none() {
            return Ok(());
        }
        if let Some(_status) = status_or_skip(
            sync_after_submission(
                &network,
                &rt,
                clients.next(),
                last_non_empty_height,
                "exchange transfers",
            ),
            "exchange transfers",
        )? {
        } else {
            return Ok(());
        }

        let seller_eth = AssetId::new(eth_def, seller_id.clone());
        let buyer_btc = AssetId::new(btc_def, buyer_id.clone());
        // after: seller has $ETH200 and buyer has $BTC10
        wait_for_asset_absent(&mut clients, &seller_btc, "seller BTC purge")?;
        wait_for_asset_absent(&mut clients, &buyer_eth, "buyer ETH purge")?;
        wait_for_asset_value(
            &mut clients,
            &seller_eth,
            &numeric!(200),
            "seller ETH balance",
        )?;
        wait_for_asset_value(&mut clients, &buyer_btc, &numeric!(10), "buyer BTC balance")?;

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
    let mut clients = ClientPool::new(&network);
    if status_or_skip(
        get_status_with_retry_or_storage(&network, clients.next(), "initial status"),
        "initial status",
    )?
    .is_none()
    {
        return Ok(());
    }

    if submit_or_tolerate_timeout(
        &mut clients,
        Register::asset_definition(AssetDefinition::numeric(asset_definition_id.clone())),
        "register transferable asset definition",
    )?
    .is_none()
    {
        return Ok(());
    }

    wait_for_asset_definition_owner(
        &mut clients,
        &asset_definition_id,
        &alice_id,
        "asset definition not found after registration",
        "unexpected asset definition owner",
        "asset definition registration",
    )?;

    if submit_or_tolerate_timeout(
        &mut clients,
        Transfer::asset_definition(alice_id, asset_definition_id.clone(), new_owner_id.clone()),
        "transfer asset definition",
    )?
    .is_none()
    {
        return Ok(());
    }

    wait_for_asset_definition_owner(
        &mut clients,
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
        let mut clients = ClientPool::new(&network);
        let torii = clients.current().torii_url.clone();
        let mut last_non_empty_height = match status_or_skip(
            get_status_with_retry_or_storage(&network, clients.next(), "initial status"),
            "initial status",
        )
        .map_err(|err| {
            err.wrap_err(format!(
                "initial status; torii={torii}, env_dir={}",
                env_dir.display()
            ))
        })? {
            Some(status) => status.blocks_non_empty,
            None => return Ok(()),
        };

        // Register and seed the asset definition under Alice's authority after genesis.
        if submit_or_tolerate_timeout(
            &mut clients,
            Register::asset_definition(asset_definition),
            "register integer-only asset definition",
        )?
        .is_none()
        {
            return Ok(());
        }
        last_non_empty_height = match status_or_skip(
            sync_after_submission(
                &network,
                &rt,
                clients.next(),
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
            &mut clients,
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
        last_non_empty_height = match status_or_skip(
            sync_after_submission(
                &network,
                &rt,
                clients.next(),
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
        })? {
            Some(status) => status.blocks_non_empty,
            None => return Ok(()),
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
        let before = asset_value(&mut clients, &asset_id)?;
        let dest_asset_id = AssetId::new(asset_definition_id.clone(), dest_id.clone());
        for op in isi(fractional_value) {
            let client = clients.next();
            match client.submit_blocking::<InstructionBox>(op) {
                Ok(_) => panic!("Should be rejected due to non integer value"),
                Err(err) => {
                    if let Some(reason) = sandbox::sandbox_reason(&eyre!(err.to_string())) {
                        return Err(eyre!(
                            "sandboxed network restriction detected during fractional rejection flow: {reason}"
                        ));
                    }
                }
            }
            last_non_empty_height = match status_or_skip(
                sync_after_submission(
                    &network,
                    &rt,
                    clients.next(),
                    last_non_empty_height,
                    "fractional rejection",
                ),
                "fractional rejection",
            )? {
                Some(status) => status.blocks_non_empty,
                None => return Ok(()),
            };

            // State unchanged for source and no asset created for destination
            let now = asset_value(&mut clients, &asset_id)?;
            assert_eq!(now, before, "fractional op must not change balance");
            let dest_exists = asset_exists(&mut clients, &dest_asset_id)?;
            assert!(
                !dest_exists,
                "fractional transfer must not create destination asset"
            );
        }

        // Everything works fine when submitting proper integer value
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
            &mut clients,
            Mint::asset_numeric(integer_value.clone(), asset_id.clone()),
            "integer mint",
        )?
        .is_none()
        {
            return Ok(());
        }
        {
            let mut sync_after = |context: &str| -> Result<Option<()>> {
                match status_or_skip(
                    sync_after_submission(
                        &network,
                        &rt,
                        clients.next(),
                        last_non_empty_height,
                        context,
                    ),
                    context,
                )? {
                    Some(status) => {
                        last_non_empty_height = status.blocks_non_empty;
                        Ok(Some(()))
                    }
                    None => Ok(None),
                }
            };
            if sync_after("integer mint")?.is_none() {
                return Ok(());
            }
        }
        wait_for_asset_value(
            &mut clients,
            &asset_id,
            &expected_after_mint,
            "integer mint",
        )?;

        if submit_or_tolerate_timeout(
            &mut clients,
            Burn::asset_numeric(integer_value.clone(), asset_id.clone()),
            "integer burn",
        )?
        .is_none()
        {
            return Ok(());
        }
        {
            let mut sync_after = |context: &str| -> Result<Option<()>> {
                match status_or_skip(
                    sync_after_submission(
                        &network,
                        &rt,
                        clients.next(),
                        last_non_empty_height,
                        context,
                    ),
                    context,
                )? {
                    Some(status) => {
                        last_non_empty_height = status.blocks_non_empty;
                        Ok(Some(()))
                    }
                    None => Ok(None),
                }
            };
            if sync_after("integer burn")?.is_none() {
                return Ok(());
            }
        }
        wait_for_asset_value(&mut clients, &asset_id, &before, "integer burn")?;

        if submit_or_tolerate_timeout(
            &mut clients,
            Transfer::asset_numeric(asset_id.clone(), integer_value, dest_id.clone()),
            "integer transfer",
        )?
        .is_none()
        {
            return Ok(());
        }
        {
            let mut sync_after = |context: &str| -> Result<Option<()>> {
                match status_or_skip(
                    sync_after_submission(
                        &network,
                        &rt,
                        clients.next(),
                        last_non_empty_height,
                        context,
                    ),
                    context,
                )? {
                    Some(status) => {
                        last_non_empty_height = status.blocks_non_empty;
                        Ok(Some(()))
                    }
                    None => Ok(None),
                }
            };
            if sync_after("integer transfer")?.is_none() {
                return Ok(());
            }
        }

        // After integer ops: asset moved to destination, zero at source (purged)
        wait_for_asset_value(
            &mut clients,
            &dest_asset_id,
            &numeric!(1),
            "integer transfer destination",
        )
        .wrap_err(format!(
            "final balance check; torii={torii}, env_dir={}",
            env_dir.display()
        ))?;
        if expected_after_transfer.is_zero() {
            wait_for_asset_absent(&mut clients, &asset_id, "integer transfer source purge")
                .wrap_err(format!(
                    "final balance check; torii={torii}, env_dir={}",
                    env_dir.display()
                ))?;
        } else {
            wait_for_asset_value(
                &mut clients,
                &asset_id,
                &expected_after_transfer,
                "integer transfer source",
            )
            .wrap_err(format!(
                "final balance check; torii={torii}, env_dir={}",
                env_dir.display()
            ))?;
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
}

#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn tx_confirmation_timeout_includes_fallback_status() {
        let err = eyre!("transaction confirmation timed out; fallback status check failed");
        assert!(is_tx_confirmation_timeout(&err));
    }

    #[test]
    fn tx_confirmation_timeout_includes_queue_stall() {
        let err = eyre!("transaction queued for too long");
        assert!(is_tx_confirmation_timeout(&err));
    }

    #[test]
    fn duplicate_tx_error_detects_queue_conflicts() {
        let committed = eyre!(
            "Unexpected transaction response; status: 409 Conflict; reject code: PRTRY:ALREADY_COMMITTED; response body: transaction already committed to the blockchain"
        );
        assert!(is_duplicate_tx_error(&committed));
        let enqueued = eyre!(
            "Unexpected transaction response; status: 409 Conflict; reject code: PRTRY:ALREADY_ENQUEUED; response body: transaction already present in the queue"
        );
        assert!(is_duplicate_tx_error(&enqueued));
        let other = eyre!(
            "Unexpected transaction response; status: 400 Bad Request; response body: invalid transaction"
        );
        assert!(!is_duplicate_tx_error(&other));
    }

    #[test]
    fn transient_client_error_detects_request_failures() {
        let err = eyre!("Failed to send http POST request to http://127.0.0.1:1/query");
        assert!(is_transient_client_error(&err));
        let non_transient = eyre!("Transaction rejected");
        assert!(!is_transient_client_error(&non_transient));
    }

    #[test]
    fn quiet_network_builder_uses_fast_pipeline_time() {
        let network = quiet_network_builder().build();
        assert_eq!(network.pipeline_time(), FAST_PIPELINE_TIME);
    }
}
