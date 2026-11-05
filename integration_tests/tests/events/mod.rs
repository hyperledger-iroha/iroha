//! Event subsystem integration tests.
/// Data event smoke tests.
mod data;
/// Notification event end-to-end tests.
mod notification;
/// Pipeline event propagation scenarios.
mod pipeline;
/// Proof-related event coverage for ZK verification status.
mod proof;
/// Server-sent events endpoint contract tests.
mod sse_smoke;

/// Quote the exact event fixture payload before its hash is used in a stream filter.
async fn quoted_transaction(
    client: &iroha::blocking::Client,
    draft: iroha::client::AccountTransactionDraft,
) -> eyre::Result<iroha::data_model::prelude::SignedTransaction> {
    let account = client.account_client();
    let mut payload = account.prepare_transaction(draft)?;
    let quote = account
        .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })
        .await?;
    eyre::ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "event fixture quote changed payer or gas bound"
    );
    payload.fee_payment = quote.intent;
    account.sign_transaction(payload).map_err(Into::into)
}
