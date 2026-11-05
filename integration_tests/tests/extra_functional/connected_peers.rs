//! Connected peer counts under safe validator restarts and candidate registration.
use eyre::{Result, WrapErr, eyre};
use futures_util::{StreamExt, future::try_join_all, stream::FuturesUnordered};
use integration_tests::{sandbox, sync::spawn_blocking};
use iroha::data_model::{
    Level,
    isi::{InstructionBox, Log, Unregister, register::RegisterPeerWithPop},
    prelude::QueryBuilderExt,
    query::peer::prelude::FindPeers,
};
use iroha_test_network::*;
use rand::{SeedableRng, prelude::IteratorRandom};
use rand_chacha::ChaCha8Rng;
use std::{collections::BTreeSet, fmt::Write as _, time::Duration};
use tokio::time::{Instant, sleep, timeout};
fn leader_peer<'a>(peers: impl IntoIterator<Item = &'a NetworkPeer>) -> &'a NetworkPeer {
    peers
        .into_iter()
        .min_by(|left, right| left.id().cmp(&right.id()))
        .expect("network should have at least one peer")
}
#[tokio::test]
async fn connected_peers_with_f_2_1_2() -> Result<()> {
    Box::pin(connected_peers_with_f(
        stringify!(connected_peers_with_f_2_1_2),
        2,
    ))
    .await
}
#[tokio::test]
async fn connected_peers_with_f_1_0_1() -> Result<()> {
    Box::pin(connected_peers_with_f(
        stringify!(connected_peers_with_f_1_0_1),
        1,
    ))
    .await
}
#[tokio::test]
async fn register_new_peer() -> Result<()> {
    let Some(network) = start_validators_with_offline_observer(
        NetworkBuilder::new()
            .with_block_cadence(std::time::Duration::from_secs(2))
            .with_peers(4)
            .with_max_validator_capacity(5),
        stringify!(register_new_peer),
    )
    .await?
    else {
        return Ok(());
    };
    // Networking trust is predeclared; on-chain registration creates a candidate
    // key and never changes the original four-validator voting committee.
    let peer = &network.observers()[0];
    let submit_timeout = network
        .sync_timeout()
        .saturating_add(network.da_commit_quorum_timeout());
    let peer_sync_timeout = network
        .sync_timeout()
        .saturating_mul(2)
        .saturating_add(network.da_commit_quorum_timeout());
    let register = RegisterPeerWithPop::new(
        peer.id(),
        peer.bls_pop()
            .expect("network peer should have BLS PoP")
            .to_vec(),
    );
    let client = integration_tests::sync::rebind_blocking_client(&network.client(), |client| {
        client.transaction_status_timeout = submit_timeout;
        client.transaction_ttl = Some(submit_timeout.saturating_add(Duration::from_secs(120)));
    });
    submit_instruction_or_warn(client, register, "register_new_peer").await?;
    if sandbox::handle_result(
        network.ensure_blocks(2).await,
        stringify!(register_new_peer),
    )?
    .is_none()
    {
        return Ok(());
    }
    let genesis = network.genesis();
    peer.start(network.config_layers_for_peer(peer), Some(&genesis))
        .await?;
    if sandbox::handle_result(
        timeout(peer_sync_timeout, peer.once_block(2))
            .await
            .map_err(eyre::Report::new),
        stringify!(register_new_peer),
    )?
    .is_none()
    {
        return Ok(());
    }
    let client = integration_tests::sync::rebind_blocking_client(&network.client(), |client| {
        client.transaction_status_timeout = submit_timeout;
        client.transaction_ttl = Some(submit_timeout.saturating_add(Duration::from_secs(120)));
    });
    submit_instruction_or_warn(
        client,
        Log::new(Level::INFO, "register_new_peer_sync".to_string()),
        "register_new_peer",
    )
    .await?;
    if sandbox::handle_result(
        network.ensure_blocks(3).await,
        stringify!(register_new_peer),
    )?
    .is_none()
    {
        return Ok(());
    }
    if sandbox::handle_result(
        timeout(peer_sync_timeout, peer.once_block(3))
            .await
            .map_err(eyre::Report::new),
        stringify!(register_new_peer),
    )?
    .is_none()
    {
        return Ok(());
    }
    Ok(())
}
/// Start the four original voters while retaining one authorized, offline sync replica.
/// Candidate registration and immutable P2P trust are separate authorities.
pub(super) async fn start_validators_with_offline_observer(
    builder: NetworkBuilder,
    context: &'static str,
) -> Result<Option<sandbox::SerializedNetwork>> {
    let builder = builder.with_observer_p2p_bootstrap(ObserverP2pBootstrap::new(1)?)?;
    let Some(network) = sandbox::build_network_or_skip(builder, context) else {
        return Ok(None);
    };
    assert_eq!(network.validators().len(), 4);
    assert_eq!(network.observers().len(), 1);
    let genesis = network.genesis();
    let voters = iroha_core::sumeragi::schedule::genesis_validators(&genesis)?;
    assert_eq!(voters.len(), 4);
    assert!(!voters.contains_key(&network.observers()[0].id()));
    timeout(network.peer_startup_timeout(), async {
        try_join_all(
            network.validators().iter().map(|peer| {
                peer.start_checked(network.config_layers_for_peer(peer), Some(&genesis))
            }),
        )
        .await?;
        network.ensure_blocks(1).await?;
        Ok::<(), eyre::Report>(())
    })
    .await??;
    assert!(!network.observers()[0].is_running());
    Ok(Some(network))
}

#[test]
fn offline_replica_bootstrap_preserves_four_signed_voters_and_explicit_trust() -> Result<()> {
    let network = NetworkBuilder::new()
        .with_peers(4)
        .with_max_validator_capacity(5)
        .with_observer_p2p_bootstrap(ObserverP2pBootstrap::new(1)?)?
        .build();
    let replica = &network.observers()[0];
    assert!(!replica.is_running());
    let voters = iroha_core::sumeragi::schedule::genesis_validators(&network.genesis())?;
    assert_eq!(voters.len(), 4);
    assert!(!voters.contains_key(&replica.id()));
    let layers = network.config_layers_for_peer(replica).collect::<Vec<_>>();
    let base = &layers[0];
    let trusted = base
        .get("trusted_peers")
        .and_then(toml::Value::as_array)
        .expect("explicit P2P trust");
    assert!(
        trusted
            .iter()
            .filter_map(toml::Value::as_str)
            .any(|entry| entry.starts_with(&format!("{}@", replica.id())))
    );
    let pops = base
        .get("trusted_peers_pop")
        .and_then(toml::Value::as_array)
        .expect("exact voting PoPs");
    assert_eq!(pops.len(), 4);
    assert!(pops.iter().all(
        |entry| entry.get("public_key").and_then(toml::Value::as_str)
            != Some(replica.id().public_key().to_string().as_str())
    ));
    assert_eq!(
        layers
            .last()
            .unwrap()
            .get("sumeragi")
            .unwrap()
            .get("role")
            .and_then(toml::Value::as_str),
        Some("observer")
    );
    Ok(())
}

/// The authenticated `3f + 1` roster stays fixed while one validator disconnects and rejoins.
#[allow(clippy::too_many_lines)]
async fn connected_peers_with_f(context: &'static str, faults: usize) -> Result<()> {
    let n_peers = 3 * faults + 1;
    let builder = NetworkBuilder::new()
        .with_peers(n_peers)
        .with_block_cadence(Duration::from_secs(2));
    let Some(network) = sandbox::start_network_async_or_skip(builder, context).await? else {
        return Ok(());
    };
    let sync_timeout = network.sync_timeout().saturating_mul(2);
    let submit_timeout = sync_timeout.saturating_add(network.da_commit_quorum_timeout());
    assert_peers_status(
        network.peers(),
        1,
        expected_connected_peers(n_peers),
        sync_timeout,
    )
    .await?;
    let mut randomized_peers = {
        let mut rng = ChaCha8Rng::seed_from_u64(0x434f_4e4e);
        network.peers().iter().sample(&mut rng, n_peers)
    };
    let restarted_peer = randomized_peers.remove(0);
    let client = leader_peer(randomized_peers.iter().copied()).client();
    let client = integration_tests::sync::rebind_blocking_client(&client, |client| {
        client.transaction_status_timeout = submit_timeout;
        client.transaction_ttl = Some(submit_timeout.saturating_add(Duration::from_secs(120)));
    });
    let expected_roster = network
        .peers()
        .iter()
        .map(NetworkPeer::id)
        .collect::<BTreeSet<_>>();
    let rejection = submit_instruction_or_warn(
        client.clone(),
        Unregister::peer(restarted_peer.id()),
        context,
    )
    .await
    .expect_err("an active authenticated validator cannot be unregistered");
    assert!(
        is_active_roster_rejection(&rejection),
        "unexpected unregister rejection: {rejection:?}"
    );
    let roster_client = client.clone();
    let retained_roster =
        spawn_blocking(move || roster_client.client().query(FindPeers).execute_all())
            .await??
            .into_iter()
            .collect::<BTreeSet<_>>();
    assert_eq!(
        retained_roster, expected_roster,
        "rejected removal must preserve every roster identity"
    );
    // Rejection is itself a committed result; every validator must retain it before shutdown.
    wait_for_block_height(network.peers(), 2, sync_timeout).await?;
    restarted_peer.shutdown().await;
    assert_peers_status(
        randomized_peers.iter().copied(),
        2,
        expected_connected_peers(n_peers - 1),
        sync_timeout,
    )
    .await?;
    // The original committee still has its exact quorum (3/4 or at least 5/7).
    // A real transaction must apply while the one owned validator is offline.
    submit_instruction_or_warn(
        client,
        Log::new(
            Level::INFO,
            "connected_peers_with_one_validator_offline".to_owned(),
        ),
        context,
    )
    .await?;
    wait_for_block_height(randomized_peers.iter().copied(), 3, sync_timeout).await?;
    let genesis = network.genesis();
    restarted_peer
        .start(
            network.config_layers_for_peer(restarted_peer),
            Some(&genesis),
        )
        .await?;
    wait_for_block_height(std::iter::once(restarted_peer), 3, sync_timeout).await?;
    assert_peers_status(
        network.peers(),
        3,
        expected_connected_peers(n_peers),
        sync_timeout,
    )
    .await?;
    Ok(())
}

fn is_active_roster_rejection(error: &eyre::Report) -> bool {
    use iroha_data_model::{
        ValidationFail,
        isi::error::{InstructionExecutionError, InvalidParameterError},
        transaction::error::TransactionRejectionReason,
    };
    error.chain().any(|cause| matches!(
        cause.downcast_ref::<TransactionRejectionReason>(),
        Some(TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(reason))
        ))) if reason == "cannot unregister a peer from the current authenticated consensus roster"
    ))
}

#[test]
fn active_roster_rejection_requires_the_exact_native_reason() {
    use iroha_data_model::{
        ValidationFail,
        isi::error::{InstructionExecutionError, InvalidParameterError},
        transaction::error::TransactionRejectionReason,
    };
    let original = TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            "cannot unregister a peer from the current authenticated consensus roster".to_owned(),
        )),
    ));
    assert!(is_active_roster_rejection(
        &eyre::Report::new(original).wrap_err("submission failed")
    ));
    assert!(!is_active_roster_rejection(&eyre!(
        "cannot unregister a peer from the current authenticated consensus roster"
    )));
    assert!(!is_active_roster_rejection(&eyre::Report::new(
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "cannot unregister a peer from the current authenticated consensus roster".to_owned()
        ),)
    )));
}

async fn assert_peers_status(
    peers: impl IntoIterator<Item = &'_ NetworkPeer>,
    expected_blocks: u64,
    expected_peers: u64,
    timeout: Duration,
) -> Result<()> {
    let peers: Vec<_> = peers.into_iter().collect();
    let deadline = Instant::now() + timeout;
    loop {
        let mismatches = peers
            .iter()
            .map(|peer| async {
                match peer.status().await {
                    Err(err) if !peer.is_running() => Some(format!(
                        "{}: peer not running; status error: {err}; stdout={:?} stderr={:?}",
                        peer.id(),
                        peer.latest_stdout_log_path(),
                        peer.latest_stderr_log_path(),
                    )),
                    Ok(status) => {
                        let fallback_height = peer.best_effort_block_height();
                        let last_peers = peer.last_known_peers();
                        // Only a current response proves that the disconnected peer count fell;
                        // cached larger counts must not satisfy the down-transition assertion.
                        let peers_ok = status.peers == expected_peers;
                        let blocks_ok = status.blocks >= expected_blocks
                            || fallback_height
                                .is_some_and(|height| height.total >= expected_blocks);
                        if peers_ok && blocks_ok {
                            return None;
                        }
                        let mut message = format!(
                            "{}: peers={}, blocks={}, blocks_non_empty={}",
                            peer.id(),
                            status.peers,
                            status.blocks,
                            status.blocks_non_empty
                        );
                        if let Some(height) = fallback_height {
                            let _ = write!(
                                message,
                                "; fallback_blocks_total={} non_empty={}",
                                height.total, height.non_empty
                            );
                        }
                        if let Some(peers) = last_peers {
                            let _ = write!(message, "; last_known_peers={peers}");
                        }
                        Some(message)
                    }
                    Err(err) => {
                        let fallback_height = peer.best_effort_block_height();
                        let last_peers = peer.last_known_peers();
                        let mut message = format!(
                            "{}: status error: {err}; stdout={:?} stderr={:?}",
                            peer.id(),
                            peer.latest_stdout_log_path(),
                            peer.latest_stderr_log_path()
                        );
                        if let Some(height) = fallback_height {
                            let _ = write!(
                                message,
                                "; fallback_blocks_total={} non_empty={}",
                                height.total, height.non_empty
                            );
                        }
                        if let Some(peers) = last_peers {
                            let _ = write!(message, "; last_known_peers={peers}");
                        }
                        Some(message)
                    }
                }
            })
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        if mismatches.is_empty() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "deadline has elapsed waiting for peer status: {:?}",
                mismatches
            ));
        }
        sleep(Duration::from_millis(200)).await;
    }
}
async fn wait_for_block_height(
    peers: impl IntoIterator<Item = &'_ NetworkPeer>,
    expected_blocks: u64,
    timeout: Duration,
) -> Result<()> {
    let peers: Vec<_> = peers.into_iter().collect();
    let deadline = Instant::now() + timeout;
    loop {
        let lagging = peers
            .iter()
            .map(|peer| async {
                match peer.status().await {
                    Err(err) if !peer.is_running() => Some(format!(
                        "{}: peer not running; status error: {err}; stdout={:?} stderr={:?}",
                        peer.id(),
                        peer.latest_stdout_log_path(),
                        peer.latest_stderr_log_path(),
                    )),
                    Ok(status) => {
                        let fallback_height = peer.best_effort_block_height();
                        let blocks_ok = status.blocks >= expected_blocks
                            || fallback_height
                                .is_some_and(|height| height.total >= expected_blocks);
                        if blocks_ok {
                            return None;
                        }
                        let mut message = format!(
                            "{}: blocks={}, blocks_non_empty={}",
                            peer.id(),
                            status.blocks,
                            status.blocks_non_empty
                        );
                        if let Some(height) = fallback_height {
                            let _ = write!(
                                message,
                                "; fallback_blocks_total={} non_empty={}",
                                height.total, height.non_empty
                            );
                        }
                        Some(message)
                    }
                    Err(err) => {
                        let fallback_height = peer.best_effort_block_height();
                        if fallback_height.is_some_and(|height| height.total >= expected_blocks) {
                            return None;
                        }
                        let mut message = format!(
                            "{}: status error: {err}; stdout={:?} stderr={:?}",
                            peer.id(),
                            peer.latest_stdout_log_path(),
                            peer.latest_stderr_log_path()
                        );
                        if let Some(height) = fallback_height {
                            let _ = write!(
                                message,
                                "; fallback_blocks_total={} non_empty={}",
                                height.total, height.non_empty
                            );
                        }
                        Some(message)
                    }
                }
            })
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        if lagging.is_empty() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "deadline has elapsed waiting for block height {expected_blocks}: {:?}",
                lagging
            ));
        }
        sleep(Duration::from_millis(200)).await;
    }
}
async fn submit_instruction_or_warn(
    client: iroha::blocking::Client,
    instruction: impl Into<InstructionBox>,
    context: &str,
) -> Result<()> {
    let instruction = instruction.into();
    let context = context.to_string();
    spawn_blocking(move || {
        client
            .submit(
                instruction,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .map(|_| ())
            .wrap_err(context)
    })
    .await??;
    Ok(())
}
fn expected_connected_peers(running_peers: usize) -> u64 {
    u64::try_from(running_peers.saturating_sub(1)).expect("fixture peer count fits u64")
}

#[test]
fn connected_peer_counts_track_running_processes_without_resizing_quorums() {
    assert_eq!(expected_connected_peers(4), 3);
    assert_eq!(expected_connected_peers(3), 2);
    assert_eq!(expected_connected_peers(7), 6);
    assert_eq!(expected_connected_peers(6), 5);
    assert_eq!(iroha_sumeragi::types::quorum(4), 3);
    assert_eq!(iroha_sumeragi::types::quorum(7), 5);
}
