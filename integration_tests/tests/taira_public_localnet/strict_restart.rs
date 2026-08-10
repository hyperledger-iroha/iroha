//! Fail-closed four-validator restart and exact catch-up qualification.

use super::*;
use iroha::{
    crypto::HashOf,
    data_model::{
        block::BlockHeader,
        metadata::Metadata,
        query::block::prelude::FindBlocks,
        transaction::{FeePaymentIntent, SignedTransaction},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExactCommittedBlock {
    height: u64,
    hash: HashOf<BlockHeader>,
    parent_hash: Option<HashOf<BlockHeader>>,
}

#[derive(Clone, Copy, Debug)]
struct TipObservation {
    block: ExactCommittedBlock,
    contains_transaction: bool,
}

fn query_tip(client: &Client, transaction: Option<&SignedTransaction>) -> Result<TipObservation> {
    let blocks = client
        .query(FindBlocks)
        .execute_all()
        .wrap_err_with(|| format!("query committed blocks from {}", client.torii_url))?;
    let tip = blocks
        .iter()
        .max_by_key(|block| block.header().height().get())
        .ok_or_else(|| eyre!("{} returned an empty committed chain", client.torii_url))?;
    let contains_transaction = if let Some(transaction) = transaction {
        let expected = transaction.hash_as_entrypoint();
        let matches = tip
            .entrypoint_hashes()
            .filter(|observed| observed == &expected)
            .count();
        ensure!(
            matches <= 1,
            "{} duplicated signed transaction {} in its committed tip",
            client.torii_url,
            transaction.hash()
        );
        if matches == 1 {
            ensure!(
                !tip.is_empty() && tip.external_entrypoint_count() > 0,
                "{} exposed signed transaction {} through an empty or non-external tip",
                client.torii_url,
                transaction.hash()
            );
        }
        matches == 1
    } else {
        false
    };
    Ok(TipObservation {
        block: ExactCommittedBlock {
            height: tip.header().height().get(),
            hash: tip.hash(),
            parent_hash: tip.header().prev_block_hash(),
        },
        contains_transaction,
    })
}

async fn wait_for_all_common_tip(
    clients: &[Client],
    timeout: Duration,
    context: &str,
) -> Result<ExactCommittedBlock> {
    ensure!(!clients.is_empty(), "{context}: validator list is empty");
    let deadline = Instant::now() + timeout;
    let mut last_observed = Vec::new();
    loop {
        let mut tips = Vec::with_capacity(clients.len());
        last_observed.clear();
        for (index, client) in clients.iter().enumerate() {
            match query_tip(client, None) {
                Ok(observed) => {
                    tips.push((index, observed.block));
                    last_observed.push(format!("peer {index}: {:?}", observed.block));
                }
                Err(err) => last_observed.push(format!("peer {index}: {err:?}")),
            }
        }
        for (left_position, (left_index, left)) in tips.iter().enumerate() {
            for (right_index, right) in tips.iter().skip(left_position + 1) {
                ensure!(
                    left.height != right.height || left.hash == right.hash,
                    "{context}: validators {left_index} and {right_index} expose different hashes at height {}: {} != {}",
                    left.height,
                    left.hash,
                    right.hash
                );
            }
        }
        if tips.len() == clients.len() {
            let expected = tips[0].1;
            if tips.iter().all(|(_, tip)| *tip == expected) {
                return Ok(expected);
            }
        }
        ensure!(
            Instant::now() < deadline,
            "{context}: all {} validators failed to expose one exact tip within {timeout:?}: {}",
            clients.len(),
            last_observed.join("; ")
        );
        sleep(STATUS_POLL).await;
    }
}

async fn wait_for_all_signed_tip(
    clients: &[Client],
    transaction: &SignedTransaction,
    required: Option<ExactCommittedBlock>,
    timeout: Duration,
    context: &str,
) -> Result<ExactCommittedBlock> {
    ensure!(!clients.is_empty(), "{context}: validator list is empty");
    let deadline = Instant::now() + timeout;
    let mut canonical = required;
    let mut last_observed = Vec::new();
    loop {
        let mut matching = 0_usize;
        last_observed.clear();
        for (index, client) in clients.iter().enumerate() {
            match query_tip(client, Some(transaction)) {
                Ok(observed) => {
                    if observed.contains_transaction && canonical.is_none() {
                        canonical = Some(observed.block);
                    }
                    if let Some(expected) = canonical {
                        ensure!(
                            observed.block.height <= expected.height,
                            "{context}: validator {index} advanced past required exact tip {expected:?}: {:?}",
                            observed.block
                        );
                        ensure!(
                            observed.block.height != expected.height
                                || observed.block.hash == expected.hash,
                            "{context}: validator {index} exposed a stale/divergent hash at height {}: {} != {}",
                            expected.height,
                            observed.block.hash,
                            expected.hash
                        );
                        if observed.block == expected && observed.contains_transaction {
                            matching = matching.saturating_add(1);
                        }
                    }
                    last_observed.push(format!(
                        "peer {index}: {:?}, contains={}",
                        observed.block, observed.contains_transaction
                    ));
                }
                Err(err) => last_observed.push(format!("peer {index}: {err:?}")),
            }
        }
        if matching == clients.len() {
            return canonical.ok_or_else(|| eyre!("{context}: exact tip identity is absent"));
        }
        ensure!(
            Instant::now() < deadline,
            "{context}: only {matching}/{} validators finalized signed transaction {} at exact tip {canonical:?} within {timeout:?}: {}",
            clients.len(),
            transaction.hash(),
            last_observed.join("; ")
        );
        sleep(STATUS_POLL).await;
    }
}

fn signed_probe(client: &Client, message: String) -> SignedTransaction {
    client.build_transaction(
        [InstructionBox::from(Log::new(Level::INFO, message))],
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    )
}

fn submit_signed(client: &Client, transaction: &SignedTransaction, context: &str) -> Result<()> {
    let submitted = client
        .submit_transaction_blocking(transaction)
        .wrap_err_with(|| context.to_owned())?;
    ensure!(
        *submitted.as_ref() == *transaction.hash().as_ref(),
        "{context}: submitted hash differs from the signed transaction"
    );
    Ok(())
}

async fn strict_process_restart_catchup(
    harness: &mut TairaHarness,
    restart_index: usize,
) -> Result<()> {
    ensure!(
        harness.validator_clients.len() == usize::from(TAIRA_VALIDATORS),
        "strict restart gate requires exactly {TAIRA_VALIDATORS} validators, got {}",
        harness.validator_clients.len()
    );
    ensure!(restart_index < harness.validator_clients.len());
    let baseline = wait_for_all_common_tip(
        &harness.validator_clients,
        READY_TIMEOUT,
        "strict four-validator baseline convergence",
    )
    .await?;
    let healthy = harness
        .validator_clients
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != restart_index)
        .map(|(_, client)| client.clone())
        .collect::<Vec<_>>();
    ensure!(healthy.len() + 1 == harness.validator_clients.len());

    harness.localnet.stop_validator(restart_index)?;
    ensure!(
        get_status_with_retry(&harness.validator_clients[restart_index]).is_err(),
        "validator {restart_index} continued serving status after shutdown"
    );
    ensure!(
        collect_statuses(&healthy)?.len() == healthy.len(),
        "a live validator disappeared during the outage"
    );

    let sentinel = signed_probe(
        &healthy[0],
        format!("strict restart sentinel after height {}", baseline.height),
    );
    submit_signed(
        &healthy[0],
        &sentinel,
        "submit signed sentinel while one validator is down",
    )?;
    let sentinel_block = wait_for_all_signed_tip(
        &healthy,
        &sentinel,
        None,
        READY_TIMEOUT,
        "three live validators must agree on the exact sentinel height/hash",
    )
    .await?;
    ensure!(
        sentinel_block.height
            == baseline
                .height
                .checked_add(1)
                .ok_or_else(|| eyre!("sentinel height overflowed"))?
            && sentinel_block.parent_hash == Some(baseline.hash),
        "sentinel is not the exact successor of the common baseline: baseline={baseline:?}, sentinel={sentinel_block:?}"
    );

    harness.localnet.start_validator(restart_index)?;
    wait_for_status_ready(
        &harness.validator_clients[restart_index],
        Duration::from_secs(RESTART_CATCHUP_TIMEOUT_SECS),
    )
    .await?;
    let recovered = wait_for_all_signed_tip(
        &harness.validator_clients,
        &sentinel,
        Some(sentinel_block),
        Duration::from_secs(RESTART_CATCHUP_TIMEOUT_SECS),
        "restarted validator must reach the exact sentinel height/hash",
    )
    .await?;
    ensure!(recovered == sentinel_block);

    let successor = signed_probe(
        &healthy[0],
        format!(
            "strict restart successor after height {}",
            sentinel_block.height
        ),
    );
    submit_signed(&healthy[0], &successor, "submit signed successor")?;
    let successor_block = wait_for_all_signed_tip(
        &harness.validator_clients,
        &successor,
        None,
        READY_TIMEOUT,
        "all four validators must finalize the exact successor",
    )
    .await?;
    ensure!(
        successor_block.height
            == sentinel_block
                .height
                .checked_add(1)
                .ok_or_else(|| eyre!("successor height overflowed"))?
            && successor_block.parent_hash == Some(sentinel_block.hash)
            && successor_block.hash != sentinel_block.hash,
        "post-restart block is not the exact successor: sentinel={sentinel_block:?}, successor={successor_block:?}"
    );
    ensure!(successor_block.height > baseline.height);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn taira_localnet_restart_catchup_behavior() -> Result<()> {
    init_instruction_registry();
    let _guard = sandbox::serial_guard();

    let temp_dir = localnet_tempdir("taira-restart")?;
    let out_dir = temp_dir.path().join("localnet");
    let result: Result<()> = async {
        let mut harness = setup_taira_harness::<true>(&out_dir, "taira-restart", 0).await?;
        let restart_index = harness
            .validator_clients
            .len()
            .checked_sub(1)
            .ok_or_else(|| eyre!("strict restart gate has no validator to restart"))?;
        strict_process_restart_catchup(&mut harness, restart_index).await
    }
    .await;

    result
}
