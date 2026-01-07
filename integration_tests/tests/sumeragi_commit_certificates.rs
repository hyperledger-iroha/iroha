//! Integration coverage for commit certificates in permissioned and `NPoS` modes.

use std::time::{Duration, Instant};

use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::{metrics::MetricsReader, sandbox};
use iroha::data_model::{
    Level,
    account::Account,
    asset::{AssetDefinitionId, id::AssetId},
    consensus::{HsmBinding, Qc},
    domain::DomainId,
    isi::{
        Log, Mint, Register, register::RegisterPeerWithPop, staking::RegisterPublicLaneValidator,
    },
    metadata::Metadata,
    nexus::LaneId,
    peer::PeerId,
    prelude::*,
};
use iroha_core::sumeragi::{consensus::qc_signer_count, network_topology::commit_quorum_from_len};
use iroha_primitives::numeric::Numeric;
use iroha_test_network::{NetworkBuilder, genesis_factory, init_instruction_registry};
use norito::json;
use tokio::time::{sleep, timeout};
use toml::Table;

const COMMIT_CERT_TIMEOUT: Duration = Duration::from_secs(120);
const COMMIT_CERT_POLL: Duration = Duration::from_millis(200);
const STAKE_ASSET_ID: &str = "rose#wonderland";
const STAKE_ESCROW_ACCOUNT: &str = "alice@wonderland";
const HIGH_STAKE: u64 = 70;
const LOW_STAKE: u64 = 10;
const STAKE_QUORUM_WAIT: Duration = Duration::from_secs(5);

type CommitCertificate = Qc;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn permissioned_commit_certificates_reach_quorum() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
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
        let status = client.get_status()?;
        ensure!(
            status.blocks_non_empty >= baseline.saturating_add(1),
            "expected non-empty block to commit"
        );
        let expected_height = status.blocks;
        let required = commit_quorum_from_len(network.peers().len());
        let http = reqwest::Client::new();
        let torii_urls = network.torii_urls();
        let metrics_url = client
            .torii_url
            .join("metrics")
            .wrap_err("compose metrics URL")?;
        wait_for_commit_certificate_quorum(
            &http,
            &torii_urls,
            expected_height,
            required,
            network.peers().len(),
        )
        .await?;
        wait_for_commit_quorum_status(&client, expected_height, required).await?;
        wait_for_commit_vote_metrics(&http, &metrics_url, required).await?;
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
        .with_config_layer(|layer| {
            layer.write(["sumeragi", "consensus_mode"], "permissioned");
        });

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
        let status = client.get_status()?;
        ensure!(
            status.blocks_non_empty >= baseline.saturating_add(1),
            "expected non-empty block to commit"
        );
        let expected_height = status.blocks;

        restart_peer
            .start_checked(config_layers.iter().cloned(), None)
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

        let http = reqwest::Client::new();
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

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["nexus", "staking", "stake_asset_id"], STAKE_ASSET_ID)
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    STAKE_ESCROW_ACCOUNT,
                )
                .write(
                    ["nexus", "staking", "slash_sink_account_id"],
                    STAKE_ESCROW_ACCOUNT,
                )
                .write(["sumeragi", "collectors_k"], 1_i64)
                .write(["sumeragi", "collectors_redundant_send_r"], 1_i64);
        })
        .with_genesis_block(|topology, topology_entries| {
            let instructions = stake_genesis_instructions(topology.as_ref(), &topology_entries);
            genesis_factory(vec![instructions], topology, topology_entries)
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
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
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|layer| ConfigLayer(layer.into_owned()))
            .collect();

        high_stake_peer.shutdown().await;

        let client = submit_peer.client();
        let baseline = client.get_status()?.blocks_non_empty;
        client.submit(Log::new(Level::INFO, "npos stake quorum".to_string()))?;

        let blocked = wait_for_non_empty_blocks(&client, baseline + 1, STAKE_QUORUM_WAIT).await?;
        ensure!(
            blocked.is_none(),
            "non-empty block committed without stake quorum"
        );

        high_stake_peer
            .start_checked(config_layers.iter().cloned(), None)
            .await
            .wrap_err("restart high-stake peer")?;
        let status = wait_for_non_empty_blocks(&client, baseline + 1, COMMIT_CERT_TIMEOUT)
            .await?
            .ok_or_else(|| eyre!("timed out waiting for stake quorum to commit"))?;
        let expected_height = status.blocks;

        let http = reqwest::Client::new();
        let submit_torii = submit_peer.torii_url();
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
                        missing.push(format!("{torii} signatures {} < {required}", signer_count));
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
    client: &iroha::client::Client,
    expected_height: u64,
    required: usize,
) -> Result<()> {
    let deadline = Instant::now() + COMMIT_CERT_TIMEOUT;
    let required_u64 = u64::try_from(required).unwrap_or(u64::MAX);
    loop {
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for commit quorum status at height {expected_height}"
            ));
        }
        if let Ok(status) = client.get_sumeragi_status() {
            let quorum = status.commit_quorum;
            if quorum.height >= expected_height
                && quorum.signatures_required >= required_u64
                && quorum.signatures_present >= required_u64
                && quorum.signatures_counted >= required_u64
            {
                return Ok(());
            }
        }
        sleep(COMMIT_CERT_POLL).await;
    }
}

async fn wait_for_commit_vote_metrics(
    http: &reqwest::Client,
    metrics_url: &reqwest::Url,
    required: usize,
) -> Result<()> {
    let deadline = Instant::now() + COMMIT_CERT_TIMEOUT;
    let required_f64 = f64::from(u32::try_from(required).unwrap_or(u32::MAX));
    loop {
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for commit vote metrics (required >= {required})"
            ));
        }
        if let Ok(reader) = fetch_metrics(http, metrics_url).await {
            let present = reader.get_optional("sumeragi_commit_signatures_present");
            let counted = reader.get_optional("sumeragi_commit_signatures_counted");
            let required_metric = reader.get_optional("sumeragi_commit_signatures_required");
            if let (Some(present), Some(counted), Some(required_metric)) =
                (present, counted, required_metric)
                && present >= required_f64
                && counted >= required_f64
                && required_metric >= required_f64
            {
                return Ok(());
            }
        }
        sleep(COMMIT_CERT_POLL).await;
    }
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

fn stake_genesis_instructions(
    topology: &[PeerId],
    topology_entries: &[iroha_genesis::GenesisTopologyEntry],
) -> Vec<InstructionBox> {
    let domain: DomainId = "wonderland".parse().expect("wonderland domain");
    let asset_def: AssetDefinitionId = STAKE_ASSET_ID.parse().expect("stake asset definition");
    let mut instructions = Vec::new();

    let mut pop_map = std::collections::BTreeMap::new();
    for entry in topology_entries {
        let Some(pop) = entry
            .pop_bytes()
            .expect("topology entry should have valid pop_hex")
        else {
            continue;
        };
        pop_map.insert(entry.peer.public_key().clone(), pop);
    }
    for peer in topology {
        let pop = pop_map
            .remove(peer.public_key())
            .expect("missing PoP for topology peer");
        let hsm_binding = HsmBinding {
            provider: "softkey".to_owned(),
            key_label: peer.public_key().to_string(),
            slot: None,
        };
        let register = RegisterPeerWithPop::new(peer.clone(), pop).with_hsm(hsm_binding);
        instructions.push(InstructionBox::from(register));
    }

    for (idx, peer) in topology.iter().enumerate() {
        let account_id = AccountId::new(domain.clone(), peer.public_key().clone());
        let stake = if idx == 0 { HIGH_STAKE } else { LOW_STAKE };
        instructions.push(Register::account(Account::new(account_id.clone())).into());
        instructions.push(
            Mint::asset_numeric(stake, AssetId::new(asset_def.clone(), account_id.clone())).into(),
        );
        instructions.push(
            RegisterPublicLaneValidator {
                lane_id: LaneId::SINGLE,
                validator: account_id.clone(),
                stake_account: account_id.clone(),
                initial_stake: Numeric::new(stake, 0),
                metadata: Metadata::default(),
            }
            .into(),
        );
    }
    instructions
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
