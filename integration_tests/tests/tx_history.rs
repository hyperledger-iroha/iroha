#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Tests covering Torii transaction history pagination and filters.

use std::time::Duration;

use eyre::{Result, bail, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        prelude::*,
        query::parameters::Pagination,
        transaction::signed::{
            SealedTransactionCommitmentPayload, SealedTransactionReveal,
            SignedSealedTransactionCommitment, compute_sealed_transaction_commitment,
        },
    },
};
use iroha_crypto::{Hash, HashOf};
use iroha_test_network::*;
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;
use tokio::time::{Instant, sleep};

#[test]
fn client_has_rejected_and_accepted_txs_should_return_tx_history() -> Result<()> {
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new(),
        stringify!(client_has_rejected_and_accepted_txs_should_return_tx_history),
    )?
    else {
        return Ok(());
    };
    let client = network.client();

    // Given
    let account_id = ALICE_ID.clone();
    let asset_definition_id = AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal")?,
        "xor".parse()?,
    );
    let create_asset = Register::asset_definition({
        let __asset_definition_id = asset_definition_id.clone();
        AssetDefinition::numeric(__asset_definition_id.clone())
            .with_name(__asset_definition_id.name().to_string())
    });
    client.submit_blocking(create_asset)?;

    //When
    let quantity = numeric!(200);
    let asset_id = AssetId::new(asset_definition_id, account_id.clone());
    let mint_existed_asset = Mint::asset_numeric(quantity.clone(), asset_id);
    let mint_not_existed_asset = Mint::asset_numeric(
        quantity,
        AssetId::new(
            AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal")?,
                "foo".parse()?,
            ),
            account_id.clone(),
        ),
    );

    let transactions_count = 10;

    for i in 0..transactions_count {
        let mint_asset = if i % 2 == 0 {
            &mint_existed_asset
        } else {
            &mint_not_existed_asset
        };
        let instructions: Vec<InstructionBox> = vec![mint_asset.clone().into()];
        let transaction = client.build_transaction(instructions, Metadata::default());
        let _ = client.submit_transaction_blocking(&transaction);
    }

    let transactions = client
        .query(FindTransactions::new())
        .with_pagination(Pagination::new(Some(nonzero!(5_u64)), 1))
        .execute_all()?
        .into_iter()
        .filter(|tx| tx.entrypoint().authority() == &account_id)
        .collect::<Vec<_>>();
    assert_eq!(transactions.len(), 5);

    transactions
        .iter()
        .fold(Duration::MAX, |prev_timestamp, tx| {
            assert_eq!(tx.entrypoint().authority(), &account_id);
            match tx.entrypoint() {
                TransactionEntrypoint::External(entrypoint) => {
                    let curr_timestamp = entrypoint.creation_time();
                    // FindTransactions returns transactions in descending order.
                    assert!(prev_timestamp > curr_timestamp);
                    curr_timestamp
                }
                TransactionEntrypoint::PrivateKaigi(_) => {
                    panic!("unexpected private Kaigi entrypoint");
                }
                TransactionEntrypoint::SealedCommitment(_) => {
                    panic!("unexpected sealed commitment entrypoint");
                }
                TransactionEntrypoint::SealedReveal(_) => {
                    panic!("unexpected sealed reveal entrypoint");
                }
                TransactionEntrypoint::Time(_) => {
                    panic!("unexpected time-triggered entrypoint");
                }
            }
        });

    Ok(())
}

fn encode_versioned_entrypoint(entrypoint: &TransactionEntrypoint) -> Vec<u8> {
    let mut bytes = vec![1];
    bytes.extend(norito::codec::encode_adaptive(entrypoint));
    bytes
}

fn entrypoint_status_hash(
    entrypoint_hash: HashOf<TransactionEntrypoint>,
) -> HashOf<SignedTransaction> {
    HashOf::from_untyped_unchecked(Hash::from(entrypoint_hash))
}

async fn submit_entrypoint(
    http: &reqwest::Client,
    client: &Client,
    entrypoint: TransactionEntrypoint,
    timeout: Duration,
) -> Result<HashOf<TransactionEntrypoint>> {
    let entrypoint_hash = entrypoint.hash();
    let body = encode_versioned_entrypoint(&entrypoint);
    let deadline = Instant::now() + timeout;
    loop {
        let response = http
            .post(client.torii_url.join("/transaction/entrypoint")?)
            .header("content-type", "application/x-norito")
            .body(body.clone())
            .send()
            .await?;
        let status = response.status();
        let header_hash = response
            .headers()
            .get("x-iroha-transaction-hash")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let response_body = response.text().await.unwrap_or_default();
        if status == reqwest::StatusCode::ACCEPTED {
            assert_eq!(
                header_hash.as_deref(),
                Some(entrypoint_hash.to_string().as_str()),
                "Torii should return the submitted entrypoint hash"
            );
            return Ok(entrypoint_hash);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS
            && is_retryable_queue_pressure(&response_body)
            && Instant::now() < deadline
        {
            sleep(Duration::from_secs(1)).await;
            continue;
        }
        ensure!(
            status == reqwest::StatusCode::ACCEPTED,
            "entrypoint submit failed with {status}: {response_body}"
        );
    }
}

async fn wait_for_entrypoint_applied(
    client: &Client,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    timeout: Duration,
) -> Result<()> {
    let hash = entrypoint_status_hash(entrypoint_hash);
    let deadline = Instant::now() + timeout;
    let mut last_status = None;
    while Instant::now() < deadline {
        let poll_client = client.clone();
        let status =
            tokio::task::spawn_blocking(move || poll_client.get_transaction_status_response(hash))
                .await??;
        if let Some(status) = status {
            let kind = status.status.kind.clone();
            if kind == "Applied" {
                return Ok(());
            }
            if kind == "Rejected" {
                bail!("entrypoint {entrypoint_hash} was rejected: {status:?}");
            }
            last_status = Some(kind);
        }
        sleep(Duration::from_millis(250)).await;
    }
    Err(eyre!(
        "timed out waiting for entrypoint {entrypoint_hash} to apply; last status={last_status:?}"
    ))
}

fn is_retryable_queue_pressure(error: &(impl std::fmt::Display + ?Sized)) -> bool {
    let message = error.to_string();
    message.contains("PRTRY:QUEUE_LATENCY")
        || message.contains("queue_latency_saturated")
        || message.contains("429 Too Many Requests")
}

async fn advance_to_height(network: &Network, target: u64) -> Result<()> {
    let client = network.client();
    let deadline = Instant::now() + network.sync_timeout();
    loop {
        let status_client = client.clone();
        let blocks = tokio::task::spawn_blocking(move || status_client.get_status())
            .await??
            .blocks;
        if blocks >= target {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("timed out advancing chain to height {target}; last height={blocks}");
        }

        let submit_client = network.client();
        let submitted = tokio::task::spawn_blocking(move || {
            submit_client.submit(Log::new(
                Level::INFO,
                "sealed reveal height tick".to_owned(),
            ))
        })
        .await?;
        if let Err(error) = submitted
            && !is_retryable_queue_pressure(&error)
        {
            return Err(error.into());
        }
        sleep(Duration::from_secs(1)).await;
    }
}

#[tokio::test]
async fn sealed_commitment_reveal_gossips_and_explorer_lookup_uses_entrypoint_hash() -> Result<()> {
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new().with_min_peers(4),
        stringify!(sealed_commitment_reveal_gossips_and_explorer_lookup_uses_entrypoint_hash),
    )
    .await?
    else {
        return Ok(());
    };
    let client = network.client();
    let http = reqwest::Client::new();
    let starting_height = client.get_status()?.blocks;
    let reveal_after_height = starting_height + 2;
    let reveal_deadline_height = starting_height + 100;
    let marker = "sealed_reveal_marker".parse::<Name>()?;
    let inner_tx = TransactionBuilder::new(client.chain.clone(), client.account.clone())
        .with_instructions([SetKeyValue::account(
            client.account.clone(),
            marker,
            Json::new("revealed"),
        )])
        .sign(client.key_pair.private_key());
    let salt = [0xC3; 32];
    let commitment_hash = compute_sealed_transaction_commitment(
        &client.chain,
        &inner_tx,
        salt,
        reveal_deadline_height,
    );
    let commitment_payload = SealedTransactionCommitmentPayload::new(
        client.chain.clone(),
        client.account.clone(),
        commitment_hash,
        reveal_after_height,
        reveal_deadline_height,
        None,
    );
    let commitment =
        SignedSealedTransactionCommitment::sign(commitment_payload, client.key_pair.private_key());
    let commitment_entrypoint = TransactionEntrypoint::SealedCommitment(commitment);
    let commitment_entrypoint_hash = submit_entrypoint(
        &http,
        &client,
        commitment_entrypoint,
        network.sync_timeout(),
    )
    .await?;
    wait_for_entrypoint_applied(
        &client,
        commitment_entrypoint_hash,
        network.sync_timeout().max(Duration::from_secs(60)),
    )
    .await?;

    advance_to_height(&network, reveal_after_height).await?;

    let reveal = SealedTransactionReveal::new(commitment_hash, inner_tx, salt);
    let reveal_entrypoint = TransactionEntrypoint::SealedReveal(reveal);
    let reveal_entrypoint_hash =
        submit_entrypoint(&http, &client, reveal_entrypoint, network.sync_timeout()).await?;
    wait_for_entrypoint_applied(
        &client,
        reveal_entrypoint_hash,
        network.sync_timeout().max(Duration::from_secs(60)),
    )
    .await?;

    let detail_url = client.torii_url.join(&format!(
        "/v1/explorer/transactions/{reveal_entrypoint_hash}"
    ))?;
    let response = http
        .get(detail_url)
        .header("accept", "application/json")
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    ensure!(
        status.is_success(),
        "explorer detail lookup failed with {status}: {body}"
    );
    let payload: norito::json::Value = norito::json::from_str(&body)?;
    assert_eq!(
        payload.get("hash").and_then(norito::json::Value::as_str),
        Some(reveal_entrypoint_hash.to_string().as_str())
    );
    assert_eq!(
        payload.get("status").and_then(norito::json::Value::as_str),
        Some("Committed")
    );

    Ok(())
}
