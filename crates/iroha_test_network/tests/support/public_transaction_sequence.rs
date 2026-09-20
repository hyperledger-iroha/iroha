//! Public transaction and signed-snapshot assertions shared by real-custody fixtures.
//! Process, routing and credential ownership stay with the calling fixture.

use color_eyre::eyre::{Result, WrapErr as _, ensure, eyre};
use futures::future::try_join_all;
use iroha::client::{AccountTransactionDraft, Client, FeeQuoteRequest};
use iroha_data_model::{
    Level,
    isi::{InstructionBox, Log},
    transaction::{FeePaymentIntent, TransactionAdmissionIntent},
};
use iroha_model_base::metadata::Metadata;
use iroha_test_network::read_on_dedicated_thread;
use norito::json::{self, Value};
use std::{
    fs,
    io::{BufRead as _, BufReader},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::time::{Instant, sleep, timeout_at};

/// Submit one exact signed public transaction, then prove its common committed
/// carrier through every validator's own state as well as global routed state.
/// The caller restarts its real-custody validator between sequence two and three.
pub(super) async fn submit_and_observe(
    client: &Client,
    observers: &[Client],
    sequence: u8,
    preceding_applied_height: u64,
) -> Result<u64> {
    ensure!(
        observers.len() == 4,
        "public transaction observation requires four validators"
    );
    ensure!(
        (1..=3).contains(&sequence),
        "public transaction sequence must be one through three"
    );
    ensure!(
        preceding_applied_height >= 1,
        "preceding height must include applied genesis"
    );
    let mut builder = client.to_builder();
    builder.transaction_status_timeout = crate::FUNCTIONAL_FINALITY_TIMEOUT;
    builder.torii_request_timeout = iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT;
    let client = builder.build()?;
    let account = client.account_client()?;
    let mut payload = account.prepare_transaction(
        AccountTransactionDraft::new(
            vec![InstructionBox::from(Log::new(
                Level::INFO,
                format!("strict Taira public transaction {sequence}"),
            ))],
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        )
        .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced),
    )?;
    let quote = account
        .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })
        .await?;
    ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "fee quote changed the selected payer or gas bound"
    );
    payload.fee_payment = quote.intent;
    let transaction = account.sign_transaction(payload)?;
    let expected_hash = transaction.hash();
    let submitted_hash = account
        .submit_transaction_and_wait(&transaction)
        .await
        .wrap_err("the exact public transaction did not reach state-resolved Applied")?;
    ensure!(
        submitted_hash == expected_hash,
        "submission returned a different signed transaction hash"
    );
    let expected_hex: String = expected_hash
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    // As before extraction, all-peer observation begins after SDK reconciliation.
    // It cannot cancel a potentially durable admission or cause resubmission.
    let deadline = Instant::now() + Duration::from_secs(90);
    let applied_height = timeout_at(deadline, async {
        loop {
            let observations = try_join_all(observers.iter().map(|observer| async move {
                let observation_client = observer.clone();
                let bounded_client = move || -> Result<Client> {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    ensure!(!remaining.is_zero(), "four-peer public transaction observation exceeded its fixed 90-second deadline");
                    let mut builder = observation_client.to_builder();
                    builder.torii_request_timeout = iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT.min(remaining);
                    Ok(builder.build()?)
                };
                let global = bounded_client()?.fetch_transaction_status_response_global(expected_hash).await?;
                // Global fanout alone cannot demonstrate this validator's state.
                let status = crate::validator_status_until(&bounded_client()?, deadline).await?;
                let local = read_on_dedicated_thread(move || {
                    // Recompute after scheduling, immediately before blocking I/O.
                    bounded_client()?.get_transaction_status_response_local(expected_hash)
                }).await?;
                Ok::<_, color_eyre::Report>((status.blocks, global, local))
            })).await?;
            let all_applied = observations.iter().all(|(height, global, local)| {
                [("global", global), ("local", local)].iter().all(|(scope, response)| {
                    response.as_ref().is_some_and(|response| {
                        response.hash == expected_hex
                            && response.scope == *scope
                            && response.resolved_from == "state"
                            && response.status.kind == "Applied"
                            && response.status.block_height.is_some_and(|applied| applied > 1 && *height >= applied)
                    })
                })
            });
            if all_applied {
                let applied_height = observations[0].1.as_ref().unwrap().status.block_height;
                ensure!(observations.iter().all(|(_, global, local)| {
                    global.as_ref().unwrap().status.block_height == applied_height
                        && local.as_ref().unwrap().status.block_height == applied_height
                }), "peers disagree on the exact transaction's applied height");
                eprintln!("Taira four-peer public transaction Applied in local and global state: hash={expected_hex}, height={applied_height:?}, peer_heights={:?}", observations.iter().map(|(height, _, _)| *height).collect::<Vec<_>>());
                return Ok::<_, color_eyre::Report>(applied_height.expect("all observations have an Applied height"));
            }
            eprintln!("waiting for all four peers to apply exact public transaction {expected_hex}: {observations:?}");
            sleep(Duration::from_millis(200)).await;
        }
    }).await.map_err(|_| eyre!("four-peer public transaction observation exceeded its fixed 90-second deadline"))??;
    ensure!(
        applied_height > preceding_applied_height,
        "each sequential transaction must reach a later committed height"
    );
    Ok(applied_height)
}

/// Inspect the exact snapshot directory containing `current` and `generations`.
/// The native daemon authenticates the signed snapshot during restart; this
/// check requires the complete artifact set before asking it to stop.
pub(super) fn snapshot_height(root: &Path) -> Result<Option<u64>> {
    let pointer = match fs::read_to_string(root.join("current")) {
        Ok(pointer) => pointer,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let digest = pointer.trim();
    ensure!(
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "snapshot generation pointer is malformed"
    );
    let generation = root.join("generations").join(digest);
    for artifact in [
        "snapshot.data",
        "snapshot.sha256",
        "snapshot.sig",
        "snapshot.fast.norito",
        "snapshot.merkle.json",
    ] {
        let metadata = fs::metadata(generation.join(artifact))?;
        ensure!(
            metadata.is_file() && metadata.len() > 0,
            "published snapshot is missing its complete signed artifact set"
        );
    }
    let snapshot: Value = json::from_slice(&fs::read(generation.join("snapshot.data"))?)?;
    let hashes = snapshot
        .get("block_hashes")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("published snapshot has no committed block-hash vector"))?;
    Ok(Some(u64::try_from(hashes.len())?))
}

/// Require the native JSON event to identify the exact created or loaded height.
pub(super) fn snapshot_log_contains_height(
    logs: &[PathBuf],
    message: &str,
    height: u64,
) -> Result<bool> {
    for path in logs {
        for line in BufReader::new(fs::File::open(path)?).lines() {
            let line = line?;
            if !line.contains(message) {
                continue;
            }
            let Ok(record) = json::from_str::<Value>(&line) else {
                continue;
            };
            if record.get("fields").is_some_and(|fields| {
                fields.get("message").and_then(Value::as_str) == Some(message)
                    && fields.get("at_height").and_then(Value::as_u64) == Some(height)
            }) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
