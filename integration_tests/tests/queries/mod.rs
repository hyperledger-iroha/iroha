//! Integration tests covering Torii query APIs.
use integration_tests::sandbox;
use iroha::{
    blocking::Client,
    client::QueryError,
    collections::{Collection, ListQuery, field},
    data_model::{
        prelude::*,
        query::{error::QueryExecutionFail, parameters::MAX_FETCH_SIZE},
    },
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_test_network::*;
use std::time::Duration;
const QUERY_TX_STATUS_TIMEOUT: Duration = Duration::from_secs(120);
fn query_network_builder() -> NetworkBuilder {
    NetworkBuilder::new().with_block_cadence(std::time::Duration::from_secs(2))
}
fn query_client(network: &Network) -> Client {
    let client = network.client();
    let status_timeout = client
        .client()
        .transaction_status_timeout()
        .max(QUERY_TX_STATUS_TIMEOUT)
        .max(network.sync_timeout());
    let min_ttl = status_timeout.saturating_add(Duration::from_secs(120));
    integration_tests::sync::rebind_blocking_client(&client, |client| {
        client.transaction_status_timeout = status_timeout;
        if client.transaction_ttl.is_none_or(|ttl| ttl < min_ttl) {
            client.transaction_ttl = Some(min_ttl);
        }
    })
}
/// Account query scenarios.
mod account;
/// Asset query regression coverage.
mod asset;
/// Metadata query workflow tests.
mod metadata;
/// Proof record query validation.
mod proof;
/// Query error surface verification.
mod query_errors;
/// Role-related queries.
mod role;
/// Smart contract query scenarios.
mod smart_contract;
#[test]
fn query_basic_scenarios() -> eyre::Result<()> {
    let Some((network, rt)) = sandbox::start_network_blocking_or_skip(
        query_network_builder(),
        stringify!(query_basic_scenarios),
    )?
    else {
        return Ok(());
    };
    let client = query_client(&network);
    // too_big_fetch_size_is_not_allowed
    {
        let err = client
            .client()
            .query(FindPeers::new())
            .with_fetch_size(FetchSize::new(Some(MAX_FETCH_SIZE.checked_add(1).unwrap())))
            .execute()
            .expect_err("Should fail");
        assert!(matches!(
            err,
            QueryError::Validation(ValidationFail::QueryFailed(
                QueryExecutionFail::FetchSizeTooBig
            ))
        ));
    }
    // find_blocks_reversed
    {
        submit_ensure_domain_for_network(
            &network,
            &client,
            Domain::new(DomainId::try_new("domain1-blocks", "universal")?),
        )?;
        rt.block_on(async { network.ensure_blocks(2).await })?;
        submit_ensure_domain_for_network(
            &network,
            &client,
            Domain::new(DomainId::try_new("domain2-blocks", "universal")?),
        )?;
        rt.block_on(async { network.ensure_blocks(3).await })?;
        #[derive(norito::JsonDeserialize)]
        struct BlockRow {
            hash: HashOf<iroha_data_model::block::BlockHeader>,
            height: u64,
            prev_block_hash: Option<HashOf<iroha_data_model::block::BlockHeader>>,
        }
        let blocks = client
            .list(Collection::ExplorerBlocks, ListQuery::new().limit(64))
            .map(|row| norito::json::from_value::<BlockRow>(row?).map_err(Into::into))
            .collect::<eyre::Result<Vec<_>>>()?;
        assert!(
            blocks.len() >= 3,
            "expected at least genesis plus two committed blocks"
        );
        assert_eq!(blocks.last().unwrap().height, 1);
        assert_eq!(
            blocks.last().unwrap().prev_block_hash,
            None,
            "genesis block should be last"
        );
        for pair in blocks.windows(2) {
            assert_eq!(pair[0].height, pair[1].height + 1);
            assert_eq!(pair[0].prev_block_hash, Some(pair[1].hash));
        }
    }
    // find_transactions_reversed
    {
        let domain_id: DomainId = DomainId::try_new("domain1-txs", "universal")?;
        let register_domain = domain_setup_instruction(&domain_id, client.client().account())?;
        let account = client.account_client();
        let mut payload =
            account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
                [register_domain.clone()],
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            ))?;
        let quote = client
            .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })?;
        eyre::ensure!(
            payload
                .fee_payment
                .has_same_payer_and_gas_bound(&quote.intent),
            "query fixture quote changed fee authority or gas bound"
        );
        payload.fee_payment = quote.intent;
        let transaction = account.sign_transaction(payload)?;
        let expected_hash = transaction.hash_as_entrypoint();
        client.submit_transaction_and_wait(&transaction)?;
        let latest = client.list_page(&Collection::Transactions, &ListQuery::new().limit(1))?;
        assert_eq!(latest.items.len(), 1);
        let row = &latest.items[0];
        assert_eq!(
            row.get("entrypoint_kind")
                .and_then(norito::json::Value::as_str),
            Some("external")
        );
        // Transaction summaries expose the raw hash selector, while the native
        // instruction DTO below uses Norito's checksum-prefixed hash literal.
        assert_eq!(
            row.get("entrypoint_hash")
                .and_then(norito::json::Value::as_str),
            Some(expected_hash.to_string().as_str()),
            "newest history row must be the submitted transaction"
        );
        assert_eq!(
            row.get("result_ok").and_then(norito::json::Value::as_bool),
            Some(true)
        );
        // The native instruction feed preserves canonical InstructionBox frames;
        // transaction summaries deliberately omit executable payloads.
        let instructions = client.list_page(
            &Collection::ExplorerInstructions,
            &ListQuery::new()
                .limit(64)
                .filter(field("transaction_hash").eq(expected_hash.to_string())),
        )?;
        assert!(
            instructions.next_cursor.is_none(),
            "exact transaction instructions must be complete"
        );
        assert_eq!(instructions.items.len(), 1);
        let instruction_row = &instructions.items[0];
        let instruction_hash: HashOf<TransactionEntrypoint> =
            norito::json::from_value(instruction_row.get("transaction_hash").unwrap().clone())?;
        assert_eq!(instruction_hash, expected_hash);
        assert_eq!(
            instruction_row
                .get("index")
                .and_then(norito::json::Value::as_u64),
            Some(0)
        );
        let instruction: InstructionBox = norito::json::from_value(
            instruction_row
                .get("box")
                .unwrap()
                .get("instruction")
                .unwrap()
                .clone(),
        )?;
        assert_eq!(instruction, register_domain);
    }
    Ok(())
}
