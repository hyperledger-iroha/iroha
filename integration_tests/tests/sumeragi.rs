#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Sumeragi on real peers (`specs/sumeragi_goals.md` S4): four validators over P2P commit
//! transactions, restart, and survive a crashed leader.
use std::time::{Duration, Instant};

use eyre::{Result, bail};
use integration_tests::{
    sandbox::{self, SerializedNetwork},
    sync::sumeragi_statuses_reach_height,
};
use iroha::data_model::{account::Account, isi::Register, prelude::*, sumeragi::SumeragiStatus};
use iroha_test_network::{Network, NetworkBuilder, NetworkPeer, init_instruction_registry};
use iroha_test_samples::gen_account_in;
use tokio::runtime::Runtime;

const TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES: i64 = 1024 * 1024 * 1024;

fn builder() -> NetworkBuilder {
    NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
                )
                // Isolate consensus from fee funding: keep the fee asset, quote zero charges.
                .write(["nexus", "fees", "base_fee"], "0")
                .write(["nexus", "fees", "per_byte_fee"], "0")
                .write(["nexus", "fees", "per_instruction_fee"], "0")
                .write(["nexus", "fees", "per_gas_unit_fee"], "0");
        })
}

fn start(name: &str) -> Result<Option<(SerializedNetwork, Runtime)>> {
    init_instruction_registry();
    sandbox::start_network_blocking_or_skip(builder(), name)
}

/// Every configured peer's Sumeragi status for the all-validator assertions.
fn statuses(network: &Network) -> Vec<Result<SumeragiStatus>> {
    network
        .peers()
        .iter()
        .map(|peer| peer.client().client().get_sumeragi_status())
        .collect()
}

/// Wait for every configured peer except seats the scenario explicitly stopped.
fn wait_for_committed(
    network: &Network,
    height: u64,
    limit: Duration,
    stopped: &[&NetworkPeer],
) -> Result<()> {
    let peers: Vec<_> = network
        .peers()
        .iter()
        .filter(|peer| !stopped.iter().any(|stopped| stopped.id() == peer.id()))
        .collect();
    let deadline = Instant::now() + limit;
    loop {
        let statuses: Vec<_> = peers
            .iter()
            .map(|peer| peer.client().client().get_sumeragi_status())
            .collect();
        if sumeragi_statuses_reach_height(&statuses, height)? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            let summary = statuses
                .iter()
                .map(|status| match status {
                    Ok(status) => format!(
                        "h{}/v{} committed={} signing={} unanchored={}",
                        status.height,
                        status.view,
                        status.committed_height,
                        status.is_signing(),
                        status.unanchored
                    ),
                    Err(error) => format!("error: {error}"),
                })
                .collect::<Vec<_>>();
            bail!("peers did not commit height {height} in {limit:?}: {summary:?}");
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// The first running peer: clients must not talk to a peer a test shut down (the crashed leader
/// may be any peer, including the first).
fn running_peer(network: &Network) -> Result<&NetworkPeer> {
    network
        .peers()
        .iter()
        .find(|peer| peer.is_running())
        .ok_or_else(|| eyre::eyre!("no peer is running"))
}

fn committed_height(network: &Network) -> Result<u64> {
    Ok(running_peer(network)?
        .client()
        .client()
        .get_sumeragi_status()?
        .committed_height)
}

/// Register a fresh account through a running peer (the client waits until the transaction is
/// applied), then wait until every seat the scenario expects committed its block.
fn register_account_everywhere(network: &Network, stopped: &[&NetworkPeer]) -> Result<AccountId> {
    let (account, _) = gen_account_in("wonderland");
    running_peer(network)?.client().submit(
        Register::account(Account::new(account.clone())),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let height = committed_height(network)?;
    wait_for_committed(network, height, Duration::from_secs(60), stopped)?;
    Ok(account)
}

#[test]
fn four_validators_commit_a_transaction() -> Result<()> {
    let Some((network, rt)) = start(stringify!(four_validators_commit_a_transaction))? else {
        return Ok(());
    };
    let result = (|| -> Result<()> {
        wait_for_committed(&network, 1, network.sync_timeout(), &[])?;
        let before = committed_height(&network)?;
        register_account_everywhere(&network, &[])?;
        let after = committed_height(&network)?;
        assert!(after > before, "the transaction's block is committed");
        for status in statuses(&network) {
            let status = status?;
            assert!(status.is_signing(), "every validator signs: {status:?}");
            assert_eq!(status.applied_height, status.committed_height);
        }
        Ok(())
    })();
    rt.block_on(async { network.shutdown().await });
    result
}

/// One validator restarts (records and Kura kept) while the other three commit, catches up and
/// signs again; then the whole cluster restarts and goes on.
#[test]
fn validators_restart_one_and_all() -> Result<()> {
    let Some((network, rt)) = start(stringify!(validators_restart_one_and_all))? else {
        return Ok(());
    };
    let result = (|| -> Result<()> {
        wait_for_committed(&network, 1, network.sync_timeout(), &[])?;
        // One validator down: the remaining quorum (3 of 4) commits.
        let restarted = &network.peers()[1];
        rt.block_on(restarted.shutdown());
        register_account_everywhere(&network, &[restarted])?;
        let height = committed_height(&network)?;
        let layers: Vec<_> = network.config_layers_for_peer(restarted).collect();
        rt.block_on(restarted.start_checked(layers.iter(), None))?;
        wait_for_committed(&network, height, Duration::from_secs(60), &[])?;
        register_account_everywhere(&network, &[])?;
        // Whole cluster down and up: every validator rebuilds its state from Kura.
        let height = committed_height(&network)?;
        for peer in network.peers() {
            rt.block_on(peer.shutdown());
        }
        for peer in network.peers() {
            let layers: Vec<_> = network.config_layers_for_peer(peer).collect();
            rt.block_on(peer.start_checked(layers.iter(), None))?;
        }
        wait_for_committed(&network, height, Duration::from_secs(60), &[])?;
        register_account_everywhere(&network, &[])?;
        for status in statuses(&network) {
            let status = status?;
            assert!(
                status.is_signing(),
                "every validator signs again: {status:?}"
            );
        }
        Ok(())
    })();
    rt.block_on(async { network.shutdown().await });
    result
}

/// The leader of the next round crashes: the others time out, change the view, and commit a
/// transaction under the next leader.
#[test]
fn a_crashed_leader_is_replaced() -> Result<()> {
    let Some((network, rt)) = start(stringify!(a_crashed_leader_is_replaced))? else {
        return Ok(());
    };
    let result = (|| -> Result<()> {
        wait_for_committed(&network, 1, network.sync_timeout(), &[])?;
        let status = network.client().client().get_sumeragi_status()?;
        let leader = status
            .leader
            .clone()
            .ok_or_else(|| eyre::eyre!("no leader while awaiting: {status:?}"))?;
        let leader_peer = network
            .peers()
            .iter()
            .find(|peer| peer.id().public_key() == &leader)
            .ok_or_else(|| eyre::eyre!("the leader {leader} is not a peer"))?;
        rt.block_on(leader_peer.shutdown());
        let started = Instant::now();
        register_account_everywhere(&network, &[leader_peer])?;
        eprintln!(
            "committed without the crashed leader in {:?}",
            started.elapsed()
        );
        Ok(())
    })();
    rt.block_on(async { network.shutdown().await });
    result
}
