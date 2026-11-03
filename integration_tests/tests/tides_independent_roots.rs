//! Eight real validators, two independent signed roots, direct IVM movement and
//! certified irreversible unit migration. Native validators deliver the receipt;
//! player clients issue no writes between schedule installation and final checks.
//! This is a unit transport prototype, not full-combat throughput qualification.

use eyre::{Result, ensure, eyre};
use integration_tests::sandbox;
use iroha::blocking::Client;
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::{Account, AccountId},
    asset::{AssetDefinitionId, AssetId},
    block::consensus::SumeragiRootScope,
    events::time::{ExecutionTime, Schedule, TimeEventFilter},
    game_partition::{GamePartitionStateV1, GameUnitImportProofV1, game_unit_id},
    isi::{InstructionBox, Register, Transfer, game_partition::*},
    prelude::{FindAssetById, Quantity},
    sumeragi_finality::{FinalityValidator, SumeragiFinalityVerifier},
    transaction::{Executable, FeePaymentIntent},
    trigger::{
        Trigger,
        action::{Action, Repeats},
    },
};
use iroha_model_base::metadata::Metadata;
use iroha_test_network::{Network, NetworkBuilder, read_on_dedicated_thread};
use std::{
    num::NonZeroU64,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const TIMEOUT: Duration = Duration::from_secs(240);

fn builder(label: &str, seed: u8) -> NetworkBuilder {
    NetworkBuilder::new()
        .with_base_seed(label)
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_npos_consensus()
        .with_block_cadence(Duration::from_secs(1))
        .with_genesis_keypair(KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        // The signed minimal genesis already grants Alice CanSetParameters.
        // A second grant is a genuine genesis Repetition rejection.
        .with_config_layer(|layer| {
            layer
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    1_073_741_824_i64,
                )
                .write(["logger", "level"], "INFO")
                .write(
                    ["logger", "filter"],
                    "info,irohad::sccp_keeper=debug,irohad::game_partition=debug,iroha_core::smartcontracts::isi::sccp=debug",
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
        })
}

async fn submit(client: &Client, instructions: Vec<InstructionBox>) -> Result<()> {
    read_on_dedicated_thread({
        let client = client.clone();
        move || client.submit_all(instructions, FeePaymentIntent::authority(Vec::new(), None))
    })
    .await?;
    Ok(())
}

async fn snapshot(
    http: &reqwest::Client,
    endpoint: &reqwest::Url,
    world: Hash,
) -> Result<GamePartitionStateV1> {
    let response = http
        .get(endpoint.join(&format!("v1/game-partitions/{world}/state"))?)
        .header("Accept", "application/json")
        .timeout(Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?;
    let result: GamePartitionStateV1 = norito::json::from_slice(&response.bytes().await?)?;
    result.validate().map_err(eyre::Report::msg)?;
    Ok(result)
}

async fn await_state(
    http: &reqwest::Client,
    client: &Client,
    world: Hash,
    predicate: impl Fn(&GamePartitionStateV1) -> bool,
) -> Result<GamePartitionStateV1> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let state = snapshot(http, client.client().endpoint(), world).await?;
        if predicate(&state) {
            return Ok(state);
        }
        ensure!(
            Instant::now() < deadline,
            "native independent-root state timed out: {state:?}"
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

fn scheduled(
    name: &str,
    instruction: InstructionBox,
    at: Duration,
    repeats: u32,
    attempts: u32,
    period: Option<Duration>,
) -> Result<InstructionBox> {
    let schedule = period.map_or_else(
        || Schedule::starting_at(at),
        |p| Schedule::starting_at(at).with_period(p),
    );
    let action = Action::new(
        Executable::from(vec![instruction]),
        Repeats::Exactly(repeats),
        iroha_test_samples::ALICE_ID.clone(),
        TimeEventFilter::new(ExecutionTime::Schedule(schedule)),
    )
    .map_err(|e| eyre!("schedule: {e}"))?;
    let mut metadata = Metadata::default();
    metadata.insert(
        "native_progress_attempts".parse()?,
        iroha_primitives::json::Json::new(attempts),
    );
    Ok(Register::trigger(Trigger::new(name.parse()?, action.with_metadata(metadata))).into())
}

async fn anchors(client: &Client) -> Result<(Vec<u8>, Vec<u8>)> {
    let scope = SumeragiRootScope::Global;
    let first = client
        .client()
        .get_sumeragi_finality_proof_async(NonZeroU64::new(1).unwrap(), scope)
        .await?;
    let second = client
        .client()
        .get_sumeragi_finality_proof_async(NonZeroU64::new(2).unwrap(), scope)
        .await?;
    Ok((first.block_wire, second.block_wire))
}

async fn verify_all(network: &Network) -> Result<()> {
    let genesis = network.genesis().0;
    let validators = iroha_genesis::signed_genesis_validator_pops(&genesis)?
        .into_iter()
        .map(|(public_key, proof_of_possession)| FinalityValidator {
            public_key,
            proof_of_possession,
        })
        .collect();
    let verifier =
        SumeragiFinalityVerifier::new(&genesis, network.chain_id().as_ref(), validators)?;
    let height = network.peers()[0]
        .client()
        .client()
        .status()
        .get()
        .await?
        .blocks;
    network.ensure_blocks(height).await?;
    let mut context = None;
    for peer in network.peers() {
        let result = read_on_dedicated_thread({
            let client = peer.client().client().clone();
            let mut verifier = verifier.clone();
            move || -> Result<_> {
                let mut tip = None;
                for h in 1..=height {
                    let proof = client.get_sumeragi_finality_proof(
                        NonZeroU64::new(h).unwrap(),
                        verifier.root_scope(),
                    )?;
                    tip = Some(verifier.verify(&proof)?);
                }
                Ok(tip
                    .ok_or_else(|| eyre!("missing native finality"))?
                    .context_id())
            }
        })
        .await?;
        if let Some(expected) = context {
            ensure!(expected == result, "validators disagree on native finality");
        }
        context = Some(result);
    }
    Ok(())
}

// TODO(`specs/runtime_upgrades.md`): game partitions are the release addition
// `iroha.game_partition.v1`, so both roots must activate it through a governed change set
// (Parliament enactment plus every validator's support attestation) before the first
// `InstallGamePartitionV1`. Restore this test once the test network can drive that activation.
#[test]
#[ignore = "needs governed activation of the iroha.game_partition.v1 addition on both roots"]
fn tides_independent_roots_native_restart() -> Result<()> {
    let permits = std::env::var("IROHA_TEST_NETWORK_PARALLELISM")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1);
    ensure!(
        permits >= 2 && std::env::var_os("IROHA_TEST_SERIALIZE_NETWORKS").is_none(),
        "eight-validator independent-root test requires IROHA_TEST_NETWORK_PARALLELISM=2, IROHA_TEST_SERIALIZE_NETWORKS unset, an isolated permit directory and --test-threads=1"
    );
    let worker = std::thread::Builder::new()
        .name("tides-independent-roots".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let _guard = sandbox::serial_guard();
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .thread_stack_size(64 * 1024 * 1024)
                .enable_all()
                .build()
                .unwrap()
                .block_on(run())
        })?;
    match worker.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

async fn run() -> Result<()> {
    let a = builder("tides-independent-source", 91).build();
    let b = builder("tides-independent-destination", 92).build();
    a.start_all().await?;
    b.start_all().await?;
    ensure!(
        a.peers().len() == 4 && b.peers().len() == 4,
        "eight actual validators required"
    );
    ensure!(
        a.network_id() != b.network_id(),
        "test roots must be independently signed"
    );
    let ca = a.peers()[0].client();
    let cb = b.peers()[0].client();
    let http = integration_tests::http::client();
    let world = Hash::new(b"tides-independent-world-v1");
    for client in [&ca, &cb] {
        submit(
            client,
            vec![
                InstallGamePartitionV1 {
                    world_id: world,
                    kernel_hash: ivm::game_unit_step::kernel_hash(),
                }
                .into(),
            ],
        )
        .await?;
    }
    // The fixture's normal NPoS bootstrap funds each peer's Ed25519 streaming
    // account, not its BLS consensus identity. Native delivery signs with that
    // existing consensus key and pays ordinary fees, so provision its exact
    // account explicitly before every player client stops writing.
    let fee_asset: AssetDefinitionId =
        iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?;
    for network in [&a, &b] {
        let mut funding = Vec::new();
        for peer in network.peers() {
            let validator = AccountId::new(peer.public_key().clone());
            ensure!(
                validator != peer.account_id(),
                "fixture must distinguish consensus and streaming accounts"
            );
            funding.push(Register::account(Account::new(validator.clone())).into());
            funding.push(
                Transfer::asset_quantity(
                    AssetId::new(fee_asset.clone(), iroha_test_samples::ALICE_ID.clone()),
                    1_u32,
                    validator,
                )
                .into(),
            );
        }
        submit(&network.peers()[0].client(), funding).await?;
        let funding_height = network.peers()[0]
            .client()
            .client()
            .status()
            .get()
            .await?
            .blocks;
        network.ensure_blocks(funding_height).await?;
        let buckets = network
            .peers()
            .iter()
            .map(|peer| AssetId::new(fee_asset.clone(), AccountId::new(peer.public_key().clone())))
            .collect::<Vec<_>>();
        for peer in network.peers() {
            read_on_dedicated_thread({
                let client = peer.client();
                let buckets = buckets.clone();
                move || -> Result<()> {
                    for bucket in buckets {
                        let balance = client.client().query_single(FindAssetById::new(bucket))?;
                        ensure!(balance.value() == &Quantity::from(1_u32),
                            "native consensus signer must hold exactly one fee unit before clients stop");
                    }
                    Ok(())
                }
            }).await?;
        }
    }
    eprintln!(
        "TIDES_INDEPENDENT_NATIVE_FEE_PREFUND validators=8 verified_balance_reads=32 units_per_validator=1 ordinary_fee_payment=true player_writes_after_start=0"
    );
    // Installation provides the real H2 needed to authenticate original genesis execution.
    a.ensure_blocks(2).await?;
    b.ensure_blocks(2).await?;
    let (ga, ha) = anchors(&ca).await?;
    let (gb, hb) = anchors(&cb).await?;
    submit(
        &ca,
        vec![
            RegisterGamePartitionPeerV1 {
                world_id: world,
                chain_id: b.chain_id().clone(),
                network_id: b.network_id(),
                genesis: gb,
                successor: hb,
                endpoints: b.peers().iter().map(|p| p.torii_url()).collect(),
            }
            .into(),
        ],
    )
    .await?;
    submit(
        &cb,
        vec![
            RegisterGamePartitionPeerV1 {
                world_id: world,
                chain_id: a.chain_id().clone(),
                network_id: a.network_id(),
                genesis: ga,
                successor: ha,
                endpoints: a.peers().iter().map(|p| p.torii_url()).collect(),
            }
            .into(),
        ],
    )
    .await?;
    let sa = snapshot(&http, ca.client().endpoint(), world).await?;
    let sb = snapshot(&http, cb.client().endpoint(), world).await?;
    ensure!(
        sa.local_instance != sb.local_instance,
        "independent execution identities required"
    );
    let unit = game_unit_id(world, sa.local_instance, 1);
    submit(
        &ca,
        vec![
            SeedGameUnitV1 {
                world_id: world,
                nonce: 1,
                owner: iroha_test_samples::ALICE_ID.clone(),
                hp: 73,
                x: 0,
                work_remaining: 24,
            }
            .into(),
        ],
    )
    .await?;
    let before = snapshot(&http, ca.client().endpoint(), world).await?;
    // The second export is impossible. The first must roll back with the entire transaction.
    let rejected = submit(
        &ca,
        vec![
            ExportGameUnitV1 {
                world_id: world,
                unit_id: unit,
                destination: sb.local_instance,
            }
            .into(),
            ExportGameUnitV1 {
                world_id: world,
                unit_id: unit,
                destination: sb.local_instance,
            }
            .into(),
        ],
    )
    .await;
    let refusal = rejected
        .err()
        .ok_or_else(|| eyre!("double export unexpectedly committed"))?;
    ensure!(
        format!("{refusal:?}").contains("game unit is absent or exported"),
        "transport, fee or unrelated refusal is not an atomic double-export test: {refusal:?}"
    );
    ensure!(
        snapshot(&http, ca.client().endpoint(), world).await? == before,
        "rejected export leaked a debit or receipt"
    );
    let start = SystemTime::now().duration_since(UNIX_EPOCH)? + Duration::from_secs(45);
    submit(
        &ca,
        vec![
            scheduled(
                "independent_source_00_move",
                AdvanceGameUnitV1 {
                    world_id: world,
                    unit_id: unit,
                }
                .into(),
                start,
                8,
                8,
                Some(Duration::from_secs(2)),
            )?,
            scheduled(
                "independent_source_01_export",
                ExportGameUnitV1 {
                    world_id: world,
                    unit_id: unit,
                    destination: sb.local_instance,
                }
                .into(),
                start + Duration::from_secs(20),
                1,
                1,
                None,
            )?,
        ],
    )
    .await?;
    submit(
        &cb,
        vec![scheduled(
            "independent_destination_move",
            AdvanceGameUnitV1 {
                world_id: world,
                unit_id: unit,
            }
            .into(),
            start + Duration::from_secs(50),
            16,
            64,
            Some(Duration::from_secs(2)),
        )?],
    )
    .await?;
    let off = Instant::now();
    println!(
        "TIDES_INDEPENDENT_CLIENTS_OFF source={} destination={} execution_proofs=unavailable validators=8",
        a.network_id(),
        b.network_id()
    );
    let moving = await_state(&http, &ca, world, |s| {
        s.units.first().is_some_and(|u| u.x >= 2)
    })
    .await?;
    ensure!(moving.exports.is_empty(), "export happened before outage");
    let peer = b.peers()[3].clone();
    ensure!(
        peer.shutdown_if_started().await,
        "destination restart peer was not running"
    );
    let dark_before = snapshot(&http, ca.client().endpoint(), world).await?;
    let dark_before_x = dark_before
        .units
        .first()
        .map(|u| u.x)
        .or_else(|| dark_before.exports.first().map(|r| r.unit.x))
        .unwrap_or(-1);
    println!(
        "TIDES_INDEPENDENT_OBSERVER_DARK_BEGIN duration_ms=10000 source_x={dark_before_x} source_exports={} player_writes=0",
        dark_before.exports.len()
    );
    // Native validator-to-validator discovery continues; all test-client queries stop.
    tokio::time::sleep(Duration::from_secs(10)).await;
    let dark_after = snapshot(&http, ca.client().endpoint(), world).await?;
    let dark_after_x = dark_after
        .units
        .first()
        .map(|u| u.x)
        .or_else(|| dark_after.exports.first().map(|r| r.unit.x))
        .unwrap_or(-1);
    println!(
        "TIDES_INDEPENDENT_OBSERVER_DARK_END source_x={dark_after_x} source_exports={} position_delta={}",
        dark_after.exports.len(),
        dark_after_x - dark_before_x
    );
    let imported = await_state(&http, &cb, world, |s| s.imported.len() == 1).await?;
    ensure!(
        imported.units.len() == 1
            && (8..=24).contains(&imported.units[0].x)
            && imported.units[0].hp == 73
            && imported.units[0].x as u32 + imported.units[0].work_remaining == 24,
        "certified import altered source state"
    );
    let source = await_state(&http, &ca, world, |s| s.exports.len() == 1).await?;
    ensure!(
        source.units.is_empty(),
        "source retained active exported unit"
    );
    ensure!(
        source.exports[0].unit.x == 8
            && source.exports[0].unit.hp == 73
            && source.exports[0].unit.work_remaining == 16
            && source.exports[0].unit.sequence == 1,
        "certified source export differs from the eight-step IVM oracle"
    );
    tokio::time::timeout(
        b.peer_startup_timeout(),
        peer.start_checked(b.config_layers_for_peer(&peer), None),
    )
    .await
    .map_err(|_| eyre!("destination validator restart timed out"))??;
    let done = await_state(&http, &cb, world, |s| {
        s.units.first().is_some_and(|u| u.work_remaining == 0)
    })
    .await?;
    ensure!(
        done.units.len() == 1
            && done.units[0].x == 24
            && done.units[0].hp == 73
            && done.units[0].sequence == 1
            && done.imported.len() == 1,
        "independent native IVM movement/conservation failed"
    );
    println!(
        "TIDES_INDEPENDENT_CLIENTS_OFF_COMPLETE elapsed_ms={} position={} hp={} imports={}",
        off.elapsed().as_millis(),
        done.units[0].x,
        done.units[0].hp,
        done.imported.len()
    );
    for (network, expected) in [(&a, &source), (&b, &done)] {
        for peer in network.peers() {
            let actual = await_state(&http, &peer.client(), world, |s| s == expected).await?;
            ensure!(&actual == expected, "restart state mismatch");
        }
        verify_all(network).await?;
    }
    let receipt = source.exports[0].receipt_id();
    let proof: GameUnitImportProofV1 = norito::json::from_slice(
        &http
            .get(ca.client().endpoint().join(&format!(
                "v1/game-partitions/{world}/exports/{receipt}/proof"
            ))?)
            .header("Accept", "application/json")
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?,
    )?;
    // Explicit post-observation replay is not a client progression input.
    submit(
        &cb,
        vec![
            ImportGameUnitV1 {
                proof: proof.clone(),
            }
            .into(),
        ],
    )
    .await?;
    ensure!(
        snapshot(&http, cb.client().endpoint(), world).await? == done,
        "duplicate import changed native state"
    );
    b.shutdown().await;
    b.start_all().await?;
    await_state(&http, &cb, world, |s| s == &done).await?;
    submit(&cb, vec![ImportGameUnitV1 { proof }.into()]).await?;
    ensure!(
        snapshot(&http, cb.client().endpoint(), world).await? == done,
        "cold restart lost import dedupe"
    );
    verify_all(&b).await?;
    a.shutdown().await;
    b.shutdown().await;
    Ok(())
}
