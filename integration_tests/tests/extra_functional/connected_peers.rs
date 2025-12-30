//! Connected peers count and reconfiguration under register/unregister.

use std::{fmt::Write as _, iter::once, time::Duration};

use assert_matches::assert_matches;
use eyre::{Result, WrapErr, eyre};
use futures_util::{StreamExt, stream::FuturesUnordered};
use integration_tests::sandbox;
use iroha::data_model::{
    Level,
    isi::{InstructionBox, Log, SetParameter, Unregister, register::RegisterPeerWithPop},
    parameter::{Parameter, SumeragiParameter},
};
use iroha_test_network::*;
use rand::{SeedableRng, prelude::IteratorRandom};
use rand_chacha::ChaCha8Rng;
use tokio::{
    task::spawn_blocking,
    time::{Instant, sleep, timeout},
};

fn leader_peer<'a>(peers: impl IntoIterator<Item = &'a NetworkPeer>) -> &'a NetworkPeer {
    peers
        .into_iter()
        .min_by(|left, right| left.id().cmp(&right.id()))
        .expect("network should have at least one peer")
}

#[tokio::test]
async fn connected_peers_with_f_2_1_2() -> Result<()> {
    connected_peers_with_f(stringify!(connected_peers_with_f_2_1_2), 2).await
}

#[tokio::test]
async fn connected_peers_with_f_1_0_1() -> Result<()> {
    connected_peers_with_f(stringify!(connected_peers_with_f_1_0_1), 1).await
}

#[tokio::test]
async fn register_new_peer() -> Result<()> {
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new()
            .with_default_pipeline_time()
            .with_peers(4),
        stringify!(register_new_peer),
    )
    .await?
    else {
        return Ok(());
    };

    let peer = NetworkPeerBuilder::new().build(network.env());
    let register = RegisterPeerWithPop::new(
        peer.id(),
        peer.bls_pop()
            .expect("network peer should have BLS PoP")
            .to_vec(),
    );
    submit_instruction_or_warn(network.client(), register, "register_new_peer").await?;
    if sandbox::handle_result(
        network.ensure_blocks(2).await,
        stringify!(register_new_peer),
    )?
    .is_none()
    {
        return Ok(());
    }

    let genesis = network.genesis();
    peer.start(
        network.config_layers_with_additional_peers([&peer]),
        Some(&genesis),
    )
    .await?;

    submit_instruction_or_warn(
        network.client(),
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
        timeout(network.sync_timeout(), peer.once_block(3))
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

/// Test the number of connected peers, changing the number of faults tolerated down and up.
#[allow(clippy::too_many_lines)]
async fn connected_peers_with_f(context: &'static str, faults: usize) -> Result<()> {
    let n_peers = 3 * faults + 1;

    let mut builder = NetworkBuilder::new()
        .with_peers(n_peers)
        .with_data_availability_enabled(true)
        .with_default_pipeline_time();
    if n_peers > 4 {
        const COLLECTORS_K: u16 = 3;
        const REDUNDANT_SEND_R: u8 = 2;

        builder = builder
            .with_config_layer(|layer| {
                layer
                    .write(["sumeragi", "collectors_k"], i64::from(COLLECTORS_K))
                    .write(
                        ["sumeragi", "collectors_redundant_send_r"],
                        i64::from(REDUNDANT_SEND_R),
                    );
            })
            .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
                SumeragiParameter::CollectorsK(COLLECTORS_K),
            )))
            .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
                SumeragiParameter::RedundantSendR(REDUNDANT_SEND_R),
            )));
    } else {
        builder = builder.with_default_pipeline_time();
    }
    let Some(network) = sandbox::start_network_async_or_skip(builder, context).await? else {
        return Ok(());
    };

    if sandbox::handle_result(
        assert_peers_status(
            network.peers().iter(),
            1,
            n_peers as u64 - 1,
            network.sync_timeout(),
        )
        .await,
        context,
    )?
    .is_none()
    {
        return Ok(());
    }

    let mut randomized_peers = {
        let mut rng = ChaCha8Rng::seed_from_u64(0x434f_4e4e);
        network.peers().iter().choose_multiple(&mut rng, n_peers)
    };
    let removed_peer = randomized_peers.remove(0);

    // Unregister a peer: committed with f = `faults` then `status.peers` decrements
    let client = leader_peer(randomized_peers.iter().copied()).client();
    let unregister_peer = Unregister::peer(removed_peer.id());
    submit_instruction_or_warn(client.clone(), unregister_peer, context).await?;
    timeout(
        network.sync_timeout(),
        randomized_peers
            .iter()
            .map(|peer| peer.once_block(2))
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>(),
    )
    .await?;
    if sandbox::handle_result(
        assert_peers_status(
            randomized_peers.iter().copied(),
            2,
            n_peers as u64 - 2,
            network.sync_timeout(),
        )
        .await,
        context,
    )?
    .is_none()
    {
        return Ok(());
    }

    // Wait for peer to disconnect
    let disconnect_result = timeout(network.sync_timeout(), async {
        loop {
            let status = removed_peer.status().await?;
            if status.peers == 0 {
                break;
            }
        }
        Ok::<(), eyre::Report>(())
    })
    .await;
    match disconnect_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(err),
        Err(_) => {
            return Err(eyre!(
                "{context} timed out waiting for removed peer to disconnect"
            ));
        }
    }

    let status = removed_peer.status().await?;
    // Peer might have been disconnected before getting the block, or see one extra commit.
    assert_matches!(status.blocks_non_empty, 1 | 2 | 3);
    assert_eq!(status.peers, 0);

    // Re-register the peer: committed with f = `faults` - 1 then `status.peers` increments
    let register_peer = RegisterPeerWithPop::new(
        removed_peer.id(),
        removed_peer
            .bls_pop()
            .expect("network peer should have BLS PoP")
            .to_vec(),
    );
    let client = leader_peer(randomized_peers.iter().copied()).client();
    submit_instruction_or_warn(client.clone(), register_peer, context).await?;
    network.ensure_blocks(3).await?;

    if sandbox::handle_result(
        assert_peers_status(
            randomized_peers.iter().copied().chain(once(removed_peer)),
            3,
            n_peers as u64 - 1,
            network.sync_timeout(),
        )
        .await,
        context,
    )?
    .is_none()
    {
        return Ok(());
    }

    Ok(())
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
                    Ok(status)
                        if status.peers >= expected_peers
                            && status.blocks_non_empty >= expected_blocks =>
                    {
                        None
                    }
                    Ok(status) => Some(format!(
                        "{}: peers={}, blocks_non_empty={}",
                        peer.id(),
                        status.peers,
                        status.blocks_non_empty
                    )),
                    Err(err) => {
                        let fallback_height = peer.best_effort_block_height();
                        let last_peers = peer.last_known_peers();
                        if let (Some(height), Some(peers)) = (fallback_height, last_peers)
                            && height.non_empty >= expected_blocks
                            && peers >= expected_peers
                        {
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
                                "; fallback_blocks_non_empty={} total={}",
                                height.non_empty, height.total
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

async fn submit_instruction_or_warn(
    client: iroha::client::Client,
    instruction: impl Into<InstructionBox>,
    context: &str,
) -> Result<()> {
    let instruction = instruction.into();
    let context = context.to_string();
    spawn_blocking(move || client.submit_blocking(instruction).wrap_err(context)).await??;
    Ok(())
}
