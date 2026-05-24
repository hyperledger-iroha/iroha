#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Bounded-latency localnet smoke and throughput tests for permissioned and `NPoS` Sumeragi.

use std::{
    cmp::Ordering,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering as AtomicOrdering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use blake3::Hasher as Blake3Hasher;
use eyre::{Result, WrapErr, bail, ensure, eyre};
use futures_util::{
    StreamExt, TryStreamExt,
    future::try_join_all,
    stream::{self, FuturesUnordered},
};
use integration_tests::sandbox;
use iroha::{
    crypto::{Algorithm, KeyPair},
    data_model::{
        Level,
        account::{Account, AccountId, OpaqueAccountId},
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        block::consensus::SumeragiStatusWire,
        da::commitment::DaProofPolicyBundle,
        domain::{Domain, DomainId},
        identifier::{
            IdentifierNormalization, IdentifierPolicy, IdentifierPolicyId,
            IdentifierResolutionReceipt, IdentifierResolutionReceiptPayload,
        },
        isi::{
            Instruction, InstructionBox, Log, Mint, Register, SetParameter, Transfer,
            identifier::{ActivateIdentifierPolicy, ClaimIdentifier, RegisterIdentifierPolicy},
            ram_lfe::{ActivateRamLfeProgramPolicy, RegisterRamLfeProgramPolicy},
            staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
        },
        metadata::Metadata,
        name::Name,
        nexus::{
            DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneVisibility,
            UniversalAccountId,
        },
        parameter::{BlockParameter, Parameter, SumeragiParameter, system::SumeragiNposParameters},
        peer::PeerId,
        prelude::{FindAccountById, FindAssetById, Numeric},
        ram_lfe::{
            RamLfeExecutionReceiptPayload, RamLfeOutputOpening, RamLfeOutputOpeningPayload,
            RamLfeProgramId, RamLfeProgramPolicy, RamLfeReceiptAttestation,
        },
    },
};
use iroha_config::parameters::actual::LaneConfig as ActualLaneConfig;
use iroha_core::da::proof_policy_bundle;
use iroha_crypto::{
    BfvEvaluationKeyBundle, BfvParameters, Hash, RamLfeBackend, RamLfeVerificationMode, Signature,
    SignatureOf, bfv_programmed_policy_commitment_with_program,
    bfv_programmed_public_parameters_with_program, decode_bfv_programmed_public_parameters,
    default_bfv_programmed_hidden_program, derive_identifier_key_material_from_seed,
    identifier_hashes_from_output_hash, ram_lfe_bfv_parameters_v1, ram_lfe_output_hash,
};
use iroha_test_network::{
    Network, NetworkBuilder, genesis_factory_with_post_topology, init_instruction_registry,
};
use iroha_test_samples::{
    ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR, REAL_GENESIS_ACCOUNT_ID,
    REAL_GENESIS_ACCOUNT_KEYPAIR,
};
use nonzero_ext::nonzero;
use norito::json::{Map, Value};
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use reqwest::Client as HttpClient;
use tempfile::tempdir;
use tokio::{sync::Mutex, task, time::sleep};
use toml::{Table, Value as TomlValue};

static LOCALNET_SMOKE_GUARD: OnceLock<Mutex<()>> = OnceLock::new();
const SMOKE_PIPELINE_TIME: Duration = Duration::from_secs(2);
const SMOKE_BLOCK_TIME_MS: u64 = 1_000;
const SMOKE_COMMIT_TIME_MS: u64 = 1_000;
const STATUS_POLL_TIMEOUT: Duration = Duration::from_secs(15);
const STATUS_LOG_INTERVAL: Duration = Duration::from_secs(2);
const SOAK_PIPELINE_TIME: Duration = Duration::from_millis(300);
const SOAK_BLOCK_TIME_MS: u64 = 100;
const SOAK_COMMIT_TIME_MS: u64 = 100;
const SOAK_STATUS_POLL_TIMEOUT: Duration = Duration::from_secs(20);
const SOAK_TARGET_BLOCKS: u64 = 2_000;
const SOAK_SUBMIT_BATCH: u64 = 200;
const SOAK_QUEUE_SOFT_LIMIT: u64 = 2_000;
const SOAK_QUEUE_PROGRESS_TIMEOUT: Duration = Duration::from_secs(3 * 60);
const SOAK_STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);
const SOAK_PROGRESS_LOG_INTERVAL: Duration = Duration::from_secs(5);
const SOAK_STALL_THRESHOLD: Duration = Duration::from_secs(90);
const SOAK_CLIENT_TTL: Duration = Duration::from_secs(2 * 60 * 60);
const THROUGHPUT_PIPELINE_TIME: Duration = Duration::from_secs(2);
const THROUGHPUT_BLOCK_TIME_MS: u64 = 1_000;
const THROUGHPUT_COMMIT_TIME_MS: u64 = 1_000;
const THROUGHPUT_BLOCK_MAX_TXS: u64 = 10_000;
const THROUGHPUT_LANE_TEU_FLOOR: i64 = 1;
const THROUGHPUT_LANE_TEU_CAPACITY: i64 = 1_000_000_000;
const THROUGHPUT_SUBMIT_BATCH: u64 = 512;
const THROUGHPUT_SUBMIT_PARALLELISM: u64 = 128;
const THROUGHPUT_QUEUE_SOFT_LIMIT: u64 = 20_000;
const THROUGHPUT_STALL_THRESHOLD: Duration = Duration::from_secs(60);
const THROUGHPUT_COMMIT_TIME_MAX_MULTIPLIER: u64 = 2;
const THROUGHPUT_CLIENT_TTL: Duration = Duration::from_secs(2 * 60 * 60);
const THROUGHPUT_WARMUP_BLOCKS: u64 = 10;
const THROUGHPUT_STEADY_BLOCKS: u64 = 30;
const THROUGHPUT_SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
const THROUGHPUT_PROGRESS_LOG_INTERVAL: Duration = Duration::from_secs(10);
const THROUGHPUT_METRICS_TIMEOUT: Duration = Duration::from_secs(5);
const THROUGHPUT_PAYLOAD_BYTES: usize = 512;
const THROUGHPUT_RNG_SEED: u64 = 0x0049_524f_4841;
const THROUGHPUT_SLO_P95_MS: u64 = 1_500;
const THROUGHPUT_SLO_P99_MS: u64 = 2_000;
const THROUGHPUT_SLO_VIEW_CHANGE_RATE_MAX: f64 = 0.1;
const THROUGHPUT_SLO_BACKPRESSURE_RATE_MAX: f64 = 2.0;
const THROUGHPUT_SLO_QUEUE_SAT_FRAC_MAX: f64 = 0.2;
const THROUGHPUT_NPOS_SLO_P95_MS: u64 = 2_000;
const THROUGHPUT_NPOS_SLO_P99_MS: u64 = 3_000;
const THROUGHPUT_NPOS_SLO_VIEW_CHANGE_RATE_MAX: f64 = 0.2;
const THROUGHPUT_NPOS_SLO_BACKPRESSURE_RATE_MAX: f64 = 3.0;
const THROUGHPUT_NPOS_SLO_QUEUE_SAT_FRAC_MAX: f64 = 0.3;
const THROUGHPUT_QUEUE_PROGRESS_TIMEOUT_ENV: &str = "IROHA_THROUGHPUT_QUEUE_PROGRESS_TIMEOUT_SECS";
const REALISTIC_30TPS_PEERS: usize = 4;
const REALISTIC_30TPS_DURATION_SECS: u64 = 7_200;
// Default to the transaction-capacity floor; env can raise this for stricter
// block-cadence checks.
const REALISTIC_30TPS_TARGET_BLOCKS: u64 = 0;
const REALISTIC_30TPS_TARGET_TPS: u64 = 30;
const REALISTIC_30TPS_BLOCK_TIME_MS: u64 = 400;
const REALISTIC_30TPS_COMMIT_TIME_MS: u64 = 500;
const REALISTIC_30TPS_BLOCK_MAX_TXS: u64 = 128;
const REALISTIC_30TPS_SUBMIT_PARALLELISM: usize = 64;
const REALISTIC_30TPS_QUEUE_SOFT_LIMIT: u64 = 3_000;
const REALISTIC_30TPS_STALL_THRESHOLD: Duration = Duration::from_secs(20);
const REALISTIC_30TPS_SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
const REALISTIC_30TPS_PROGRESS_LOG_INTERVAL: Duration = Duration::from_secs(10);
const REALISTIC_30TPS_TRANSFER_ACCOUNTS: usize = 640;
const REALISTIC_30TPS_TRANSFER_MAX_AMOUNT: u64 = 5;
const REALISTIC_30TPS_NPOS_FEE_FUNDING_CHUNK: usize = 128;
const REALISTIC_30TPS_RAM_LFE_EMAIL_POLICY_ID: &str = "email#realistic";
const REALISTIC_30TPS_RAM_LFE_EMAIL_PROGRAM_ID: &str = "email_realistic";
const FAIL_ON_SANDBOX_SKIP_ENV: &str = "IROHA_FAIL_ON_SANDBOX_SKIP";
// Grouped localnet runs can take longer to publish authoritative Nexus bindings
// than the earlier exact-test-only timeout budget.
const ROUTE_BINDING_TIMEOUT: Duration = Duration::from_secs(120);
const ROUTE_BINDING_POLL: Duration = Duration::from_millis(200);
const ROUTE_VALIDATOR_STAKE: u32 = 2_000;
const ROUTE_VALIDATOR_FEE_SEED_AMOUNT: u32 = 1_000_000;
const ROUTE_STAKE_ASSET_NAME: &str = "Route Stake";
const ROUTE_FEE_ASSET_NAME: &str = "Route Fee";

#[allow(unsafe_code)]
fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
    // Safety: tests serialize env mutation with LOCALNET_SMOKE_GUARD.
    unsafe {
        std::env::set_var(key, value);
    }
}

#[allow(unsafe_code)]
fn remove_env_var(key: &str) {
    // Safety: tests serialize env mutation with LOCALNET_SMOKE_GUARD.
    unsafe {
        std::env::remove_var(key);
    }
}

fn env_or_default(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn env_or_default_usize(key: &str, default: usize) -> usize {
    let default_u64 = u64::try_from(default).unwrap_or(u64::MAX);
    let value = env_or_default(key, default_u64);
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn env_or_default_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(default)
}

async fn submit_route_probe_with_retry(
    client: &iroha::client::Client,
    message: &str,
    timeout: Duration,
    context: &str,
) -> Result<bool> {
    let deadline = Instant::now() + timeout;
    loop {
        match client.submit::<InstructionBox>(Log::new(Level::INFO, message.to_owned()).into()) {
            Ok(_) => return Ok(true),
            Err(err)
                if Instant::now() < deadline && err.to_string().contains("route_unavailable") =>
            {
                sleep(ROUTE_BINDING_POLL).await;
            }
            Err(err) if err.to_string().contains("route_unavailable") => return Ok(false),
            Err(err) => return Err(err).wrap_err(context.to_owned()),
        }
    }
}

fn route_lane_validator_account(index: usize) -> AccountId {
    let key_pair = KeyPair::from_seed(
        format!("integration_tests::sumeragi_localnet_smoke::route-validator::{index}")
            .into_bytes(),
        Algorithm::Ed25519,
    );
    AccountId::new(key_pair.public_key().clone())
}

fn route_bootstrap_gas_account_id() -> AccountId {
    let key_pair = KeyPair::from_seed(
        b"integration_tests::sumeragi_localnet_smoke::route-bootstrap-gas".to_vec(),
        Algorithm::Ed25519,
    );
    AccountId::new(key_pair.public_key().clone())
}

fn route_stake_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("nexus", "universal").expect("nexus domain"),
        "xor".parse().expect("stake asset name"),
    )
}

fn route_fee_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("universal", "universal").expect("fee asset domain"),
        "xor".parse().expect("fee asset name"),
    )
}

fn route_multilane_da_proof_policy_bundle() -> DaProofPolicyBundle {
    let lane_count = std::num::NonZeroU32::new(3).expect("lane count");
    let lanes = vec![
        ModelLaneConfig {
            id: LaneId::new(0),
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: "lane-universal".to_owned(),
            visibility: LaneVisibility::Public,
            ..ModelLaneConfig::default()
        },
        ModelLaneConfig {
            id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(1),
            alias: "lane-alice".to_owned(),
            visibility: LaneVisibility::Public,
            ..ModelLaneConfig::default()
        },
        ModelLaneConfig {
            id: LaneId::new(2),
            dataspace_id: DataSpaceId::new(2),
            alias: "lane-bob".to_owned(),
            visibility: LaneVisibility::Public,
            ..ModelLaneConfig::default()
        },
    ];
    let catalog = LaneCatalog::new(lane_count, lanes).expect("route lane catalog");
    let lane_config = ActualLaneConfig::from_catalog(&catalog);
    proof_policy_bundle(&lane_config)
}

fn route_multilane_genesis_post_topology_transactions(
    topology: &[PeerId],
) -> Vec<Vec<InstructionBox>> {
    let stake_asset_id = route_stake_asset_definition_id();
    let fee_asset_id = route_fee_asset_definition_id();
    let gas_account_id = route_bootstrap_gas_account_id();
    let lane_ids = [LaneId::new(0), LaneId::new(1), LaneId::new(2)];
    let mint_amount = ROUTE_VALIDATOR_STAKE
        .saturating_mul(u32::try_from(lane_ids.len()).expect("lane count fits into u32"));
    let mut bootstrap_tx = vec![
        Register::domain(Domain::new(
            DomainId::try_new("nexus", "universal").expect("nexus domain"),
        ))
        .into(),
        Register::domain(Domain::new(
            DomainId::try_new("universal", "universal").expect("universal domain"),
        ))
        .into(),
        Register::account(Account::new(gas_account_id.clone())).into(),
        Register::asset_definition(
            AssetDefinition::new(stake_asset_id.clone(), Default::default())
                .with_name(ROUTE_STAKE_ASSET_NAME.to_owned())
                .with_metadata(Metadata::default()),
        )
        .into(),
        Register::asset_definition(
            AssetDefinition::new(fee_asset_id.clone(), Default::default())
                .with_name(ROUTE_FEE_ASSET_NAME.to_owned())
                .with_metadata(Metadata::default()),
        )
        .into(),
        Mint::asset_numeric(
            ROUTE_VALIDATOR_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), ALICE_ID.clone()),
        )
        .into(),
        Mint::asset_numeric(
            ROUTE_VALIDATOR_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), BOB_ID.clone()),
        )
        .into(),
        Mint::asset_numeric(
            ROUTE_VALIDATOR_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), gas_account_id),
        )
        .into(),
    ];
    let mut validator_tx = Vec::with_capacity(topology.len() * 2);

    for (index, peer_id) in topology.iter().enumerate() {
        let validator_id = route_lane_validator_account(index);
        bootstrap_tx.push(Register::account(Account::new(validator_id.clone())).into());
        bootstrap_tx.push(
            Mint::asset_numeric(
                mint_amount,
                AssetId::new(stake_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        bootstrap_tx.push(
            Mint::asset_numeric(
                ROUTE_VALIDATOR_FEE_SEED_AMOUNT,
                AssetId::new(fee_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        for lane_id in lane_ids {
            validator_tx.push(
                RegisterPublicLaneValidator::new(
                    lane_id,
                    validator_id.clone(),
                    peer_id.clone(),
                    validator_id.clone(),
                    Numeric::from(ROUTE_VALIDATOR_STAKE),
                    Metadata::default(),
                )
                .into(),
            );
            validator_tx
                .push(ActivatePublicLaneValidator::new(lane_id, validator_id.clone()).into());
        }
    }

    vec![bootstrap_tx, validator_tx]
}

fn queue_progress_timeout() -> Duration {
    let default_secs = SOAK_QUEUE_PROGRESS_TIMEOUT.as_secs();
    let secs = env_or_default(THROUGHPUT_QUEUE_PROGRESS_TIMEOUT_ENV, default_secs);
    Duration::from_secs(secs)
}

fn fail_on_sandbox_skip() -> bool {
    let Ok(raw) = std::env::var(FAIL_ON_SANDBOX_SKIP_ENV) else {
        return false;
    };
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[allow(clippy::too_many_arguments)]
async fn submit_logs(
    start_idx: u64,
    tx_count: u64,
    network: &Network,
    submit_clients: &[iroha::client::Client],
    submit_batch: u64,
    submit_parallelism: usize,
    queue_soft_limit: u64,
    payload_bytes: usize,
    rng_seed: u64,
) -> Result<Duration> {
    let submit_clients = std::sync::Arc::new(submit_clients.to_vec());
    ensure!(
        !submit_clients.is_empty(),
        "submit_logs requires at least one client"
    );
    let client_count = u64::try_from(submit_clients.len()).unwrap_or(1);
    let submit_start = Instant::now();
    let mut submitted = 0_u64;
    while submitted < tx_count {
        let remaining = tx_count.saturating_sub(submitted);
        let batch_count = remaining.min(submit_batch);
        let batch_start = start_idx.saturating_add(submitted);
        stream::iter(batch_start..batch_start.saturating_add(batch_count))
            .map(|idx| {
                let submit_clients = std::sync::Arc::clone(&submit_clients);
                async move {
                    if let Ok(delay) = std::env::var("IROHA_THROUGHPUT_DELAY_MS") {
                        if let Ok(ms) = delay.parse::<u64>() {
                            tokio::time::sleep(Duration::from_millis(ms)).await;
                        }
                    }
                    let payload = throughput_payload(idx, payload_bytes, rng_seed);
                    let client_idx = usize::try_from(idx % client_count).unwrap_or_default();
                    let client = submit_clients[client_idx].clone();
                    let handle = task::spawn_blocking(move || {
                        client
                            .submit::<InstructionBox>(Log::new(Level::INFO, payload).into())
                            .wrap_err_with(|| format!("failed to submit log instruction {idx}"))
                    });
                    handle.await.wrap_err("submit task join failed")?
                }
            })
            .buffer_unordered(submit_parallelism)
            .try_for_each(|_| async { Ok(()) })
            .await?;
        submitted = submitted.saturating_add(batch_count);
        wait_for_queue_depth(network, queue_soft_limit, SOAK_STATUS_POLL_TIMEOUT).await?;
    }
    Ok(submit_start.elapsed())
}

#[derive(Clone)]
struct TransferLoadAccount {
    id: AccountId,
    key_pair: KeyPair,
}

#[derive(Clone)]
struct TransferSubmitAccount {
    id: AccountId,
    client: iroha::client::Client,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Realistic30TpsLoadKind {
    Transfer,
    RamLfeEmail,
}

impl Realistic30TpsLoadKind {
    fn from_env() -> Result<Self> {
        let Some(raw) = std::env::var("IROHA_REALISTIC_30TPS_LOAD_KIND")
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
        else {
            return Ok(Self::Transfer);
        };
        match raw.as_str() {
            "transfer" | "transfers" => Ok(Self::Transfer),
            "ram-lfe-email" | "ram_lfe_email" | "ram-lfe-emails" | "ram_lfe_emails" | "email"
            | "emails" => Ok(Self::RamLfeEmail),
            _ => bail!(
                "unsupported IROHA_REALISTIC_30TPS_LOAD_KIND={raw}; expected transfer or ram-lfe-email"
            ),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Transfer => "transfer",
            Self::RamLfeEmail => "ram-lfe-email",
        }
    }
}

#[derive(Clone)]
struct RamLfeEmailLoadAccount {
    id: AccountId,
    uaid: UniversalAccountId,
}

#[derive(Clone)]
struct RamLfeEmailSubmitAccount {
    id: AccountId,
    client: iroha::client::Client,
    uaid: UniversalAccountId,
}

#[derive(Clone)]
struct RamLfeEmailPolicyContext {
    policy_id: IdentifierPolicyId,
    program_id: RamLfeProgramId,
    program_id_bytes: Vec<u8>,
    program_digest: Hash,
    parameter_digest: Hash,
    evaluation_key_digest: Hash,
    backend: RamLfeBackend,
    verification_mode: RamLfeVerificationMode,
}

fn realistic_transfer_domain_id() -> DomainId {
    DomainId::try_new("realistic", "universal").expect("realistic transfer domain")
}

fn realistic_transfer_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        realistic_transfer_domain_id(),
        "transfer_coin".parse().expect("transfer asset name"),
    )
}

fn realistic_transfer_accounts(account_count: usize, rng_seed: u64) -> Vec<TransferLoadAccount> {
    (0..account_count)
        .map(|index| {
            let key_pair = KeyPair::from_seed(
                format!("integration_tests::realistic-transfer::{rng_seed}::{index}").into_bytes(),
                Algorithm::Ed25519,
            );
            let id = AccountId::new(key_pair.public_key().clone());
            TransferLoadAccount { id, key_pair }
        })
        .collect()
}

fn realistic_npos_fee_funding_instruction_chunks(
    accounts: &[TransferLoadAccount],
) -> Vec<Vec<InstructionBox>> {
    let fee_asset_definition_id = route_fee_asset_definition_id();
    accounts
        .chunks(REALISTIC_30TPS_NPOS_FEE_FUNDING_CHUNK)
        .map(|chunk| {
            chunk
                .iter()
                .map(|account| {
                    Mint::asset_numeric(
                        ROUTE_VALIDATOR_FEE_SEED_AMOUNT,
                        AssetId::new(fee_asset_definition_id.clone(), account.id.clone()),
                    )
                    .into()
                })
                .collect()
        })
        .collect()
}

async fn fund_realistic_npos_transfer_fee_accounts(
    network: &Network,
    accounts: &[TransferLoadAccount],
) -> Result<()> {
    if accounts.is_empty() {
        return Ok(());
    }

    let before_statuses = collect_statuses(network, STATUS_POLL_TIMEOUT)
        .await
        .wrap_err("failed to collect baseline status before NPoS fee funding")?;
    let baseline_approved = before_statuses
        .iter()
        .map(|status| status.txs_approved)
        .min()
        .unwrap_or_default();
    let instruction_chunks = realistic_npos_fee_funding_instruction_chunks(accounts);
    let target_approved = baseline_approved.saturating_add(
        u64::try_from(instruction_chunks.len()).expect("funding chunk count fits u64"),
    );
    let client = network.client();

    task::spawn_blocking(move || -> Result<()> {
        for (chunk_index, instructions) in instruction_chunks.into_iter().enumerate() {
            client.submit_all_blocking(instructions).wrap_err_with(|| {
                format!("failed to fund NPoS fee assets for transfer account chunk {chunk_index}")
            })?;
        }
        Ok(())
    })
    .await
    .wrap_err("NPoS fee funding task join failed")??;

    wait_for_min_txs_approved(network, target_approved, Duration::from_secs(60)).await?;
    Ok(())
}

fn realistic_ram_lfe_email_policy_id() -> IdentifierPolicyId {
    REALISTIC_30TPS_RAM_LFE_EMAIL_POLICY_ID
        .parse()
        .expect("realistic RAM-LFE email policy id")
}

fn realistic_ram_lfe_email_program_id() -> RamLfeProgramId {
    REALISTIC_30TPS_RAM_LFE_EMAIL_PROGRAM_ID
        .parse()
        .expect("realistic RAM-LFE email program id")
}

fn realistic_ram_lfe_email_bfv_parameters() -> BfvParameters {
    ram_lfe_bfv_parameters_v1()
}

fn realistic_ram_lfe_email_policy_bundle(
    owner: &AccountId,
    resolver: &KeyPair,
) -> (IdentifierPolicy, RamLfeProgramPolicy) {
    let policy_id = realistic_ram_lfe_email_policy_id();
    let program_id = realistic_ram_lfe_email_program_id();
    let secret = b"realistic-email-resolver-secret";
    let hidden_program = default_bfv_programmed_hidden_program();
    let program_id_bytes = norito::to_bytes(&program_id).expect("encode RAM-LFE program id");
    let (public_parameters, _, relinearization_key) = derive_identifier_key_material_from_seed(
        &realistic_ram_lfe_email_bfv_parameters(),
        63,
        secret,
        &program_id_bytes,
    )
    .expect("derive RAM-LFE email public parameters");
    let evaluation_keys = BfvEvaluationKeyBundle {
        relinearization_key,
        rotation_keys: Vec::new(),
        bootstrap_key: None,
    };
    let programmed_public_parameters = bfv_programmed_public_parameters_with_program(
        public_parameters,
        evaluation_keys,
        &hidden_program,
        RamLfeVerificationMode::Signed,
        None,
    );
    let encoded_public_parameters =
        norito::to_bytes(&programmed_public_parameters).expect("encode public parameters");
    let commitment = bfv_programmed_policy_commitment_with_program(
        secret,
        &encoded_public_parameters,
        &hidden_program,
    )
    .expect("build RAM-LFE email policy commitment");
    let program_policy = RamLfeProgramPolicy::new(
        program_id.clone(),
        owner.clone(),
        RamLfeBackend::BfvProgrammedSha3_256V1,
        RamLfeVerificationMode::Signed,
        commitment,
        resolver.public_key().clone(),
    )
    .with_note("realistic RAM-LFE email identifier resolver");
    let policy = IdentifierPolicy::new(
        policy_id,
        owner.clone(),
        IdentifierNormalization::EmailAddress,
        program_id,
    )
    .with_note("realistic RAM-LFE email identifier policy");
    (policy, program_policy)
}

fn realistic_ram_lfe_email_policy_context(
    policy_id: IdentifierPolicyId,
    program_policy: &RamLfeProgramPolicy,
) -> Result<RamLfeEmailPolicyContext> {
    let programmed =
        decode_bfv_programmed_public_parameters(&program_policy.commitment.public_parameters)
            .wrap_err("decode realistic RAM-LFE email public parameters")?;
    let program_id_bytes = norito::to_bytes(&program_policy.program_id)
        .wrap_err("encode realistic RAM-LFE email program id")?;
    Ok(RamLfeEmailPolicyContext {
        policy_id,
        program_id: program_policy.program_id.clone(),
        program_id_bytes,
        program_digest: programmed.hidden_program_digest,
        parameter_digest: programmed.parameter_digest,
        evaluation_key_digest: programmed.evaluation_key_digest,
        backend: program_policy.backend,
        verification_mode: program_policy.verification_mode,
    })
}

fn realistic_ram_lfe_email_accounts(
    account_count: usize,
    rng_seed: u64,
) -> Vec<RamLfeEmailLoadAccount> {
    (0..account_count)
        .map(|index| {
            let key_pair = KeyPair::from_seed(
                format!("integration_tests::realistic-ram-lfe-email::{rng_seed}::{index}")
                    .into_bytes(),
                Algorithm::Ed25519,
            );
            let id = AccountId::new(key_pair.public_key().clone());
            let uaid = UniversalAccountId::from_hash(Hash::new(
                format!("integration_tests::realistic-ram-lfe-email-uaid::{rng_seed}::{index}")
                    .as_bytes(),
            ));
            RamLfeEmailLoadAccount { id, uaid }
        })
        .collect()
}

fn realistic_transfer_route(
    index: u64,
    account_count: usize,
    max_amount: u64,
    rng_seed: u64,
) -> (usize, usize, u64) {
    debug_assert!(account_count >= 2);
    let mut rng = ChaCha8Rng::seed_from_u64(
        rng_seed ^ index.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5452_414e_5346_4552,
    );
    let source = usize::try_from(rng.next_u64() % account_count as u64).unwrap_or_default();
    let mut destination = usize::try_from(rng.next_u64() % account_count.saturating_sub(1) as u64)
        .unwrap_or_default();
    if destination >= source {
        destination = destination.saturating_add(1);
    }
    let amount = 1 + rng.next_u64() % max_amount.max(1);
    (source, destination, amount)
}

fn expected_realistic_transfer_balances(
    account_count: usize,
    tx_count: u64,
    initial_balance: u64,
    max_amount: u64,
    rng_seed: u64,
) -> Vec<u64> {
    let mut balances = vec![i128::from(initial_balance); account_count];
    for index in 0..tx_count {
        let (source, destination, amount) =
            realistic_transfer_route(index, account_count, max_amount, rng_seed);
        let amount = i128::from(amount);
        balances[source] -= amount;
        balances[destination] += amount;
    }
    balances
        .into_iter()
        .map(|balance| u64::try_from(balance).expect("transfer load balance should stay positive"))
        .collect()
}

fn realistic_ram_lfe_email_account_index(index: u64, account_count: usize, rng_seed: u64) -> usize {
    debug_assert!(account_count > 0);
    let mut rng = ChaCha8Rng::seed_from_u64(
        rng_seed ^ index.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x454d_4149_4c5f_4c46,
    );
    usize::try_from(rng.next_u64() % account_count as u64).unwrap_or_default()
}

fn realistic_ram_lfe_email_address(index: u64, rng_seed: u64) -> String {
    const DOMAINS: [&str; 8] = [
        "retail.example",
        "payments.example",
        "wallet.example",
        "merchant.example",
        "support.example",
        "identity.example",
        "ops.example",
        "settlement.example",
    ];
    let mut rng = ChaCha8Rng::seed_from_u64(
        rng_seed ^ index.wrapping_mul(0xA076_1D64_78BD_642F) ^ 0x454d_4149_4c5f_4944,
    );
    let domain =
        DOMAINS[usize::try_from(rng.next_u64() % DOMAINS.len() as u64).unwrap_or_default()];
    format!(
        "notice.{:08x}.{:08x}+{:08}@{domain}",
        (index >> 32) as u32,
        index as u32,
        rng.next_u64() as u32,
    )
}

fn realistic_ram_lfe_email_receipt(
    context: &RamLfeEmailPolicyContext,
    resolver: &KeyPair,
    account_id: &AccountId,
    uaid: UniversalAccountId,
    index: u64,
    rng_seed: u64,
) -> IdentifierResolutionReceipt {
    let normalized_email = IdentifierNormalization::EmailAddress
        .normalize(&realistic_ram_lfe_email_address(index, rng_seed))
        .expect("generated email address should normalize");
    let opened_output_hash = ram_lfe_output_hash(normalized_email.as_bytes());
    let (opaque_id, receipt_hash) =
        identifier_hashes_from_output_hash(&context.program_id_bytes, &opened_output_hash);
    let input_ciphertext_hash =
        Hash::new(format!("{normalized_email}:encrypted-input:{index}").as_bytes());
    let output_ciphertext_hash =
        Hash::new(format!("{normalized_email}:encrypted-output:{index}").as_bytes());
    let execution = RamLfeExecutionReceiptPayload {
        program_id: context.program_id.clone(),
        program_digest: context.program_digest,
        backend: context.backend,
        verification_mode: context.verification_mode,
        input_ciphertext_hash,
        output_ciphertext_hash,
        parameter_digest: context.parameter_digest,
        evaluation_key_digest: context.evaluation_key_digest,
        output_hash: output_ciphertext_hash,
        associated_data_hash: Hash::new(&context.program_id_bytes),
        executed_at_ms: 0,
        expires_at_ms: None,
    };
    let opening_payload = RamLfeOutputOpeningPayload {
        program_id: context.program_id.clone(),
        input_ciphertext_hash,
        output_ciphertext_hash,
        parameter_digest: context.parameter_digest,
        evaluation_key_digest: context.evaluation_key_digest,
        opened_output_hash,
        opened_at_ms: 0,
        expires_at_ms: None,
    };
    let opening = RamLfeOutputOpening {
        signature: SignatureOf::new(resolver.private_key(), &opening_payload).into(),
        payload: opening_payload,
    };
    let payload = IdentifierResolutionReceiptPayload {
        policy_id: context.policy_id.clone(),
        execution,
        opening,
        opaque_id: OpaqueAccountId::from(opaque_id),
        receipt_hash,
        uaid,
        account_id: account_id.clone(),
    };
    let signature: Signature = SignatureOf::new(resolver.private_key(), &payload).into();
    IdentifierResolutionReceipt {
        payload,
        attestation: RamLfeReceiptAttestation::Signed(signature),
    }
}

fn expected_realistic_ram_lfe_email_claim_counts(
    account_count: usize,
    tx_count: u64,
    rng_seed: u64,
) -> Vec<usize> {
    let mut counts = vec![0_usize; account_count];
    for index in 0..tx_count {
        let account_index = realistic_ram_lfe_email_account_index(index, account_count, rng_seed);
        counts[account_index] = counts[account_index].saturating_add(1);
    }
    counts
}

fn realistic_load_metadata(index: u64) -> Metadata {
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("tx_sequence").expect("tx_sequence metadata key"),
        index,
    );
    metadata
}

#[allow(clippy::too_many_arguments)]
async fn submit_ram_lfe_emails_paced(
    tx_count: u64,
    target_tps: u64,
    submit_accounts: Vec<RamLfeEmailSubmitAccount>,
    policy_context: RamLfeEmailPolicyContext,
    resolver: KeyPair,
    rng_seed: u64,
    submit_parallelism: usize,
    submitted_counter: Arc<AtomicU64>,
) -> Result<Duration> {
    ensure!(
        !submit_accounts.is_empty(),
        "submit_ram_lfe_emails_paced requires at least one account"
    );
    let submit_accounts = Arc::new(submit_accounts);
    let account_count = submit_accounts.len();
    let submit_parallelism = submit_parallelism.max(1);
    let nanos_per_tx = 1_000_000_000_u64 / target_tps.max(1);
    let submit_start = Instant::now();
    let mut pending: FuturesUnordered<task::JoinHandle<Result<()>>> = FuturesUnordered::new();

    for index in 0..tx_count {
        let target_elapsed = Duration::from_nanos(nanos_per_tx.saturating_mul(index));
        if let Some(target_instant) = submit_start.checked_add(target_elapsed) {
            let now = Instant::now();
            if target_instant > now {
                sleep(target_instant.duration_since(now)).await;
            }
        }

        let account_index = realistic_ram_lfe_email_account_index(index, account_count, rng_seed);
        let submit_account = submit_accounts[account_index].clone();
        let policy_context = policy_context.clone();
        let resolver = resolver.clone();
        let submitted_counter = Arc::clone(&submitted_counter);
        pending.push(task::spawn_blocking(move || {
            let receipt = realistic_ram_lfe_email_receipt(
                &policy_context,
                &resolver,
                &submit_account.id,
                submit_account.uaid,
                index,
                rng_seed,
            );
            submit_account
                .client
                .submit_with_metadata::<InstructionBox>(
                    ClaimIdentifier {
                        account: submit_account.id,
                        receipt,
                    }
                    .into(),
                    realistic_load_metadata(index),
                )
                .wrap_err_with(|| {
                    format!(
                        "failed to submit paced RAM-LFE email claim instruction {index} from account index {account_index}"
                    )
                })?;
            submitted_counter.fetch_add(1, AtomicOrdering::Relaxed);
            Ok(())
        }));

        if pending.len() >= submit_parallelism {
            let result = pending
                .next()
                .await
                .ok_or_else(|| eyre!("paced RAM-LFE email worker set unexpectedly empty"))?;
            result.wrap_err("paced RAM-LFE email task join failed")??;
        }
    }

    while let Some(result) = pending.next().await {
        result.wrap_err("paced RAM-LFE email task join failed")??;
    }
    Ok(submit_start.elapsed())
}

#[allow(clippy::too_many_arguments)]
async fn submit_transfers_paced(
    tx_count: u64,
    target_tps: u64,
    submit_accounts: Vec<TransferSubmitAccount>,
    asset_definition_id: AssetDefinitionId,
    transfer_max_amount: u64,
    rng_seed: u64,
    submit_parallelism: usize,
    submitted_counter: Arc<AtomicU64>,
) -> Result<Duration> {
    ensure!(
        submit_accounts.len() >= 2,
        "submit_transfers_paced requires at least two funded accounts"
    );
    let submit_accounts = Arc::new(submit_accounts);
    let account_count = submit_accounts.len();
    let submit_parallelism = submit_parallelism.max(1);
    let nanos_per_tx = 1_000_000_000_u64 / target_tps.max(1);
    let submit_start = Instant::now();
    let mut pending: FuturesUnordered<task::JoinHandle<Result<()>>> = FuturesUnordered::new();

    for index in 0..tx_count {
        let target_elapsed = Duration::from_nanos(nanos_per_tx.saturating_mul(index));
        if let Some(target_instant) = submit_start.checked_add(target_elapsed) {
            let now = Instant::now();
            if target_instant > now {
                sleep(target_instant.duration_since(now)).await;
            }
        }

        let (source, destination, amount) =
            realistic_transfer_route(index, account_count, transfer_max_amount, rng_seed);
        let source_account = submit_accounts[source].clone();
        let destination_id = submit_accounts[destination].id.clone();
        let asset_definition_id = asset_definition_id.clone();
        let submitted_counter = Arc::clone(&submitted_counter);
        pending.push(task::spawn_blocking(move || {
            let source_asset_id = AssetId::new(asset_definition_id, source_account.id.clone());
            source_account
                .client
                .submit_with_metadata::<InstructionBox>(
                    Transfer::asset_numeric(source_asset_id, Numeric::from(amount), destination_id)
                        .into(),
                    realistic_load_metadata(index),
                )
                .wrap_err_with(|| {
                    format!(
                        "failed to submit paced transfer instruction {index} from account index {source} to {destination}"
                    )
                })?;
            submitted_counter.fetch_add(1, AtomicOrdering::Relaxed);
            Ok(())
        }));

        if pending.len() >= submit_parallelism {
            let result = pending
                .next()
                .await
                .ok_or_else(|| eyre!("paced transfer worker set unexpectedly empty"))?;
            result.wrap_err("paced transfer task join failed")??;
        }
    }

    while let Some(result) = pending.next().await {
        result.wrap_err("paced transfer task join failed")??;
    }
    Ok(submit_start.elapsed())
}

fn verify_realistic_transfer_balances(
    client: &iroha::client::Client,
    asset_definition_id: &AssetDefinitionId,
    accounts: &[TransferLoadAccount],
    tx_count: u64,
    initial_balance: u64,
    max_amount: u64,
    rng_seed: u64,
) -> Result<()> {
    let expected = expected_realistic_transfer_balances(
        accounts.len(),
        tx_count,
        initial_balance,
        max_amount,
        rng_seed,
    );
    for (account, expected_balance) in accounts.iter().zip(expected) {
        let asset = client.query_single(FindAssetById::new(AssetId::new(
            asset_definition_id.clone(),
            account.id.clone(),
        )))?;
        ensure!(
            *asset.value() == Numeric::from(expected_balance),
            "unexpected final transfer balance for {}: expected {}, got {:?}",
            account.id,
            expected_balance,
            asset.value()
        );
    }
    Ok(())
}

fn verify_realistic_ram_lfe_email_claim_counts(
    client: &iroha::client::Client,
    accounts: &[RamLfeEmailLoadAccount],
    tx_count: u64,
    rng_seed: u64,
) -> Result<()> {
    let expected =
        expected_realistic_ram_lfe_email_claim_counts(accounts.len(), tx_count, rng_seed);
    for (account, expected_count) in accounts.iter().zip(expected) {
        let stored_account = client.query_single(FindAccountById::new(account.id.clone()))?;
        ensure!(
            stored_account.uaid() == Some(&account.uaid),
            "unexpected UAID for RAM-LFE email account {}",
            account.id
        );
        ensure!(
            stored_account.opaque_ids().len() == expected_count,
            "unexpected RAM-LFE email claim count for {}: expected {}, got {}",
            account.id,
            expected_count,
            stored_account.opaque_ids().len()
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "long-running 4-peer localnet regression (30 TPS for 2 hours)"]
#[allow(clippy::too_many_lines, clippy::cast_precision_loss)]
async fn permissioned_localnet_realistic_30tps_2h() -> Result<()> {
    run_realistic_30tps_localnet(
        Realistic30TpsConsensusMode::Permissioned,
        stringify!(permissioned_localnet_realistic_30tps_2h),
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "long-running 4-peer localnet regression (30 TPS for 2 hours, NPoS)"]
#[allow(clippy::too_many_lines, clippy::cast_precision_loss)]
async fn npos_localnet_realistic_30tps_2h() -> Result<()> {
    run_realistic_30tps_localnet(
        Realistic30TpsConsensusMode::Npos,
        stringify!(npos_localnet_realistic_30tps_2h),
    )
    .await
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Realistic30TpsConsensusMode {
    Permissioned,
    Npos,
}

impl Realistic30TpsConsensusMode {
    fn as_config_str(self) -> &'static str {
        match self {
            Self::Permissioned => "permissioned",
            Self::Npos => "npos",
        }
    }

    fn is_npos(self) -> bool {
        matches!(self, Self::Npos)
    }
}

async fn run_realistic_30tps_localnet(
    consensus_mode: Realistic30TpsConsensusMode,
    test_name: &'static str,
) -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;

    let duration_secs = env_or_default(
        "IROHA_REALISTIC_30TPS_DURATION_SECS",
        REALISTIC_30TPS_DURATION_SECS,
    );
    let target_tps = env_or_default(
        "IROHA_REALISTIC_30TPS_TARGET_TPS",
        REALISTIC_30TPS_TARGET_TPS,
    );
    let configured_target_blocks = env_or_default(
        "IROHA_REALISTIC_30TPS_TARGET_BLOCKS",
        REALISTIC_30TPS_TARGET_BLOCKS,
    );
    let block_time_ms = env_or_default(
        "IROHA_REALISTIC_30TPS_BLOCK_TIME_MS",
        REALISTIC_30TPS_BLOCK_TIME_MS,
    )
    .max(1);
    let commit_time_ms = env_or_default(
        "IROHA_REALISTIC_30TPS_COMMIT_TIME_MS",
        REALISTIC_30TPS_COMMIT_TIME_MS,
    )
    .max(block_time_ms);
    let stall_secs = env_or_default(
        "IROHA_REALISTIC_30TPS_STALL_SECS",
        REALISTIC_30TPS_STALL_THRESHOLD.as_secs(),
    );
    let stall_threshold = Duration::from_secs(stall_secs.max(1));
    let max_avg_secs_per_block =
        env_or_default_f64("IROHA_REALISTIC_30TPS_MAX_AVG_SECS_PER_BLOCK", 3.0);
    let submit_parallelism = env_or_default_usize(
        "IROHA_REALISTIC_30TPS_PARALLELISM",
        REALISTIC_30TPS_SUBMIT_PARALLELISM,
    )
    .max(1);
    let queue_soft_limit = env_or_default(
        "IROHA_REALISTIC_30TPS_QUEUE_SOFT_LIMIT",
        REALISTIC_30TPS_QUEUE_SOFT_LIMIT,
    );
    let configured_transfer_accounts = env_or_default_usize(
        "IROHA_REALISTIC_30TPS_TRANSFER_ACCOUNTS",
        REALISTIC_30TPS_TRANSFER_ACCOUNTS,
    );
    let transfer_accounts = configured_transfer_accounts.max(2);
    let ram_lfe_email_accounts = env_or_default_usize(
        "IROHA_REALISTIC_30TPS_RAM_LFE_EMAIL_ACCOUNTS",
        configured_transfer_accounts,
    )
    .max(1);
    let transfer_max_amount = env_or_default(
        "IROHA_REALISTIC_30TPS_TRANSFER_MAX_AMOUNT",
        REALISTIC_30TPS_TRANSFER_MAX_AMOUNT,
    )
    .max(1);
    let block_max_txs = env_or_default(
        "IROHA_REALISTIC_30TPS_BLOCK_MAX_TXS",
        REALISTIC_30TPS_BLOCK_MAX_TXS,
    );
    let rng_seed = env_or_default("IROHA_REALISTIC_30TPS_RNG_SEED", THROUGHPUT_RNG_SEED);
    let total_txs = duration_secs.saturating_mul(target_tps);
    let minimum_initial_balance = total_txs
        .saturating_mul(transfer_max_amount)
        .saturating_add(1);
    let transfer_initial_balance = env_or_default(
        "IROHA_REALISTIC_30TPS_TRANSFER_INITIAL_BALANCE",
        minimum_initial_balance,
    )
    .max(minimum_initial_balance);
    ensure!(
        total_txs > 0 && block_max_txs > 0,
        "realistic 30 TPS run requires a positive duration and TPS"
    );
    let target_blocks = realistic_target_blocks(configured_target_blocks, total_txs, block_max_txs);
    let load_kind = Realistic30TpsLoadKind::from_env()?;
    let transfer_asset_definition_id = realistic_transfer_asset_definition_id();
    let transfer_load_accounts = if load_kind == Realistic30TpsLoadKind::Transfer {
        realistic_transfer_accounts(transfer_accounts, rng_seed)
    } else {
        Vec::new()
    };
    let ram_lfe_email_load_accounts = if load_kind == Realistic30TpsLoadKind::RamLfeEmail {
        realistic_ram_lfe_email_accounts(ram_lfe_email_accounts, rng_seed)
    } else {
        Vec::new()
    };
    let ram_lfe_email_resolver = KeyPair::from_seed(
        format!("integration_tests::realistic-ram-lfe-email-resolver::{rng_seed}").into_bytes(),
        Algorithm::Ed25519,
    );
    let ram_lfe_email_owner = (*REAL_GENESIS_ACCOUNT_ID).clone();
    let (ram_lfe_email_policy, ram_lfe_email_program_policy) =
        realistic_ram_lfe_email_policy_bundle(&ram_lfe_email_owner, &ram_lfe_email_resolver);
    let ram_lfe_email_policy_context = realistic_ram_lfe_email_policy_context(
        ram_lfe_email_policy.id.clone(),
        &ram_lfe_email_program_policy,
    )?;

    let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");
    let client_ttl = Duration::from_secs(duration_secs.saturating_add(120));
    set_env_var(
        "IROHA_TEST_CLIENT_TTL_MS",
        client_ttl.as_millis().to_string(),
    );

    let mut builder = NetworkBuilder::new()
        .with_peers(REALISTIC_30TPS_PEERS)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_pipeline_time(THROUGHPUT_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(
                std::num::NonZeroU64::new(block_max_txs).expect("checked non-zero"),
            ),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(block_time_ms),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(commit_time_ms),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CollectorsK(2),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(2),
        )))
        .with_config_layer(|layer| {
            let writer = layer
                .write(
                    ["sumeragi", "consensus_mode"],
                    consensus_mode.as_config_str(),
                )
                .write(["logger", "level"], "WARN")
                .write(["network", "transaction_gossip_period_ms"], 20_i64)
                .write(["network", "transaction_gossip_size"], 64_i64)
                .write(["network", "max_frame_bytes_tx_gossip"], 65_536_i64)
                .write(["network", "transaction_gossip_public_target_cap"], 8_i64)
                .write(
                    ["network", "transaction_gossip_restricted_target_cap"],
                    8_i64,
                )
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                .write(["network", "p2p_post_queue_cap"], 8192_i64)
                .write(["network", "p2p_queue_cap_high"], 16384_i64)
                .write(["network", "p2p_queue_cap_low"], 65536_i64)
                .write(["network", "disconnect_on_post_overflow"], false)
                .write(
                    ["sumeragi", "advanced", "worker", "parallel_ingress"],
                    false,
                )
                .write(
                    [
                        "sumeragi",
                        "advanced",
                        "worker",
                        "vote_burst_cap_with_payload_backlog",
                    ],
                    1_i64,
                )
                .write(
                    ["sumeragi", "advanced", "da", "quorum_timeout_multiplier"],
                    1_i64,
                )
                .write(
                    [
                        "sumeragi",
                        "advanced",
                        "da",
                        "availability_timeout_multiplier",
                    ],
                    1_i64,
                )
                .write(
                    [
                        "sumeragi",
                        "advanced",
                        "da",
                        "availability_timeout_floor_ms",
                    ],
                    100_i64,
                )
                .write(
                    ["sumeragi", "block", "fast_finality_max_transactions"],
                    i64::try_from(block_max_txs).expect("realistic block max txs fits i64"),
                )
                .write(
                    ["sumeragi", "advanced", "pacing_governor", "min_factor_bps"],
                    10_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacing_governor", "max_factor_bps"],
                    10_000_i64,
                )
                .write(["nexus", "fusion", "floor_teu"], THROUGHPUT_LANE_TEU_FLOOR)
                .write(
                    ["nexus", "fusion", "exit_teu"],
                    THROUGHPUT_LANE_TEU_CAPACITY,
                )
                .write(
                    ["torii", "preauth_allow_cidrs"],
                    TomlValue::Array(vec![
                        TomlValue::String("127.0.0.0/8".into()),
                        TomlValue::String("::1/128".into()),
                    ]),
                )
                .write(
                    ["torii", "api_allow_cidrs"],
                    TomlValue::Array(vec![
                        TomlValue::String("127.0.0.0/8".into()),
                        TomlValue::String("::1/128".into()),
                    ]),
                )
                .write(["torii", "preauth_rate_per_ip_per_sec"], 1_000_000_i64)
                .write(["torii", "preauth_burst_per_ip"], 2_000_000_i64)
                .write(["torii", "query_rate_per_authority_per_sec"], 0_i64)
                .write(["torii", "query_burst_per_authority"], 0_i64)
                .write(["torii", "tx_rate_per_authority_per_sec"], 0_i64)
                .write(["torii", "tx_burst_per_authority"], 0_i64)
                .write(["torii", "api_high_load_tx_threshold"], 262_144_i64);
            if consensus_mode.is_npos() {
                let _ = writer
                    .write(["sumeragi", "collectors", "k"], 2_i64)
                    .write(["sumeragi", "collectors", "redundant_send_r"], 2_i64)
                    .write(["sumeragi", "npos", "use_stake_snapshot_roster"], true)
                    .write(["sumeragi", "npos", "election", "max_validators"], 4_i64)
                    .write(["sumeragi", "npos", "epoch_length_blocks"], 3600_i64)
                    .write(
                        ["sumeragi", "advanced", "npos", "timeouts", "propose_ms"],
                        i64::try_from(block_time_ms).expect("block time fits i64"),
                    )
                    .write(
                        ["sumeragi", "advanced", "npos", "timeouts", "prevote_ms"],
                        i64::try_from(commit_time_ms.saturating_mul(2))
                            .expect("prevote timeout fits i64"),
                    )
                    .write(
                        ["sumeragi", "advanced", "npos", "timeouts", "precommit_ms"],
                        i64::try_from(commit_time_ms.saturating_mul(3))
                            .expect("precommit timeout fits i64"),
                    )
                    .write(
                        ["sumeragi", "advanced", "npos", "timeouts", "commit_ms"],
                        i64::try_from(commit_time_ms.saturating_mul(4))
                            .expect("commit timeout fits i64"),
                    )
                    .write(
                        ["sumeragi", "advanced", "npos", "timeouts", "da_ms"],
                        i64::try_from(commit_time_ms.saturating_mul(2))
                            .expect("DA timeout fits i64"),
                    );
            }
        });
    match load_kind {
        Realistic30TpsLoadKind::Transfer => {
            builder = builder
                .with_genesis_instruction(Register::domain(Domain::new(
                    realistic_transfer_domain_id(),
                )))
                .with_genesis_instruction(Register::asset_definition(
                    AssetDefinition::numeric(transfer_asset_definition_id.clone())
                        .with_name("Realistic Transfer Coin".to_owned()),
                ));
            for account in &transfer_load_accounts {
                builder = builder
                    .with_genesis_instruction(Register::account(Account::new(account.id.clone())))
                    .with_genesis_instruction(Mint::asset_numeric(
                        Numeric::from(transfer_initial_balance),
                        AssetId::new(transfer_asset_definition_id.clone(), account.id.clone()),
                    ));
            }
        }
        Realistic30TpsLoadKind::RamLfeEmail => {
            builder = builder
                .with_genesis_instruction(
                    Box::new(RegisterRamLfeProgramPolicy {
                        policy: ram_lfe_email_program_policy.clone(),
                    })
                    .into_instruction_box(),
                )
                .with_genesis_instruction(
                    Box::new(ActivateRamLfeProgramPolicy {
                        program_id: ram_lfe_email_program_policy.program_id.clone(),
                    })
                    .into_instruction_box(),
                )
                .with_genesis_instruction(RegisterIdentifierPolicy {
                    policy: ram_lfe_email_policy.clone(),
                })
                .with_genesis_instruction(ActivateIdentifierPolicy {
                    policy_id: ram_lfe_email_policy.id.clone(),
                });
            for account in &ram_lfe_email_load_accounts {
                builder = builder.with_genesis_instruction(Register::account(
                    Account::new(account.id.clone()).with_uaid(Some(account.uaid)),
                ));
            }
        }
    }

    let result: Result<()> = async {
        let Some(network) = sandbox::start_network_async_or_skip(builder, test_name).await?
        else {
            return Ok(());
        };

        let network_dir = network.env_dir().to_path_buf();
        let http = HttpClient::builder()
            .tls_built_in_root_certs(false)
            .build()
            .wrap_err("build local HTTP client for realistic throughput soak")?;
        let mut artifacts = ThroughputArtifacts::default();

        let run_result: Result<()> = async {
            wait_for_status_responses(&network, Duration::from_secs(30)).await?;
            if consensus_mode.is_npos() && load_kind == Realistic30TpsLoadKind::Transfer {
                fund_realistic_npos_transfer_fee_accounts(&network, &transfer_load_accounts)
                    .await
                    .wrap_err("failed to fund realistic NPoS transfer fee accounts")?;
            }
            let baseline_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
            let baseline_non_empty = baseline_statuses
                .iter()
                .map(|status| status.blocks_non_empty)
                .min()
                .unwrap_or_default();
            let baseline_approved = baseline_statuses
                .iter()
                .map(|status| status.txs_approved)
                .min()
                .unwrap_or_default();
            let target_non_empty = baseline_non_empty.saturating_add(target_blocks);
            let target_approved = baseline_approved.saturating_add(total_txs);
            artifacts.recipe = Some(ThroughputArtifactRecipe {
                peers: network.peers().len() as u64,
                block_time_ms,
                commit_time_ms,
                block_max_txs,
                warmup_blocks: 0,
                steady_blocks: target_blocks,
                total_blocks: target_blocks,
                warmup_txs: 0,
                steady_txs: total_txs,
                total_txs,
                submit_batch: target_tps,
                submit_parallelism: submit_parallelism as u64,
                queue_soft_limit,
                payload_bytes: if load_kind == Realistic30TpsLoadKind::RamLfeEmail {
                    realistic_ram_lfe_email_address(0, rng_seed).len() as u64
                } else {
                    0
                },
                load_kind: load_kind.as_str().to_owned(),
                transfer_accounts: if load_kind == Realistic30TpsLoadKind::Transfer {
                    transfer_accounts as u64
                } else {
                    0
                },
                transfer_initial_balance: if load_kind == Realistic30TpsLoadKind::Transfer {
                    transfer_initial_balance
                } else {
                    0
                },
                transfer_max_amount: if load_kind == Realistic30TpsLoadKind::Transfer {
                    transfer_max_amount
                } else {
                    0
                },
                ram_lfe_email_accounts: if load_kind == Realistic30TpsLoadKind::RamLfeEmail {
                    ram_lfe_email_accounts as u64
                } else {
                    0
                },
                ram_lfe_email_policy: if load_kind == Realistic30TpsLoadKind::RamLfeEmail {
                    ram_lfe_email_policy.id.to_string()
                } else {
                    String::new()
                },
                ram_lfe_program: if load_kind == Realistic30TpsLoadKind::RamLfeEmail {
                    ram_lfe_email_program_policy.program_id.to_string()
                } else {
                    String::new()
                },
                rng_seed,
                rbc_encoding: "plain".to_owned(),
                rbc_data_shards: 0,
                rbc_parity_shards: 0,
            });

            let submitted_counter = Arc::new(AtomicU64::new(0));
            let submitted_for_task = Arc::clone(&submitted_counter);
            let submit_handle = match load_kind {
                Realistic30TpsLoadKind::Transfer => {
                    let submit_accounts: Vec<_> = transfer_load_accounts
                        .iter()
                        .enumerate()
                        .map(|(index, account)| {
                            let peer = &network.peers()[index % network.peers().len()];
                            TransferSubmitAccount {
                                id: account.id.clone(),
                                client: peer.client_for(
                                    &account.id,
                                    account.key_pair.private_key().clone(),
                                ),
                            }
                        })
                        .collect();
                    tokio::spawn(submit_transfers_paced(
                        total_txs,
                        target_tps,
                        submit_accounts,
                        transfer_asset_definition_id.clone(),
                        transfer_max_amount,
                        rng_seed,
                        submit_parallelism,
                        submitted_for_task,
                    ))
                }
                Realistic30TpsLoadKind::RamLfeEmail => {
                    // ClaimIdentifier accepts the policy owner as authority; using it here keeps
                    // the generated UAID-bearing accounts from needing space-directory lane
                    // bindings in this local soak harness.
                    let policy_owner_private_key =
                        REAL_GENESIS_ACCOUNT_KEYPAIR.private_key().clone();
                    let submit_accounts: Vec<_> = ram_lfe_email_load_accounts
                        .iter()
                        .enumerate()
                        .map(|(index, account)| {
                            let peer = &network.peers()[index % network.peers().len()];
                            RamLfeEmailSubmitAccount {
                                id: account.id.clone(),
                                client: peer.client_for(
                                    &ram_lfe_email_owner,
                                    policy_owner_private_key.clone(),
                                ),
                                uaid: account.uaid,
                            }
                        })
                        .collect();
                    tokio::spawn(submit_ram_lfe_emails_paced(
                        total_txs,
                        target_tps,
                        submit_accounts,
                        ram_lfe_email_policy_context.clone(),
                        ram_lfe_email_resolver.clone(),
                        rng_seed,
                        submit_parallelism,
                        submitted_for_task,
                    ))
                }
            };

            eprintln!(
                "realistic localnet recipe: peers={}, target_tps={}, duration_secs={}, total_txs={}, target_non_empty_delta={}, block_time_ms={}, commit_time_ms={}, block_max_txs={}, load_kind={}, transfer_accounts={}, transfer_initial_balance={}, transfer_max_amount={}, ram_lfe_email_accounts={}, ram_lfe_email_policy={}, ram_lfe_program={}, submit_parallelism={}, queue_soft_limit={}, max_avg_secs_per_block={max_avg_secs_per_block:.3}, baseline_non_empty={}, baseline_approved={}",
                network.peers().len(),
                target_tps,
                duration_secs,
                total_txs,
                target_blocks,
                block_time_ms,
                commit_time_ms,
                block_max_txs,
                load_kind.as_str(),
                if load_kind == Realistic30TpsLoadKind::Transfer {
                    transfer_accounts
                } else {
                    0
                },
                if load_kind == Realistic30TpsLoadKind::Transfer {
                    transfer_initial_balance
                } else {
                    0
                },
                if load_kind == Realistic30TpsLoadKind::Transfer {
                    transfer_max_amount
                } else {
                    0
                },
                if load_kind == Realistic30TpsLoadKind::RamLfeEmail {
                    ram_lfe_email_accounts
                } else {
                    0
                },
                ram_lfe_email_policy.id,
                ram_lfe_email_program_policy.program_id,
                submit_parallelism,
                queue_soft_limit,
                baseline_non_empty,
                baseline_approved,
            );

            artifacts.samples.clear();
            let mut last_progress = Instant::now();
            let mut last_min_non_empty = baseline_non_empty;
            let mut last_min_approved = baseline_approved;
            let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();
            let mut last_log = Instant::now()
                .checked_sub(REALISTIC_30TPS_PROGRESS_LOG_INTERVAL)
                .unwrap_or_else(Instant::now);
            let run_start = Instant::now();

            loop {
                match collect_statuses(&network, STATUS_POLL_TIMEOUT).await {
                    Ok(statuses) => {
                        let min_non_empty = statuses
                            .iter()
                            .map(|status| status.blocks_non_empty)
                            .min()
                            .unwrap_or_default();
                        let max_non_empty = statuses
                            .iter()
                            .map(|status| status.blocks_non_empty)
                            .max()
                            .unwrap_or_default();
                        let min_approved = statuses
                            .iter()
                            .map(|status| status.txs_approved)
                            .min()
                            .unwrap_or_default();
                        let max_queue = statuses
                            .iter()
                            .map(|status| status.queue_size)
                            .max()
                            .unwrap_or_default();
                        if min_non_empty > last_min_non_empty || min_approved > last_min_approved {
                            last_min_non_empty = min_non_empty;
                            last_min_approved = min_approved;
                            last_progress = Instant::now();
                        }
                        let status_snapshots: Vec<StatusSnapshot> =
                            statuses.iter().map(StatusSnapshot::from_status).collect();
                        let sumeragi_snapshots = match collect_sumeragi_statuses(
                            &network,
                            STATUS_POLL_TIMEOUT,
                        )
                        .await
                        {
                            Ok(sumeragi) => sumeragi
                                .iter()
                                .map(SumeragiStatusSnapshot::from_status)
                                .collect(),
                            Err(err) => {
                                eprintln!("sumeragi status sample failed: {err:?}");
                                Vec::new()
                            }
                        };
                        let timestamp_ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis();
                        artifacts.samples.push(ThroughputSample {
                            phase: Some("load".to_owned()),
                            timestamp_ms: u64::try_from(timestamp_ms).unwrap_or(u64::MAX),
                            statuses: status_snapshots.clone(),
                            sumeragi: sumeragi_snapshots,
                        });
                        last_snapshot = status_snapshots;

                        if last_log.elapsed() >= REALISTIC_30TPS_PROGRESS_LOG_INTERVAL {
                            eprintln!(
                                "realistic localnet progress elapsed={:?} submitted={} target_non_empty={} min_non_empty={} max_non_empty={} min_approved={} target_approved={} max_queue={}: {:?}",
                                run_start.elapsed(),
                                submitted_counter.load(AtomicOrdering::Relaxed),
                                target_non_empty,
                                min_non_empty,
                                max_non_empty,
                                min_approved,
                                target_approved,
                                max_queue,
                                last_snapshot,
                            );
                            last_log = Instant::now();
                        }
                    }
                    Err(err) => {
                        if last_log.elapsed() >= REALISTIC_30TPS_PROGRESS_LOG_INTERVAL {
                            eprintln!("realistic localnet status poll failed: {err:?}");
                            last_log = Instant::now();
                        }
                    }
                }

                if submit_handle.is_finished() {
                    break;
                }
                if last_progress.elapsed() >= stall_threshold {
                    submit_handle.abort();
                    let _ = submit_handle.await;
                    let elapsed = run_start.elapsed();
                    let submitted = submitted_counter.load(AtomicOrdering::Relaxed);
                    let produced_blocks = last_min_non_empty.saturating_sub(baseline_non_empty);
                    let status_summary = ThroughputStatusSummary::from_statuses(&last_snapshot);
                    artifacts.realistic = Some(realistic_artifact_summary(
                        baseline_non_empty,
                        baseline_approved,
                        target_non_empty,
                        target_approved,
                        submitted,
                        elapsed,
                        elapsed,
                        elapsed,
                        status_summary.clone(),
                        status_summary,
                        produced_blocks,
                        produced_blocks,
                        &artifacts.samples,
                    ));
                    return Err(eyre!(
                        "realistic localnet stalled for {:?}: elapsed={:?}, submitted={}, last_min_non_empty={}, last_min_approved={}, last_snapshot={last_snapshot:?}",
                        stall_threshold,
                        run_start.elapsed(),
                        submitted_counter.load(AtomicOrdering::Relaxed),
                        last_min_non_empty,
                        last_min_approved,
                    ));
                }
                sleep(REALISTIC_30TPS_SAMPLE_INTERVAL).await;
            }

            let submit_elapsed = submit_handle
                .await
                .wrap_err("paced submit task join failed")??;
            let load_end_elapsed = run_start.elapsed();
            let load_end_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
            let load_end_min_non_empty = load_end_statuses
                .iter()
                .map(|status| status.blocks_non_empty)
                .min()
                .unwrap_or_default();
            let load_end_snapshots: Vec<StatusSnapshot> = load_end_statuses
                .iter()
                .map(StatusSnapshot::from_status)
                .collect();
            let load_end_summary =
                ThroughputStatusSummary::from_statuses(&load_end_snapshots);
            let load_end_produced_blocks =
                load_end_min_non_empty.saturating_sub(baseline_non_empty);
            let max_load_queue_size = max_queue_size_for_phase(&artifacts.samples, "load");
            let mut after_statuses = load_end_statuses;
            let mut drain_last_progress = Instant::now();
            let mut drain_last_min_approved = after_statuses
                .iter()
                .map(|status| status.txs_approved)
                .min()
                .unwrap_or_default();
            let mut drain_last_max_queue = after_statuses
                .iter()
                .map(|status| status.queue_size)
                .max()
                .unwrap_or_default();
            while drain_last_min_approved < target_approved {
                if drain_last_progress.elapsed() >= stall_threshold {
                    let final_snapshots: Vec<StatusSnapshot> =
                        after_statuses.iter().map(StatusSnapshot::from_status).collect();
                    let final_summary = ThroughputStatusSummary::from_statuses(&final_snapshots);
                    let produced_blocks = final_summary
                        .min_blocks_non_empty
                        .saturating_sub(baseline_non_empty);
                    artifacts.realistic = Some(realistic_artifact_summary(
                        baseline_non_empty,
                        baseline_approved,
                        target_non_empty,
                        target_approved,
                        submitted_counter.load(AtomicOrdering::Relaxed),
                        submit_elapsed,
                        load_end_elapsed,
                        run_start.elapsed(),
                        load_end_summary.clone(),
                        final_summary,
                        load_end_produced_blocks,
                        produced_blocks,
                        &artifacts.samples,
                    ));
                    return Err(eyre!(
                        "realistic localnet stalled while draining after load for {:?}: load_elapsed={:?}, submitted={}, min_approved={}, target_approved={}, max_queue={}, last_statuses={after_statuses:?}",
                        stall_threshold,
                        load_end_elapsed,
                        submitted_counter.load(AtomicOrdering::Relaxed),
                        drain_last_min_approved,
                        target_approved,
                        drain_last_max_queue,
                    ));
                }
                sleep(REALISTIC_30TPS_SAMPLE_INTERVAL).await;
                after_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
                let status_snapshots: Vec<StatusSnapshot> =
                    after_statuses.iter().map(StatusSnapshot::from_status).collect();
                let sumeragi_snapshots = match collect_sumeragi_statuses(
                    &network,
                    STATUS_POLL_TIMEOUT,
                )
                .await
                {
                    Ok(sumeragi) => sumeragi
                        .iter()
                        .map(SumeragiStatusSnapshot::from_status)
                        .collect(),
                    Err(err) => {
                        eprintln!("sumeragi status drain sample failed: {err:?}");
                        Vec::new()
                    }
                };
                let timestamp_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                artifacts.samples.push(ThroughputSample {
                    phase: Some("drain".to_owned()),
                    timestamp_ms: u64::try_from(timestamp_ms).unwrap_or(u64::MAX),
                    statuses: status_snapshots,
                    sumeragi: sumeragi_snapshots,
                });
                let min_approved = after_statuses
                    .iter()
                    .map(|status| status.txs_approved)
                    .min()
                    .unwrap_or_default();
                let max_queue = after_statuses
                    .iter()
                    .map(|status| status.queue_size)
                    .max()
                    .unwrap_or_default();
                if min_approved > drain_last_min_approved || max_queue < drain_last_max_queue {
                    drain_last_min_approved = min_approved;
                    drain_last_max_queue = max_queue;
                    drain_last_progress = Instant::now();
                }
            }
            let min_non_empty = after_statuses
                .iter()
                .map(|status| status.blocks_non_empty)
                .min()
                .unwrap_or_default();
            let min_approved = after_statuses
                .iter()
                .map(|status| status.txs_approved)
                .min()
                .unwrap_or_default();
            let max_rejected = after_statuses
                .iter()
                .map(|status| status.txs_rejected)
                .max()
                .unwrap_or_default();
            let produced_blocks = min_non_empty.saturating_sub(baseline_non_empty);
            let load_avg_secs_per_block =
                seconds_per_block(load_end_elapsed, load_end_produced_blocks);
            let avg_secs_per_block = seconds_per_block(run_start.elapsed(), produced_blocks);

            if let Ok(after_metrics) =
                collect_metrics_snapshots(&network, &http, THROUGHPUT_METRICS_TIMEOUT).await
            {
                artifacts.after_metrics = after_metrics;
            }
            let final_snapshots: Vec<StatusSnapshot> =
                after_statuses.iter().map(StatusSnapshot::from_status).collect();
            let final_summary = ThroughputStatusSummary::from_statuses(&final_snapshots);
            let total_elapsed = run_start.elapsed();
            let submitted = submitted_counter.load(AtomicOrdering::Relaxed);
            artifacts.realistic = Some(realistic_artifact_summary(
                baseline_non_empty,
                baseline_approved,
                target_non_empty,
                target_approved,
                submitted,
                submit_elapsed,
                load_end_elapsed,
                total_elapsed,
                load_end_summary.clone(),
                final_summary.clone(),
                load_end_produced_blocks,
                produced_blocks,
                &artifacts.samples,
            ));
            eprintln!(
                "realistic localnet summary: load_elapsed={:?}, elapsed={:?}, submit_elapsed={:?}, submitted={}, load_end_produced_blocks={}, produced_blocks={}, min_non_empty={}, target_non_empty={}, final_min_non_empty={}, min_approved={}, target_approved={}, max_rejected={}, load_avg_secs_per_block={load_avg_secs_per_block:.3}, avg_secs_per_block={avg_secs_per_block:.3}",
                load_end_elapsed,
                run_start.elapsed(),
                submit_elapsed,
                submitted_counter.load(AtomicOrdering::Relaxed),
                load_end_produced_blocks,
                produced_blocks,
                load_end_min_non_empty,
                target_non_empty,
                min_non_empty,
                min_approved,
                target_approved,
                max_rejected,
            );
            ensure!(
                load_end_produced_blocks >= target_blocks,
                "realistic 30 TPS load did not keep up with submitted TPS: produced {load_end_produced_blocks} non-empty blocks during load, target {target_blocks}, block_max_txs={block_max_txs}, load_elapsed={load_end_elapsed:?}"
            );
            ensure!(
                max_load_queue_size <= queue_soft_limit,
                "realistic 30 TPS queue exceeded soft limit during load: max_queue_size={max_load_queue_size}, queue_soft_limit={queue_soft_limit}, load_committed_tps={:.3}, submitted_tps={:.3}",
                rate_per_second(
                    load_end_summary
                        .min_txs_approved
                        .saturating_sub(baseline_approved),
                    load_end_elapsed,
                ),
                rate_per_second(submitted, submit_elapsed),
            );
            ensure!(
                min_non_empty >= target_non_empty,
                "expected at least {target_blocks} final non-empty blocks for {duration_secs}s at {target_tps} TPS; produced {produced_blocks} (baseline={baseline_non_empty}, load_end_min_non_empty={load_end_min_non_empty}, final_min_non_empty={min_non_empty})"
            );
            ensure!(
                load_avg_secs_per_block <= max_avg_secs_per_block,
                "average block interval during load exceeded {max_avg_secs_per_block:.3}s: load_avg_secs_per_block={load_avg_secs_per_block:.3}, load_end_produced_blocks={load_end_produced_blocks}, load_elapsed={load_end_elapsed:?}"
            );
            ensure!(
                min_approved >= target_approved,
                "not all submitted transactions were approved: min_approved={min_approved}, target_approved={target_approved}, submitted={}",
                submitted_counter.load(AtomicOrdering::Relaxed)
            );
            ensure!(
                max_rejected == 0,
                "transactions were rejected during realistic localnet run: max_rejected={max_rejected}"
            );
            match load_kind {
                Realistic30TpsLoadKind::Transfer => {
                    verify_realistic_transfer_balances(
                        &network.client(),
                        &transfer_asset_definition_id,
                        &transfer_load_accounts,
                        total_txs,
                        transfer_initial_balance,
                        transfer_max_amount,
                        rng_seed,
                    )
                    .wrap_err("realistic transfer balances did not match submitted random graph")?;
                }
                Realistic30TpsLoadKind::RamLfeEmail => {
                    verify_realistic_ram_lfe_email_claim_counts(
                        &network.client(),
                        &ram_lfe_email_load_accounts,
                        total_txs,
                        rng_seed,
                    )
                    .wrap_err("realistic RAM-LFE email claim counts did not match submitted route")?;
                }
            }
            Ok(())
        }
        .await;

        if let Err(err) = &run_result {
            artifacts.error = Some(err.to_string());
        }
        if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {
            let root = PathBuf::from(artifact_root);
            let peer_logs: Vec<PeerLogInfo> = network
                .peers()
                .iter()
                .enumerate()
                .map(|(index, peer)| PeerLogInfo {
                    index: index as u64,
                    mnemonic: peer.mnemonic().to_string(),
                    stdout_log: peer
                        .latest_stdout_log_path()
                        .map(|path| path.to_string_lossy().to_string()),
                    stderr_log: peer
                        .latest_stderr_log_path()
                        .map(|path| path.to_string_lossy().to_string()),
                })
                .collect();
            if let Err(err) =
                write_throughput_artifacts(&root, &network_dir, &peer_logs, &artifacts)
            {
                eprintln!("throughput artifact write failed: {err:?}");
            }
        }

        network.shutdown().await;
        run_result
    }
    .await;

    if let Some(previous_ttl) = previous_ttl {
        set_env_var("IROHA_TEST_CLIENT_TTL_MS", previous_ttl);
    } else {
        remove_env_var("IROHA_TEST_CLIENT_TTL_MS");
    }

    if sandbox::handle_result(result, test_name)?.is_none() {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn permissioned_localnet_produces_blocks_within_bound() -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_pipeline_time(SMOKE_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(SMOKE_BLOCK_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(SMOKE_COMMIT_TIME_MS),
        )))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
                .write(["network", "transaction_gossip_period_ms"], 200_i64)
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                // Tighten local timeouts to keep proposal/view-change cadence bounded.
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "propose_ms"],
                    400_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "prevote_ms"],
                    800_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "precommit_ms"],
                    1_200_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "commit_ms"],
                    1_600_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "da_ms"],
                    800_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    2_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    1_i64,
                );
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(permissioned_localnet_produces_blocks_within_bound),
    )
    .await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        let baseline_statuses = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        let baseline_height = baseline_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let warmup_height = baseline_height.saturating_add(1);
        for peer in network.peers() {
            let message = format!("localnet warmup block {}", peer.mnemonic());
            peer.client()
                .submit::<InstructionBox>(Log::new(Level::INFO, message).into())
                .wrap_err_with(|| {
                    format!("failed to submit warmup log instruction to {}", peer.mnemonic())
                })?;
        }
        wait_for_converged_height(&network, warmup_height, Duration::from_secs(45)).await?;
        let warmup_statuses = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        let baseline_height = warmup_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let baseline_view_changes: Vec<u64> = warmup_statuses
            .iter()
            .map(|status| status.view_changes.into())
            .collect();
        let peer_count = network.peers().len();
        let fault_tolerance = peer_count.saturating_sub(1) / 3;
        let max_extra_view_changes = u64::try_from(fault_tolerance.saturating_add(2))
            .unwrap_or(u64::MAX);

        ensure!(!network.peers().is_empty(), "network must have at least one peer");
        for peer in network.peers() {
            let message = format!("localnet bounded block {}", peer.mnemonic());
            peer.client()
                .submit::<InstructionBox>(Log::new(Level::INFO, message).into())
                .wrap_err_with(|| {
                    format!("failed to submit log instruction to {}", peer.mnemonic())
                })?;
        }

        let target_height = baseline_height.saturating_add(1);
        let start = Instant::now();
        wait_for_converged_height(&network, target_height, Duration::from_secs(45)).await?;
        let elapsed = start.elapsed();
        ensure!(
            elapsed <= Duration::from_secs(15),
            "block production exceeded bound: elapsed={:?}",
            elapsed
        );

        let after_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        ensure!(
            after_statuses
                .iter()
                .all(|status| status.blocks >= target_height),
            "not all peers reached target height {target_height}: {after_statuses:?}"
        );
        for (idx, status) in after_statuses.iter().enumerate() {
            let before = baseline_view_changes.get(idx).copied().unwrap_or_default();
            ensure!(
                u64::from(status.view_changes) <= before.saturating_add(max_extra_view_changes),
                "peer {idx} experienced repeated view changes: before={before}, after={}, max_extra={max_extra_view_changes}",
                status.view_changes,
            );
        }
        let min_view_changes = after_statuses
            .iter()
            .map(|status| u64::from(status.view_changes))
            .min()
            .unwrap_or_default();
        let max_view_changes = after_statuses
            .iter()
            .map(|status| u64::from(status.view_changes))
            .max()
            .unwrap_or_default();
        ensure!(
            max_view_changes.saturating_sub(min_view_changes) <= max_extra_view_changes,
            "view_change counters diverged across peers: {after_statuses:?}"
        );

        network.shutdown().await;
        Ok(())
    }
    .await;

    if sandbox::handle_result(
        result,
        stringify!(permissioned_localnet_produces_blocks_within_bound),
    )?
    .is_none()
    {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_status_json_endpoint_decodes_to_wire_end_to_end() -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;

    let mut lane_universal = Table::new();
    lane_universal.insert("index".into(), TomlValue::Integer(0));
    lane_universal.insert(
        "alias".into(),
        TomlValue::String("lane-universal".to_owned()),
    );
    lane_universal.insert(
        "dataspace".into(),
        TomlValue::String("universal".to_owned()),
    );
    lane_universal.insert("visibility".into(), TomlValue::String("public".to_owned()));
    lane_universal.insert("metadata".into(), TomlValue::Table(Table::new()));

    let mut lane_alice = Table::new();
    lane_alice.insert("index".into(), TomlValue::Integer(1));
    lane_alice.insert("alias".into(), TomlValue::String("lane-alice".to_owned()));
    lane_alice.insert("dataspace".into(), TomlValue::String("ds1".to_owned()));
    lane_alice.insert("visibility".into(), TomlValue::String("public".to_owned()));
    lane_alice.insert("metadata".into(), TomlValue::Table(Table::new()));

    let mut lane_bob = Table::new();
    lane_bob.insert("index".into(), TomlValue::Integer(2));
    lane_bob.insert("alias".into(), TomlValue::String("lane-bob".to_owned()));
    lane_bob.insert("dataspace".into(), TomlValue::String("ds2".to_owned()));
    lane_bob.insert("visibility".into(), TomlValue::String("public".to_owned()));
    lane_bob.insert("metadata".into(), TomlValue::Table(Table::new()));

    let mut ds_universal = Table::new();
    ds_universal.insert("alias".into(), TomlValue::String("universal".to_owned()));
    ds_universal.insert("id".into(), TomlValue::Integer(0));
    ds_universal.insert(
        "description".into(),
        TomlValue::String("default dataspace".to_owned()),
    );
    ds_universal.insert("fault_tolerance".into(), TomlValue::Integer(1));

    let mut ds1 = Table::new();
    ds1.insert("alias".into(), TomlValue::String("ds1".to_owned()));
    ds1.insert("id".into(), TomlValue::Integer(1));
    ds1.insert(
        "manifest_hash".into(),
        TomlValue::String(
            "0100000000000000000000000000000000000000000000000000000000000000".to_owned(),
        ),
    );
    ds1.insert(
        "description".into(),
        TomlValue::String("alice route dataspace".to_owned()),
    );
    ds1.insert("fault_tolerance".into(), TomlValue::Integer(1));

    let mut ds2 = Table::new();
    ds2.insert("alias".into(), TomlValue::String("ds2".to_owned()));
    ds2.insert("id".into(), TomlValue::Integer(2));
    ds2.insert(
        "manifest_hash".into(),
        TomlValue::String(
            "0200000000000000000000000000000000000000000000000000000000000000".to_owned(),
        ),
    );
    ds2.insert(
        "description".into(),
        TomlValue::String("bob route dataspace".to_owned()),
    );
    ds2.insert("fault_tolerance".into(), TomlValue::Integer(1));

    let mut matcher_alice = Table::new();
    matcher_alice.insert("account".into(), TomlValue::String(ALICE_ID.to_string()));
    let mut rule_alice = Table::new();
    rule_alice.insert("lane".into(), TomlValue::Integer(1));
    rule_alice.insert("dataspace".into(), TomlValue::String("ds1".to_owned()));
    rule_alice.insert("matcher".into(), TomlValue::Table(matcher_alice));

    let mut matcher_bob = Table::new();
    matcher_bob.insert("account".into(), TomlValue::String(BOB_ID.to_string()));
    let mut rule_bob = Table::new();
    rule_bob.insert("lane".into(), TomlValue::Integer(2));
    rule_bob.insert("dataspace".into(), TomlValue::String("ds2".to_owned()));
    rule_bob.insert("matcher".into(), TomlValue::Table(matcher_bob));

    let mut policy = Table::new();
    policy.insert("default_lane".into(), TomlValue::Integer(0));
    policy.insert(
        "default_dataspace".into(),
        TomlValue::String("universal".to_owned()),
    );
    policy.insert(
        "rules".into(),
        TomlValue::Array(vec![
            TomlValue::Table(rule_alice),
            TomlValue::Table(rule_bob),
        ]),
    );
    let gas_account_str = route_bootstrap_gas_account_id()
        .canonical_i105()
        .expect("canonical I105 bootstrap gas account literal");
    let stake_asset_id_literal = route_stake_asset_definition_id().to_string();
    let fee_asset_id_literal = route_fee_asset_definition_id().to_string();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .without_npos_genesis_bootstrap()
        .with_genesis_block(|topology, topology_entries| {
            let post_topology =
                route_multilane_genesis_post_topology_transactions(topology.as_ref());
            let mut genesis = genesis_factory_with_post_topology(
                Vec::new(),
                post_topology,
                topology,
                topology_entries,
            );
            genesis
                .0
                .set_da_proof_policies(Some(route_multilane_da_proof_policy_bundle()));
            genesis
        })
        .with_pipeline_time(SMOKE_PIPELINE_TIME)
        .with_config_layer(move |layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["nexus", "enabled"], true)
                .write(["nexus", "lane_count"], 3_i64)
                .write(
                    ["nexus", "lane_catalog"],
                    TomlValue::Array(vec![
                        TomlValue::Table(lane_universal.clone()),
                        TomlValue::Table(lane_alice.clone()),
                        TomlValue::Table(lane_bob.clone()),
                    ]),
                )
                .write(
                    ["nexus", "dataspace_catalog"],
                    TomlValue::Array(vec![
                        TomlValue::Table(ds_universal.clone()),
                        TomlValue::Table(ds1.clone()),
                        TomlValue::Table(ds2.clone()),
                    ]),
                )
                .write(
                    ["nexus", "routing_policy"],
                    TomlValue::Table(policy.clone()),
                )
                .write(
                    ["nexus", "fees", "fee_asset_id"],
                    fee_asset_id_literal.clone(),
                )
                .write(
                    ["nexus", "staking", "stake_asset_id"],
                    stake_asset_id_literal.clone(),
                )
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    gas_account_str.clone(),
                )
                .write(
                    ["nexus", "staking", "slash_sink_account_id"],
                    gas_account_str.clone(),
                )
                .write(
                    ["nexus", "staking", "restricted_validator_mode"],
                    "stake_elected",
                )
                .write(
                    ["nexus", "staking", "public_validator_mode"],
                    "stake_elected",
                )
                .write(["nexus", "staking", "max_validators"], 4_i64)
                .write(["sumeragi", "npos", "use_stake_snapshot_roster"], true)
                .write(["sumeragi", "npos", "election", "max_validators"], 4_i64)
                .write(["sumeragi", "npos", "epoch_length_blocks"], 3600_i64)
                .write(
                    ["sumeragi", "npos", "vrf", "commit_deadline_offset_blocks"],
                    100_i64,
                )
                .write(
                    ["sumeragi", "npos", "vrf", "reveal_deadline_offset_blocks"],
                    40_i64,
                );
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(sumeragi_status_json_endpoint_decodes_to_wire_end_to_end),
    )
    .await?
    else {
        ensure!(
            !fail_on_sandbox_skip(),
            "sandbox denied localnet startup and {} is enabled",
            FAIL_ON_SANDBOX_SKIP_ENV
        );
        return Ok(());
    };

    let result: Result<()> = async {
        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        network.client().submit::<InstructionBox>(
            Log::new(Level::INFO, "status endpoint bootstrap tick".to_owned()).into(),
        )?;
        wait_for_converged_height(&network, 2, Duration::from_secs(45)).await?;
        let peer = network
            .peers()
            .first()
            .cloned()
            .ok_or_else(|| eyre!("network started without peers"))?;

        let before_height = collect_statuses(&network, STATUS_POLL_TIMEOUT)
            .await?
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let alice_client = peer.client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
        let bob_client = peer.client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone());
        let alice_probe_submitted = submit_route_probe_with_retry(
            &alice_client,
            "cross-lane route probe alice",
            ROUTE_BINDING_TIMEOUT,
            "submit cross-lane route probe from alice",
        )
        .await?;
        let bob_probe_submitted = submit_route_probe_with_retry(
            &bob_client,
            "cross-lane route probe bob",
            ROUTE_BINDING_TIMEOUT,
            "submit cross-lane route probe from bob",
        )
        .await?;
        if alice_probe_submitted || bob_probe_submitted {
            wait_for_converged_height(
                &network,
                before_height.saturating_add(1),
                Duration::from_secs(45),
            )
            .await?;

            let routing_deadline = Instant::now() + Duration::from_secs(45);
            let mut observed_cross_lane_routing = false;
            while Instant::now() < routing_deadline {
                let statuses = collect_sumeragi_statuses(&network, STATUS_POLL_TIMEOUT).await?;
                observed_cross_lane_routing = statuses.iter().any(|status| {
                    status
                        .lane_commitments
                        .iter()
                        .any(|commitment| commitment.lane_id.as_u32() != 0)
                        || status
                            .dataspace_commitments
                            .iter()
                            .any(|commitment| commitment.dataspace_id.as_u64() != 0)
                        || status.lane_relay_envelopes.iter().any(|relay| {
                            relay.lane_id.as_u32() != 0 || relay.dataspace_id.as_u64() != 0
                        })
                });
                if observed_cross_lane_routing {
                    break;
                }
                sleep(Duration::from_millis(200)).await;
            }
            if !observed_cross_lane_routing {
                eprintln!(
                    "cross-lane probes were accepted but no lane commitments or relay envelopes appeared within {:?}; continuing with status-endpoint decode coverage only",
                    Duration::from_secs(45)
                );
            }
        } else {
            eprintln!(
                "cross-lane route bindings stayed unavailable within {:?}; continuing with status-endpoint decode coverage only",
                ROUTE_BINDING_TIMEOUT
            );
        }

        let url = format!(
            "{}/v1/sumeragi/status",
            peer.torii_url().trim_end_matches('/')
        );
        let response = HttpClient::new()
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
            .wrap_err("fetch sumeragi status endpoint as JSON")?;
        let status = response.status();
        ensure!(
            status.is_success(),
            "sumeragi status endpoint returned {status}"
        );

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        ensure!(
            content_type.starts_with("application/json"),
            "expected JSON status payload, got content-type={content_type}"
        );

        let body = response
            .bytes()
            .await
            .wrap_err("read sumeragi status JSON body")?;
        let payload: Value =
            norito::json::from_slice(&body).wrap_err("parse sumeragi status JSON payload")?;
        let mode_tag = payload
            .get("mode_tag")
            .and_then(Value::as_str)
            .unwrap_or_default();
        ensure!(
            !mode_tag.is_empty(),
            "decoded sumeragi status JSON payload has empty mode_tag"
        );
        network.shutdown().await;
        Ok(())
    }
    .await;

    if sandbox::handle_result(
        result,
        stringify!(sumeragi_status_json_endpoint_decodes_to_wire_end_to_end),
    )?
    .is_none()
    {
        ensure!(
            !fail_on_sandbox_skip(),
            "sandboxed skip surfaced in result handling and {} is enabled",
            FAIL_ON_SANDBOX_SKIP_ENV
        );
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn permissioned_localnet_reaches_100_blocks() -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;

    let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");
    set_env_var(
        "IROHA_TEST_CLIENT_TTL_MS",
        SOAK_CLIENT_TTL.as_millis().to_string(),
    );

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_pipeline_time(SMOKE_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(SMOKE_BLOCK_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(SMOKE_COMMIT_TIME_MS),
        )))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
                .write(["sumeragi", "collectors", "k"], 3_i64)
                .write(["sumeragi", "collectors", "redundant_send_r"], 2_i64)
                .write(["network", "transaction_gossip_period_ms"], 200_i64)
                .write(["network", "transaction_gossip_public_target_cap"], 3_i64)
                .write(
                    ["network", "transaction_gossip_restricted_target_cap"],
                    3_i64,
                )
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                .write(["network", "p2p_post_queue_cap"], 8192_i64)
                .write(["network", "p2p_queue_cap_high"], 16384_i64)
                .write(["network", "p2p_queue_cap_low"], 65536_i64)
                .write(["network", "disconnect_on_post_overflow"], false)
                // Tighten local timeouts to keep proposal/view-change cadence bounded.
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "propose_ms"],
                    200_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "prevote_ms"],
                    400_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "precommit_ms"],
                    600_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "commit_ms"],
                    800_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "da_ms"],
                    400_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    2_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    1_i64,
                );
        });

    let result: Result<()> = async {
        let Some(network) = sandbox::start_network_async_or_skip(
            builder,
            stringify!(permissioned_localnet_reaches_100_blocks),
        )
        .await?
        else {
            return Ok(());
        };

        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        let baseline_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let baseline_height = baseline_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let warmup_height = baseline_height.saturating_add(1);
        for peer in network.peers() {
            let message = format!("localnet warmup block {}", peer.mnemonic());
            peer.client()
                .submit::<InstructionBox>(Log::new(Level::INFO, message).into())
                .wrap_err_with(|| {
                    format!("failed to submit warmup log instruction to {}", peer.mnemonic())
                })?;
        }
        wait_for_converged_height(&network, warmup_height, Duration::from_secs(45)).await?;
        let warmup_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let baseline_height = warmup_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();

        let target_blocks = if cfg!(debug_assertions) { 30_u64 } else { 100_u64 };
        let target_height = baseline_height.saturating_add(target_blocks);
        let peers = network.peers();
        ensure!(!peers.is_empty(), "network must have at least one peer");
        let peer_count = peers.len();
        let sequence_key = Name::from_str("tx_sequence").expect("tx_sequence metadata key");
        let debug_multiplier = if cfg!(debug_assertions) { 3 } else { 1 };
        let timeout = scale_duration(
            scale_duration(network.pipeline_time(), target_blocks.saturating_mul(3))
                .saturating_add(Duration::from_secs(30)),
            debug_multiplier,
        );
        let per_block_timeout = scale_duration(
            scale_duration(network.pipeline_time(), 8).saturating_add(Duration::from_secs(4)),
            debug_multiplier,
        );
        let start = Instant::now();
        let mut next_height = baseline_height;
        for idx in 0..target_blocks {
            let peer = &peers[usize::try_from(idx).unwrap_or(0) % peer_count];
            let message = format!("localnet block {idx} via {}", peer.mnemonic());
            let mut metadata = Metadata::default();
            metadata.insert(sequence_key.clone(), idx.saturating_add(1));
            peer.client()
                .submit_with_metadata::<InstructionBox>(
                    Log::new(Level::INFO, message).into(),
                    metadata,
                )
                .wrap_err_with(|| {
                    format!(
                        "failed to submit log instruction {idx} to {}",
                        peer.mnemonic()
                    )
                })?;
            next_height = next_height.saturating_add(1);
            let remaining = timeout.saturating_sub(start.elapsed());
            let block_timeout = if remaining < per_block_timeout {
                remaining
            } else {
                per_block_timeout
            };
            let deadline = Instant::now() + block_timeout;
            let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();
            let mut last_log = Instant::now()
                .checked_sub(STATUS_LOG_INTERVAL)
                .unwrap_or_else(Instant::now);
            loop {
                match collect_statuses(&network, STATUS_POLL_TIMEOUT).await {
                    Ok(statuses) => {
                        let snapshot: Vec<StatusSnapshot> =
                            statuses.iter().map(StatusSnapshot::from_status).collect();
                        if snapshot != last_snapshot || last_log.elapsed() >= STATUS_LOG_INTERVAL {
                            eprintln!(
                                "localnet status snapshot (target_height={next_height}): {snapshot:?}"
                            );
                            last_log = Instant::now();
                        }
                        last_snapshot = snapshot;
                        let max_height = statuses
                            .iter()
                            .map(|status| status.blocks)
                            .max()
                            .unwrap_or_default();
                        if max_height >= next_height {
                            break;
                        }
                        if Instant::now() >= deadline {
                            return Err(eyre!(
                                "height failed to reach {next_height} within {:?}: last_snapshot={last_snapshot:?}",
                                block_timeout
                            ));
                        }
                    }
                    Err(err) => {
                        if Instant::now() >= deadline {
                            return Err(eyre!(
                                "height failed to reach {next_height} within {:?}: last_snapshot={last_snapshot:?}, last_error={err:?}",
                                block_timeout
                            ));
                        }
                    }
                }
                sleep(Duration::from_millis(200)).await;
            }
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        let catch_up_timeout = per_block_timeout.min(remaining);
        if catch_up_timeout > Duration::ZERO {
            let deadline = Instant::now() + catch_up_timeout;
            let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();
            let mut last_log = Instant::now()
                .checked_sub(STATUS_LOG_INTERVAL)
                .unwrap_or_else(Instant::now);
            loop {
                match collect_statuses(&network, STATUS_POLL_TIMEOUT).await {
                    Ok(statuses) => {
                        let snapshot: Vec<StatusSnapshot> =
                            statuses.iter().map(StatusSnapshot::from_status).collect();
                        if snapshot != last_snapshot || last_log.elapsed() >= STATUS_LOG_INTERVAL {
                            eprintln!(
                                "localnet catch-up snapshot (target_height={target_height}): {snapshot:?}"
                            );
                            last_log = Instant::now();
                        }
                        last_snapshot = snapshot;
                        if statuses
                            .iter()
                            .all(|status| status.blocks >= target_height)
                        {
                            break;
                        }
                        if Instant::now() >= deadline {
                            return Err(eyre!(
                                "peers failed to catch up to {target_height} within {:?}: last_snapshot={last_snapshot:?}",
                                catch_up_timeout
                            ));
                        }
                    }
                    Err(err) => {
                        if Instant::now() >= deadline {
                            return Err(eyre!(
                                "peers failed to catch up to {target_height} within {:?}: last_snapshot={last_snapshot:?}, last_error={err:?}",
                                catch_up_timeout
                            ));
                        }
                    }
                }
                sleep(Duration::from_millis(200)).await;
            }
        }

        let elapsed = start.elapsed();
        ensure!(
            elapsed <= timeout,
            "block production exceeded bound: elapsed={:?}",
            elapsed
        );

        let after_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        ensure!(
            after_statuses
                .iter()
                .all(|status| status.blocks >= target_height),
            "not all peers reached target height {target_height}: {after_statuses:?}"
        );

        network.shutdown().await;
        Ok(())
    }
    .await;

    if let Some(previous_ttl) = previous_ttl {
        set_env_var("IROHA_TEST_CLIENT_TTL_MS", previous_ttl);
    } else {
        remove_env_var("IROHA_TEST_CLIENT_TTL_MS");
    }

    if sandbox::handle_result(result, stringify!(permissioned_localnet_reaches_100_blocks))?
        .is_none()
    {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "long-running localnet soak (thousands of blocks/tx)"]
#[allow(clippy::too_many_lines)]
async fn permissioned_localnet_soak_thousands() -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;

    let soak_block_time_ms = env_or_default("IROHA_SOAK_BLOCK_TIME_MS", SOAK_BLOCK_TIME_MS);
    let soak_commit_time_ms =
        env_or_default("IROHA_SOAK_COMMIT_TIME_MS", SOAK_COMMIT_TIME_MS).max(soak_block_time_ms);
    let soak_max_secs_per_block = env_or_default_f64("IROHA_SOAK_MAX_SEC_PER_BLOCK", 1.0);

    let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");
    // Extend TTL so early transactions do not expire during the soak run.
    set_env_var(
        "IROHA_TEST_CLIENT_TTL_MS",
        SOAK_CLIENT_TTL.as_millis().to_string(),
    );

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_pipeline_time(SOAK_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(soak_block_time_ms),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(soak_commit_time_ms),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CollectorsK(1),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(1),
        )))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
                .write(["logger", "level"], "WARN")
                .write(["telemetry_profile"], "full")
                .write(["network", "transaction_gossip_period_ms"], 20_i64)
                .write(["network", "transaction_gossip_public_target_cap"], 8_i64)
                .write(
                    ["network", "transaction_gossip_restricted_target_cap"],
                    8_i64,
                )
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                .write(["network", "p2p_post_queue_cap"], 8192_i64)
                .write(["network", "p2p_queue_cap_high"], 16384_i64)
                .write(["network", "p2p_queue_cap_low"], 65536_i64)
                .write(["network", "disconnect_on_post_overflow"], false)
                // Match the sustained-load DA quorum profile used by the other soak cases.
                .write(
                    ["sumeragi", "advanced", "da", "quorum_timeout_multiplier"],
                    7_i64,
                )
                .write(
                    [
                        "sumeragi",
                        "advanced",
                        "da",
                        "availability_timeout_multiplier",
                    ],
                    3_i64,
                )
                .write(
                    [
                        "sumeragi",
                        "advanced",
                        "da",
                        "availability_timeout_floor_ms",
                    ],
                    100_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    1_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    1_i64,
                )
                // Keep soak timing deterministic for perf assertions by pinning pacing at 1.0x.
                .write(
                    ["sumeragi", "advanced", "pacing_governor", "min_factor_bps"],
                    10_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacing_governor", "max_factor_bps"],
                    10_000_i64,
                );
        });

    let result: Result<()> = async {
        let Some(network) = sandbox::start_network_async_or_skip(
            builder,
            stringify!(permissioned_localnet_soak_thousands),
        )
        .await?
        else {
            return Ok(());
        };

        eprintln!(
            "localnet soak timing profile: block_time_ms={soak_block_time_ms}, commit_time_ms={soak_commit_time_ms}"
        );

        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        let baseline_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let baseline_non_empty = baseline_statuses
            .iter()
            .map(|status| status.blocks_non_empty)
            .min()
            .unwrap_or_default();
        let soak_start = Instant::now();

        // Allow shorter local runs via IROHA_SOAK_TARGET_BLOCKS while keeping the default.
        let target_blocks = env_or_default("IROHA_SOAK_TARGET_BLOCKS", SOAK_TARGET_BLOCKS);
        let submit_batch = env_or_default("IROHA_SOAK_SUBMIT_BATCH", SOAK_SUBMIT_BATCH);
        let queue_soft_limit =
            env_or_default("IROHA_SOAK_QUEUE_SOFT_LIMIT", SOAK_QUEUE_SOFT_LIMIT);
        let target_height = baseline_non_empty.saturating_add(target_blocks);
        let submit_peer = network
            .peers()
            .first()
            .cloned()
            .ok_or_else(|| eyre!("network must have at least one peer"))?;
        let client = submit_peer.client();
        for idx in 0..target_blocks {
            client
                .submit::<InstructionBox>(
                    Log::new(Level::INFO, format!("localnet soak {idx}")).into(),
                )
                .wrap_err_with(|| format!("failed to submit log instruction {idx}"))?;
            if (idx + 1) % submit_batch == 0 {
                wait_for_queue_depth(&network, queue_soft_limit, SOAK_STATUS_POLL_TIMEOUT).await?;
            }
        }

        let mut last_progress = Instant::now();
        let mut last_min_non_empty = baseline_non_empty;
        let mut last_log = Instant::now()
            .checked_sub(SOAK_PROGRESS_LOG_INTERVAL)
            .unwrap_or_else(Instant::now);
        let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();
        let mut last_phase_snapshot: Option<SoakPhaseSnapshot> = None;
        let mut phase_poll_enabled = true;

        loop {
            if let Ok(statuses) = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await {
                let min_non_empty = statuses
                    .iter()
                    .map(|status| status.blocks_non_empty)
                    .min()
                    .unwrap_or_default();
                let max_non_empty = statuses
                    .iter()
                    .map(|status| status.blocks_non_empty)
                    .max()
                    .unwrap_or_default();
                if min_non_empty > last_min_non_empty {
                    last_min_non_empty = min_non_empty;
                    last_progress = Instant::now();
                }
                last_snapshot = statuses
                    .iter()
                    .map(StatusSnapshot::from_status)
                    .collect();
                if last_log.elapsed() >= SOAK_PROGRESS_LOG_INTERVAL {
                    if phase_poll_enabled {
                        match collect_sumeragi_phase_snapshot(&network, SOAK_STATUS_POLL_TIMEOUT)
                            .await
                        {
                            Ok(phase_snapshot) => {
                                last_phase_snapshot = Some(phase_snapshot);
                            }
                            Err(err) => {
                                phase_poll_enabled = false;
                                eprintln!(
                                    "localnet soak phase snapshot unavailable; disabling phase polling: {err:?}"
                                );
                            }
                        }
                    }
                    eprintln!(
                        "localnet soak progress (target_non_empty={target_height}, min_non_empty={min_non_empty}, max_non_empty={max_non_empty}): {last_snapshot:?}"
                    );
                    if let Some(phases) = last_phase_snapshot {
                        eprintln!(
                            "localnet soak phases avg_ms: propose={}, availability={}, prevote={}, precommit={}, commit={}, total={}, ema_total={}",
                            phases.propose_ms,
                            phases.availability_ms,
                            phases.prevote_ms,
                            phases.precommit_ms,
                            phases.commit_ms,
                            phases.pipeline_total_ms,
                            phases.pipeline_total_ema_ms,
                        );
                    }
                    last_log = Instant::now();
                }
                if statuses
                    .iter()
                    .all(|status| status.blocks_non_empty >= target_height)
                {
                    break;
                }
            }

            if last_progress.elapsed() >= SOAK_STALL_THRESHOLD {
                return Err(eyre!(
                    "localnet soak stalled for {:?} (min_non_empty={last_min_non_empty}, target_non_empty={target_height}): last_snapshot={last_snapshot:?}, last_phases={last_phase_snapshot:?}",
                    SOAK_STALL_THRESHOLD,
                ));
            }

            sleep(SOAK_STATUS_POLL_INTERVAL).await;
        }

        let after_statuses = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        ensure!(
            after_statuses
                .iter()
                .all(|status| status.blocks_non_empty >= target_height),
            "not all peers reached target non-empty height {target_height}: {after_statuses:?}"
        );
        let phase_summary = if phase_poll_enabled {
            collect_sumeragi_phase_snapshot(&network, SOAK_STATUS_POLL_TIMEOUT)
                .await
                .ok()
        } else {
            None
        };
        let elapsed = soak_start.elapsed();
        let secs_per_block = if target_blocks == 0 {
            0.0
        } else {
            elapsed.as_secs_f64() / target_blocks as f64
        };
        eprintln!(
            "localnet soak summary: target_blocks={}, elapsed={:?}, secs_per_block={:.3}",
            target_blocks, elapsed, secs_per_block
        );
        if let Some(phases) = phase_summary {
            eprintln!(
                "localnet soak phase summary avg_ms: propose={}, availability={}, prevote={}, precommit={}, commit={}, total={}, ema_total={}",
                phases.propose_ms,
                phases.availability_ms,
                phases.prevote_ms,
                phases.precommit_ms,
                phases.commit_ms,
                phases.pipeline_total_ms,
                phases.pipeline_total_ema_ms,
            );
        }
        ensure!(
            secs_per_block <= soak_max_secs_per_block,
            "localnet soak too slow: secs_per_block={secs_per_block:.3}, target<={soak_max_secs_per_block:.3}"
        );

        network.shutdown().await;
        Ok(())
    }
    .await;

    if let Some(previous_ttl) = previous_ttl {
        set_env_var("IROHA_TEST_CLIENT_TTL_MS", previous_ttl);
    } else {
        remove_env_var("IROHA_TEST_CLIENT_TTL_MS");
    }

    if sandbox::handle_result(result, stringify!(permissioned_localnet_soak_thousands))?.is_none() {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "long-running 7-peer localnet throughput regression (10k tps target)"]
#[allow(clippy::too_many_lines)]
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
async fn permissioned_localnet_throughput_10k_tps() -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;

    let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");
    set_env_var(
        "IROHA_TEST_CLIENT_TTL_MS",
        THROUGHPUT_CLIENT_TTL.as_millis().to_string(),
    );
    let throughput_rbc_encoding =
        std::env::var("IROHA_THROUGHPUT_RBC_ENCODING").unwrap_or_else(|_| "plain".to_owned());
    let throughput_rbc_data_shards = env_or_default("IROHA_THROUGHPUT_RBC_DATA_SHARDS", 4);
    let throughput_rbc_parity_shards = env_or_default("IROHA_THROUGHPUT_RBC_PARITY_SHARDS", 2);
    let throughput_rbc_encoding_for_config = throughput_rbc_encoding.clone();

    let builder = NetworkBuilder::new()
        .with_peers(7)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_pipeline_time(THROUGHPUT_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(THROUGHPUT_BLOCK_MAX_TXS)),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(THROUGHPUT_BLOCK_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(THROUGHPUT_COMMIT_TIME_MS),
        )))
        .with_config_layer(move |layer| {
            let layer = layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
                .write(
                    ["sumeragi", "advanced", "rbc", "encoding"],
                    throughput_rbc_encoding_for_config.as_str(),
                )
                .write(["sumeragi", "collectors", "k"], 3_i64)
                .write(["sumeragi", "collectors", "redundant_send_r"], 2_i64)
                .write(["network", "transaction_gossip_period_ms"], 200_i64)
                .write(["network", "transaction_gossip_public_target_cap"], 3_i64)
                .write(
                    ["network", "transaction_gossip_restricted_target_cap"],
                    3_i64,
                )
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                .write(["network", "p2p_post_queue_cap"], 8192_i64)
                .write(["network", "p2p_queue_cap_high"], 16384_i64)
                .write(["network", "p2p_queue_cap_low"], 65536_i64)
                .write(["network", "disconnect_on_post_overflow"], false)
                // Keep the generated-load run focused on consensus/network stability. The
                // default Nexus lane TEU cap is sized for live economic scheduling, not
                // synthetic 10k-log blocks.
                .write(["nexus", "fusion", "floor_teu"], THROUGHPUT_LANE_TEU_FLOOR)
                .write(
                    ["nexus", "fusion", "exit_teu"],
                    THROUGHPUT_LANE_TEU_CAPACITY,
                )
                // Tighten local timeouts to keep proposal/view-change cadence bounded.
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "propose_ms"],
                    200_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "prevote_ms"],
                    400_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "precommit_ms"],
                    600_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "commit_ms"],
                    800_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "da_ms"],
                    400_i64,
                )
                // Give DA quorum extra breathing room under sustained load.
                .write(
                    ["sumeragi", "advanced", "da", "quorum_timeout_multiplier"],
                    7_i64,
                )
                .write(
                    [
                        "sumeragi",
                        "advanced",
                        "da",
                        "availability_timeout_multiplier",
                    ],
                    3_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    5_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    1_i64,
                )
                // Lift Torii limits for sustained local throughput runs.
                .write(
                    ["torii", "preauth_allow_cidrs"],
                    TomlValue::Array(vec![
                        TomlValue::String("127.0.0.0/8".into()),
                        TomlValue::String("::1/128".into()),
                    ]),
                )
                .write(
                    ["torii", "api_allow_cidrs"],
                    TomlValue::Array(vec![
                        TomlValue::String("127.0.0.0/8".into()),
                        TomlValue::String("::1/128".into()),
                    ]),
                )
                .write(["torii", "preauth_rate_per_ip_per_sec"], 1_000_000_i64)
                .write(["torii", "preauth_burst_per_ip"], 2_000_000_i64)
                .write(["torii", "query_rate_per_authority_per_sec"], 0_i64)
                .write(["torii", "query_burst_per_authority"], 0_i64)
                .write(["torii", "tx_rate_per_authority_per_sec"], 0_i64)
                .write(["torii", "tx_burst_per_authority"], 0_i64)
                .write(["torii", "api_high_load_tx_threshold"], 262_144_i64);
            if throughput_rbc_encoding_for_config == "rs16" {
                let _ = layer
                    .write(
                        ["sumeragi", "advanced", "rbc", "data_shards"],
                        i64::try_from(throughput_rbc_data_shards).unwrap_or(i64::MAX),
                    )
                    .write(
                        ["sumeragi", "advanced", "rbc", "parity_shards"],
                        i64::try_from(throughput_rbc_parity_shards).unwrap_or(i64::MAX),
                    );
            }
        });

    let result: Result<()> = async {
        let Some(network) = sandbox::start_network_async_or_skip(
            builder,
            stringify!(permissioned_localnet_throughput_10k_tps),
        )
        .await?
        else {
            return Ok(());
        };

        let network_dir = network.env_dir().to_path_buf();
        let http = HttpClient::new();
        let mut artifacts = ThroughputArtifacts::default();

        let run_result: Result<()> = async {
        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        let baseline_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let baseline_non_empty = baseline_statuses
            .iter()
            .map(|status| status.blocks_non_empty)
            .min()
            .unwrap_or_default();
        let baseline_approved = baseline_statuses
            .iter()
            .map(|status| status.txs_approved)
            .min()
            .unwrap_or_default();

        let total_blocks_default = THROUGHPUT_WARMUP_BLOCKS.saturating_add(THROUGHPUT_STEADY_BLOCKS);
        let total_blocks =
            env_or_default("IROHA_THROUGHPUT_TARGET_BLOCKS", total_blocks_default).max(1);
        let warmup_blocks = env_or_default(
            "IROHA_THROUGHPUT_WARMUP_BLOCKS",
            THROUGHPUT_WARMUP_BLOCKS,
        )
        .min(total_blocks.saturating_sub(1).max(1));
        let steady_blocks_default = total_blocks.saturating_sub(warmup_blocks).max(1);
        let steady_blocks = env_or_default(
            "IROHA_THROUGHPUT_STEADY_BLOCKS",
            steady_blocks_default,
        )
        .max(1);
        let total_blocks = warmup_blocks.saturating_add(steady_blocks);
        let submit_batch =
            env_or_default("IROHA_THROUGHPUT_SUBMIT_BATCH", THROUGHPUT_SUBMIT_BATCH).max(1);
        let submit_parallelism =
            env_or_default("IROHA_THROUGHPUT_PARALLELISM", THROUGHPUT_SUBMIT_PARALLELISM)
                .max(1)
                .min(submit_batch);
        let submit_parallelism = usize::try_from(submit_parallelism)
            .wrap_err("submit parallelism exceeds host limits")?;
        let queue_soft_limit = env_or_default(
            "IROHA_THROUGHPUT_QUEUE_SOFT_LIMIT",
            THROUGHPUT_QUEUE_SOFT_LIMIT,
        );
        let payload_bytes = env_or_default_usize(
            "IROHA_THROUGHPUT_PAYLOAD_BYTES",
            THROUGHPUT_PAYLOAD_BYTES,
        )
        .max(32);
        let rng_seed = env_or_default("IROHA_THROUGHPUT_RNG_SEED", THROUGHPUT_RNG_SEED);
        let warmup_target_height = baseline_non_empty.saturating_add(warmup_blocks);
        let steady_target_height = warmup_target_height.saturating_add(steady_blocks);
        let warmup_txs = warmup_blocks.saturating_mul(THROUGHPUT_BLOCK_MAX_TXS);
        let steady_txs = steady_blocks.saturating_mul(THROUGHPUT_BLOCK_MAX_TXS);
        let total_txs = warmup_txs.saturating_add(steady_txs);

        artifacts.recipe = Some(ThroughputArtifactRecipe {
            peers: network.peers().len() as u64,
            block_time_ms: THROUGHPUT_BLOCK_TIME_MS,
            commit_time_ms: THROUGHPUT_COMMIT_TIME_MS,
            block_max_txs: THROUGHPUT_BLOCK_MAX_TXS,
            warmup_blocks,
            steady_blocks,
            total_blocks,
            warmup_txs,
            steady_txs,
            total_txs,
            submit_batch,
            submit_parallelism: submit_parallelism as u64,
            queue_soft_limit,
            payload_bytes: payload_bytes as u64,
            load_kind: "log".to_owned(),
            transfer_accounts: 0,
            transfer_initial_balance: 0,
            transfer_max_amount: 0,
            ram_lfe_email_accounts: 0,
            ram_lfe_email_policy: String::new(),
            ram_lfe_program: String::new(),
            rng_seed,
            rbc_encoding: throughput_rbc_encoding.clone(),
            rbc_data_shards: throughput_rbc_data_shards,
            rbc_parity_shards: throughput_rbc_parity_shards,
        });

        let slo_p95_ms = env_or_default("IROHA_THROUGHPUT_SLO_P95_MS", THROUGHPUT_SLO_P95_MS);
        let slo_p99_ms = env_or_default("IROHA_THROUGHPUT_SLO_P99_MS", THROUGHPUT_SLO_P99_MS);
        let slo_view_change_rate = env_or_default_f64(
            "IROHA_THROUGHPUT_SLO_VIEW_CHANGE_RATE",
            THROUGHPUT_SLO_VIEW_CHANGE_RATE_MAX,
        );
        let slo_backpressure_rate = env_or_default_f64(
            "IROHA_THROUGHPUT_SLO_BACKPRESSURE_RATE",
            THROUGHPUT_SLO_BACKPRESSURE_RATE_MAX,
        );
        let slo_queue_saturation = env_or_default_f64(
            "IROHA_THROUGHPUT_SLO_QUEUE_SAT_FRAC",
            THROUGHPUT_SLO_QUEUE_SAT_FRAC_MAX,
        );
        artifacts.slo = Some(ThroughputArtifactSlo {
            commit_p95_ms: slo_p95_ms,
            commit_p99_ms: slo_p99_ms,
            view_change_rate_max: slo_view_change_rate,
            backpressure_rate_max: slo_backpressure_rate,
            queue_saturation_max: slo_queue_saturation,
        });

        let submit_clients: Vec<_> =
            network.peers().iter().map(|peer| peer.client()).collect();
        ensure!(
            !submit_clients.is_empty(),
            "network must have at least one peer"
        );
        eprintln!(
            "localnet throughput recipe: peers={}, block_time_ms={}, commit_time_ms={}, block_max_txs={}, warmup_blocks={}, steady_blocks={}, total_blocks={}, payload_bytes={}, submit_batch={}, submit_parallelism={}, queue_soft_limit={}, rng_seed={}, rbc_encoding={}, rbc_data_shards={}, rbc_parity_shards={}, baseline_non_empty={}, baseline_approved={}",
            network.peers().len(),
            THROUGHPUT_BLOCK_TIME_MS,
            THROUGHPUT_COMMIT_TIME_MS,
            THROUGHPUT_BLOCK_MAX_TXS,
            warmup_blocks,
            steady_blocks,
            total_blocks,
            payload_bytes,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            rng_seed,
            throughput_rbc_encoding,
            throughput_rbc_data_shards,
            throughput_rbc_parity_shards,
            baseline_non_empty,
            baseline_approved,
        );

        let warmup_submit_elapsed = submit_logs(
            0,
            warmup_txs,
            &network,
            &submit_clients,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            payload_bytes,
            rng_seed,
        )
        .await?;

        let mut last_progress = Instant::now();
        let mut last_min_non_empty = baseline_non_empty;
        let mut last_log = Instant::now()
            .checked_sub(THROUGHPUT_PROGRESS_LOG_INTERVAL)
            .unwrap_or_else(Instant::now);
        let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();
        loop {
            if let Ok(statuses) = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await {
                let min_non_empty = statuses
                    .iter()
                    .map(|status| status.blocks_non_empty)
                    .min()
                    .unwrap_or_default();
                let max_non_empty = statuses
                    .iter()
                    .map(|status| status.blocks_non_empty)
                    .max()
                    .unwrap_or_default();
                if min_non_empty > last_min_non_empty {
                    last_min_non_empty = min_non_empty;
                    last_progress = Instant::now();
                }
                last_snapshot = statuses
                    .iter()
                    .map(StatusSnapshot::from_status)
                    .collect();
                if last_log.elapsed() >= THROUGHPUT_PROGRESS_LOG_INTERVAL {
                    eprintln!(
                        "localnet throughput warmup progress (target_non_empty={warmup_target_height}, min_non_empty={min_non_empty}, max_non_empty={max_non_empty}): {last_snapshot:?}"
                    );
                    last_log = Instant::now();
                }
                if statuses
                    .iter()
                    .all(|status| status.blocks_non_empty >= warmup_target_height)
                {
                    break;
                }
            }

            if last_progress.elapsed() >= THROUGHPUT_STALL_THRESHOLD {
                return Err(eyre!(
                    "localnet throughput warmup stalled for {:?} (min_non_empty={last_min_non_empty}, target_non_empty={warmup_target_height}): last_snapshot={last_snapshot:?}",
                    THROUGHPUT_STALL_THRESHOLD
                ));
            }

            sleep(SOAK_STATUS_POLL_INTERVAL).await;
        }

        let warmup_metrics =
            collect_metrics_snapshots(&network, &http, THROUGHPUT_METRICS_TIMEOUT).await?;

        let steady_start_statuses = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        let steady_start_sumeragi = collect_sumeragi_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        let steady_start_approved = steady_start_statuses
            .iter()
            .map(|status| status.txs_approved)
            .min()
            .unwrap_or(baseline_approved);
        let steady_start = Instant::now();

        let steady_submit_elapsed = submit_logs(
            warmup_txs,
            steady_txs,
            &network,
            &submit_clients,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            payload_bytes,
            rng_seed,
        )
        .await?;

        let mut samples: Vec<ThroughputSample> = Vec::new();
        let mut last_progress = Instant::now();
        let mut last_min_non_empty = warmup_target_height;
        let mut last_log = Instant::now()
            .checked_sub(THROUGHPUT_PROGRESS_LOG_INTERVAL)
            .unwrap_or_else(Instant::now);
        let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();

        loop {
            if let Ok(statuses) = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await {
                let sumeragi_statuses =
                    collect_sumeragi_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
                let min_non_empty = statuses
                    .iter()
                    .map(|status| status.blocks_non_empty)
                    .min()
                    .unwrap_or_default();
                let max_non_empty = statuses
                    .iter()
                    .map(|status| status.blocks_non_empty)
                    .max()
                    .unwrap_or_default();
                if min_non_empty > last_min_non_empty {
                    last_min_non_empty = min_non_empty;
                    last_progress = Instant::now();
                }
                let status_snapshots: Vec<StatusSnapshot> = statuses
                    .iter()
                    .map(StatusSnapshot::from_status)
                    .collect();
                let sumeragi_snapshots: Vec<SumeragiStatusSnapshot> = sumeragi_statuses
                    .iter()
                    .map(SumeragiStatusSnapshot::from_status)
                    .collect();
                let timestamp_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                samples.push(ThroughputSample {
                    phase: None,
                    timestamp_ms: u64::try_from(timestamp_ms).unwrap_or(u64::MAX),
                    statuses: status_snapshots.clone(),
                    sumeragi: sumeragi_snapshots,
                });
                last_snapshot = status_snapshots;
                if last_log.elapsed() >= THROUGHPUT_PROGRESS_LOG_INTERVAL {
                    eprintln!(
                        "localnet throughput steady progress (target_non_empty={steady_target_height}, min_non_empty={min_non_empty}, max_non_empty={max_non_empty}): {last_snapshot:?}"
                    );
                    last_log = Instant::now();
                }
                if statuses
                    .iter()
                    .all(|status| status.blocks_non_empty >= steady_target_height)
                {
                    break;
                }
            }

            if last_progress.elapsed() >= THROUGHPUT_STALL_THRESHOLD {
                return Err(eyre!(
                    "localnet throughput stalled for {:?} (min_non_empty={last_min_non_empty}, target_non_empty={steady_target_height}): last_snapshot={last_snapshot:?}",
                    THROUGHPUT_STALL_THRESHOLD
                ));
            }

            sleep(THROUGHPUT_SAMPLE_INTERVAL).await;
        }

        let steady_elapsed = steady_start.elapsed();

        let after_statuses = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        let after_sumeragi = collect_sumeragi_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        let after_metrics =
            collect_metrics_snapshots(&network, &http, THROUGHPUT_METRICS_TIMEOUT).await?;
        ensure!(
            after_statuses
                .iter()
                .all(|status| status.blocks_non_empty >= steady_target_height),
            "not all peers reached target non-empty height {steady_target_height}: {after_statuses:?}"
        );
        let max_commit_time_ms = after_statuses
            .iter()
            .map(|status| status.commit_time_ms)
            .max()
            .unwrap_or_default();
        let max_commit_time_allowed = THROUGHPUT_COMMIT_TIME_MS
            .saturating_mul(THROUGHPUT_COMMIT_TIME_MAX_MULTIPLIER);
        let mut min_commit_time_ms = u64::MAX;
        let mut sum_commit_time_ms = 0_u128;
        for status in &after_statuses {
            let value = status.commit_time_ms;
            min_commit_time_ms = min_commit_time_ms.min(value);
            sum_commit_time_ms = sum_commit_time_ms.saturating_add(u128::from(value));
        }
        let avg_commit_time_ms = if after_statuses.is_empty() {
            0_u64
        } else {
            (sum_commit_time_ms / u128::from(after_statuses.len() as u64)) as u64
        };
        let min_commit_time_ms = if min_commit_time_ms == u64::MAX {
            0_u64
        } else {
            min_commit_time_ms
        };

        let (commit_p95_ms, commit_p99_ms, commit_hist_count) =
            commit_time_quantiles(&warmup_metrics, &after_metrics);
        let commit_p95_ms = commit_p95_ms.unwrap_or_default();
        let commit_p99_ms = commit_p99_ms.unwrap_or_default();

        let committed_approved = after_statuses
            .iter()
            .map(|status| status.txs_approved)
            .min()
            .unwrap_or(steady_start_approved)
            .saturating_sub(steady_start_approved);
        let committed_tps = if steady_elapsed.as_secs_f64() > 0.0 {
            committed_approved as f64 / steady_elapsed.as_secs_f64()
        } else {
            0.0
        };
        let submitted_tps = if steady_submit_elapsed.as_secs_f64() > 0.0 {
            steady_txs as f64 / steady_submit_elapsed.as_secs_f64()
        } else {
            0.0
        };

        let (view_change_avg, view_change_max) = rate_summary(
            steady_start_sumeragi
                .iter()
                .map(|status| status.view_change_install_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            after_sumeragi
                .iter()
                .map(|status| status.view_change_install_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            steady_elapsed,
        );
        let (backpressure_avg, backpressure_max) = rate_summary(
            steady_start_sumeragi
                .iter()
                .map(|status| status.pacemaker_backpressure_deferrals_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            after_sumeragi
                .iter()
                .map(|status| status.pacemaker_backpressure_deferrals_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            steady_elapsed,
        );

        let mut saturated_samples = 0_u64;
        let mut total_samples = 0_u64;
        let mut max_queue_depth = 0_u64;
        for sample in &samples {
            for status in &sample.sumeragi {
                total_samples = total_samples.saturating_add(1);
                if status.tx_queue_saturated {
                    saturated_samples = saturated_samples.saturating_add(1);
                }
                max_queue_depth = max_queue_depth.max(status.tx_queue_depth);
            }
        }
        let queue_saturated_frac = if total_samples > 0 {
            saturated_samples as f64 / total_samples as f64
        } else {
            0.0
        };

        let metrics = ThroughputArtifactMetrics {
            submitted_tps,
            committed_tps,
            commit_p95_ms,
            commit_p99_ms,
            commit_hist_count,
            commit_time_ms_min: min_commit_time_ms,
            commit_time_ms_avg: avg_commit_time_ms,
            commit_time_ms_max: max_commit_time_ms,
            view_change_rate_avg: view_change_avg,
            view_change_rate_max: view_change_max,
            backpressure_rate_avg: backpressure_avg,
            backpressure_rate_max: backpressure_max,
            queue_saturated_frac,
            max_queue_depth,
            steady_elapsed_ms: steady_elapsed.as_millis() as u64,
            warmup_submit_elapsed_ms: warmup_submit_elapsed.as_millis() as u64,
            steady_submit_elapsed_ms: steady_submit_elapsed.as_millis() as u64,
        };
        artifacts.metrics = Some(metrics);
        artifacts.samples = samples;
        artifacts.warmup_metrics = warmup_metrics;
        artifacts.after_metrics = after_metrics;

        eprintln!(
            "localnet throughput metrics: peers={}, warmup_blocks={}, steady_blocks={}, warmup_txs={}, steady_txs={}, submit_batch={}, submit_parallelism={}, queue_soft_limit={}, payload_bytes={}, warmup_submit_elapsed={:?}, steady_submit_elapsed={:?}, steady_elapsed={:?}, submitted_tps={:.2}, committed_tps={:.2}, commit_hist_count={}, commit_time_ms(min/avg/max/p95/p99)={}/{}/{}/{}/{}, view_change_rate(avg/max)={:.4}/{:.4}, backpressure_rate(avg/max)={:.4}/{:.4}, queue_saturated_frac={:.2}, max_queue_depth={}",
            network.peers().len(),
            warmup_blocks,
            steady_blocks,
            warmup_txs,
            steady_txs,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            payload_bytes,
            warmup_submit_elapsed,
            steady_submit_elapsed,
            steady_elapsed,
            submitted_tps,
            committed_tps,
            commit_hist_count,
            min_commit_time_ms,
            avg_commit_time_ms,
            max_commit_time_ms,
            commit_p95_ms,
            commit_p99_ms,
            view_change_avg,
            view_change_max,
            backpressure_avg,
            backpressure_max,
            queue_saturated_frac,
            max_queue_depth,
        );
        ensure!(
            max_commit_time_ms <= max_commit_time_allowed,
            "commit time exceeded target: max_commit_time_ms={max_commit_time_ms}, allowed={max_commit_time_allowed}",
        );

        if commit_hist_count > 0 {
            ensure!(
                commit_p95_ms <= slo_p95_ms,
                "p95 commit time exceeded SLO: p95_ms={commit_p95_ms}, slo_p95_ms={slo_p95_ms}",
            );
            ensure!(
                commit_p99_ms <= slo_p99_ms,
                "p99 commit time exceeded SLO: p99_ms={commit_p99_ms}, slo_p99_ms={slo_p99_ms}",
            );
        }
        if slo_view_change_rate > 0.0 {
            ensure!(
                view_change_max <= slo_view_change_rate,
                "view change rate exceeded SLO: max_rate={view_change_max:.4}, slo_rate={slo_view_change_rate:.4}",
            );
        }
        if slo_backpressure_rate > 0.0 {
            ensure!(
                backpressure_max <= slo_backpressure_rate,
                "backpressure deferral rate exceeded SLO: max_rate={backpressure_max:.4}, slo_rate={slo_backpressure_rate:.4}",
            );
        }
        if slo_queue_saturation > 0.0 {
            ensure!(
                queue_saturated_frac <= slo_queue_saturation,
                "queue saturation exceeded SLO: fraction={queue_saturated_frac:.2}, slo={slo_queue_saturation:.2}",
            );
        }

        Ok(())
    }
    .await;

        if let Err(err) = &run_result {
            artifacts.error = Some(err.to_string());
        }
        if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {
            let root = PathBuf::from(artifact_root);
            let peer_logs: Vec<PeerLogInfo> = network
                .peers()
                .iter()
                .enumerate()
                .map(|(index, peer)| PeerLogInfo {
                    index: index as u64,
                    mnemonic: peer.mnemonic().to_string(),
                    stdout_log: peer
                        .latest_stdout_log_path()
                        .map(|path| path.to_string_lossy().to_string()),
                    stderr_log: peer
                        .latest_stderr_log_path()
                        .map(|path| path.to_string_lossy().to_string()),
                })
                .collect();
            if let Err(err) =
                write_throughput_artifacts(&root, &network_dir, &peer_logs, &artifacts)
            {
                eprintln!("throughput artifact write failed: {err:?}");
            }
        }

        network.shutdown().await;
        run_result
    }
    .await;

    if let Some(previous_ttl) = previous_ttl {
        set_env_var("IROHA_TEST_CLIENT_TTL_MS", previous_ttl);
    } else {
        remove_env_var("IROHA_TEST_CLIENT_TTL_MS");
    }

    if sandbox::handle_result(result, stringify!(permissioned_localnet_throughput_10k_tps))?
        .is_none()
    {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "long-running 7-peer localnet throughput regression (10k tps target, NPoS)"]
#[allow(clippy::too_many_lines)]
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
async fn npos_localnet_throughput_10k_tps() -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;

    let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");
    set_env_var(
        "IROHA_TEST_CLIENT_TTL_MS",
        THROUGHPUT_CLIENT_TTL.as_millis().to_string(),
    );

    let npos_params = SumeragiNposParameters {
        k_aggregators: 3,
        redundant_send_r: 2,
        ..SumeragiNposParameters::default()
    };

    let builder = NetworkBuilder::new()
        .with_peers(7)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_pipeline_time(THROUGHPUT_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(THROUGHPUT_BLOCK_MAX_TXS)),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(THROUGHPUT_BLOCK_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(THROUGHPUT_COMMIT_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos_params.into_custom_parameter(),
        )))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "collectors", "k"], 3_i64)
                .write(["sumeragi", "collectors", "redundant_send_r"], 2_i64)
                .write(["network", "transaction_gossip_period_ms"], 200_i64)
                .write(["network", "transaction_gossip_public_target_cap"], 3_i64)
                .write(
                    ["network", "transaction_gossip_restricted_target_cap"],
                    3_i64,
                )
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                .write(["network", "p2p_post_queue_cap"], 8192_i64)
                .write(["network", "p2p_queue_cap_high"], 16384_i64)
                .write(["network", "p2p_queue_cap_low"], 65536_i64)
                .write(["network", "disconnect_on_post_overflow"], false)
                // Keep the generated-load run focused on consensus/network stability. The
                // default Nexus lane TEU cap is sized for live economic scheduling, not
                // synthetic 10k-log blocks.
                .write(["nexus", "fusion", "floor_teu"], THROUGHPUT_LANE_TEU_FLOOR)
                .write(
                    ["nexus", "fusion", "exit_teu"],
                    THROUGHPUT_LANE_TEU_CAPACITY,
                )
                // Give DA quorum extra breathing room under sustained load.
                .write(
                    ["sumeragi", "advanced", "da", "quorum_timeout_multiplier"],
                    7_i64,
                )
                .write(
                    [
                        "sumeragi",
                        "advanced",
                        "da",
                        "availability_timeout_multiplier",
                    ],
                    3_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    5_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    1_i64,
                )
                // Lift Torii limits for sustained local throughput runs.
                .write(
                    ["torii", "preauth_allow_cidrs"],
                    TomlValue::Array(vec![
                        TomlValue::String("127.0.0.0/8".into()),
                        TomlValue::String("::1/128".into()),
                    ]),
                )
                .write(
                    ["torii", "api_allow_cidrs"],
                    TomlValue::Array(vec![
                        TomlValue::String("127.0.0.0/8".into()),
                        TomlValue::String("::1/128".into()),
                    ]),
                )
                .write(["torii", "preauth_rate_per_ip_per_sec"], 1_000_000_i64)
                .write(["torii", "preauth_burst_per_ip"], 2_000_000_i64)
                .write(["torii", "query_rate_per_authority_per_sec"], 0_i64)
                .write(["torii", "query_burst_per_authority"], 0_i64)
                .write(["torii", "tx_rate_per_authority_per_sec"], 0_i64)
                .write(["torii", "tx_burst_per_authority"], 0_i64)
                .write(["torii", "api_high_load_tx_threshold"], 262_144_i64);
        });

    let result: Result<()> = async {
        let Some(network) = sandbox::start_network_async_or_skip(
            builder,
            stringify!(npos_localnet_throughput_10k_tps),
        )
        .await?
        else {
            return Ok(());
        };

        let network_dir = network.env_dir().to_path_buf();
        let http = HttpClient::new();
        let mut artifacts = ThroughputArtifacts::default();

        let run_result: Result<()> = async {
        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        let baseline_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let baseline_non_empty = baseline_statuses
            .iter()
            .map(|status| status.blocks_non_empty)
            .min()
            .unwrap_or_default();
        let baseline_approved = baseline_statuses
            .iter()
            .map(|status| status.txs_approved)
            .min()
            .unwrap_or_default();

        let total_blocks_default =
            THROUGHPUT_WARMUP_BLOCKS.saturating_add(THROUGHPUT_STEADY_BLOCKS);
        let total_blocks =
            env_or_default("IROHA_THROUGHPUT_TARGET_BLOCKS", total_blocks_default).max(1);
        let warmup_blocks = env_or_default(
            "IROHA_THROUGHPUT_WARMUP_BLOCKS",
            THROUGHPUT_WARMUP_BLOCKS,
        )
        .min(total_blocks.saturating_sub(1).max(1));
        let steady_blocks_default = total_blocks.saturating_sub(warmup_blocks).max(1);
        let steady_blocks = env_or_default("IROHA_THROUGHPUT_STEADY_BLOCKS", steady_blocks_default)
            .max(1);
        let total_blocks = warmup_blocks.saturating_add(steady_blocks);
        let submit_batch =
            env_or_default("IROHA_THROUGHPUT_SUBMIT_BATCH", THROUGHPUT_SUBMIT_BATCH).max(1);
        let submit_parallelism =
            env_or_default("IROHA_THROUGHPUT_PARALLELISM", THROUGHPUT_SUBMIT_PARALLELISM)
                .max(1)
                .min(submit_batch);
        let submit_parallelism = usize::try_from(submit_parallelism)
            .wrap_err("submit parallelism exceeds host limits")?;
        let queue_soft_limit = env_or_default(
            "IROHA_THROUGHPUT_QUEUE_SOFT_LIMIT",
            THROUGHPUT_QUEUE_SOFT_LIMIT,
        );
        let payload_bytes = env_or_default_usize(
            "IROHA_THROUGHPUT_PAYLOAD_BYTES",
            THROUGHPUT_PAYLOAD_BYTES,
        )
        .max(32);
        let rng_seed = env_or_default("IROHA_THROUGHPUT_RNG_SEED", THROUGHPUT_RNG_SEED);
        let warmup_target_height = baseline_non_empty.saturating_add(warmup_blocks);
        let steady_target_height = warmup_target_height.saturating_add(steady_blocks);
        let warmup_txs = warmup_blocks.saturating_mul(THROUGHPUT_BLOCK_MAX_TXS);
        let steady_txs = steady_blocks.saturating_mul(THROUGHPUT_BLOCK_MAX_TXS);
        let total_txs = warmup_txs.saturating_add(steady_txs);
        artifacts.recipe = Some(ThroughputArtifactRecipe {
            peers: network.peers().len() as u64,
            block_time_ms: THROUGHPUT_BLOCK_TIME_MS,
            commit_time_ms: THROUGHPUT_COMMIT_TIME_MS,
            block_max_txs: THROUGHPUT_BLOCK_MAX_TXS,
            warmup_blocks,
            steady_blocks,
            total_blocks,
            warmup_txs,
            steady_txs,
            total_txs,
            submit_batch,
            submit_parallelism: submit_parallelism as u64,
            queue_soft_limit,
            payload_bytes: payload_bytes as u64,
            load_kind: "log".to_owned(),
            transfer_accounts: 0,
            transfer_initial_balance: 0,
            transfer_max_amount: 0,
            ram_lfe_email_accounts: 0,
            ram_lfe_email_policy: String::new(),
            ram_lfe_program: String::new(),
            rng_seed,
            rbc_encoding: "plain".to_owned(),
            rbc_data_shards: 4,
            rbc_parity_shards: 2,
        });

        let slo_p95_ms =
            env_or_default("IROHA_THROUGHPUT_SLO_P95_MS", THROUGHPUT_NPOS_SLO_P95_MS);
        let slo_p99_ms =
            env_or_default("IROHA_THROUGHPUT_SLO_P99_MS", THROUGHPUT_NPOS_SLO_P99_MS);
        let slo_view_change_rate = env_or_default_f64(
            "IROHA_THROUGHPUT_SLO_VIEW_CHANGE_RATE",
            THROUGHPUT_NPOS_SLO_VIEW_CHANGE_RATE_MAX,
        );
        let slo_backpressure_rate = env_or_default_f64(
            "IROHA_THROUGHPUT_SLO_BACKPRESSURE_RATE",
            THROUGHPUT_NPOS_SLO_BACKPRESSURE_RATE_MAX,
        );
        let slo_queue_saturation = env_or_default_f64(
            "IROHA_THROUGHPUT_SLO_QUEUE_SAT_FRAC",
            THROUGHPUT_NPOS_SLO_QUEUE_SAT_FRAC_MAX,
        );
        artifacts.slo = Some(ThroughputArtifactSlo {
            commit_p95_ms: slo_p95_ms,
            commit_p99_ms: slo_p99_ms,
            view_change_rate_max: slo_view_change_rate,
            backpressure_rate_max: slo_backpressure_rate,
            queue_saturation_max: slo_queue_saturation,
        });

        let submit_clients: Vec<_> =
            network.peers().iter().map(|peer| peer.client()).collect();
        ensure!(
            !submit_clients.is_empty(),
            "network must have at least one peer"
        );
        eprintln!(
            "localnet throughput recipe: peers={}, block_time_ms={}, commit_time_ms={}, block_max_txs={}, warmup_blocks={}, steady_blocks={}, total_blocks={}, payload_bytes={}, submit_batch={}, submit_parallelism={}, queue_soft_limit={}, rng_seed={}, baseline_non_empty={}, baseline_approved={}",
            network.peers().len(),
            THROUGHPUT_BLOCK_TIME_MS,
            THROUGHPUT_COMMIT_TIME_MS,
            THROUGHPUT_BLOCK_MAX_TXS,
            warmup_blocks,
            steady_blocks,
            total_blocks,
            payload_bytes,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            rng_seed,
            baseline_non_empty,
            baseline_approved,
        );

        let warmup_submit_elapsed = submit_logs(
            0,
            warmup_txs,
            &network,
            &submit_clients,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            payload_bytes,
            rng_seed,
        )
        .await?;

        let mut last_progress = Instant::now();
        let mut last_min_non_empty = baseline_non_empty;
        let mut last_log = Instant::now()
            .checked_sub(THROUGHPUT_PROGRESS_LOG_INTERVAL)
            .unwrap_or_else(Instant::now);
        let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();
        loop {
            if let Ok(statuses) = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await {
                let min_non_empty = statuses
                    .iter()
                    .map(|status| status.blocks_non_empty)
                    .min()
                    .unwrap_or_default();
                let max_non_empty = statuses
                    .iter()
                    .map(|status| status.blocks_non_empty)
                    .max()
                    .unwrap_or_default();
                last_snapshot = statuses
                    .iter()
                    .map(StatusSnapshot::from_status)
                    .collect();
                if min_non_empty >= warmup_target_height {
                    break;
                }
                if min_non_empty > last_min_non_empty {
                    last_min_non_empty = min_non_empty;
                    last_progress = Instant::now();
                }
                if last_log.elapsed() >= THROUGHPUT_PROGRESS_LOG_INTERVAL {
                    last_log = Instant::now();
                    eprintln!(
                        "localnet throughput warmup progress (target_non_empty={warmup_target_height}, min_non_empty={min_non_empty}, max_non_empty={max_non_empty}): {last_snapshot:?}"
                    );
                }
            }
            if last_progress.elapsed() >= THROUGHPUT_STALL_THRESHOLD {
                bail!(
                    "localnet throughput warmup stalled for {:?} (min_non_empty={last_min_non_empty}, target_non_empty={warmup_target_height}): last_snapshot={last_snapshot:?}",
                    THROUGHPUT_STALL_THRESHOLD
                );
            }
            sleep(THROUGHPUT_SAMPLE_INTERVAL).await;
        }

        let warmup_metrics =
            collect_metrics_snapshots(&network, &http, THROUGHPUT_METRICS_TIMEOUT).await?;
        let steady_start_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let steady_start_sumeragi =
            collect_sumeragi_statuses(&network, STATUS_POLL_TIMEOUT).await?;

        let steady_submit_elapsed = submit_logs(
            warmup_txs,
            steady_txs,
            &network,
            &submit_clients,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            payload_bytes,
            rng_seed,
        )
        .await?;

        let steady_start = Instant::now();
        let mut samples: Vec<ThroughputSample> = Vec::new();
        let mut last_progress = Instant::now();
        let mut last_min_non_empty = warmup_target_height;
        let mut last_log = Instant::now()
            .checked_sub(THROUGHPUT_PROGRESS_LOG_INTERVAL)
            .unwrap_or_else(Instant::now);
        let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();
        loop {
            if let Ok(statuses) = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await {
                let min_non_empty = statuses
                    .iter()
                    .map(|status| status.blocks_non_empty)
                    .min()
                    .unwrap_or_default();
                let max_non_empty = statuses
                    .iter()
                    .map(|status| status.blocks_non_empty)
                    .max()
                    .unwrap_or_default();
                last_snapshot = statuses
                    .iter()
                    .map(StatusSnapshot::from_status)
                    .collect();
                if min_non_empty >= steady_target_height {
                    break;
                }
                if min_non_empty > last_min_non_empty {
                    last_min_non_empty = min_non_empty;
                    last_progress = Instant::now();
                }
                if last_log.elapsed() >= THROUGHPUT_PROGRESS_LOG_INTERVAL {
                    last_log = Instant::now();
                    eprintln!(
                        "localnet throughput steady progress (target_non_empty={steady_target_height}, min_non_empty={min_non_empty}, max_non_empty={max_non_empty}): {last_snapshot:?}"
                    );
                }
            }
            if last_progress.elapsed() >= THROUGHPUT_STALL_THRESHOLD {
                bail!(
                    "localnet throughput stalled for {:?} (min_non_empty={last_min_non_empty}, target_non_empty={steady_target_height}): last_snapshot={last_snapshot:?}",
                    THROUGHPUT_STALL_THRESHOLD
                );
            }
            let statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
            let sumeragi = collect_sumeragi_statuses(&network, STATUS_POLL_TIMEOUT).await?;
            let status_snapshots: Vec<StatusSnapshot> =
                statuses.iter().map(StatusSnapshot::from_status).collect();
            let sumeragi_snapshots: Vec<SumeragiStatusSnapshot> = sumeragi
                .iter()
                .map(SumeragiStatusSnapshot::from_status)
                .collect();
            let timestamp_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            samples.push(ThroughputSample {
                phase: None,
                timestamp_ms: u64::try_from(timestamp_ms).unwrap_or(u64::MAX),
                statuses: status_snapshots,
                sumeragi: sumeragi_snapshots,
            });
            sleep(THROUGHPUT_SAMPLE_INTERVAL).await;
        }

        let steady_elapsed = steady_start.elapsed();
        let after_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let after_sumeragi = collect_sumeragi_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let after_metrics =
            collect_metrics_snapshots(&network, &http, THROUGHPUT_METRICS_TIMEOUT).await?;

        let max_commit_time_ms = after_statuses
            .iter()
            .map(|status| status.commit_time_ms)
            .max()
            .unwrap_or_default();
        let max_commit_time_allowed = THROUGHPUT_COMMIT_TIME_MS
            .saturating_mul(THROUGHPUT_COMMIT_TIME_MAX_MULTIPLIER);
        let mut min_commit_time_ms = u64::MAX;
        let mut sum_commit_time_ms = 0_u128;
        for status in &after_statuses {
            let value = status.commit_time_ms;
            min_commit_time_ms = min_commit_time_ms.min(value);
            sum_commit_time_ms = sum_commit_time_ms.saturating_add(u128::from(value));
        }
        let avg_commit_time_ms = if after_statuses.is_empty() {
            0_u64
        } else {
            (sum_commit_time_ms / u128::from(after_statuses.len() as u64)) as u64
        };
        let min_commit_time_ms = if min_commit_time_ms == u64::MAX {
            0_u64
        } else {
            min_commit_time_ms
        };

        let (commit_p95_ms, commit_p99_ms, commit_hist_count) =
            commit_time_quantiles(&warmup_metrics, &after_metrics);
        let commit_p95_ms = commit_p95_ms.unwrap_or_default();
        let commit_p99_ms = commit_p99_ms.unwrap_or_default();

        let steady_approved = after_statuses
            .iter()
            .map(|status| status.txs_approved)
            .min()
            .unwrap_or_default()
            .saturating_sub(
                steady_start_statuses
                    .iter()
                    .map(|status| status.txs_approved)
                    .min()
                    .unwrap_or_default(),
            );
        let committed_approved = steady_approved.saturating_sub(baseline_approved);
        let committed_tps = if steady_elapsed.as_secs_f64() > 0.0 {
            committed_approved as f64 / steady_elapsed.as_secs_f64()
        } else {
            0.0
        };
        let submitted_tps = if steady_submit_elapsed.as_secs_f64() > 0.0 {
            steady_txs as f64 / steady_submit_elapsed.as_secs_f64()
        } else {
            0.0
        };

        let (view_change_avg, view_change_max) = rate_summary(
            steady_start_sumeragi
                .iter()
                .map(|status| status.view_change_install_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            after_sumeragi
                .iter()
                .map(|status| status.view_change_install_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            steady_elapsed,
        );
        let (backpressure_avg, backpressure_max) = rate_summary(
            steady_start_sumeragi
                .iter()
                .map(|status| status.pacemaker_backpressure_deferrals_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            after_sumeragi
                .iter()
                .map(|status| status.pacemaker_backpressure_deferrals_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            steady_elapsed,
        );

        let mut saturated_samples = 0_u64;
        let mut total_samples = 0_u64;
        let mut max_queue_depth = 0_u64;
        for sample in &samples {
            for status in &sample.sumeragi {
                total_samples = total_samples.saturating_add(1);
                if status.tx_queue_saturated {
                    saturated_samples = saturated_samples.saturating_add(1);
                }
                max_queue_depth = max_queue_depth.max(status.tx_queue_depth);
            }
        }
        let queue_saturated_frac = if total_samples > 0 {
            saturated_samples as f64 / total_samples as f64
        } else {
            0.0
        };

        let metrics = ThroughputArtifactMetrics {
            submitted_tps,
            committed_tps,
            commit_p95_ms,
            commit_p99_ms,
            commit_hist_count,
            commit_time_ms_min: min_commit_time_ms,
            commit_time_ms_avg: avg_commit_time_ms,
            commit_time_ms_max: max_commit_time_ms,
            view_change_rate_avg: view_change_avg,
            view_change_rate_max: view_change_max,
            backpressure_rate_avg: backpressure_avg,
            backpressure_rate_max: backpressure_max,
            queue_saturated_frac,
            max_queue_depth,
            steady_elapsed_ms: steady_elapsed.as_millis() as u64,
            warmup_submit_elapsed_ms: warmup_submit_elapsed.as_millis() as u64,
            steady_submit_elapsed_ms: steady_submit_elapsed.as_millis() as u64,
        };
        artifacts.metrics = Some(metrics);
        artifacts.samples = samples;
        artifacts.warmup_metrics = warmup_metrics;
        artifacts.after_metrics = after_metrics;

        eprintln!(
            "localnet throughput metrics: peers={}, warmup_blocks={}, steady_blocks={}, warmup_txs={}, steady_txs={}, submit_batch={}, submit_parallelism={}, queue_soft_limit={}, payload_bytes={}, warmup_submit_elapsed={:?}, steady_submit_elapsed={:?}, steady_elapsed={:?}, submitted_tps={:.2}, committed_tps={:.2}, commit_hist_count={}, commit_time_ms(min/avg/max/p95/p99)={}/{}/{}/{}/{}, view_change_rate(avg/max)={:.4}/{:.4}, backpressure_rate(avg/max)={:.4}/{:.4}, queue_saturated_frac={:.2}, max_queue_depth={}",
            network.peers().len(),
            warmup_blocks,
            steady_blocks,
            warmup_txs,
            steady_txs,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            payload_bytes,
            warmup_submit_elapsed,
            steady_submit_elapsed,
            steady_elapsed,
            submitted_tps,
            committed_tps,
            commit_hist_count,
            min_commit_time_ms,
            avg_commit_time_ms,
            max_commit_time_ms,
            commit_p95_ms,
            commit_p99_ms,
            view_change_avg,
            view_change_max,
            backpressure_avg,
            backpressure_max,
            queue_saturated_frac,
            max_queue_depth,
        );
        ensure!(
            max_commit_time_ms <= max_commit_time_allowed,
            "commit time exceeded target: max_commit_time_ms={max_commit_time_ms}, allowed={max_commit_time_allowed}",
        );

        if commit_hist_count > 0 {
            ensure!(
                commit_p95_ms <= slo_p95_ms,
                "p95 commit time exceeded SLO: p95_ms={commit_p95_ms}, slo_p95_ms={slo_p95_ms}",
            );
            ensure!(
                commit_p99_ms <= slo_p99_ms,
                "p99 commit time exceeded SLO: p99_ms={commit_p99_ms}, slo_p99_ms={slo_p99_ms}",
            );
        }
        if slo_view_change_rate > 0.0 {
            ensure!(
                view_change_max <= slo_view_change_rate,
                "view change rate exceeded SLO: max_rate={view_change_max:.4}, slo_rate={slo_view_change_rate:.4}",
            );
        }
        if slo_backpressure_rate > 0.0 {
            ensure!(
                backpressure_max <= slo_backpressure_rate,
                "backpressure deferral rate exceeded SLO: max_rate={backpressure_max:.4}, slo_rate={slo_backpressure_rate:.4}",
            );
        }
        if slo_queue_saturation > 0.0 {
            ensure!(
                queue_saturated_frac <= slo_queue_saturation,
                "queue saturation exceeded SLO: fraction={queue_saturated_frac:.2}, slo={slo_queue_saturation:.2}",
            );
        }

        Ok(())
    }
    .await;

        if let Err(err) = &run_result {
            artifacts.error = Some(err.to_string());
        }
        if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {
            let root = PathBuf::from(artifact_root);
            let peer_logs: Vec<PeerLogInfo> = network
                .peers()
                .iter()
                .enumerate()
                .map(|(index, peer)| PeerLogInfo {
                    index: index as u64,
                    mnemonic: peer.mnemonic().to_string(),
                    stdout_log: peer
                        .latest_stdout_log_path()
                        .map(|path| path.to_string_lossy().to_string()),
                    stderr_log: peer
                        .latest_stderr_log_path()
                        .map(|path| path.to_string_lossy().to_string()),
                })
                .collect();
            if let Err(err) =
                write_throughput_artifacts(&root, &network_dir, &peer_logs, &artifacts)
            {
                eprintln!("throughput artifact write failed: {err:?}");
            }
        }

        network.shutdown().await;
        run_result
    }
    .await;

    if let Some(previous_ttl) = previous_ttl {
        set_env_var("IROHA_TEST_CLIENT_TTL_MS", previous_ttl);
    } else {
        remove_env_var("IROHA_TEST_CLIENT_TTL_MS");
    }

    if sandbox::handle_result(result, stringify!(npos_localnet_throughput_10k_tps))?.is_none() {
        return Ok(());
    }
    Ok(())
}

async fn collect_statuses(
    network: &Network,
    status_timeout: Duration,
) -> Result<Vec<iroha::client::Status>> {
    try_join_all(network.peers().iter().map(|peer| async move {
        tokio::time::timeout(status_timeout, peer.status())
            .await
            .map_or_else(
                |_| {
                    eprintln!(
                        "status request timed out for peer {} after {:?} (best_effort={:?}, last_known_peers={:?}, stdout={:?})",
                        peer.mnemonic(),
                        status_timeout,
                        peer.best_effort_block_height(),
                        peer.last_known_peers(),
                        peer.latest_stdout_log_path()
                    );
                    Err(eyre!(
                        "status request timed out after {:?} for peer {}",
                        status_timeout,
                        peer.mnemonic()
                    ))
                },
                |result| {
                    result
                        .map_err(|err| {
                            eprintln!(
                                "status request failed for peer {}: {err:?} (best_effort={:?}, last_known_peers={:?}, stdout={:?})",
                                peer.mnemonic(),
                                peer.best_effort_block_height(),
                                peer.last_known_peers(),
                                peer.latest_stdout_log_path()
                            );
                            err
                        })
                        .wrap_err_with(|| format!("status request failed for peer {}", peer.mnemonic()))
                },
            )
    }))
    .await
}

async fn collect_sumeragi_statuses(
    network: &Network,
    status_timeout: Duration,
) -> Result<Vec<SumeragiStatusWire>> {
    try_join_all(network.peers().iter().map(|peer| async move {
        let client = peer.client();
        let handle = task::spawn_blocking(move || client.get_sumeragi_status_wire());
        if let Ok(joined) = tokio::time::timeout(status_timeout, handle).await {
            joined
                .map_err(|err| {
                    eyre!(
                        "sumeragi status join failed for peer {}: {err:?}",
                        peer.mnemonic()
                    )
                })?
                .map_err(|err| {
                    eprintln!(
                        "sumeragi status request failed for peer {}: {err:?} (best_effort={:?}, stdout={:?})",
                        peer.mnemonic(),
                        peer.best_effort_block_height(),
                        peer.latest_stdout_log_path()
                    );
                    err
                })
                .wrap_err_with(|| format!("sumeragi status request failed for peer {}", peer.mnemonic()))
        } else {
            eprintln!(
                "sumeragi status request timed out for peer {} after {:?} (best_effort={:?}, stdout={:?})",
                peer.mnemonic(),
                status_timeout,
                peer.best_effort_block_height(),
                peer.latest_stdout_log_path()
            );
            Err(eyre!(
                "sumeragi status request timed out after {:?} for peer {}",
                status_timeout,
                peer.mnemonic()
            ))
        }
    }))
    .await
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SoakPhaseSnapshot {
    propose_ms: u64,
    availability_ms: u64,
    prevote_ms: u64,
    precommit_ms: u64,
    commit_ms: u64,
    pipeline_total_ms: u64,
    propose_ema_ms: u64,
    availability_ema_ms: u64,
    prevote_ema_ms: u64,
    precommit_ema_ms: u64,
    commit_ema_ms: u64,
    pipeline_total_ema_ms: u64,
}

impl SoakPhaseSnapshot {
    fn from_json(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let ema = object.get("ema_ms").and_then(Value::as_object);
        Some(Self {
            propose_ms: json_object_u64(object, "propose_ms"),
            availability_ms: json_object_u64(object, "collect_da_ms"),
            prevote_ms: json_object_u64(object, "collect_prevote_ms"),
            precommit_ms: json_object_u64(object, "collect_precommit_ms"),
            commit_ms: json_object_u64(object, "commit_ms"),
            pipeline_total_ms: json_object_u64(object, "pipeline_total_ms"),
            propose_ema_ms: ema.map_or(0, |obj| json_object_u64(obj, "propose_ms")),
            availability_ema_ms: ema.map_or(0, |obj| json_object_u64(obj, "collect_da_ms")),
            prevote_ema_ms: ema.map_or(0, |obj| json_object_u64(obj, "collect_prevote_ms")),
            precommit_ema_ms: ema.map_or(0, |obj| json_object_u64(obj, "collect_precommit_ms")),
            commit_ema_ms: ema.map_or(0, |obj| json_object_u64(obj, "commit_ms")),
            pipeline_total_ema_ms: ema.map_or(0, |obj| json_object_u64(obj, "pipeline_total_ms")),
        })
    }
}

fn json_object_u64(map: &Map, key: &str) -> u64 {
    map.get(key).and_then(Value::as_u64).unwrap_or_default()
}

async fn collect_sumeragi_phase_snapshot(
    network: &Network,
    status_timeout: Duration,
) -> Result<SoakPhaseSnapshot> {
    let client = network.client();
    let handle = task::spawn_blocking(move || client.get_sumeragi_phases_json());
    let value = if let Ok(joined) = tokio::time::timeout(status_timeout, handle).await {
        joined
            .map_err(|err| eyre!("sumeragi phase join failed: {err:?}"))?
            .wrap_err("sumeragi phase request failed")?
    } else {
        return Err(eyre!(
            "sumeragi phase request timed out after {:?}",
            status_timeout
        ));
    };
    SoakPhaseSnapshot::from_json(&value)
        .ok_or_else(|| eyre!("sumeragi phase payload malformed: {:?}", value))
}

async fn collect_metrics_snapshots(
    network: &Network,
    http: &HttpClient,
    timeout: Duration,
) -> Result<Vec<PeerMetricsSnapshot>> {
    try_join_all(network.peers().iter().map(|peer| async move {
        let url = metrics_url(&peer.torii_url());
        let response = http
            .get(url.clone())
            .timeout(timeout)
            .send()
            .await
            .wrap_err_with(|| format!("metrics request failed for peer {}", peer.mnemonic()))?;
        let response = response.error_for_status().wrap_err_with(|| {
            format!(
                "metrics request returned error for peer {}",
                peer.mnemonic()
            )
        })?;
        let payload = response
            .text()
            .await
            .wrap_err_with(|| format!("metrics body decode failed for peer {}", peer.mnemonic()))?;
        let commit_time_hist = parse_prom_histogram(&payload, "commit_time_ms");
        Ok(PeerMetricsSnapshot {
            peer: peer.mnemonic().to_string(),
            payload,
            commit_time_hist,
        })
    }))
    .await
}

async fn wait_for_status_responses(network: &Network, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_log = Instant::now()
        .checked_sub(STATUS_LOG_INTERVAL)
        .unwrap_or_else(Instant::now);
    loop {
        match collect_statuses(network, STATUS_POLL_TIMEOUT).await {
            Ok(_) => return Ok(()),
            Err(err) => {
                if last_log.elapsed() >= STATUS_LOG_INTERVAL {
                    eprintln!(
                        "waiting for status responses (timeout={timeout:?}): last_error={err:?}"
                    );
                    last_log = Instant::now();
                }
                if Instant::now() >= deadline {
                    return Err(eyre!(
                        "status responses did not converge within {:?}; last_error={err:?}",
                        timeout,
                    ));
                }
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn wait_for_min_txs_approved(
    network: &Network,
    target_approved: u64,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_min_approved = 0;
    let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();
    let mut last_log = Instant::now()
        .checked_sub(STATUS_LOG_INTERVAL)
        .unwrap_or_else(Instant::now);
    loop {
        match collect_statuses(network, STATUS_POLL_TIMEOUT).await {
            Ok(statuses) => {
                let min_approved = statuses
                    .iter()
                    .map(|status| status.txs_approved)
                    .min()
                    .unwrap_or_default();
                let snapshot: Vec<StatusSnapshot> =
                    statuses.iter().map(StatusSnapshot::from_status).collect();
                if min_approved > last_min_approved
                    || snapshot != last_snapshot
                    || last_log.elapsed() >= STATUS_LOG_INTERVAL
                {
                    eprintln!(
                        "waiting for approved transactions: min_approved={min_approved}, target_approved={target_approved}, snapshot={snapshot:?}"
                    );
                    last_min_approved = min_approved;
                    last_snapshot = snapshot;
                    last_log = Instant::now();
                }
                if min_approved >= target_approved {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err(eyre!(
                        "approved transactions did not reach {target_approved} within {:?}: min_approved={min_approved}, last_snapshot={last_snapshot:?}",
                        timeout
                    ));
                }
            }
            Err(err) => {
                if Instant::now() >= deadline {
                    return Err(eyre!(
                        "approved transaction status did not converge within {:?}: target_approved={target_approved}, last_snapshot={last_snapshot:?}, last_error={err:?}",
                        timeout
                    ));
                }
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn wait_for_queue_depth(
    network: &Network,
    max_queue: u64,
    status_timeout: Duration,
) -> Result<()> {
    let progress_timeout = queue_progress_timeout();
    let mut last_progress = Instant::now();
    let mut last_queue: Option<u64> = None;
    let mut last_blocks_non_empty: Option<u64> = None;
    let mut last_log = Instant::now()
        .checked_sub(STATUS_LOG_INTERVAL)
        .unwrap_or_else(Instant::now);
    loop {
        match collect_statuses(network, status_timeout).await {
            Ok(statuses) => {
                let submitter_queue = statuses
                    .iter()
                    .map(|status| status.queue_size)
                    .max()
                    .unwrap_or_default();
                let min_non_empty = statuses
                    .iter()
                    .map(|status| status.blocks_non_empty)
                    .min()
                    .unwrap_or_default();
                if submitter_queue <= max_queue {
                    return Ok(());
                }
                let progressed = last_queue.is_some_and(|prev| submitter_queue < prev)
                    || last_blocks_non_empty.is_some_and(|prev| min_non_empty > prev);
                if progressed {
                    last_progress = Instant::now();
                }
                last_queue = Some(submitter_queue);
                last_blocks_non_empty = Some(min_non_empty);
                if last_log.elapsed() >= STATUS_LOG_INTERVAL {
                    eprintln!(
                        "waiting for submit queue to drain (queue_size={submitter_queue}, limit={max_queue}, min_non_empty={min_non_empty})"
                    );
                    last_log = Instant::now();
                }
            }
            Err(err) => {
                if last_log.elapsed() >= STATUS_LOG_INTERVAL {
                    eprintln!("waiting for submit queue to drain: status poll failed: {err:?}");
                    last_log = Instant::now();
                }
            }
        }

        if last_progress.elapsed() >= progress_timeout {
            return Err(eyre!(
                "submit queue did not drain below {max_queue} within {:?}",
                progress_timeout
            ));
        }

        sleep(SOAK_STATUS_POLL_INTERVAL).await;
    }
}

#[tokio::test]
async fn env_or_default_reads_positive_values() {
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    let key = "IROHA_ENV_OR_DEFAULT_TEST";
    set_env_var(key, "42");
    assert_eq!(env_or_default(key, 7), 42);
    remove_env_var(key);
}

#[tokio::test]
async fn env_or_default_ignores_invalid_or_zero() {
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    let key = "IROHA_ENV_OR_DEFAULT_TEST";
    set_env_var(key, "0");
    assert_eq!(env_or_default(key, 7), 7);
    set_env_var(key, "nope");
    assert_eq!(env_or_default(key, 7), 7);
    remove_env_var(key);
}

#[tokio::test]
async fn env_or_default_usize_reads_positive_values() {
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    let key = "IROHA_ENV_OR_DEFAULT_USIZE_TEST";
    set_env_var(key, "64");
    assert_eq!(env_or_default_usize(key, 8), 64);
    remove_env_var(key);
}

#[tokio::test]
async fn env_or_default_f64_reads_values() {
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    let key = "IROHA_ENV_OR_DEFAULT_F64_TEST";
    set_env_var(key, "1.25");
    assert!((env_or_default_f64(key, 0.5) - 1.25).abs() < f64::EPSILON);
    set_env_var(key, "-1.0");
    assert!((env_or_default_f64(key, 0.5) - 0.5).abs() < f64::EPSILON);
    remove_env_var(key);
}

#[tokio::test]
async fn queue_progress_timeout_reads_override() {
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    set_env_var(THROUGHPUT_QUEUE_PROGRESS_TIMEOUT_ENV, "12");
    assert_eq!(queue_progress_timeout(), Duration::from_secs(12));
    set_env_var(THROUGHPUT_QUEUE_PROGRESS_TIMEOUT_ENV, "0");
    assert_eq!(queue_progress_timeout(), SOAK_QUEUE_PROGRESS_TIMEOUT);
    remove_env_var(THROUGHPUT_QUEUE_PROGRESS_TIMEOUT_ENV);
}

#[tokio::test]
async fn fail_on_sandbox_skip_parses_truthy_values() {
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    set_env_var(FAIL_ON_SANDBOX_SKIP_ENV, "1");
    assert!(fail_on_sandbox_skip());
    set_env_var(FAIL_ON_SANDBOX_SKIP_ENV, "true");
    assert!(fail_on_sandbox_skip());
    set_env_var(FAIL_ON_SANDBOX_SKIP_ENV, "yes");
    assert!(fail_on_sandbox_skip());
    remove_env_var(FAIL_ON_SANDBOX_SKIP_ENV);
}

#[tokio::test]
async fn fail_on_sandbox_skip_defaults_to_false() {
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    remove_env_var(FAIL_ON_SANDBOX_SKIP_ENV);
    assert!(!fail_on_sandbox_skip());
    set_env_var(FAIL_ON_SANDBOX_SKIP_ENV, "0");
    assert!(!fail_on_sandbox_skip());
    set_env_var(FAIL_ON_SANDBOX_SKIP_ENV, "off");
    assert!(!fail_on_sandbox_skip());
    remove_env_var(FAIL_ON_SANDBOX_SKIP_ENV);
}

#[tokio::test]
async fn realistic_30tps_load_kind_parses_email_mode_and_defaults_to_transfer() {
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    remove_env_var("IROHA_REALISTIC_30TPS_LOAD_KIND");
    assert_eq!(
        Realistic30TpsLoadKind::from_env().expect("default load kind"),
        Realistic30TpsLoadKind::Transfer
    );
    set_env_var("IROHA_REALISTIC_30TPS_LOAD_KIND", "ram-lfe-email");
    assert_eq!(
        Realistic30TpsLoadKind::from_env().expect("email load kind"),
        Realistic30TpsLoadKind::RamLfeEmail
    );
    set_env_var("IROHA_REALISTIC_30TPS_LOAD_KIND", "emails");
    assert_eq!(
        Realistic30TpsLoadKind::from_env().expect("email alias load kind"),
        Realistic30TpsLoadKind::RamLfeEmail
    );
    set_env_var("IROHA_REALISTIC_30TPS_LOAD_KIND", "unsupported");
    assert!(Realistic30TpsLoadKind::from_env().is_err());
    remove_env_var("IROHA_REALISTIC_30TPS_LOAD_KIND");
}

#[test]
fn realistic_load_metadata_stamps_unique_sequence() {
    let sequence_key = Name::from_str("tx_sequence").expect("tx_sequence metadata key");
    let first = realistic_load_metadata(7);
    let second = realistic_load_metadata(8);

    assert_eq!(
        first.get(&sequence_key).map(ToString::to_string).as_deref(),
        Some("7")
    );
    assert_eq!(
        second
            .get(&sequence_key)
            .map(ToString::to_string)
            .as_deref(),
        Some("8")
    );
    assert_ne!(first, second);
}

#[test]
fn realistic_npos_fee_funding_instruction_chunks_target_fee_asset() {
    let accounts = realistic_transfer_accounts(3, 7);
    let chunks = realistic_npos_fee_funding_instruction_chunks(&accounts);
    let fee_asset_definition_id = route_fee_asset_definition_id();

    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].len(), accounts.len());
    for (instruction, account) in chunks[0].iter().zip(&accounts) {
        let Some(iroha::data_model::isi::MintBox::Asset(mint)) = instruction
            .as_any()
            .downcast_ref::<iroha::data_model::isi::MintBox>(
        ) else {
            panic!("expected asset mint instruction");
        };
        assert_eq!(
            mint.destination(),
            &AssetId::new(fee_asset_definition_id.clone(), account.id.clone())
        );
        assert_eq!(
            mint.object(),
            &Numeric::from(ROUTE_VALIDATOR_FEE_SEED_AMOUNT)
        );
    }
}

#[test]
fn realistic_ram_lfe_email_receipt_is_signed_for_generated_email_claim() {
    let resolver = KeyPair::from_seed(
        b"integration_tests::realistic-ram-lfe-email-receipt-test".to_vec(),
        Algorithm::Ed25519,
    );
    let owner = (*ALICE_ID).clone();
    let (policy, program_policy) = realistic_ram_lfe_email_policy_bundle(&owner, &resolver);
    let context = realistic_ram_lfe_email_policy_context(policy.id.clone(), &program_policy)
        .expect("policy context");
    let account = realistic_ram_lfe_email_accounts(1, 9)
        .pop()
        .expect("account");

    let receipt =
        realistic_ram_lfe_email_receipt(&context, &resolver, &account.id, account.uaid, 7, 9);

    receipt
        .verify(resolver.public_key())
        .expect("receipt signature should verify");
    assert_eq!(receipt.payload.policy_id, policy.id);
    assert_eq!(receipt.payload.account_id, account.id);
    assert_eq!(receipt.payload.uaid, account.uaid);
    assert_eq!(
        receipt.payload.execution.associated_data_hash,
        Hash::new(&context.program_id_bytes)
    );
}

#[test]
fn realistic_ram_lfe_email_claim_counts_are_deterministic() {
    let first = expected_realistic_ram_lfe_email_claim_counts(4, 25, 123);
    let second = expected_realistic_ram_lfe_email_claim_counts(4, 25, 123);
    assert_eq!(first, second);
    assert_eq!(first.iter().sum::<usize>(), 25);
    assert_ne!(
        first,
        expected_realistic_ram_lfe_email_claim_counts(4, 25, 124)
    );
}

#[test]
fn write_throughput_artifacts_writes_error_summary() {
    let dir = tempdir().expect("tempdir");
    let artifact_root = dir.path().join("artifacts");
    let network_dir = dir.path().join("network");
    let artifacts = ThroughputArtifacts {
        error: Some("boom".to_string()),
        samples: vec![ThroughputSample {
            phase: Some("load".to_string()),
            timestamp_ms: 7,
            statuses: Vec::new(),
            sumeragi: Vec::new(),
        }],
        ..ThroughputArtifacts::default()
    };
    let peer_logs = vec![PeerLogInfo {
        index: 0,
        mnemonic: "peer0".to_string(),
        stdout_log: None,
        stderr_log: None,
    }];

    let run_dir = write_throughput_artifacts(&artifact_root, &network_dir, &peer_logs, &artifacts)
        .expect("write artifacts");
    let summary_path = run_dir.join("summary.json");
    let summary_json = fs::read_to_string(&summary_path).expect("read summary");
    let Value::Object(map) =
        norito::json::from_json::<Value>(&summary_json).expect("parse summary")
    else {
        panic!("expected summary object");
    };
    assert_eq!(map.get("error"), Some(&Value::String("boom".to_string())));
    let samples_json =
        fs::read_to_string(run_dir.join("status_samples.json")).expect("read samples");
    let Value::Array(samples) =
        norito::json::from_json::<Value>(&samples_json).expect("parse samples")
    else {
        panic!("expected samples array");
    };
    let Some(Value::Object(sample)) = samples.first() else {
        panic!("expected first sample object");
    };
    assert_eq!(
        sample.get("phase"),
        Some(&Value::String("load".to_string()))
    );
}

#[test]
fn throughput_status_summary_uses_min_and_max_peer_values() {
    let statuses = vec![
        StatusSnapshot {
            blocks: 10,
            blocks_non_empty: 9,
            queue_size: 4,
            txs_approved: 90,
            txs_rejected: 0,
            view_changes: 0,
            leader_index: None,
            highest_qc_height: None,
            locked_qc_height: None,
            tx_queue_depth: None,
            tx_queue_saturated: None,
            block_created_dropped_by_lock_total: None,
            block_created_hint_mismatch_total: None,
            block_created_proposal_mismatch_total: None,
            commit_signatures_present: None,
            commit_signatures_required: None,
        },
        StatusSnapshot {
            blocks: 12,
            blocks_non_empty: 11,
            queue_size: 7,
            txs_approved: 100,
            txs_rejected: 2,
            view_changes: 0,
            leader_index: None,
            highest_qc_height: None,
            locked_qc_height: None,
            tx_queue_depth: None,
            tx_queue_saturated: None,
            block_created_dropped_by_lock_total: None,
            block_created_hint_mismatch_total: None,
            block_created_proposal_mismatch_total: None,
            commit_signatures_present: None,
            commit_signatures_required: None,
        },
    ];

    let summary = ThroughputStatusSummary::from_statuses(&statuses);

    assert_eq!(summary.min_blocks, 10);
    assert_eq!(summary.max_blocks, 12);
    assert_eq!(summary.min_blocks_non_empty, 9);
    assert_eq!(summary.max_blocks_non_empty, 11);
    assert_eq!(summary.min_txs_approved, 90);
    assert_eq!(summary.max_txs_rejected, 2);
    assert_eq!(summary.max_queue_size, 7);
}

#[test]
fn realistic_artifact_summary_counts_load_samples_and_keeps_zero_block_rates_finite() {
    let load_end = ThroughputStatusSummary {
        min_txs_approved: 10,
        ..ThroughputStatusSummary::default()
    };
    let final_status = ThroughputStatusSummary {
        min_txs_approved: 25,
        ..ThroughputStatusSummary::default()
    };
    let samples = vec![
        ThroughputSample {
            phase: Some("load".to_string()),
            timestamp_ms: 1,
            statuses: Vec::new(),
            sumeragi: Vec::new(),
        },
        ThroughputSample {
            phase: Some("drain".to_string()),
            timestamp_ms: 2,
            statuses: Vec::new(),
            sumeragi: Vec::new(),
        },
    ];

    let summary = realistic_artifact_summary(
        0,
        10,
        5,
        30,
        15,
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(8),
        load_end,
        final_status,
        0,
        0,
        &samples,
    );

    assert_eq!(summary.load_sample_count, 1);
    assert_eq!(summary.drain_elapsed_ms, 3_000);
    assert_eq!(summary.load_avg_secs_per_block, 0.0);
    assert_eq!(summary.avg_secs_per_block, 0.0);
    assert!(summary.final_committed_tps.is_finite());
}

#[test]
fn realistic_target_blocks_cover_load_capacity() {
    assert_eq!(realistic_target_blocks(0, 216_000, 512), 422);
    assert_eq!(realistic_target_blocks(0, 36_000, 100), 360);
    assert_eq!(realistic_target_blocks(600, 36_000, 100), 600);
    assert_eq!(realistic_target_blocks(0, 36_001, 50), 721);
    assert_eq!(realistic_target_blocks(600, 36_000, 0), 600);
}

#[test]
fn realistic_target_blocks_handle_extreme_counters() {
    assert_eq!(realistic_target_blocks(0, u64::MAX, 1), u64::MAX);
    assert_eq!(realistic_target_blocks(0, u64::MAX, 2), 1u64 << 63);
    assert_eq!(realistic_target_blocks(u64::MAX, 1, 1), u64::MAX);
    assert_eq!(realistic_target_blocks(u64::MAX - 1, 1, 0), u64::MAX - 1);
}

#[test]
fn realistic_target_blocks_zero_transactions_never_inflates_default() {
    assert_eq!(realistic_target_blocks(0, 0, 100), 0);
    assert_eq!(realistic_target_blocks(7, 0, 100), 7);
    assert_eq!(realistic_target_blocks(7, 0, 0), 7);
}

#[test]
fn realistic_target_blocks_rounds_up_partial_capacity() {
    assert_eq!(realistic_target_blocks(0, 1, 100), 1);
    assert_eq!(realistic_target_blocks(0, 100, 100), 1);
    assert_eq!(realistic_target_blocks(0, 101, 100), 2);
    assert_eq!(realistic_target_blocks(1, 101, 100), 2);
}

#[test]
fn realistic_target_blocks_handles_near_max_capacity_without_overflow() {
    assert_eq!(realistic_target_blocks(0, u64::MAX, u64::MAX), 1);
    assert_eq!(realistic_target_blocks(0, u64::MAX - 1, u64::MAX), 1);
    assert_eq!(
        realistic_target_blocks(0, u64::MAX - 1, (u64::MAX / 2) + 1),
        2
    );
}

#[test]
fn max_queue_size_for_phase_filters_samples() {
    let samples = vec![
        ThroughputSample {
            phase: Some("load".to_string()),
            timestamp_ms: 1,
            statuses: vec![StatusSnapshot {
                queue_size: 7,
                ..StatusSnapshot::default()
            }],
            sumeragi: Vec::new(),
        },
        ThroughputSample {
            phase: Some("drain".to_string()),
            timestamp_ms: 2,
            statuses: vec![StatusSnapshot {
                queue_size: 99,
                ..StatusSnapshot::default()
            }],
            sumeragi: Vec::new(),
        },
        ThroughputSample {
            phase: Some("load".to_string()),
            timestamp_ms: 3,
            statuses: vec![StatusSnapshot {
                queue_size: 11,
                ..StatusSnapshot::default()
            }],
            sumeragi: Vec::new(),
        },
    ];

    assert_eq!(max_queue_size_for_phase(&samples, "load"), 11);
    assert_eq!(max_queue_size_for_phase(&samples, "drain"), 99);
    assert_eq!(max_queue_size_for_phase(&samples, "missing"), 0);
}

#[test]
fn max_queue_size_for_phase_ignores_adversarial_non_matching_samples() {
    let samples = vec![
        ThroughputSample {
            phase: None,
            timestamp_ms: 1,
            statuses: vec![StatusSnapshot {
                queue_size: u64::MAX,
                ..StatusSnapshot::default()
            }],
            sumeragi: Vec::new(),
        },
        ThroughputSample {
            phase: Some("Load".to_string()),
            timestamp_ms: 2,
            statuses: vec![StatusSnapshot {
                queue_size: u64::MAX - 1,
                ..StatusSnapshot::default()
            }],
            sumeragi: Vec::new(),
        },
        ThroughputSample {
            phase: Some("load".to_string()),
            timestamp_ms: 3,
            statuses: Vec::new(),
            sumeragi: Vec::new(),
        },
        ThroughputSample {
            phase: Some("load".to_string()),
            timestamp_ms: 4,
            statuses: vec![
                StatusSnapshot {
                    queue_size: 13,
                    ..StatusSnapshot::default()
                },
                StatusSnapshot {
                    queue_size: 21,
                    ..StatusSnapshot::default()
                },
            ],
            sumeragi: Vec::new(),
        },
    ];

    assert_eq!(max_queue_size_for_phase(&samples, "load"), 21);
    assert_eq!(max_queue_size_for_phase(&samples, "Load"), u64::MAX - 1);
    assert_eq!(max_queue_size_for_phase(&samples, ""), 0);
}

#[test]
fn max_queue_size_for_phase_counts_all_matching_statuses_without_summing() {
    let samples = vec![
        ThroughputSample {
            phase: Some("load".to_string()),
            timestamp_ms: 1,
            statuses: vec![
                StatusSnapshot {
                    queue_size: u64::MAX - 10,
                    ..StatusSnapshot::default()
                },
                StatusSnapshot {
                    queue_size: 9,
                    ..StatusSnapshot::default()
                },
            ],
            sumeragi: Vec::new(),
        },
        ThroughputSample {
            phase: Some("drain".to_string()),
            timestamp_ms: 2,
            statuses: vec![StatusSnapshot {
                queue_size: u64::MAX,
                ..StatusSnapshot::default()
            }],
            sumeragi: Vec::new(),
        },
        ThroughputSample {
            phase: Some("load".to_string()),
            timestamp_ms: 3,
            statuses: vec![StatusSnapshot {
                queue_size: u64::MAX - 1,
                ..StatusSnapshot::default()
            }],
            sumeragi: Vec::new(),
        },
    ];

    assert_eq!(max_queue_size_for_phase(&samples, "load"), u64::MAX - 1);
    assert_eq!(max_queue_size_for_phase(&samples, "drain"), u64::MAX);
}

#[test]
fn max_queue_size_for_phase_requires_exact_phase_match() {
    let samples = vec![
        ThroughputSample {
            phase: Some("load ".to_string()),
            timestamp_ms: 1,
            statuses: vec![StatusSnapshot {
                queue_size: u64::MAX,
                ..StatusSnapshot::default()
            }],
            sumeragi: Vec::new(),
        },
        ThroughputSample {
            phase: Some("load\n".to_string()),
            timestamp_ms: 2,
            statuses: vec![StatusSnapshot {
                queue_size: u64::MAX - 1,
                ..StatusSnapshot::default()
            }],
            sumeragi: Vec::new(),
        },
        ThroughputSample {
            phase: Some("load\0".to_string()),
            timestamp_ms: 3,
            statuses: vec![StatusSnapshot {
                queue_size: u64::MAX - 2,
                ..StatusSnapshot::default()
            }],
            sumeragi: Vec::new(),
        },
        ThroughputSample {
            phase: Some("load".to_string()),
            timestamp_ms: 4,
            statuses: vec![StatusSnapshot {
                queue_size: 5,
                ..StatusSnapshot::default()
            }],
            sumeragi: Vec::new(),
        },
    ];

    assert_eq!(max_queue_size_for_phase(&samples, "load"), 5);
    assert_eq!(max_queue_size_for_phase(&samples, "load "), u64::MAX);
}

#[test]
fn write_throughput_artifacts_writes_realistic_summary_and_sample_phases() {
    let dir = tempdir().expect("tempdir");
    let artifact_root = dir.path().join("artifacts");
    let network_dir = dir.path().join("network");
    let status = StatusSnapshot {
        blocks: 5,
        blocks_non_empty: 4,
        queue_size: 3,
        txs_approved: 120,
        txs_rejected: 0,
        view_changes: 1,
        leader_index: Some(0),
        highest_qc_height: Some(4),
        locked_qc_height: Some(4),
        tx_queue_depth: Some(3),
        tx_queue_saturated: Some(false),
        block_created_dropped_by_lock_total: Some(0),
        block_created_hint_mismatch_total: Some(0),
        block_created_proposal_mismatch_total: Some(0),
        commit_signatures_present: Some(3),
        commit_signatures_required: Some(3),
    };
    let summary = ThroughputStatusSummary::from_statuses(std::slice::from_ref(&status));
    let artifacts = ThroughputArtifacts {
        realistic: Some(ThroughputArtifactRealistic {
            baseline_non_empty: 1,
            baseline_approved: 8,
            target_non_empty: 4,
            target_approved: 98,
            submitted: 90,
            load_sample_count: 1,
            load_elapsed_ms: 3_000,
            submit_elapsed_ms: 3_000,
            drain_elapsed_ms: 1_000,
            total_elapsed_ms: 4_000,
            load_end: summary.clone(),
            final_status: summary,
            load_end_produced_blocks: 3,
            produced_blocks: 3,
            load_submitted_tps: 30.0,
            load_committed_tps: 28.0,
            final_committed_tps: 22.5,
            load_avg_secs_per_block: 1.0,
            avg_secs_per_block: 1.333,
        }),
        samples: vec![ThroughputSample {
            phase: Some("load".to_string()),
            timestamp_ms: 42,
            statuses: vec![status],
            sumeragi: Vec::new(),
        }],
        ..ThroughputArtifacts::default()
    };
    let peer_logs = vec![PeerLogInfo {
        index: 0,
        mnemonic: "peer0".to_string(),
        stdout_log: None,
        stderr_log: None,
    }];

    let run_dir = write_throughput_artifacts(&artifact_root, &network_dir, &peer_logs, &artifacts)
        .expect("write artifacts");
    let summary_json = fs::read_to_string(run_dir.join("summary.json")).expect("read summary");
    let Value::Object(summary_map) =
        norito::json::from_json::<Value>(&summary_json).expect("parse summary")
    else {
        panic!("expected summary object");
    };
    let Some(Value::Object(realistic)) = summary_map.get("realistic") else {
        panic!("expected realistic summary");
    };
    assert_eq!(realistic.get("submitted"), Some(&Value::from(90_u64)));
    assert_eq!(
        realistic.get("load_end_produced_blocks"),
        Some(&Value::from(3_u64))
    );

    let samples_json =
        fs::read_to_string(run_dir.join("status_samples.json")).expect("read samples");
    let Value::Array(samples) =
        norito::json::from_json::<Value>(&samples_json).expect("parse samples")
    else {
        panic!("expected samples array");
    };
    let Some(Value::Object(sample)) = samples.first() else {
        panic!("expected first sample object");
    };
    assert_eq!(
        sample.get("phase"),
        Some(&Value::String("load".to_string()))
    );
    let Some(Value::Array(statuses)) = sample.get("status") else {
        panic!("expected status array");
    };
    let Some(Value::Object(status)) = statuses.first() else {
        panic!("expected status object");
    };
    assert_eq!(status.get("blocks_non_empty"), Some(&Value::from(4_u64)));
}

#[test]
fn throughput_payload_is_deterministic() {
    let payload = throughput_payload(7, 64, 123);
    let payload_repeat = throughput_payload(7, 64, 123);
    assert_eq!(payload, payload_repeat);
    assert_eq!(payload.len(), 64);
    assert!(payload.contains("localnet throughput 7"));
    let different = throughput_payload(8, 64, 123);
    assert_ne!(payload, different);
}

#[test]
fn metrics_url_handles_variants() {
    assert_eq!(
        metrics_url("http://127.0.0.1:8080"),
        "http://127.0.0.1:8080/metrics"
    );
    assert_eq!(
        metrics_url("http://127.0.0.1:8080/"),
        "http://127.0.0.1:8080/metrics"
    );
    assert_eq!(
        metrics_url("http://127.0.0.1:8080/metrics"),
        "http://127.0.0.1:8080/metrics"
    );
}

#[test]
fn parse_prom_histogram_extracts_quantiles() {
    let payload = r#"
# HELP commit_time_ms Average block commit time on this peer
# TYPE commit_time_ms histogram
commit_time_ms_bucket{le="5"} 1
commit_time_ms_bucket{le="10"} 3
commit_time_ms_bucket{le="+Inf"} 4
commit_time_ms_sum 27
commit_time_ms_count 4
"#;
    let hist = parse_prom_histogram(payload, "commit_time_ms");
    assert_eq!(hist.count, 4);
    assert_eq!(hist.buckets.len(), 3);
    let p50 = hist.quantile(0.5).expect("p50");
    assert!((p50 - 7.5).abs() < 0.25);
}

#[test]
fn aggregate_histograms_sums_counts() {
    let h1 = HistogramSnapshot {
        buckets: vec![(1.0, 2), (2.0, 3)],
        sum: 5.0,
        count: 3,
    };
    let h2 = HistogramSnapshot {
        buckets: vec![(1.0, 1), (2.0, 2)],
        sum: 4.0,
        count: 2,
    };
    let merged = aggregate_histograms([&h1, &h2]);
    assert_eq!(merged.count, 5);
    assert!((merged.sum - 9.0).abs() < f64::EPSILON);
    assert_eq!(merged.buckets.len(), 2);
}

#[test]
fn commit_time_quantiles_use_delta_histogram() {
    let warmup = PeerMetricsSnapshot {
        peer: "peer0".to_string(),
        payload: String::new(),
        commit_time_hist: HistogramSnapshot {
            buckets: vec![(5.0, 1), (10.0, 2), (f64::INFINITY, 2)],
            sum: 15.0,
            count: 2,
        },
    };
    let steady = PeerMetricsSnapshot {
        peer: "peer0".to_string(),
        payload: String::new(),
        commit_time_hist: HistogramSnapshot {
            buckets: vec![(5.0, 1), (10.0, 4), (f64::INFINITY, 4)],
            sum: 35.0,
            count: 4,
        },
    };
    let (p95, p99, count) = commit_time_quantiles(&[warmup], &[steady]);
    assert_eq!(count, 2);
    assert_eq!(p95, Some(10));
    assert_eq!(p99, Some(10));
}

#[test]
fn rate_summary_reports_avg_and_max() {
    let (avg, max) = rate_summary(&[0, 10], &[10, 40], Duration::from_secs(10));
    assert!((avg - 2.0).abs() < f64::EPSILON);
    assert!((max - 3.0).abs() < f64::EPSILON);
}

#[test]
fn config_fingerprint_changes_on_update() {
    let dir = tempdir().expect("tempdir");
    let config_path = dir.path().join("config.base.toml");
    fs::write(&config_path, "a = 1").expect("write config");
    let first = config_fingerprint(dir.path())
        .expect("fingerprint")
        .expect("fingerprint value");
    fs::write(&config_path, "a = 2").expect("write config");
    let second = config_fingerprint(dir.path())
        .expect("fingerprint")
        .expect("fingerprint value");
    assert_ne!(first, second);
}

#[test]
fn status_snapshot_value_handles_options() {
    let snapshot = StatusSnapshot {
        blocks: 1,
        blocks_non_empty: 1,
        queue_size: 2,
        txs_approved: 3,
        txs_rejected: 4,
        view_changes: 5,
        leader_index: None,
        highest_qc_height: Some(9),
        locked_qc_height: None,
        tx_queue_depth: Some(11),
        tx_queue_saturated: Some(true),
        block_created_dropped_by_lock_total: None,
        block_created_hint_mismatch_total: Some(13),
        block_created_proposal_mismatch_total: None,
        commit_signatures_present: Some(15),
        commit_signatures_required: None,
    };
    let value = status_snapshot_value(&snapshot);
    let Value::Object(map) = value else {
        panic!("expected object");
    };
    assert_eq!(map.get("blocks"), Some(&Value::from(1)));
    assert_eq!(map.get("leader_index"), Some(&Value::Null));
    assert_eq!(map.get("highest_qc_height"), Some(&Value::from(9)));
    assert_eq!(map.get("tx_queue_saturated"), Some(&Value::from(true)));
}

#[test]
fn sumeragi_snapshot_value_maps_fields() {
    let snapshot = SumeragiStatusSnapshot {
        view_change_install_total: 1,
        pacemaker_backpressure_deferrals_total: 2,
        tx_queue_depth: 3,
        tx_queue_capacity: 4,
        tx_queue_saturated: true,
        commit_qc_height: 5,
    };
    let value = sumeragi_snapshot_value(&snapshot);
    let Value::Object(map) = value else {
        panic!("expected object");
    };
    assert_eq!(map.get("commit_qc_height"), Some(&Value::from(5)));
    assert_eq!(map.get("tx_queue_saturated"), Some(&Value::from(true)));
}

async fn wait_for_converged_height(
    network: &Network,
    target_height: u64,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();
    let mut last_log = Instant::now()
        .checked_sub(STATUS_LOG_INTERVAL)
        .unwrap_or_else(Instant::now);
    loop {
        match collect_statuses(network, STATUS_POLL_TIMEOUT).await {
            Ok(statuses) => {
                let snapshot: Vec<StatusSnapshot> =
                    statuses.iter().map(StatusSnapshot::from_status).collect();
                if snapshot != last_snapshot || last_log.elapsed() >= STATUS_LOG_INTERVAL {
                    eprintln!(
                        "localnet status snapshot (target_height={target_height}): {snapshot:?}"
                    );
                    last_log = Instant::now();
                }
                last_snapshot = snapshot;
                if statuses.iter().all(|status| status.blocks >= target_height) {
                    let first_height = statuses.first().map(|s| s.blocks);
                    if statuses
                        .iter()
                        .all(|status| Some(status.blocks) == first_height)
                    {
                        return Ok(());
                    }
                }
                if Instant::now() >= deadline {
                    return Err(eyre!(
                        "heights failed to converge to {target_height} within {:?}: last_snapshot={last_snapshot:?}",
                        timeout
                    ));
                }
            }
            Err(err) => {
                if Instant::now() >= deadline {
                    return Err(eyre!(
                        "heights failed to converge to {target_height} within {:?}: last_snapshot={last_snapshot:?}, last_error={err:?}",
                        timeout
                    ));
                }
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
}

fn scale_duration(duration: Duration, factor: u64) -> Duration {
    let total_ms = duration.as_millis().saturating_mul(u128::from(factor));
    Duration::from_millis(u64::try_from(total_ms).unwrap_or(u64::MAX))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)]
struct StatusSnapshot {
    blocks: u64,
    blocks_non_empty: u64,
    queue_size: u64,
    txs_approved: u64,
    txs_rejected: u64,
    view_changes: u32,
    leader_index: Option<u64>,
    highest_qc_height: Option<u64>,
    locked_qc_height: Option<u64>,
    tx_queue_depth: Option<u64>,
    tx_queue_saturated: Option<bool>,
    block_created_dropped_by_lock_total: Option<u64>,
    block_created_hint_mismatch_total: Option<u64>,
    block_created_proposal_mismatch_total: Option<u64>,
    commit_signatures_present: Option<u64>,
    commit_signatures_required: Option<u64>,
}

impl StatusSnapshot {
    fn from_status(status: &iroha::client::Status) -> Self {
        let sumeragi = status.sumeragi.as_ref();
        Self {
            blocks: status.blocks,
            blocks_non_empty: status.blocks_non_empty,
            queue_size: status.queue_size,
            txs_approved: status.txs_approved,
            txs_rejected: status.txs_rejected,
            view_changes: status.view_changes,
            leader_index: sumeragi.map(|s| s.leader_index),
            highest_qc_height: sumeragi.map(|s| s.highest_qc_height),
            locked_qc_height: sumeragi.map(|s| s.locked_qc_height),
            tx_queue_depth: sumeragi.map(|s| s.tx_queue_depth),
            tx_queue_saturated: sumeragi.map(|s| s.tx_queue_saturated),
            block_created_dropped_by_lock_total: sumeragi
                .map(|s| s.block_created_dropped_by_lock_total),
            block_created_hint_mismatch_total: sumeragi
                .map(|s| s.block_created_hint_mismatch_total),
            block_created_proposal_mismatch_total: sumeragi
                .map(|s| s.block_created_proposal_mismatch_total),
            commit_signatures_present: sumeragi.map(|s| s.commit_signatures_present),
            commit_signatures_required: sumeragi.map(|s| s.commit_signatures_required),
        }
    }
}

#[derive(Debug)]
struct ThroughputSample {
    phase: Option<String>,
    timestamp_ms: u64,
    statuses: Vec<StatusSnapshot>,
    sumeragi: Vec<SumeragiStatusSnapshot>,
}

#[derive(Clone, Debug, Default)]
struct ThroughputStatusSummary {
    min_blocks: u64,
    max_blocks: u64,
    min_blocks_non_empty: u64,
    max_blocks_non_empty: u64,
    min_txs_approved: u64,
    max_txs_approved: u64,
    max_txs_rejected: u64,
    max_queue_size: u64,
}

impl ThroughputStatusSummary {
    fn from_statuses(statuses: &[StatusSnapshot]) -> Self {
        if statuses.is_empty() {
            return Self::default();
        }
        Self {
            min_blocks: statuses
                .iter()
                .map(|status| status.blocks)
                .min()
                .unwrap_or_default(),
            max_blocks: statuses
                .iter()
                .map(|status| status.blocks)
                .max()
                .unwrap_or_default(),
            min_blocks_non_empty: statuses
                .iter()
                .map(|status| status.blocks_non_empty)
                .min()
                .unwrap_or_default(),
            max_blocks_non_empty: statuses
                .iter()
                .map(|status| status.blocks_non_empty)
                .max()
                .unwrap_or_default(),
            min_txs_approved: statuses
                .iter()
                .map(|status| status.txs_approved)
                .min()
                .unwrap_or_default(),
            max_txs_approved: statuses
                .iter()
                .map(|status| status.txs_approved)
                .max()
                .unwrap_or_default(),
            max_txs_rejected: statuses
                .iter()
                .map(|status| status.txs_rejected)
                .max()
                .unwrap_or_default(),
            max_queue_size: statuses
                .iter()
                .map(|status| status.queue_size)
                .max()
                .unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SumeragiStatusSnapshot {
    view_change_install_total: u64,
    pacemaker_backpressure_deferrals_total: u64,
    tx_queue_depth: u64,
    tx_queue_capacity: u64,
    tx_queue_saturated: bool,
    commit_qc_height: u64,
}

impl SumeragiStatusSnapshot {
    fn from_status(status: &SumeragiStatusWire) -> Self {
        Self {
            view_change_install_total: status.view_change_install_total,
            pacemaker_backpressure_deferrals_total: status.pacemaker_backpressure_deferrals_total,
            tx_queue_depth: status.tx_queue_depth,
            tx_queue_capacity: status.tx_queue_capacity,
            tx_queue_saturated: status.tx_queue_saturated,
            commit_qc_height: status.commit_qc.height,
        }
    }
}

fn status_snapshot_value(snapshot: &StatusSnapshot) -> Value {
    let mut map = Map::new();
    let opt_u64 = |value: Option<u64>| value.map_or(Value::Null, Value::from);
    let opt_bool = |value: Option<bool>| value.map_or(Value::Null, Value::from);

    map.insert("blocks".to_string(), Value::from(snapshot.blocks));
    map.insert(
        "blocks_non_empty".to_string(),
        Value::from(snapshot.blocks_non_empty),
    );
    map.insert("queue_size".to_string(), Value::from(snapshot.queue_size));
    map.insert(
        "txs_approved".to_string(),
        Value::from(snapshot.txs_approved),
    );
    map.insert(
        "txs_rejected".to_string(),
        Value::from(snapshot.txs_rejected),
    );
    map.insert(
        "view_changes".to_string(),
        Value::from(u64::from(snapshot.view_changes)),
    );
    map.insert("leader_index".to_string(), opt_u64(snapshot.leader_index));
    map.insert(
        "highest_qc_height".to_string(),
        opt_u64(snapshot.highest_qc_height),
    );
    map.insert(
        "locked_qc_height".to_string(),
        opt_u64(snapshot.locked_qc_height),
    );
    map.insert(
        "tx_queue_depth".to_string(),
        opt_u64(snapshot.tx_queue_depth),
    );
    map.insert(
        "tx_queue_saturated".to_string(),
        opt_bool(snapshot.tx_queue_saturated),
    );
    map.insert(
        "block_created_dropped_by_lock_total".to_string(),
        opt_u64(snapshot.block_created_dropped_by_lock_total),
    );
    map.insert(
        "block_created_hint_mismatch_total".to_string(),
        opt_u64(snapshot.block_created_hint_mismatch_total),
    );
    map.insert(
        "block_created_proposal_mismatch_total".to_string(),
        opt_u64(snapshot.block_created_proposal_mismatch_total),
    );
    map.insert(
        "commit_signatures_present".to_string(),
        opt_u64(snapshot.commit_signatures_present),
    );
    map.insert(
        "commit_signatures_required".to_string(),
        opt_u64(snapshot.commit_signatures_required),
    );
    Value::Object(map)
}

fn throughput_status_summary_value(summary: &ThroughputStatusSummary) -> Value {
    let mut map = Map::new();
    map.insert("min_blocks".to_string(), Value::from(summary.min_blocks));
    map.insert("max_blocks".to_string(), Value::from(summary.max_blocks));
    map.insert(
        "min_blocks_non_empty".to_string(),
        Value::from(summary.min_blocks_non_empty),
    );
    map.insert(
        "max_blocks_non_empty".to_string(),
        Value::from(summary.max_blocks_non_empty),
    );
    map.insert(
        "min_txs_approved".to_string(),
        Value::from(summary.min_txs_approved),
    );
    map.insert(
        "max_txs_approved".to_string(),
        Value::from(summary.max_txs_approved),
    );
    map.insert(
        "max_txs_rejected".to_string(),
        Value::from(summary.max_txs_rejected),
    );
    map.insert(
        "max_queue_size".to_string(),
        Value::from(summary.max_queue_size),
    );
    Value::Object(map)
}

fn sumeragi_snapshot_value(snapshot: &SumeragiStatusSnapshot) -> Value {
    let mut map = Map::new();
    map.insert(
        "view_change_install_total".to_string(),
        Value::from(snapshot.view_change_install_total),
    );
    map.insert(
        "pacemaker_backpressure_deferrals_total".to_string(),
        Value::from(snapshot.pacemaker_backpressure_deferrals_total),
    );
    map.insert(
        "tx_queue_depth".to_string(),
        Value::from(snapshot.tx_queue_depth),
    );
    map.insert(
        "tx_queue_capacity".to_string(),
        Value::from(snapshot.tx_queue_capacity),
    );
    map.insert(
        "tx_queue_saturated".to_string(),
        Value::from(snapshot.tx_queue_saturated),
    );
    map.insert(
        "commit_qc_height".to_string(),
        Value::from(snapshot.commit_qc_height),
    );
    Value::Object(map)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PeerLogInfo {
    index: u64,
    mnemonic: String,
    stdout_log: Option<String>,
    stderr_log: Option<String>,
}

#[derive(Clone, Debug)]
struct ThroughputArtifactRecipe {
    peers: u64,
    block_time_ms: u64,
    commit_time_ms: u64,
    block_max_txs: u64,
    warmup_blocks: u64,
    steady_blocks: u64,
    total_blocks: u64,
    warmup_txs: u64,
    steady_txs: u64,
    total_txs: u64,
    submit_batch: u64,
    submit_parallelism: u64,
    queue_soft_limit: u64,
    payload_bytes: u64,
    load_kind: String,
    transfer_accounts: u64,
    transfer_initial_balance: u64,
    transfer_max_amount: u64,
    ram_lfe_email_accounts: u64,
    ram_lfe_email_policy: String,
    ram_lfe_program: String,
    rng_seed: u64,
    rbc_encoding: String,
    rbc_data_shards: u64,
    rbc_parity_shards: u64,
}

#[derive(Clone, Debug)]
struct ThroughputArtifactSlo {
    commit_p95_ms: u64,
    commit_p99_ms: u64,
    view_change_rate_max: f64,
    backpressure_rate_max: f64,
    queue_saturation_max: f64,
}

#[derive(Clone, Debug)]
struct ThroughputArtifactMetrics {
    submitted_tps: f64,
    committed_tps: f64,
    commit_p95_ms: u64,
    commit_p99_ms: u64,
    commit_hist_count: u64,
    commit_time_ms_min: u64,
    commit_time_ms_avg: u64,
    commit_time_ms_max: u64,
    view_change_rate_avg: f64,
    view_change_rate_max: f64,
    backpressure_rate_avg: f64,
    backpressure_rate_max: f64,
    queue_saturated_frac: f64,
    max_queue_depth: u64,
    steady_elapsed_ms: u64,
    warmup_submit_elapsed_ms: u64,
    steady_submit_elapsed_ms: u64,
}

#[derive(Clone, Debug)]
struct ThroughputArtifactRealistic {
    baseline_non_empty: u64,
    baseline_approved: u64,
    target_non_empty: u64,
    target_approved: u64,
    submitted: u64,
    load_sample_count: u64,
    load_elapsed_ms: u64,
    submit_elapsed_ms: u64,
    drain_elapsed_ms: u64,
    total_elapsed_ms: u64,
    load_end: ThroughputStatusSummary,
    final_status: ThroughputStatusSummary,
    load_end_produced_blocks: u64,
    produced_blocks: u64,
    load_submitted_tps: f64,
    load_committed_tps: f64,
    final_committed_tps: f64,
    load_avg_secs_per_block: f64,
    avg_secs_per_block: f64,
}

#[derive(Debug, Default)]
struct ThroughputArtifacts {
    recipe: Option<ThroughputArtifactRecipe>,
    slo: Option<ThroughputArtifactSlo>,
    metrics: Option<ThroughputArtifactMetrics>,
    realistic: Option<ThroughputArtifactRealistic>,
    warmup_metrics: Vec<PeerMetricsSnapshot>,
    after_metrics: Vec<PeerMetricsSnapshot>,
    samples: Vec<ThroughputSample>,
    error: Option<String>,
}

fn write_throughput_artifacts(
    artifact_root: &Path,
    network_dir: &Path,
    peer_logs: &[PeerLogInfo],
    artifacts: &ThroughputArtifacts,
) -> Result<PathBuf> {
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let run_dir = artifact_root.join(format!(
        "throughput-{}",
        u64::try_from(timestamp_ms).unwrap_or(u64::MAX)
    ));
    fs::create_dir_all(&run_dir).wrap_err("create throughput artifact dir")?;

    let metrics_dir = run_dir.join("metrics");
    fs::create_dir_all(&metrics_dir).wrap_err("create metrics dir")?;
    let logs_dir = run_dir.join("logs");
    fs::create_dir_all(&logs_dir).wrap_err("create logs dir")?;

    for snapshot in &artifacts.warmup_metrics {
        let path = metrics_dir.join(format!("{}-warmup.prom", snapshot.peer));
        fs::write(&path, &snapshot.payload)
            .wrap_err_with(|| format!("write warmup metrics {}", path.display()))?;
    }
    for snapshot in &artifacts.after_metrics {
        let path = metrics_dir.join(format!("{}-steady.prom", snapshot.peer));
        fs::write(&path, &snapshot.payload)
            .wrap_err_with(|| format!("write steady metrics {}", path.display()))?;
    }

    let status_samples_value = Value::Array(
        artifacts
            .samples
            .iter()
            .map(|sample| {
                let mut map = Map::new();
                if let Some(phase) = sample.phase.as_ref() {
                    map.insert("phase".to_string(), Value::String(phase.clone()));
                }
                map.insert("timestamp_ms".to_string(), Value::from(sample.timestamp_ms));
                map.insert(
                    "status".to_string(),
                    Value::Array(sample.statuses.iter().map(status_snapshot_value).collect()),
                );
                map.insert(
                    "sumeragi".to_string(),
                    Value::Array(
                        sample
                            .sumeragi
                            .iter()
                            .map(sumeragi_snapshot_value)
                            .collect(),
                    ),
                );
                Value::Object(map)
            })
            .collect(),
    );

    let status_path = run_dir.join("status_samples.json");
    let status_json = norito::json::to_json_pretty(&status_samples_value)
        .map_err(|err| eyre!(err.to_string()))?;
    fs::write(&status_path, status_json)
        .wrap_err_with(|| format!("write {}", status_path.display()))?;

    let config_fingerprint = config_fingerprint(network_dir)?;

    let mut summary = Map::new();
    summary.insert(
        "run_id".to_string(),
        Value::String(
            run_dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        ),
    );
    summary.insert(
        "timestamp_ms".to_string(),
        Value::from(u64::try_from(timestamp_ms).unwrap_or(u64::MAX)),
    );
    summary.insert(
        "network_dir".to_string(),
        Value::String(network_dir.to_string_lossy().to_string()),
    );
    summary.insert(
        "config_fingerprint".to_string(),
        config_fingerprint.map_or(Value::Null, Value::String),
    );
    if let Some(error) = artifacts.error.as_ref() {
        summary.insert("error".to_string(), Value::String(error.clone()));
    }

    if let Some(recipe) = artifacts.recipe.as_ref() {
        let mut recipe_map = Map::new();
        recipe_map.insert("peers".to_string(), Value::from(recipe.peers));
        recipe_map.insert(
            "block_time_ms".to_string(),
            Value::from(recipe.block_time_ms),
        );
        recipe_map.insert(
            "commit_time_ms".to_string(),
            Value::from(recipe.commit_time_ms),
        );
        recipe_map.insert(
            "block_max_txs".to_string(),
            Value::from(recipe.block_max_txs),
        );
        recipe_map.insert(
            "warmup_blocks".to_string(),
            Value::from(recipe.warmup_blocks),
        );
        recipe_map.insert(
            "steady_blocks".to_string(),
            Value::from(recipe.steady_blocks),
        );
        recipe_map.insert("total_blocks".to_string(), Value::from(recipe.total_blocks));
        recipe_map.insert("warmup_txs".to_string(), Value::from(recipe.warmup_txs));
        recipe_map.insert("steady_txs".to_string(), Value::from(recipe.steady_txs));
        recipe_map.insert("total_txs".to_string(), Value::from(recipe.total_txs));
        recipe_map.insert("submit_batch".to_string(), Value::from(recipe.submit_batch));
        recipe_map.insert(
            "submit_parallelism".to_string(),
            Value::from(recipe.submit_parallelism),
        );
        recipe_map.insert(
            "queue_soft_limit".to_string(),
            Value::from(recipe.queue_soft_limit),
        );
        recipe_map.insert(
            "payload_bytes".to_string(),
            Value::from(recipe.payload_bytes),
        );
        recipe_map.insert(
            "load_kind".to_string(),
            Value::from(recipe.load_kind.clone()),
        );
        recipe_map.insert(
            "transfer_accounts".to_string(),
            Value::from(recipe.transfer_accounts),
        );
        recipe_map.insert(
            "transfer_initial_balance".to_string(),
            Value::from(recipe.transfer_initial_balance),
        );
        recipe_map.insert(
            "transfer_max_amount".to_string(),
            Value::from(recipe.transfer_max_amount),
        );
        recipe_map.insert(
            "ram_lfe_email_accounts".to_string(),
            Value::from(recipe.ram_lfe_email_accounts),
        );
        recipe_map.insert(
            "ram_lfe_email_policy".to_string(),
            Value::from(recipe.ram_lfe_email_policy.clone()),
        );
        recipe_map.insert(
            "ram_lfe_program".to_string(),
            Value::from(recipe.ram_lfe_program.clone()),
        );
        recipe_map.insert("rng_seed".to_string(), Value::from(recipe.rng_seed));
        recipe_map.insert(
            "rbc_encoding".to_string(),
            Value::from(recipe.rbc_encoding.clone()),
        );
        recipe_map.insert(
            "rbc_data_shards".to_string(),
            Value::from(recipe.rbc_data_shards),
        );
        recipe_map.insert(
            "rbc_parity_shards".to_string(),
            Value::from(recipe.rbc_parity_shards),
        );
        summary.insert("recipe".to_string(), Value::Object(recipe_map));
    }

    if let Some(slo) = artifacts.slo.as_ref() {
        let mut slo_map = Map::new();
        slo_map.insert("commit_p95_ms".to_string(), Value::from(slo.commit_p95_ms));
        slo_map.insert("commit_p99_ms".to_string(), Value::from(slo.commit_p99_ms));
        slo_map.insert(
            "view_change_rate_max".to_string(),
            Value::from(slo.view_change_rate_max),
        );
        slo_map.insert(
            "backpressure_rate_max".to_string(),
            Value::from(slo.backpressure_rate_max),
        );
        slo_map.insert(
            "queue_saturation_max".to_string(),
            Value::from(slo.queue_saturation_max),
        );
        summary.insert("slo".to_string(), Value::Object(slo_map));
    }

    if let Some(metrics) = artifacts.metrics.as_ref() {
        let mut metrics_map = Map::new();
        metrics_map.insert(
            "submitted_tps".to_string(),
            Value::from(metrics.submitted_tps),
        );
        metrics_map.insert(
            "committed_tps".to_string(),
            Value::from(metrics.committed_tps),
        );
        metrics_map.insert(
            "commit_p95_ms".to_string(),
            Value::from(metrics.commit_p95_ms),
        );
        metrics_map.insert(
            "commit_p99_ms".to_string(),
            Value::from(metrics.commit_p99_ms),
        );
        metrics_map.insert(
            "commit_hist_count".to_string(),
            Value::from(metrics.commit_hist_count),
        );
        metrics_map.insert(
            "commit_time_ms_min".to_string(),
            Value::from(metrics.commit_time_ms_min),
        );
        metrics_map.insert(
            "commit_time_ms_avg".to_string(),
            Value::from(metrics.commit_time_ms_avg),
        );
        metrics_map.insert(
            "commit_time_ms_max".to_string(),
            Value::from(metrics.commit_time_ms_max),
        );
        metrics_map.insert(
            "view_change_rate_avg".to_string(),
            Value::from(metrics.view_change_rate_avg),
        );
        metrics_map.insert(
            "view_change_rate_max".to_string(),
            Value::from(metrics.view_change_rate_max),
        );
        metrics_map.insert(
            "backpressure_rate_avg".to_string(),
            Value::from(metrics.backpressure_rate_avg),
        );
        metrics_map.insert(
            "backpressure_rate_max".to_string(),
            Value::from(metrics.backpressure_rate_max),
        );
        metrics_map.insert(
            "queue_saturated_frac".to_string(),
            Value::from(metrics.queue_saturated_frac),
        );
        metrics_map.insert(
            "max_queue_depth".to_string(),
            Value::from(metrics.max_queue_depth),
        );
        metrics_map.insert(
            "steady_elapsed_ms".to_string(),
            Value::from(metrics.steady_elapsed_ms),
        );
        metrics_map.insert(
            "warmup_submit_elapsed_ms".to_string(),
            Value::from(metrics.warmup_submit_elapsed_ms),
        );
        metrics_map.insert(
            "steady_submit_elapsed_ms".to_string(),
            Value::from(metrics.steady_submit_elapsed_ms),
        );
        summary.insert("metrics".to_string(), Value::Object(metrics_map));
    }

    if let Some(realistic) = artifacts.realistic.as_ref() {
        let mut realistic_map = Map::new();
        realistic_map.insert(
            "baseline_non_empty".to_string(),
            Value::from(realistic.baseline_non_empty),
        );
        realistic_map.insert(
            "baseline_approved".to_string(),
            Value::from(realistic.baseline_approved),
        );
        realistic_map.insert(
            "target_non_empty".to_string(),
            Value::from(realistic.target_non_empty),
        );
        realistic_map.insert(
            "target_approved".to_string(),
            Value::from(realistic.target_approved),
        );
        realistic_map.insert("submitted".to_string(), Value::from(realistic.submitted));
        realistic_map.insert(
            "load_sample_count".to_string(),
            Value::from(realistic.load_sample_count),
        );
        realistic_map.insert(
            "load_elapsed_ms".to_string(),
            Value::from(realistic.load_elapsed_ms),
        );
        realistic_map.insert(
            "submit_elapsed_ms".to_string(),
            Value::from(realistic.submit_elapsed_ms),
        );
        realistic_map.insert(
            "drain_elapsed_ms".to_string(),
            Value::from(realistic.drain_elapsed_ms),
        );
        realistic_map.insert(
            "total_elapsed_ms".to_string(),
            Value::from(realistic.total_elapsed_ms),
        );
        realistic_map.insert(
            "load_end".to_string(),
            throughput_status_summary_value(&realistic.load_end),
        );
        realistic_map.insert(
            "final".to_string(),
            throughput_status_summary_value(&realistic.final_status),
        );
        realistic_map.insert(
            "load_end_produced_blocks".to_string(),
            Value::from(realistic.load_end_produced_blocks),
        );
        realistic_map.insert(
            "produced_blocks".to_string(),
            Value::from(realistic.produced_blocks),
        );
        realistic_map.insert(
            "load_submitted_tps".to_string(),
            Value::from(realistic.load_submitted_tps),
        );
        realistic_map.insert(
            "load_committed_tps".to_string(),
            Value::from(realistic.load_committed_tps),
        );
        realistic_map.insert(
            "final_committed_tps".to_string(),
            Value::from(realistic.final_committed_tps),
        );
        realistic_map.insert(
            "load_avg_secs_per_block".to_string(),
            Value::from(realistic.load_avg_secs_per_block),
        );
        realistic_map.insert(
            "avg_secs_per_block".to_string(),
            Value::from(realistic.avg_secs_per_block),
        );
        summary.insert("realistic".to_string(), Value::Object(realistic_map));
    }

    let peer_logs_value: Vec<Value> = peer_logs
        .iter()
        .map(|peer| {
            let mut map = Map::new();
            let copied_stdout = copy_peer_log_into_artifacts(&logs_dir, peer, "stdout");
            let copied_stderr = copy_peer_log_into_artifacts(&logs_dir, peer, "stderr");
            map.insert("index".to_string(), Value::from(peer.index));
            map.insert("mnemonic".to_string(), Value::String(peer.mnemonic.clone()));
            map.insert(
                "stdout_log".to_string(),
                copied_stdout.map_or(Value::Null, Value::String),
            );
            map.insert(
                "stderr_log".to_string(),
                copied_stderr.map_or(Value::Null, Value::String),
            );
            map.insert(
                "stdout_log_original".to_string(),
                peer.stdout_log
                    .as_ref()
                    .map_or(Value::Null, |path| Value::String(path.clone())),
            );
            map.insert(
                "stderr_log_original".to_string(),
                peer.stderr_log
                    .as_ref()
                    .map_or(Value::Null, |path| Value::String(path.clone())),
            );
            Value::Object(map)
        })
        .collect();
    summary.insert("peer_logs".to_string(), Value::Array(peer_logs_value));
    summary.insert(
        "status_samples_path".to_string(),
        Value::String(status_path.to_string_lossy().to_string()),
    );
    summary.insert(
        "metrics_dir".to_string(),
        Value::String(metrics_dir.to_string_lossy().to_string()),
    );

    let summary_value = Value::Object(summary);
    let summary_path = run_dir.join("summary.json");
    let summary_json =
        norito::json::to_json_pretty(&summary_value).map_err(|err| eyre!(err.to_string()))?;
    fs::write(&summary_path, summary_json)
        .wrap_err_with(|| format!("write {}", summary_path.display()))?;

    Ok(run_dir)
}

fn copy_peer_log_into_artifacts(
    logs_dir: &Path,
    peer: &PeerLogInfo,
    stream: &'static str,
) -> Option<String> {
    let source = match stream {
        "stdout" => peer.stdout_log.as_ref(),
        "stderr" => peer.stderr_log.as_ref(),
        _ => None,
    }?;
    let source_path = Path::new(source);
    if !source_path.is_file() {
        return Some(source.clone());
    }
    let file_name = peer_log_artifact_file_name(peer.index, &peer.mnemonic, stream);
    let dest = logs_dir.join(file_name);
    if let Err(err) = fs::copy(source_path, &dest) {
        eprintln!(
            "failed to copy peer {stream} log for {} from {} to {}: {err:?}",
            peer.mnemonic,
            source_path.display(),
            dest.display()
        );
        return Some(source.clone());
    }
    Some(dest.to_string_lossy().to_string())
}

fn peer_log_artifact_file_name(index: u64, mnemonic: &str, stream: &str) -> String {
    let mut sanitized: String = mnemonic
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        sanitized.push_str("peer");
    }
    format!("{index}-{sanitized}-{stream}.log")
}

#[derive(Clone, Debug, Default)]
struct HistogramSnapshot {
    buckets: Vec<(f64, u64)>,
    sum: f64,
    count: u64,
}

impl HistogramSnapshot {
    #[allow(clippy::float_cmp)]
    fn saturating_sub(&self, baseline: &Self) -> Self {
        let buckets = self
            .buckets
            .iter()
            .map(|(le, count)| {
                let base = baseline
                    .buckets
                    .iter()
                    .find_map(|(base_le, base_count)| (*base_le == *le).then_some(*base_count))
                    .unwrap_or(0);
                (*le, count.saturating_sub(base))
            })
            .collect();
        let sum = (self.sum - baseline.sum).max(0.0);
        let count = self.count.saturating_sub(baseline.count);
        Self {
            buckets,
            sum,
            count,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn quantile(&self, quantile: f64) -> Option<f64> {
        if !(0.0..=1.0).contains(&quantile) || self.count == 0 {
            return None;
        }
        let mut buckets = self.buckets.clone();
        buckets.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
        let target = (quantile * self.count as f64).ceil();
        let mut prev_count = 0_u64;
        let mut prev_le = 0.0;
        for (le, count) in buckets {
            if (count as f64) >= target {
                if le.is_infinite() {
                    return Some(prev_le);
                }
                let bucket_count = count.saturating_sub(prev_count);
                if bucket_count == 0 {
                    return Some(le);
                }
                let ratio = (target - prev_count as f64) / bucket_count as f64;
                return Some((le - prev_le).mul_add(ratio, prev_le));
            }
            prev_count = count;
            prev_le = le;
        }
        Some(prev_le)
    }
}

#[derive(Clone, Debug)]
struct PeerMetricsSnapshot {
    peer: String,
    payload: String,
    commit_time_hist: HistogramSnapshot,
}

#[allow(clippy::float_cmp, single_use_lifetimes)]
fn aggregate_histograms<'a>(
    histograms: impl IntoIterator<Item = &'a HistogramSnapshot>,
) -> HistogramSnapshot {
    let mut buckets: Vec<(f64, u64)> = Vec::new();
    let mut sum = 0.0;
    let mut count = 0_u64;
    for hist in histograms {
        sum += hist.sum;
        count = count.saturating_add(hist.count);
        for (le, bucket_count) in &hist.buckets {
            if let Some(entry) = buckets.iter_mut().find(|(bound, _)| *bound == *le) {
                entry.1 = entry.1.saturating_add(*bucket_count);
            } else {
                buckets.push((*le, *bucket_count));
            }
        }
    }
    buckets.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
    HistogramSnapshot {
        buckets,
        sum,
        count,
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp
)]
fn parse_prom_histogram(payload: &str, metric: &str) -> HistogramSnapshot {
    let mut buckets: Vec<(f64, u64)> = Vec::new();
    let mut sum = 0.0;
    let mut count = 0_u64;
    let bucket_prefix = format!("{metric}_bucket");
    let sum_prefix = format!("{metric}_sum");
    let count_prefix = format!("{metric}_count");

    for line in payload.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let name = match parts.next() {
            Some(name) => name,
            None => continue,
        };
        let value_raw = match parts.next() {
            Some(value) => value,
            None => continue,
        };
        if name.starts_with(&bucket_prefix) {
            let le = name.find("le=\"").and_then(|pos| {
                let rest = &name[pos + 4..];
                rest.find('"').and_then(|end| {
                    let value = &rest[..end];
                    if value == "+Inf" {
                        Some(f64::INFINITY)
                    } else {
                        value.parse::<f64>().ok()
                    }
                })
            });
            let le = match le {
                Some(value) => value,
                None => continue,
            };
            let value = value_raw
                .parse::<u64>()
                .ok()
                .or_else(|| value_raw.parse::<f64>().ok().map(|v| v.round() as u64));
            if let Some(value) = value {
                if let Some(entry) = buckets.iter_mut().find(|(bound, _)| *bound == le) {
                    entry.1 = value;
                } else {
                    buckets.push((le, value));
                }
            }
        } else if name.starts_with(&sum_prefix) {
            if let Ok(value) = value_raw.parse::<f64>() {
                sum = value;
            }
        } else if name.starts_with(&count_prefix) {
            if let Ok(value) = value_raw.parse::<f64>() {
                count = value.round() as u64;
            } else if let Ok(value) = value_raw.parse::<u64>() {
                count = value;
            }
        }
    }

    buckets.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
    HistogramSnapshot {
        buckets,
        sum,
        count,
    }
}

fn metrics_url(torii_url: &str) -> String {
    if torii_url.ends_with("/metrics") {
        torii_url.to_string()
    } else if torii_url.ends_with('/') {
        format!("{torii_url}metrics")
    } else {
        format!("{torii_url}/metrics")
    }
}

fn throughput_payload(index: u64, payload_bytes: usize, seed: u64) -> String {
    let prefix = format!("localnet throughput {index} ");
    let mut payload = String::with_capacity(payload_bytes.max(prefix.len()));
    payload.push_str(&prefix);
    if payload.len() < payload_bytes {
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ index);
        let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789";
        let remaining = payload_bytes - payload.len();
        for _ in 0..remaining {
            let idx = (rng.next_u32() as usize) % alphabet.len();
            payload.push(alphabet[idx] as char);
        }
    }
    if payload.len() > payload_bytes {
        payload.truncate(payload_bytes);
    }
    payload
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn commit_time_quantiles(
    warmup: &[PeerMetricsSnapshot],
    steady: &[PeerMetricsSnapshot],
) -> (Option<u64>, Option<u64>, u64) {
    if warmup.is_empty() && steady.is_empty() {
        return (None, None, 0);
    }
    let warmup_hist = aggregate_histograms(warmup.iter().map(|s| &s.commit_time_hist));
    let steady_hist = aggregate_histograms(steady.iter().map(|s| &s.commit_time_hist));
    let delta = steady_hist.saturating_sub(&warmup_hist);
    let p95 = delta.quantile(0.95).map(|v| v.round() as u64);
    let p99 = delta.quantile(0.99).map(|v| v.round() as u64);
    (p95, p99, delta.count)
}

#[allow(clippy::cast_precision_loss)]
fn rate_summary(start: &[u64], end: &[u64], elapsed: Duration) -> (f64, f64) {
    let count = start.len().min(end.len());
    let secs = elapsed.as_secs_f64();
    if count == 0 || secs <= 0.0 {
        return (0.0, 0.0);
    }
    let mut sum = 0.0;
    let mut max_rate = 0.0;
    for (start, end) in start.iter().zip(end.iter()).take(count) {
        let delta = end.saturating_sub(*start) as f64 / secs;
        sum += delta;
        if delta > max_rate {
            max_rate = delta;
        }
    }
    (sum / count as f64, max_rate)
}

fn rate_per_second(count: u64, elapsed: Duration) -> f64 {
    let secs = elapsed.as_secs_f64();
    if secs <= 0.0 {
        return 0.0;
    }
    count as f64 / secs
}

fn duration_millis_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn seconds_per_block(elapsed: Duration, blocks: u64) -> f64 {
    if blocks == 0 {
        return 0.0;
    }
    elapsed.as_secs_f64() / blocks as f64
}

fn realistic_target_blocks(
    configured_target_blocks: u64,
    total_txs: u64,
    block_max_txs: u64,
) -> u64 {
    if block_max_txs == 0 {
        return configured_target_blocks;
    }
    configured_target_blocks.max(total_txs.div_ceil(block_max_txs))
}

fn max_queue_size_for_phase(samples: &[ThroughputSample], phase: &str) -> u64 {
    samples
        .iter()
        .filter(|sample| sample.phase.as_deref() == Some(phase))
        .flat_map(|sample| sample.statuses.iter().map(|status| status.queue_size))
        .max()
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn realistic_artifact_summary(
    baseline_non_empty: u64,
    baseline_approved: u64,
    target_non_empty: u64,
    target_approved: u64,
    submitted: u64,
    submit_elapsed: Duration,
    load_elapsed: Duration,
    total_elapsed: Duration,
    load_end: ThroughputStatusSummary,
    final_status: ThroughputStatusSummary,
    load_end_produced_blocks: u64,
    produced_blocks: u64,
    samples: &[ThroughputSample],
) -> ThroughputArtifactRealistic {
    let drain_elapsed = total_elapsed.saturating_sub(load_elapsed);
    let load_committed = load_end.min_txs_approved.saturating_sub(baseline_approved);
    let final_committed = final_status
        .min_txs_approved
        .saturating_sub(baseline_approved);
    ThroughputArtifactRealistic {
        baseline_non_empty,
        baseline_approved,
        target_non_empty,
        target_approved,
        submitted,
        load_sample_count: samples
            .iter()
            .filter(|sample| sample.phase.as_deref() == Some("load"))
            .count() as u64,
        load_elapsed_ms: duration_millis_u64(load_elapsed),
        submit_elapsed_ms: duration_millis_u64(submit_elapsed),
        drain_elapsed_ms: duration_millis_u64(drain_elapsed),
        total_elapsed_ms: duration_millis_u64(total_elapsed),
        load_end,
        final_status,
        load_end_produced_blocks,
        produced_blocks,
        load_submitted_tps: rate_per_second(submitted, submit_elapsed),
        load_committed_tps: rate_per_second(load_committed, load_elapsed),
        final_committed_tps: rate_per_second(final_committed, total_elapsed),
        load_avg_secs_per_block: seconds_per_block(load_elapsed, load_end_produced_blocks),
        avg_secs_per_block: seconds_per_block(total_elapsed, produced_blocks),
    }
}

fn config_fingerprint(root: &Path) -> Result<Option<String>> {
    if !root.exists() {
        return Ok(None);
    }
    let mut paths = Vec::new();
    collect_config_paths(root, &mut paths);
    if paths.is_empty() {
        return Ok(None);
    }
    paths.sort();
    let mut hasher = Blake3Hasher::new();
    for path in paths {
        hasher.update(path.to_string_lossy().as_bytes());
        let contents = fs::read(&path).wrap_err_with(|| format!("read {}", path.display()))?;
        hasher.update(&contents);
    }
    Ok(Some(hasher.finalize().to_hex().to_string()))
}

fn collect_config_paths(root: &Path, output: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_config_paths(&path, output);
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name.contains("config")
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
        {
            output.push(path);
        }
    }
}
