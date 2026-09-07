//! Shared finalized-transaction visibility checks for four-peer privacy scenarios.

use std::time::Duration;

use eyre::{Result, WrapErr as _, ensure, eyre};
use iroha::client::Client;
use iroha_data_model::{
    prelude::QueryBuilderExt,
    query::transaction::prelude::FindTransactions,
    transaction::{SignedTransaction, TransactionEntrypoint},
};
use tokio::time::{Instant, sleep};

const POLL_INTERVAL: Duration = Duration::from_millis(200);

fn exact_applied_transaction_visible(
    client: &Client,
    transaction: &SignedTransaction,
) -> Result<bool> {
    let expected_hash = transaction.hash_as_entrypoint();
    let expected_entrypoint = TransactionEntrypoint::External(transaction.clone());
    let transactions = client
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
            match exact_applied_transaction_visible(client, transaction) {
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
