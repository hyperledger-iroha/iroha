//! Authenticated Exact12 controller helpers for release integration tests.
//!
//! These helpers deliberately keep the opaque controller handle alive for the
//! complete submit/status flow. A detached operation view is evidence to
//! archive, but it is not authority to resume a status lookup.

use std::{
    thread,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr as _, ensure, eyre};
use iroha::{
    client::{Client, privacy_exact12_action::AuthenticatedPrivacyActionHandleV1},
    data_model::{
        privacy::{
            PrivacyActionExecutionReceiptViewV1, PrivacyActionLocalStateV1,
            PrivacyActionOperationViewV1, PrivacyActionTerminalChainStateV1,
            PrivacyExact12ActionOperationV1, PrivacyExact12ActionRequestV1,
        },
        query::privacy::prelude::FindPrivacyActionExecutionReceiptV1,
        transaction::SignedTransaction,
    },
};

/// Submit one signed Exact12 action through the authenticated Rust controller
/// and wait for a stable terminal projection.
///
/// The request pins the exact fresh manifest observed immediately before
/// submission. The controller independently fetches and validates that
/// manifest, authenticates the signed transaction and Torii request, and
/// requires committed details plus the finalized ID105 receipt before it can
/// report an applied action.
///
/// # Errors
///
/// Returns an error when the time bounds are zero or invalid, fresh manifest
/// admission or submission fails, status refresh fails, or the action does not
/// reach a stable authenticated terminal state before the deadline.
pub fn submit_signed_privacy_action_and_wait_v1(
    client: &Client,
    operation: PrivacyExact12ActionOperationV1,
    transaction: &SignedTransaction,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<AuthenticatedPrivacyActionHandleV1> {
    ensure!(
        !timeout.is_zero(),
        "Exact12 controller timeout must be non-zero"
    );
    ensure!(
        !poll_interval.is_zero(),
        "Exact12 controller poll interval must be non-zero"
    );
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| eyre!("Exact12 controller deadline overflowed"))?;
    let manifest = client
        .get_privacy_capabilities()
        .wrap_err("fetch fresh Exact12 capability manifest before submission")?;
    manifest
        .validate()
        .wrap_err("validate fresh Exact12 capability manifest before submission")?;
    let prepared = Client::prepare_transaction_payload(transaction);
    let request = PrivacyExact12ActionRequestV1::try_new(
        operation,
        prepared.as_bytes().to_vec(),
        Some(manifest.manifest_digest),
    )
    .wrap_err("construct canonical signed Exact12 action request")?;
    let mut handle = client
        .submit_signed_privacy_action_v1(request)
        .wrap_err("submit signed Exact12 action through authenticated controller")?;
    ensure!(
        handle.view().local_state() == PrivacyActionLocalStateV1::Submitted,
        "new authenticated Exact12 handle did not begin in Submitted state"
    );
    ensure!(
        handle.view().operation_schema() == operation
            && handle.view().transaction_hash() == *prepared.hash().as_ref()
            && handle.view().capability_manifest_digest() == manifest.manifest_digest
            && handle.view().capability_committed_height() == manifest.committed_height,
        "authenticated Exact12 handle changed the signed action or fresh manifest binding"
    );

    loop {
        let view = client
            .get_privacy_action_status_v1(&mut handle)
            .wrap_err("refresh authenticated Exact12 action status")?;
        if view.local_state() == PrivacyActionLocalStateV1::Terminal {
            let stable = client
                .get_privacy_action_status_v1(&mut handle)
                .wrap_err("re-read terminal Exact12 action to prove stability")?;
            ensure!(
                stable == view && handle.view() == &view,
                "authenticated Exact12 terminal state changed on immediate re-read"
            );
            return Ok(handle);
        }
        let now = Instant::now();
        ensure!(
            now < deadline,
            "Exact12 action did not reach authenticated terminal state within {timeout:?}"
        );
        thread::sleep(poll_interval.min(deadline.saturating_duration_since(now)));
    }
}

/// Run [`submit_signed_privacy_action_and_wait_v1`] without blocking an async
/// integration-test executor thread.
///
/// # Errors
///
/// Returns the synchronous controller error, or an error if the dedicated
/// blocking task cannot be joined.
pub async fn submit_signed_privacy_action_and_wait_async_v1(
    client: &Client,
    operation: PrivacyExact12ActionOperationV1,
    transaction: &SignedTransaction,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<AuthenticatedPrivacyActionHandleV1> {
    let client = client.clone();
    let transaction = transaction.clone();
    tokio::task::spawn_blocking(move || {
        submit_signed_privacy_action_and_wait_v1(
            &client,
            operation,
            &transaction,
            timeout,
            poll_interval,
        )
    })
    .await
    .map_err(|error| eyre!("Exact12 controller task failed: {error}"))?
}

/// Require the authenticated controller handle to describe the exact applied
/// operation and its finalized native execution receipt.
///
/// Verification-only operations are included: their durable ID105 receipt is
/// the native ledger effect, so a merely successful transaction is not enough.
///
/// # Errors
///
/// Returns an error if the handle is nonterminal, rejected, bound to another
/// operation/effect, or omits execution-time capability or finality evidence.
pub fn require_applied_privacy_action_v1(
    handle: &AuthenticatedPrivacyActionHandleV1,
    operation: PrivacyExact12ActionOperationV1,
) -> Result<&PrivacyActionOperationViewV1> {
    let view = handle.view();
    view.validate()
        .wrap_err("validate authenticated Exact12 applied projection")?;
    ensure!(
        view.protocol_id() == operation.protocol_id()
            && view.operation_schema() == operation
            && view.ledger_effect_kind() == operation.ledger_effect_kind(),
        "authenticated Exact12 projection changed the requested operation or typed ledger effect: {view:?}"
    );
    ensure!(
        view.local_state() == PrivacyActionLocalStateV1::Terminal
            && view.terminal_chain_state() == Some(PrivacyActionTerminalChainStateV1::Applied)
            && view.committed_height().is_some()
            && view.rejection_reason().is_none()
            && handle.typed_rejection_reason().is_none(),
        "authenticated Exact12 projection is not an exact applied terminal result: {view:?}"
    );
    ensure!(
        view.execution_capability_manifest_digest().is_some()
            && view.execution_capability_committed_height().is_some()
            && view.execution_receipt_finalized_height().is_some()
            && view.execution_receipt_finalized_block_hash().is_some(),
        "authenticated Exact12 applied projection omitted execution-time capability or finalized receipt evidence: {view:?}"
    );
    Ok(view)
}

/// Query one peer's finalized ID105 record and require it to match an applied
/// authenticated controller handle exactly.
///
/// A later query may bind the immutable receipt to a newer finalized snapshot;
/// it must never regress, and an equal finality height must retain the exact
/// block hash observed by the controller.
///
/// # Errors
///
/// Returns an error if the query fails or any receipt identity, semantic,
/// admission, capability, or finality binding differs from the handle.
pub fn require_privacy_action_receipt_on_peer_v1(
    client: &Client,
    handle: &AuthenticatedPrivacyActionHandleV1,
) -> Result<PrivacyActionExecutionReceiptViewV1> {
    let view = require_applied_privacy_action_v1(handle, handle.view().operation_schema())?;
    let receipt: PrivacyActionExecutionReceiptViewV1 = client
        .query_single(FindPrivacyActionExecutionReceiptV1::new(
            view.protocol_id(),
            view.transaction_hash(),
            0,
        ))
        .wrap_err("query finalized Exact12 execution receipt from peer")?;
    receipt
        .validate()
        .wrap_err("validate finalized Exact12 execution receipt from peer")?;
    ensure!(
        receipt.network_id == client.network_id
            && receipt.protocol_id == view.protocol_id()
            && receipt.operation_schema == view.operation_schema()
            && receipt.ledger_effect_kind == view.ledger_effect_kind()
            && receipt.transaction_hash == view.transaction_hash()
            && receipt.action_index == 0
            && receipt.transaction_intent_digest == view.transaction_intent_digest()
            && receipt.statement_digest == view.statement_digest()
            && receipt.proof_envelope_hash == view.proof_envelope_hash()
            && Some(receipt.capability_manifest_digest)
                == view.execution_capability_manifest_digest()
            && Some(receipt.capability_committed_height)
                == view.execution_capability_committed_height()
            && Some(receipt.admitted_at_height) == view.committed_height(),
        "peer finalized Exact12 receipt changed an authenticated action or native ledger-effect binding: {receipt:?}"
    );
    let observed_finalized_height = view
        .execution_receipt_finalized_height()
        .expect("applied controller projection carries receipt finality");
    ensure!(
        receipt.finalized_height >= observed_finalized_height,
        "peer finalized Exact12 receipt regressed from height {observed_finalized_height} to {}",
        receipt.finalized_height
    );
    if receipt.finalized_height == observed_finalized_height {
        ensure!(
            Some(&receipt.finalized_block_hash) == view.execution_receipt_finalized_block_hash(),
            "peer Exact12 receipt changed its finalized block at height {observed_finalized_height}"
        );
    }
    Ok(receipt)
}
