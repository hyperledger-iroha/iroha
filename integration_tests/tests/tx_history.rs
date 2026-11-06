//! Tests covering Torii transaction history pagination and filters.
use eyre::{Result, WrapErr, bail, ensure, eyre};
use futures_util::{StreamExt, TryStreamExt, stream};
use integration_tests::{sandbox, sync::run_on_dedicated_thread};
use iroha::{
    blocking::Client,
    collections::{Collection, ListQuery},
    data_model::{
        parameter::SumeragiParameter,
        prelude::*,
        transaction::signed::{
            SealedTransactionCommitmentPayload, SealedTransactionReveal,
            SignedSealedTransactionCommitment, compute_sealed_transaction_commitment,
        },
    },
};
use iroha_crypto::{Hash, HashOf, Signature};
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::name::Name;
use iroha_primitives::nonzero;
use iroha_test_network::*;
use iroha_test_samples::ALICE_ID;
use iroha_torii::{
    HEADER_ACCOUNT, HEADER_NONCE, HEADER_SIGNATURE, HEADER_TIMESTAMP_MS, Method, Uri,
    canonical_network_request_signature_message, signature_header_value,
};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
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
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "xor".parse()?,
    );
    let create_asset = Register::asset_definition({
        let __asset_definition_id = asset_definition_id.clone();
        AssetDefinition::numeric(
            __asset_definition_id.clone(),
            "xor".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(DomainId::try_new("wonderland", "universal")?),
        )
    });
    client.submit(
        create_asset,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    //When
    let quantity = Quantity::from(200_u32);
    let asset_id = AssetId::new(asset_definition_id, account_id.clone());
    let mint_existed_asset = Mint::asset_quantity(quantity.clone(), asset_id);
    let mint_not_existed_asset = Mint::asset_quantity(
        quantity,
        AssetId::new(
            AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal")?,
                "foo".parse()?,
            ),
            account_id.clone(),
        ),
    );
    let transactions_count = 10;
    let mut expected = Vec::new();
    for i in 0..transactions_count {
        let mint_asset = if i % 2 == 0 {
            &mint_existed_asset
        } else {
            &mint_not_existed_asset
        };
        let transaction =
            _rt.block_on(quoted_transaction(&client, vec![mint_asset.clone().into()]))?;
        let succeeded = i % 2 == 0;
        expected.push((
            transaction.hash_as_entrypoint(),
            transaction.creation_time().as_millis(),
            succeeded,
        ));
        let result = client.submit_transaction_and_wait(&transaction);
        if succeeded {
            result?;
        } else {
            let error = result.expect_err("mint into the missing definition must reject");
            ensure!(
                error
                    .chain()
                    .any(|cause| cause.to_string().contains(&format!(
                        "Failed to find asset definition: `{}`",
                        mint_not_existed_asset.destination().definition()
                    ))),
                "unexpected rejected history transaction: {error:?}"
            );
        }
    }
    // Native history uses opaque continuation, so offset one is one real row
    // followed by the original five-row window. Limits are cursor-independent.
    let skipped = client.list_page(&Collection::Transactions, &ListQuery::new().limit(1))?;
    assert_eq!(skipped.items.len(), 1);
    assert_eq!(skipped.total, None);
    let first: HistoryRow = norito::json::from_value(skipped.items.into_iter().next().unwrap())?;
    assert_eq!(
        first.entrypoint_hash,
        expected.last().unwrap().0.to_string()
    );
    let cursor = skipped
        .next_cursor
        .expect("history continues after offset one");
    let page = client.list_page(
        &Collection::Transactions,
        &ListQuery::new().limit(5).cursor(cursor),
    )?;
    assert_eq!(page.items.len(), 5);
    assert_eq!(page.total, None);
    assert!(
        page.next_cursor.is_some(),
        "the original ten-row history continues"
    );
    let mut previous_timestamp = u64::MAX;
    for (value, (hash, timestamp, succeeded)) in page
        .items
        .into_iter()
        .zip(expected.iter().rev().skip(1).take(5))
    {
        let transaction: HistoryRow = norito::json::from_value(value)?;
        assert_eq!(transaction.authority, account_id);
        assert_eq!(transaction.entrypoint_kind, "external");
        assert_eq!(transaction.entrypoint_hash, hash.to_string());
        assert_eq!(u128::from(transaction.timestamp_ms), *timestamp);
        assert_eq!(transaction.result_ok, *succeeded);
        assert!(previous_timestamp > transaction.timestamp_ms);
        previous_timestamp = transaction.timestamp_ms;
    }
    Ok(())
}
#[derive(norito::JsonDeserialize)]
struct HistoryRow {
    // The Transactions collection exposes the raw hash selector, not the
    // checksum-prefixed Norito Hash DTO used by native instruction rows.
    entrypoint_hash: String,
    authority: AccountId,
    timestamp_ms: u64,
    entrypoint_kind: String,
    result_ok: bool,
}
async fn quoted_transaction(
    client: &Client,
    instructions: Vec<InstructionBox>,
) -> Result<SignedTransaction> {
    let account = client.account_client();
    let mut payload = account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
        instructions,
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    ))?;
    let quote = account
        .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })
        .await?;
    ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "history quote changed payer or gas bound"
    );
    payload.fee_payment = quote.intent;
    account.sign_transaction(payload).map_err(Into::into)
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
    match submit_entrypoint_maybe_rejected(http, client, entrypoint, timeout).await? {
        EntrypointSubmitOutcome::Accepted(hash) => Ok(hash),
        EntrypointSubmitOutcome::Rejected { status, body } => {
            bail!("entrypoint submit failed with {status}: {body}")
        }
    }
}
#[derive(Debug)]
enum EntrypointSubmitOutcome {
    Accepted(HashOf<TransactionEntrypoint>),
    Rejected {
        status: reqwest::StatusCode,
        body: String,
    },
}
async fn submit_entrypoint_maybe_rejected(
    http: &reqwest::Client,
    client: &Client,
    entrypoint: TransactionEntrypoint,
    timeout: Duration,
) -> Result<EntrypointSubmitOutcome> {
    let entrypoint_hash = entrypoint.hash();
    let body = encode_versioned_entrypoint(&entrypoint);
    let deadline = Instant::now() + timeout;
    loop {
        let response = http
            .post(
                client
                    .client()
                    .endpoint()
                    .join("/v1/pipeline/transaction-entrypoints")?,
            )
            .header("content-type", "application/x-norito")
            .body(body.clone())
            .send()
            .await?;
        let status = response.status();
        let header_hash = response
            .headers()
            .get("x-iroha-entrypoint-hash")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let response_body = response.text().await.unwrap_or_default();
        if status == reqwest::StatusCode::ACCEPTED {
            assert_eq!(
                header_hash.as_deref(),
                Some(entrypoint_hash.to_string().as_str()),
                "Torii should return the submitted entrypoint hash"
            );
            return Ok(EntrypointSubmitOutcome::Accepted(entrypoint_hash));
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS
            && is_retryable_queue_pressure(&response_body)
            && Instant::now() < deadline
        {
            sleep(Duration::from_secs(1)).await;
            continue;
        }
        return Ok(EntrypointSubmitOutcome::Rejected {
            status,
            body: response_body,
        });
    }
}
async fn submit_entrypoint_once_maybe_rejected(
    http: &reqwest::Client,
    client: &Client,
    entrypoint: TransactionEntrypoint,
) -> Result<EntrypointSubmitOutcome> {
    let entrypoint_hash = entrypoint.hash();
    let response = http
        .post(
            client
                .client()
                .endpoint()
                .join("/v1/pipeline/transaction-entrypoints")?,
        )
        .header("content-type", "application/x-norito")
        .body(encode_versioned_entrypoint(&entrypoint))
        .send()
        .await?;
    let status = response.status();
    let header_hash = response
        .headers()
        .get("x-iroha-entrypoint-hash")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let response_body = response.text().await.unwrap_or_default();
    if status == reqwest::StatusCode::ACCEPTED {
        assert_eq!(
            header_hash.as_deref(),
            Some(entrypoint_hash.to_string().as_str()),
            "Torii should return the submitted entrypoint hash"
        );
        return Ok(EntrypointSubmitOutcome::Accepted(entrypoint_hash));
    }
    Ok(EntrypointSubmitOutcome::Rejected {
        status,
        body: response_body,
    })
}
async fn submit_entrypoints_round_robin(
    http: &reqwest::Client,
    clients: &[Client],
    entrypoints: Vec<TransactionEntrypoint>,
    timeout: Duration,
    parallelism: usize,
) -> Result<Vec<HashOf<TransactionEntrypoint>>> {
    ensure!(
        !clients.is_empty(),
        "entrypoint submission requires at least one peer client"
    );
    let parallelism = parallelism.max(1).min(clients.len());
    stream::iter(entrypoints.into_iter().enumerate())
        .map(|(index, entrypoint)| {
            let http = http.clone();
            let client = clients[index % clients.len()].clone();
            async move { submit_entrypoint(&http, &client, entrypoint, timeout).await }
        })
        .buffer_unordered(parallelism)
        .try_collect()
        .await
}
async fn wait_for_entrypoint_applied(
    client: &Client,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    timeout: Duration,
) -> Result<Option<u64>> {
    let hash = entrypoint_status_hash(entrypoint_hash);
    let deadline = Instant::now() + timeout;
    let mut last_status = None;
    while Instant::now() < deadline {
        let poll_client = client.clone();
        let status = run_on_dedicated_thread(move || {
            poll_client.client().get_transaction_status_response(hash)
        })
        .await??;
        if let Some(status) = status {
            let kind = status.status.kind.clone();
            if kind == "Applied" {
                return Ok(status.status.block_height);
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
async fn wait_for_entrypoint_rejected(
    client: &Client,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    timeout: Duration,
) -> Result<Option<u64>> {
    let hash = entrypoint_status_hash(entrypoint_hash);
    let deadline = Instant::now() + timeout;
    let mut last_status = None;
    while Instant::now() < deadline {
        let poll_client = client.clone();
        let status = run_on_dedicated_thread(move || {
            poll_client.client().get_transaction_status_response(hash)
        })
        .await??;
        if let Some(status) = status {
            let kind = status.status.kind.clone();
            if kind == "Rejected" {
                return Ok(status.status.block_height);
            }
            last_status = Some(kind);
        }
        sleep(Duration::from_millis(250)).await;
    }
    Err(eyre!(
        "timed out waiting for entrypoint {entrypoint_hash} to reject; last status={last_status:?}"
    ))
}
fn is_retryable_queue_pressure(error: &(impl std::fmt::Display + ?Sized)) -> bool {
    let message = error.to_string();
    message.contains("PRTRY:QUEUE_LATENCY")
        || message.contains("queue_latency_saturated")
        || message.contains("429 Too Many Requests")
}
fn assert_duplicate_reveal_outcome(
    outcome: EntrypointSubmitOutcome,
    reveal_hash: HashOf<TransactionEntrypoint>,
) -> Result<()> {
    match outcome {
        EntrypointSubmitOutcome::Accepted(hash) => {
            assert_eq!(hash, reveal_hash);
        }
        EntrypointSubmitOutcome::Rejected { status, body } => {
            ensure!(
                status.is_client_error(),
                "duplicate reveal should be rejected with a client error or accepted for no-op processing, got {status}: {body}"
            );
        }
    }
    Ok(())
}
async fn advance_to_height(network: &Network, target: u64) -> Result<()> {
    let client = network.client();
    let mut tick_clients = network
        .peers()
        .iter()
        .map(NetworkPeer::client)
        .collect::<Vec<_>>();
    if tick_clients.is_empty() {
        tick_clients.push(client.clone());
    }
    let deadline = Instant::now() + network.sync_timeout().max(Duration::from_secs(480));
    let mut next_tick_client = 0usize;
    let mut last_tick_error = None;
    loop {
        let blocks = client.client().status().get().await?.blocks;
        if blocks >= target {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!(
                "timed out advancing chain to height {target}; last height={blocks}; last tick error={last_tick_error:?}"
            );
        }
        let submit_client = tick_clients[next_tick_client % tick_clients.len()].clone();
        next_tick_client = next_tick_client.wrapping_add(1);
        let submitted = run_on_dedicated_thread(move || {
            submit_client.submit(
                Log::new(Level::INFO, "sealed reveal height tick".to_owned()),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
        })
        .await?;
        match submitted {
            Ok(_) => {}
            Err(error) if is_retryable_queue_pressure(&error) => {
                last_tick_error = Some(error.to_string());
            }
            Err(error) => {
                return Err(error);
            }
        }
        sleep(Duration::from_secs(1)).await;
    }
}
async fn sealed_entrypoints_for_instructions(
    client: &Client,
    instructions: Vec<InstructionBox>,
    salt: [u8; 32],
    reveal_after_height: u64,
    reveal_deadline_height: u64,
) -> Result<(Hash, TransactionEntrypoint, TransactionEntrypoint)> {
    let inner_tx = quoted_transaction(client, instructions).await?;
    let commitment_hash = compute_sealed_transaction_commitment(
        client.client().network_id(),
        &inner_tx,
        salt,
        reveal_deadline_height,
    );
    let commitment_payload = SealedTransactionCommitmentPayload::new(
        *client.client().network_id(),
        client.client().account().clone(),
        commitment_hash,
        reveal_after_height,
        reveal_deadline_height,
        None,
    );
    let commitment = SignedSealedTransactionCommitment::sign(
        commitment_payload,
        client.client().key_pair().private_key(),
    );
    let reveal = SealedTransactionReveal::new(commitment_hash, inner_tx, salt);
    Ok((
        commitment_hash,
        TransactionEntrypoint::SealedCommitment(commitment),
        TransactionEntrypoint::SealedReveal(reveal),
    ))
}
async fn account_has_metadata(client: &Client, key: &Name) -> Result<bool> {
    let id = client.client().account().clone();
    let account = client
        .account_client()
        .query_single(FindAccountById::new(id.clone()))
        .await?;
    assert_eq!(account.id(), &id);
    Ok(account.metadata().contains(key))
}
async fn numeric_asset_value(client: &Client, asset_id: &AssetId) -> Result<Quantity> {
    let asset = client
        .account_client()
        .query_single(FindAssetById::new(asset_id.clone()))
        .await?;
    assert_eq!(asset.id(), asset_id);
    Ok(asset.value().clone())
}
async fn wait_for_all_peers_to_observe_reveals(
    clients: &[Client],
    marker_keys: &[Name],
    asset_id: &AssetId,
    expected_amount: &Quantity,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_gap = String::new();
    loop {
        let mut all_observed = true;
        for (index, client) in clients.iter().enumerate() {
            let peer_result = async {
                for marker in marker_keys {
                    ensure!(
                        account_has_metadata(client, marker).await?,
                        "metadata marker {marker} is missing"
                    );
                }
                let amount = numeric_asset_value(client, asset_id).await?;
                ensure!(
                    amount == *expected_amount,
                    "asset amount is {amount}, expected {expected_amount}"
                );
                Ok::<(), eyre::Report>(())
            }
            .await;
            if let Err(error) = peer_result {
                all_observed = false;
                last_gap = format!("peer {index}: {error}");
                break;
            }
        }
        if all_observed {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for all peers to observe reveals; last gap: {last_gap}");
        }
        sleep(Duration::from_millis(250)).await;
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
    let http = integration_tests::http::client();
    let starting_height = client.client().status().get().await?.blocks;
    let reveal_after_height = starting_height + 2;
    let reveal_deadline_height = starting_height + 100;
    let marker = "sealed_reveal_marker".parse::<Name>()?;
    let (_, commitment_entrypoint, reveal_entrypoint) = sealed_entrypoints_for_instructions(
        &client,
        vec![
            SetKeyValue::account(
                client.client().account().clone(),
                marker,
                Json::new("revealed"),
            )
            .into(),
        ],
        [0xC3; 32],
        reveal_after_height,
        reveal_deadline_height,
    )
    .await?;
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
    let reveal_entrypoint_hash =
        submit_entrypoint(&http, &client, reveal_entrypoint, network.sync_timeout()).await?;
    wait_for_entrypoint_applied(
        &client,
        reveal_entrypoint_hash,
        network.sync_timeout().max(Duration::from_secs(60)),
    )
    .await?;
    let detail_url = client.client().endpoint().join(&format!(
        "/v1/explorer/transactions/{reveal_entrypoint_hash}"
    ))?;
    let response = authenticated_get(&http, &client, detail_url)?
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
    require_explorer_entrypoint_hash(&payload, reveal_entrypoint_hash)?;
    assert_eq!(
        payload.get("status").and_then(norito::json::Value::as_str),
        Some("Committed")
    );
    Ok(())
}

fn require_explorer_entrypoint_hash(
    payload: &norito::json::Value,
    expected: HashOf<TransactionEntrypoint>,
) -> Result<()> {
    // Explorer detail uses the canonical Hash DTO; collection summaries use a
    // different, explicitly raw string field. Compare the complete identity.
    let value = payload
        .get("hash")
        .ok_or_else(|| eyre!("explorer hash missing"))?;
    let actual: Hash = norito::json::from_value(value.clone())?;
    ensure!(
        actual == Hash::from(expected),
        "explorer entrypoint hash mismatch"
    );
    Ok(())
}

#[test]
fn explorer_detail_requires_the_exact_canonical_entrypoint_hash() -> Result<()> {
    let expected =
        HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(b"entrypoint"));
    require_explorer_entrypoint_hash(&norito::json!({"hash": (expected)}), expected)?;
    for payload in [
        norito::json!({"hash": (expected.to_string())}),
        norito::json!({"hash": (Hash::new(b"different entrypoint"))}),
        norito::json!({"hash": null}),
        norito::json!({}),
    ] {
        assert!(require_explorer_entrypoint_hash(&payload, expected).is_err());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sealed_reveal_adversarial_cases_hold_on_multi_peer_network() -> Result<()> {
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new()
            .with_min_peers(4)
            .with_block_cadence(Duration::from_secs(10))
            .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
                SumeragiParameter::PayloadRetryIntervalMs(nonzero!(10_000_u64)),
            )))
            .with_sync_timeout(Duration::from_secs(480)),
        stringify!(sealed_reveal_adversarial_cases_hold_on_multi_peer_network),
    )
    .await?
    else {
        return Ok(());
    };
    let client = network.client();
    let peer_clients: Vec<_> = network.peers().iter().map(NetworkPeer::client).collect();
    ensure!(
        peer_clients.len() >= 4,
        "adversarial sealed reveal coverage requires at least 4 peers"
    );
    let http = integration_tests::http::client();
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "sealeddupmint".parse()?,
    );
    let asset_id = AssetId::new(
        asset_definition_id.clone(),
        client.client().account().clone(),
    );
    read_on_dedicated_thread({
        let client = client.clone();
        let asset_definition_id = asset_definition_id.clone();
        move || {
            client.submit(
                Register::asset_definition(AssetDefinition::numeric(
                    asset_definition_id.clone(),
                    "sealeddupmint".to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    Some(DomainId::try_new("wonderland", "universal")?),
                )),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
        }
    })
    .await?;
    let starting_height = client.client().status().get().await?.blocks;
    let reveal_after_height = starting_height + 3;
    let reveal_deadline_height = starting_height + 30;
    let timeout = network.sync_timeout();
    let status_timeout = timeout.max(Duration::from_secs(90));
    let mut commitment_entrypoints = Vec::new();
    let mut reveal_entrypoints = Vec::new();
    let mut marker_keys = Vec::new();
    let primary_submit_clients = vec![client.clone(); peer_clients.len()];
    for idx in 0..4_u8 {
        let marker = format!("sealed_batch_marker_{idx}").parse::<Name>()?;
        let (_, commitment, reveal) = sealed_entrypoints_for_instructions(
            &client,
            vec![
                SetKeyValue::account(
                    client.client().account().clone(),
                    marker.clone(),
                    Json::new(format!("batch-{idx}")),
                )
                .into(),
            ],
            [0x80 + idx; 32],
            reveal_after_height,
            reveal_deadline_height,
        )
        .await?;
        commitment_entrypoints.push(commitment);
        reveal_entrypoints.push(reveal);
        marker_keys.push(marker);
    }
    let mint_amount = Quantity::from(7_u32);
    let (_, mint_commitment, mint_reveal) = sealed_entrypoints_for_instructions(
        &client,
        vec![Mint::asset_quantity(mint_amount.clone(), asset_id.clone()).into()],
        [0xA5; 32],
        reveal_after_height,
        reveal_deadline_height,
    )
    .await?;
    commitment_entrypoints.push(mint_commitment);
    reveal_entrypoints.push(mint_reveal.clone());
    let expired_marker = "sealed_expired_marker".parse::<Name>()?;
    let expired_reveal_height = reveal_after_height + 1;
    let expired_deadline_height = expired_reveal_height;
    let (_, expired_commitment, expired_reveal) = sealed_entrypoints_for_instructions(
        &client,
        vec![
            SetKeyValue::account(
                client.client().account().clone(),
                expired_marker.clone(),
                Json::new("expired"),
            )
            .into(),
        ],
        [0xE1; 32],
        expired_reveal_height,
        expired_deadline_height,
    )
    .await?;
    commitment_entrypoints.push(expired_commitment);
    let commitment_hashes = submit_entrypoints_round_robin(
        &http,
        &primary_submit_clients,
        commitment_entrypoints,
        timeout,
        1,
    )
    .await
    .wrap_err("submitting sealed commitment batch")?;
    for hash in commitment_hashes {
        wait_for_entrypoint_applied(&client, hash, status_timeout).await?;
    }
    advance_to_height(&network, reveal_after_height).await?;
    let reveal_hashes = submit_entrypoints_round_robin(
        &http,
        &primary_submit_clients,
        reveal_entrypoints.clone(),
        timeout,
        primary_submit_clients.len(),
    )
    .await
    .wrap_err("submitting same-block sealed reveal batch")?;
    let mut reveal_heights = Vec::new();
    for hash in &reveal_hashes {
        let height = wait_for_entrypoint_applied(&client, *hash, status_timeout)
            .await?
            .ok_or_else(|| eyre!("applied reveal {hash} did not report a block height"))?;
        reveal_heights.push(height);
    }
    ensure!(
        reveal_heights
            .iter()
            .all(|height| *height == reveal_heights[0]),
        "sealed reveals should be batched into one block; observed heights={reveal_heights:?}"
    );
    wait_for_all_peers_to_observe_reveals(
        &peer_clients,
        &marker_keys,
        &asset_id,
        &mint_amount,
        status_timeout,
    )
    .await?;
    let mint_reveal_hash = mint_reveal.hash();
    let primary_duplicate =
        submit_entrypoint_maybe_rejected(&http, &client, mint_reveal.clone(), timeout).await?;
    assert_duplicate_reveal_outcome(primary_duplicate, mint_reveal_hash)?;
    let duplicate_replay_client = peer_clients
        .get(1)
        .expect("adversarial test should have a non-primary peer client");
    let duplicate_outcome =
        submit_entrypoint_once_maybe_rejected(&http, duplicate_replay_client, mint_reveal).await?;
    assert_duplicate_reveal_outcome(duplicate_outcome, mint_reveal_hash)?;
    assert_eq!(
        numeric_asset_value(&client, &asset_id).await?,
        mint_amount,
        "duplicate reveal must not execute the sealed inner transaction twice"
    );
    advance_to_height(&network, expired_deadline_height + 1).await?;
    match submit_entrypoint_maybe_rejected(&http, &client, expired_reveal, timeout).await? {
        EntrypointSubmitOutcome::Accepted(hash) => {
            wait_for_entrypoint_rejected(&client, hash, status_timeout).await?;
        }
        EntrypointSubmitOutcome::Rejected { status, body } => {
            ensure!(
                status.is_client_error(),
                "expired reveal should be rejected immediately or by the pipeline, got {status}: {body}"
            );
        }
    }
    ensure!(
        !account_has_metadata(&client, &expired_marker).await?,
        "expired delayed reveal must not execute its inner transaction"
    );
    for peer_client in &peer_clients {
        ensure!(
            !account_has_metadata(peer_client, &expired_marker).await?,
            "expired delayed reveal must not execute on any peer"
        );
    }
    Ok(())
}

fn authenticated_get(
    http: &reqwest::Client,
    client: &Client,
    url: reqwest::Url,
) -> Result<reqwest::RequestBuilder> {
    static NONCE: AtomicU64 = AtomicU64::new(0);
    let uri: Uri = match url.query() {
        Some(query) => format!("{}?{query}", url.path()).parse()?,
        None => url.path().parse()?,
    };
    let timestamp_ms = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    let nonce = format!(
        "sealed-history-{timestamp_ms}-{}",
        NONCE.fetch_add(1, Ordering::Relaxed)
    );
    let message = canonical_network_request_signature_message(
        client.client().network_id(),
        &Method::GET,
        &uri,
        &[],
        timestamp_ms,
        &nonce,
    )?;
    let signature = Signature::try_new(client.client().key_pair().private_key(), &message)?;
    Ok(http
        .get(url)
        .header(
            HEADER_ACCOUNT,
            client.account_client().authority().to_canonical_hex()?,
        )
        .header(HEADER_SIGNATURE, signature_header_value(&signature)?)
        .header(HEADER_TIMESTAMP_MS, timestamp_ms.to_string())
        .header(HEADER_NONCE, nonce))
}
