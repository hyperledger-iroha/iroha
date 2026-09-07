//! Shared finalized-transaction visibility checks for four-peer privacy scenarios.

use std::{future::Future, time::Duration};

use eyre::{Result, WrapErr as _, ensure, eyre};
use iroha::blocking::Client;
use iroha_data_model::{
    prelude::QueryBuilderExt,
    privacy::PrivacyExact12CapabilityManifestV1,
    query::transaction::prelude::FindTransactions,
    transaction::{SignedTransaction, TransactionEntrypoint},
};
use tokio::time::{Instant, sleep};

const POLL_INTERVAL: Duration = Duration::from_millis(200);
const NETWORK_STACK_BYTES: usize = 32 * 1024 * 1024;

/// Run real genesis preexecution and all async workers on bounded 32-MiB stacks.
pub(super) fn run_network_case<F, Fut>(name: &'static str, body: F) -> Result<()>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<()>>,
{
    let worker = std::thread::Builder::new()
        .name(name.to_owned())
        .stack_size(NETWORK_STACK_BYTES)
        .spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .thread_stack_size(NETWORK_STACK_BYTES)
                .enable_all()
                .build()
                .wrap_err("build bounded Exact12 network test runtime")?
                .block_on(body())
        })
        .wrap_err("start Exact12 network test thread")?;
    match worker.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

/// Read the finalized capability snapshot outside the async runtime.
pub(super) async fn privacy_capabilities(
    client: &Client,
) -> Result<PrivacyExact12CapabilityManifestV1> {
    let client = client.clone();
    iroha_test_network::read_on_dedicated_thread(move || client.client().get_privacy_capabilities())
        .await
}

fn exact_applied_transaction_visible(
    client: &Client,
    transaction: &SignedTransaction,
) -> Result<bool> {
    let expected_hash = transaction.hash_as_entrypoint();
    let expected_entrypoint = TransactionEntrypoint::External(transaction.clone());
    let transactions = client
        .client()
        .query(FindTransactions::new())
        .execute_all()
        .wrap_err("query finalized transactions")?;
    let Some(committed) = transactions
        .iter()
        .find(|committed| committed.entrypoint_hash() == &expected_hash)
    else {
        return Ok(false);
    };
    ensure!(
        committed.entrypoint() == &expected_entrypoint,
        "entrypoint hash matched different transaction bytes"
    );
    ensure!(
        committed.result().0.is_ok(),
        "exact-12 catch-up sentinel is visible but finalized as rejected"
    );
    Ok(true)
}
/// Wait for the exact successful finalized transaction on every supplied peer.
pub(super) async fn wait_for_transaction_on_peers(
    clients: &[Client],
    transaction: &SignedTransaction,
    context: &str,
    convergence_timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + convergence_timeout;
    let mut last_observed = Vec::new();
    loop {
        let mut visible = 0_usize;
        last_observed.clear();
        for (index, client) in clients.iter().enumerate() {
            let client = client.clone();
            let transaction = transaction.clone();
            match iroha_test_network::read_on_dedicated_thread(move || {
                exact_applied_transaction_visible(&client, &transaction)
            })
            .await
            {
                Ok(true) => {
                    visible += 1;
                    last_observed.push(format!("peer {index}: exact transaction visible"));
                }
                Ok(false) => last_observed.push(format!("peer {index}: transaction absent")),
                Err(error) => last_observed.push(format!("peer {index}: {error}")),
            }
        }
        if visible == clients.len() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{context}: finalized transaction did not converge within \
                 {convergence_timeout:?}; {}",
                last_observed.join("; ")
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}
