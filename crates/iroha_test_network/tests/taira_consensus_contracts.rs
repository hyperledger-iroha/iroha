//! Mandatory four-validator Ordinary-transaction qualification with production NPoS/DA defaults.
//! Requires a prebuilt native daemon; sandbox denials and missing peers always fail.
use color_eyre::eyre::{self, Result, WrapErr, ensure, eyre};
use futures::future::try_join_all;
use iroha::client::{AccountTransactionDraft, FeeQuoteRequest};
use iroha_data_model::{
    Level,
    isi::{InstructionBox, Log},
    metadata::Metadata,
    transaction::{FeePaymentIntent, TransactionAdmissionIntent},
};
use iroha_test_network::{init_instruction_registry, read_on_dedicated_thread};
use std::{path::Path, time::Duration};
use tokio::time::{Instant, sleep, timeout, timeout_at};

#[path = "support/multiroute.rs"]
mod multiroute;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn four_peer_multiroute_ordinary_transaction_reaches_applied() -> Result<()> {
    init_instruction_registry();
    for variable in ["TEST_NETWORK_BIN_IROHAD", "TEST_NETWORK_BIN_IROHA"] {
        let binary = std::env::var_os(variable)
            .ok_or_else(|| eyre!("{variable} must name the prebuilt native executable"))?;
        ensure!(
            Path::new(&binary).is_file(),
            "{variable} must name an existing executable file"
        );
    }
    let startup_deadline = Instant::now() + Duration::from_secs(180);
    let network = timeout_at(
        startup_deadline,
        tokio::task::spawn_blocking(|| {
            multiroute::network_builder()
                .with_base_seed_if_unset(stringify!(
                    four_peer_multiroute_ordinary_transaction_reaches_applied
                ))
                .build()
        }),
    )
    .await
    .wrap_err("four-peer genesis preparation exceeded its deadline")?
    .wrap_err("four-peer genesis preparation failed")?;
    let result = async {
        timeout_at(startup_deadline, async {
            network.start_all().await?;
            network.ensure_blocks(1).await?;
            Ok::<(), eyre::Report>(())
        })
        .await
        .wrap_err("four-peer startup exceeded its deadline")??;
    let result = timeout(Duration::from_secs(90), async {
        ensure!(network.peers().len() == 4, "the fixture must start all four validators");
        let initial = try_join_all(network.peers().iter().map(|peer| async move {
            let mut client = peer.client().client().clone();
            client.torii_request_timeout = Duration::from_secs(5);
            read_on_dedicated_thread(move || client.get_status()).await
        })).await?;
        ensure!(initial.iter().all(|status| status.blocks >= 1), "all peers must apply genesis");
        let mut client = network.client().client().clone();
        client.transaction_status_timeout = Duration::from_secs(75);
        client.torii_request_timeout = Duration::from_secs(5);
        let account = client.account_client()?;
        // The SDK draft defaults to QueuePlanSynced. Explicit Ordinary admission is the
        // contract under test: autonomous lanes must not starve the global work provider.
        let mut payload = account.prepare_transaction(
            AccountTransactionDraft::new(
                vec![InstructionBox::from(Log::new(Level::INFO, "strict Taira ordinary first transaction".to_owned()))],
                FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            ).with_admission_intent(TransactionAdmissionIntent::Ordinary),
        )?;
        let quote = account.quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload }).await?;
        ensure!(payload.fee_payment.has_same_payer_and_gas_bound(&quote.intent), "fee quote changed the selected payer or gas bound");
        payload.fee_payment = quote.intent;
        let transaction = account.sign_transaction(payload)?;
        let expected_hash = transaction.hash();
        let submitted_hash = account.submit_transaction_and_wait(&transaction).await
            .wrap_err("the exact Ordinary transaction did not reach state-resolved Applied")?;
        ensure!(submitted_hash == expected_hash, "submission returned a different signed transaction hash");
        let expected_hex = expected_hash
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        loop {
            let observations = try_join_all(network.peers().iter().map(|peer| async move {
                let mut client = peer.client().client().clone();
                client.torii_request_timeout = Duration::from_secs(5);
                let global = client.fetch_transaction_status_response_global(expected_hash).await?;
                // Global lookups may fan out to another validator. Prove this
                // peer's own committed state before counting it as applied.
                let (status, local) = read_on_dedicated_thread(move || {
                    Ok((client.get_status()?, client.get_transaction_status_response_local(expected_hash)?))
                }).await?;
                Ok::<_, eyre::Report>((status.blocks, global, local))
            })).await?;
            let all_applied = observations.iter().all(|(height, global, local)| {
                [("global", global), ("local", local)].iter().all(|(scope, response)| response.as_ref().is_some_and(|response| {
                    response.hash == expected_hex
                        && response.scope == *scope
                        && response.resolved_from == "state"
                        && response.status.kind == "Applied"
                        && response.status.block_height.is_some_and(|applied| applied > 1 && *height >= applied)
                }))
            });
            if all_applied {
                let applied_height = observations[0].1.as_ref().unwrap().status.block_height;
                ensure!(observations.iter().all(|(_, global, local)| global.as_ref().unwrap().status.block_height == applied_height && local.as_ref().unwrap().status.block_height == applied_height), "peers disagree on the exact transaction's applied height");
                eprintln!("Taira four-peer Ordinary transaction Applied in local and global state: hash={expected_hex}, height={applied_height:?}, peer_heights={:?}", observations.iter().map(|(height, _, _)| *height).collect::<Vec<_>>());
                return Ok(());
            }
            eprintln!("waiting for all four peers to apply exact Ordinary transaction {expected_hex}: {observations:?}");
            sleep(Duration::from_millis(200)).await;
        }
    }).await;
        result.map_err(|_| {
            eyre!("four-peer Ordinary transaction confirmation exceeded its fixed 90-second deadline")
        })?
    }
    .await;
    network.shutdown().await;
    result
}
