#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration coverage for commit certificates in permissioned and `NPoS` modes.

use std::time::{Duration, Instant};

use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::{metrics::MetricsReader, sandbox};
use iroha::crypto::{Algorithm, KeyPair};
use iroha::data_model::{
    Level,
    account::Account,
    asset::{AssetDefinition, AssetDefinitionId, id::AssetId},
    consensus::Qc,
    domain::Domain,
    domain::DomainId,
    isi::{
        Log, Mint, Register,
        staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
    },
    metadata::Metadata,
    nexus::LaneId,
    peer::PeerId,
    prelude::*,
};
use iroha_core::sumeragi::{consensus::qc_signer_count, network_topology::commit_quorum_from_len};
use iroha_primitives::numeric::{Numeric, NumericSpec};
use iroha_test_network::{
    NetworkBuilder, NetworkPeer, genesis_factory_with_post_topology, init_instruction_registry,
};
use iroha_test_samples::ALICE_ID;
use norito::json;
use tokio::time::{sleep, timeout};
use toml::Table;

const COMMIT_CERT_TIMEOUT: Duration = Duration::from_secs(120);
const COMMIT_CERT_POLL: Duration = Duration::from_millis(200);
const HIGH_STAKE: u64 = 7_000;
const LOW_STAKE: u64 = 1_000;
const NEXUS_FEE_SEED_AMOUNT: u32 = 1_000_000;
const STAKE_QUORUM_WAIT: Duration = Duration::from_secs(5);

type CommitCertificate = Qc;

fn validator_account_id_for_index(index: usize) -> AccountId {
    let key_pair = KeyPair::try_from_seed(
        format!("integration_tests::sumeragi_commit_certificates::{index}").into_bytes(),
        Algorithm::Ed25519,
    )
    .expect("fixture Sumeragi commit-certificate validator key");
    AccountId::new(key_pair.public_key().clone())
}

#[test]
fn validator_account_id_for_index_uses_checked_seed_derivation() {
    let expected_key_pair = KeyPair::try_from_seed(
        b"integration_tests::sumeragi_commit_certificates::0".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("fixture Sumeragi commit-certificate validator key");

    assert_eq!(
        validator_account_id_for_index(0),
        AccountId::new(expected_key_pair.public_key().clone()),
    );
}

fn stake_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("nexus", "universal").expect("nexus domain"),
        "xor".parse().expect("stake asset name"),
    )
}

fn stake_asset_id_literal() -> String {
    stake_asset_definition_id().to_string()
}

fn nexus_fee_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("universal", "universal").expect("fee asset domain"),
        "xor".parse().expect("fee asset name"),
    )
}

fn nexus_fee_asset_id_literal() -> String {
    nexus_fee_asset_definition_id().to_string()
}

fn npos_commit_quorum_network_builder() -> NetworkBuilder {
    NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_npos_consensus()
        .with_config_layer(|layer| {
            let gas_account_str = ALICE_ID.to_string();
            layer
                .write(["nexus", "enabled"], true)
                .write(
                    ["nexus", "fees", "fee_asset_id"],
                    nexus_fee_asset_id_literal(),
                )
                .write(
                    ["nexus", "staking", "stake_asset_id"],
                    stake_asset_id_literal(),
                )
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    gas_account_str.clone(),
                )
                .write(
                    ["nexus", "staking", "slash_sink_account_id"],
                    gas_account_str,
                )
                .write(["sumeragi", "collectors", "k"], 1_i64)
                .write(["sumeragi", "collectors", "redundant_send_r"], 1_i64);
        })
        // The test provides its own staking bootstrap and stake distribution.
        .without_npos_genesis_bootstrap()
        .with_genesis_block(|topology, topology_entries| {
            let post_topology = stake_genesis_post_topology_transactions(topology.as_ref());
            genesis_factory_with_post_topology(
                Vec::new(),
                post_topology,
                topology,
                topology_entries,
            )
        })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn permissioned_commit_certificates_reach_quorum() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_permissioned_consensus()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full");
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(permissioned_commit_certificates_reach_quorum),
    )
    .await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        let client = network.client();
        let baseline = client.get_status()?.blocks_non_empty;
        client.submit_blocking(Log::new(Level::INFO, "commit cert quorum".to_string()))?;
        let status =
            wait_for_non_empty_blocks(&client, baseline.saturating_add(1), COMMIT_CERT_TIMEOUT)
                .await?
                .ok_or_else(|| eyre!("timed out waiting for non-empty commit certificate block"))?;
        let expected_height = status.blocks;
        let required = commit_quorum_from_len(network.peers().len());
        let http = integration_tests::http::client();
        let torii_urls = network.torii_urls();
        let metrics_urls: Vec<_> = network
            .torii_urls()
            .iter()
            .map(|torii| {
                reqwest::Url::parse(torii)
                    .and_then(|url| url.join("metrics"))
                    .wrap_err_with(|| format!("compose metrics URL for {torii}"))
            })
            .collect::<Result<_>>()?;
        wait_for_commit_certificate_quorum(
            &http,
            &torii_urls,
            expected_height,
            required,
            network.peers().len(),
        )
        .await?;
        wait_for_commit_quorum_status(network.peers(), expected_height, required).await?;
        wait_for_commit_vote_metrics(&http, &metrics_urls, required).await?;
        Ok(())
    }
    .await;

    network.shutdown().await;
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn commit_certificate_block_sync_restores_restart_peer() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_permissioned_consensus();

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(commit_certificate_block_sync_restores_restart_peer),
    )
    .await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        let peers = network.peers();
        let restart_peer = peers
            .get(1)
            .cloned()
            .ok_or_else(|| eyre!("expected at least 2 peers"))?;
        let submit_peer = peers
            .first()
            .cloned()
            .ok_or_else(|| eyre!("expected at least 1 peer"))?;
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|layer| ConfigLayer(layer.into_owned()))
            .collect();

        restart_peer.shutdown().await;

        let client = submit_peer.client();
        let baseline = client.get_status()?.blocks_non_empty;
        client.submit_blocking(Log::new(Level::INFO, "block sync commit cert".to_string()))?;
        let status =
            wait_for_non_empty_blocks(&client, baseline.saturating_add(1), COMMIT_CERT_TIMEOUT)
                .await?
                .ok_or_else(|| {
                    eyre!("timed out waiting for non-empty block before peer restart")
                })?;
        let expected_height = status.blocks;

        restart_peer
            .start_checked(config_layers.iter(), None)
            .await
            .wrap_err("restart peer for block sync")?;
        timeout(
            network.sync_timeout(),
            restart_peer.once_block(expected_height),
        )
        .await
        .map_err(|_| {
            eyre!(
                "restart peer failed to reach height {expected_height} within {:?}",
                network.sync_timeout()
            )
        })?;

        let http = integration_tests::http::client();
        let restart_torii = restart_peer.torii_url();
        let cert =
            wait_for_commit_certificate(&http, restart_torii.as_str(), expected_height).await?;
        ensure!(
            cert.validator_set.len() == peers.len(),
            "commit certificate validator set length mismatch: expected {}, got {}",
            peers.len(),
            cert.validator_set.len()
        );
        let signer_count = commit_certificate_signer_count(&cert);
        ensure!(
            signer_count >= commit_quorum_from_len(peers.len()),
            "commit certificate signature quorum too small: expected >= {}, got {}",
            commit_quorum_from_len(peers.len()),
            signer_count
        );
        Ok(())
    }
    .await;

    network.shutdown().await;
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn npos_commit_quorum_requires_stake() -> Result<()> {
    init_instruction_registry();

    let Some(network) = sandbox::start_network_async_or_skip(
        npos_commit_quorum_network_builder(),
        stringify!(npos_commit_quorum_requires_stake),
    )
    .await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        let peers = network.peers();
        let high_stake_peer = peers
            .first()
            .cloned()
            .ok_or_else(|| eyre!("expected at least 1 peer"))?;
        let submit_peer = peers
            .get(1)
            .cloned()
            .ok_or_else(|| eyre!("expected at least 2 peers"))?;

        high_stake_peer.shutdown().await;

        let client = submit_peer.client();
        let baseline = client.get_status()?.blocks_non_empty;
        client.submit(Log::new(Level::INFO, "npos stake quorum".to_string()))?;

        let blocked = wait_for_non_empty_blocks(&client, baseline + 1, STAKE_QUORUM_WAIT).await?;
        ensure!(
            blocked.is_none(),
            "non-empty block committed without stake quorum"
        );
        Ok(())
    }
    .await;

    network.shutdown().await;
    drop(network);
    result?;

    let Some(network) = sandbox::start_network_async_or_skip(
        npos_commit_quorum_network_builder(),
        stringify!(npos_commit_quorum_requires_stake),
    )
    .await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        let high_stake_peer = network
            .peers()
            .first()
            .cloned()
            .ok_or_else(|| eyre!("expected at least 1 peer"))?;
        let client = network.client();
        let baseline = client.get_status()?.blocks_non_empty;
        client.submit_blocking(Log::new(
            Level::INFO,
            "npos stake quorum restored".to_string(),
        ))?;
        let status =
            wait_for_non_empty_blocks(&client, baseline.saturating_add(1), COMMIT_CERT_TIMEOUT)
                .await?
                .ok_or_else(|| {
                    eyre!("timed out waiting for NPoS quorum with high-stake validator online")
                })?;
        ensure!(
            status.blocks_non_empty >= baseline.saturating_add(1),
            "expected NPoS quorum to commit with high-stake validator online"
        );
        let expected_height = status.blocks;

        let http = integration_tests::http::client();
        let submit_torii = network
            .torii_urls()
            .into_iter()
            .next()
            .ok_or_else(|| eyre!("expected at least one Torii URL"))?;
        let cert =
            wait_for_commit_certificate(&http, submit_torii.as_str(), expected_height).await?;
        let high_stake_id = high_stake_peer.id();
        let high_index = cert
            .validator_set
            .iter()
            .position(|peer| peer == &high_stake_id)
            .ok_or_else(|| eyre!("high-stake peer missing from validator set"))?;
        ensure!(
            commit_certificate_has_signer(&cert, high_index),
            "commit certificate missing high-stake signature"
        );
        Ok(())
    }
    .await;

    network.shutdown().await;
    result
}

#[derive(Clone)]
struct ConfigLayer(Table);

impl AsRef<Table> for ConfigLayer {
    fn as_ref(&self) -> &Table {
        &self.0
    }
}

fn commit_certificate_signer_count(cert: &CommitCertificate) -> usize {
    qc_signer_count(cert)
}

fn commit_certificate_has_signer(cert: &CommitCertificate, index: usize) -> bool {
    let byte_idx = index / 8;
    let bit_idx = index % 8;
    cert.aggregate
        .signers_bitmap
        .get(byte_idx)
        .is_some_and(|byte| (byte >> bit_idx) & 1 == 1)
}

async fn wait_for_commit_certificate_quorum(
    http: &reqwest::Client,
    torii_urls: &[String],
    expected_height: u64,
    required: usize,
    validator_len: usize,
) -> Result<()> {
    let deadline = Instant::now() + COMMIT_CERT_TIMEOUT;
    let mut missing = Vec::new();

    loop {
        missing.clear();
        for torii in torii_urls {
            match fetch_commit_certificates(http, torii, Some(expected_height), Some(1)).await {
                Ok(certificates) => {
                    let Some(cert) = certificates
                        .iter()
                        .find(|cert| cert.height == expected_height)
                    else {
                        missing.push(format!("{torii} missing height {expected_height}"));
                        continue;
                    };
                    if cert.validator_set.len() != validator_len {
                        missing.push(format!(
                            "{torii} validator set len {} != {validator_len}",
                            cert.validator_set.len()
                        ));
                    }
                    let signer_count = commit_certificate_signer_count(cert);
                    if signer_count < required {
                        missing.push(format!("{torii} signatures {signer_count} < {required}"));
                    }
                }
                Err(err) => {
                    missing.push(format!("{torii} error: {err:?}"));
                }
            }
        }
        if missing.is_empty() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for commit certificates at height {expected_height}; missing={missing:?}"
            ));
        }
        sleep(COMMIT_CERT_POLL).await;
    }
}

async fn wait_for_commit_certificate(
    http: &reqwest::Client,
    torii: &str,
    expected_height: u64,
) -> Result<CommitCertificate> {
    let deadline = Instant::now() + COMMIT_CERT_TIMEOUT;
    loop {
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for commit certificate at height {expected_height} from {torii}"
            ));
        }
        if let Ok(certificates) =
            fetch_commit_certificates(http, torii, Some(expected_height), Some(1)).await
            && let Some(cert) = certificates
                .into_iter()
                .find(|cert| cert.height == expected_height)
        {
            return Ok(cert);
        }
        sleep(COMMIT_CERT_POLL).await;
    }
}

async fn fetch_commit_certificates(
    http: &reqwest::Client,
    torii: &str,
    from: Option<u64>,
    limit: Option<u64>,
) -> Result<Vec<CommitCertificate>> {
    let base = reqwest::Url::parse(torii).wrap_err_with(|| format!("parse torii url {torii}"))?;
    let mut url = base
        .join("v1/sumeragi/commit-certificates")
        .wrap_err_with(|| format!("compose commit certificates URL for {torii}"))?;
    {
        let mut pairs = url.query_pairs_mut();
        if let Some(from) = from {
            pairs.append_pair("from", &from.to_string());
        }
        if let Some(limit) = limit {
            pairs.append_pair("limit", &limit.to_string());
        }
    }
    let response = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .wrap_err("fetch commit certificates")?;
    ensure!(
        response.status().is_success(),
        "commit certificates response {}",
        response.status()
    );
    let body = response.text().await.wrap_err("commit certificates body")?;
    json::from_str(&body).wrap_err("parse commit certificates JSON")
}

async fn wait_for_commit_quorum_status(
    peers: &[NetworkPeer],
    expected_height: u64,
    required: usize,
) -> Result<()> {
    let deadline = Instant::now() + COMMIT_CERT_TIMEOUT;
    let required_u32 = u32::try_from(required)
        .map_err(|_| eyre!("required commit quorum does not fit the v2 status wire type"))?;
    let mut last = Vec::new();
    loop {
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for commit quorum status at height {expected_height}; last={last:?}"
            ));
        }
        last.clear();
        for (idx, peer) in peers.iter().enumerate() {
            match peer.client().get_sumeragi_v2_status() {
                Ok(status) => {
                    let quorum = status.authoritative.last_commit_qc;
                    last.push(format!("peer#{idx}({}): {quorum:?}", peer.mnemonic()));
                    if let Some(quorum) = quorum
                        && quorum.certificate.round.height >= expected_height
                        && quorum.min_signers >= required_u32
                        && quorum.signer_count >= required_u32
                        && quorum.has_quorum()
                    {
                        return Ok(());
                    }
                }
                Err(err) => {
                    last.push(format!("peer#{idx}({}): err={err:?}", peer.mnemonic()));
                }
            }
        }
        sleep(COMMIT_CERT_POLL).await;
    }
}

async fn wait_for_commit_vote_metrics(
    http: &reqwest::Client,
    metrics_urls: &[reqwest::Url],
    required: usize,
) -> Result<()> {
    let deadline = Instant::now() + COMMIT_CERT_TIMEOUT;
    let mut last = Vec::new();
    loop {
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for commit vote metrics (required >= {required}); last={last:?}"
            ));
        }
        last.clear();
        for metrics_url in metrics_urls {
            match fetch_metrics(http, metrics_url).await {
                Ok(reader) => {
                    let present = reader.get_optional("sumeragi_commit_signatures_present");
                    let counted = reader.get_optional("sumeragi_commit_signatures_counted");
                    let required_metric =
                        reader.get_optional("sumeragi_commit_signatures_required");
                    if commit_vote_metrics_reached(present, counted, required_metric, required) {
                        return Ok(());
                    }
                    last.push(format!(
                        "{} present={present:?} counted={counted:?} required={required_metric:?}",
                        metrics_url
                    ));
                }
                Err(err) => {
                    last.push(format!("{metrics_url}: {err:?}"));
                }
            }
        }
        sleep(COMMIT_CERT_POLL).await;
    }
}

fn commit_vote_metrics_reached(
    present: Option<f64>,
    counted: Option<f64>,
    required_metric: Option<f64>,
    required: usize,
) -> bool {
    let required_f64 = f64::from(u32::try_from(required).unwrap_or(u32::MAX));
    matches!(
        (present, counted, required_metric),
        (Some(present), Some(counted), Some(required_metric))
            if present >= required_f64
                && counted >= required_f64
                && required_metric >= required_f64
    )
}

#[test]
fn commit_vote_metrics_reached_requires_all_quorum_metrics() {
    assert!(commit_vote_metrics_reached(
        Some(3.0),
        Some(3.0),
        Some(3.0),
        3
    ));
    assert!(!commit_vote_metrics_reached(
        Some(3.0),
        Some(2.0),
        Some(3.0),
        3
    ));
    assert!(!commit_vote_metrics_reached(Some(3.0), None, Some(3.0), 3));
}

async fn fetch_metrics(
    http: &reqwest::Client,
    metrics_url: &reqwest::Url,
) -> Result<MetricsReader> {
    let response = http
        .get(metrics_url.clone())
        .send()
        .await
        .wrap_err("fetch metrics")?;
    ensure!(
        response.status().is_success(),
        "metrics response {}",
        response.status()
    );
    let body = response.text().await.wrap_err("metrics body")?;
    Ok(MetricsReader::new(&body))
}

fn stake_genesis_post_topology_transactions(topology: &[PeerId]) -> Vec<Vec<InstructionBox>> {
    let nexus_domain: DomainId = DomainId::try_new("nexus", "universal").expect("nexus domain");
    let stake_asset_id = stake_asset_definition_id();
    let fee_asset_id = nexus_fee_asset_definition_id();
    let gas_account_id = ALICE_ID.clone();

    let definition = {
        let __asset_definition_id = stake_asset_id.clone();
        AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::default())
            .with_name(__asset_definition_id.name().to_string())
    }
    .with_metadata(Metadata::default());
    let fee_definition = {
        let __asset_definition_id = fee_asset_id.clone();
        AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::default())
            .with_name(__asset_definition_id.name().to_string())
    }
    .with_metadata(Metadata::default());

    let mut bootstrap_tx = vec![
        Register::domain(Domain::new(nexus_domain.clone())).into(),
        Register::asset_definition(definition).into(),
        Register::asset_definition(fee_definition).into(),
        Mint::asset_quantity(
            HIGH_STAKE,
            AssetId::new(stake_asset_id.clone(), gas_account_id.clone()),
        )
        .into(),
        Mint::asset_quantity(
            NEXUS_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), gas_account_id.clone()),
        )
        .into(),
    ];
    for (idx, _peer) in topology.iter().enumerate() {
        let validator_id = validator_account_id_for_index(idx);
        let stake = if idx == 0 { HIGH_STAKE } else { LOW_STAKE };
        if validator_id != gas_account_id {
            bootstrap_tx.push(Register::account(Account::new(validator_id.clone())).into());
        }
        bootstrap_tx.push(
            Mint::asset_quantity(
                stake,
                AssetId::new(stake_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        bootstrap_tx.push(
            Mint::asset_quantity(
                NEXUS_FEE_SEED_AMOUNT,
                AssetId::new(fee_asset_id.clone(), validator_id),
            )
            .into(),
        );
    }

    let mut validator_tx = Vec::new();
    for (idx, peer) in topology.iter().enumerate() {
        let validator_id = validator_account_id_for_index(idx);
        let stake = if idx == 0 { HIGH_STAKE } else { LOW_STAKE };
        validator_tx.push(
            RegisterPublicLaneValidator {
                lane_id: LaneId::SINGLE,
                validator: validator_id.clone(),
                peer_id: PeerId::from(peer.public_key().clone()),
                stake_account: validator_id.clone(),
                initial_stake: Numeric::from(stake),
                metadata: Metadata::default(),
            }
            .into(),
        );
        validator_tx.push(ActivatePublicLaneValidator::new(LaneId::SINGLE, validator_id).into());
    }

    vec![bootstrap_tx, validator_tx]
}

async fn wait_for_non_empty_blocks(
    client: &iroha::client::Client,
    target: u64,
    timeout: Duration,
) -> Result<Option<iroha::client::Status>> {
    let deadline = Instant::now() + timeout;
    loop {
        let status = client.get_status()?;
        if status.blocks_non_empty >= target {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        sleep(COMMIT_CERT_POLL).await;
    }
}
