//! Supervised Torii runtime for durable native SoraFS orderbook transactions.
//!
//! The durable `sorafs_node` forwarder is the only local delivery state.
//! Policy, book status, channels, receipts, and transaction outcomes are read
//! from one immutable finalized ledger view. Signing and submission are
//! separate boundaries: an injected runtime/HSM signer sees only an exact
//! fee-quoted payload, and only strict durable Torii ingress can expose the
//! resulting signed transaction.

#![cfg(feature = "app_api")]

use std::{num::NonZeroUsize, sync::Arc, time::Duration};

use axum::http::StatusCode;
use blake3::hash as blake3_hash;
use iroha_core::{
    smartcontracts::ValidSingularQuery,
    state::{StateReadOnly, StateReadOnlyWithTransactions, TransactionsReadOnly, WorldReadOnly},
};
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::BlockHeader,
    isi::InstructionBox,
    query::{
        error::{FindError, QueryExecutionFail},
        sorafs::prelude::{
            FindSorafsOrderbookChannelById, FindSorafsOrderbookChannels,
            FindSorafsOrderbookOrders, FindSorafsOrderbookPolicy,
            FindSorafsOrderbookReceiptById, FindSorafsOrderbookStatus,
            FindSorafsReserveProviderById,
        },
    },
    sorafs::{
        orderbook::{
            ORDERBOOK_MAX_OPEN_ORDERS_V1, ORDERBOOK_MAX_OPEN_SETTLEMENT_CHANNELS_V1,
            ORDERBOOK_QUERY_MAX_ITEMS_V1, OrderbookFinalizedCursorV1, OrderbookLedgerStatusV1,
            OrderbookOrderStatusV1, OrderbookSettlementChannelStatusV1,
        },
        reserve::ReserveLifecycleStage,
    },
    transaction::{
        Executable, SignedTransaction, TransactionBuilder, signed::TransactionEntrypoint,
    },
};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_logger::{debug, warn};
use mv::storage::StorageReadOnly;
use sorafs_manifest::orderbook::{OrderSideV1, decode_order_request_v1, decode_settlement_receipt_v1};
use sorafs_node::{
    config::OrderbookWorkerPolicy,
    orderbook_transaction_forwarder::{
        ORDERBOOK_TRANSACTION_FORWARDER_MAX_SCAN_ITEMS_V1,
        ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1, OrderbookOperationV1,
        OrderbookTransactionDeliveryStateV1, OrderbookTransactionEnqueueResultV1,
        OrderbookTransactionPendingV1, OrderbookTransactionSigningRequestV1,
        validate_orderbook_pending_delivery_v1,
        validate_orderbook_reconciliation_material_v1,
    },
};

use super::orderbook_worker::{
    OrderbookEnvelopeReconciliationV1, OrderbookFinalizedSnapshotV1,
    OrderbookGenerationSnapshotV1, OrderbookMaintenanceDueV1, OrderbookWorkerActionV1,
    plan_orderbook_generation, plan_orderbook_worker_action, reconcile_orderbook_semantics,
};
use crate::{SharedAppState, SoraFsOrderbookTransactionSigner};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SorafsOrderbookTransactionForwarderCursorV1 {
    after_sequence: Option<u64>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SorafsOrderbookTransactionForwarderScanV1 {
    generated: usize,
    generation_replayed: usize,
    generation_deferred: usize,
    scanned: usize,
    finalized: usize,
    signed: usize,
    submitted: usize,
    deferred: usize,
    conflicted: usize,
    rejected: usize,
}

/// Generation is configurable; drain/reconciliation is unconditional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OrderbookWorkerSupervisionV1 {
    generation_enabled: bool,
    drain_enabled: bool,
}

fn orderbook_worker_supervision(policy: OrderbookWorkerPolicy) -> OrderbookWorkerSupervisionV1 {
    OrderbookWorkerSupervisionV1 {
        generation_enabled: policy.enabled(),
        drain_enabled: true,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderbookCommittedExternalOutcomeV1 {
    Applied,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderbookAuthoritativeTransactionOutcomeV1 {
    Absent,
    Applied,
    Rejected,
    Unavailable,
}

#[derive(Debug)]
struct OrderbookFinalizedObservationV1 {
    snapshot: OrderbookFinalizedSnapshotV1,
    transaction_outcome: Option<OrderbookAuthoritativeTransactionOutcomeV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderbookTransactionSubmissionDispositionV1 {
    Submitted,
    DefinitelyNotSubmitted,
    Rejected,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderbookTransactionSubmissionResultV1 {
    Submitted,
    Rejected,
    Deferred,
}

fn classify_local_orderbook_transaction_submission(
    error: &iroha_core::queue::Error,
) -> OrderbookTransactionSubmissionDispositionV1 {
    match error {
        iroha_core::queue::Error::InBlockchain | iroha_core::queue::Error::IsInQueue => {
            OrderbookTransactionSubmissionDispositionV1::Submitted
        }
        iroha_core::queue::Error::PlanJournalDurabilityIndeterminate { .. } => {
            OrderbookTransactionSubmissionDispositionV1::Ambiguous
        }
        iroha_core::queue::Error::Expired => {
            OrderbookTransactionSubmissionDispositionV1::Rejected
        }
        _ => OrderbookTransactionSubmissionDispositionV1::DefinitelyNotSubmitted,
    }
}

fn local_orderbook_evidence_blocks_absence_retry(
    queue_pending: bool,
    cache_kind: Option<crate::PipelineStatusKind>,
) -> bool {
    queue_pending
        || matches!(
            cache_kind,
            Some(
                crate::PipelineStatusKind::Queued
                    | crate::PipelineStatusKind::Approved
                    | crate::PipelineStatusKind::Committed
                    | crate::PipelineStatusKind::Applied
            )
        )
}

fn retained_orderbook_transaction_digest(
    retained_digest: Option<[u8; 32]>,
    signed_transaction_bytes: Option<&[u8]>,
) -> Option<[u8; 32]> {
    let retained_digest = retained_digest.filter(|digest| *digest != [0; 32])?;
    let bytes = signed_transaction_bytes?;
    (*blake3_hash(bytes).as_bytes() == retained_digest).then_some(retained_digest)
}

fn classify_orderbook_envelope(
    retained_digest: Option<[u8; 32]>,
    signed_transaction_bytes: Option<&[u8]>,
    finalized_cursor: OrderbookFinalizedCursorV1,
    outcome: OrderbookAuthoritativeTransactionOutcomeV1,
    local_evidence_blocks_absence_retry: bool,
) -> OrderbookEnvelopeReconciliationV1 {
    if finalized_cursor.height == 0 || finalized_cursor.block_hash == [0; 32] {
        return OrderbookEnvelopeReconciliationV1::Unavailable;
    }
    let Some(transaction_digest) =
        retained_orderbook_transaction_digest(retained_digest, signed_transaction_bytes)
    else {
        return OrderbookEnvelopeReconciliationV1::Unavailable;
    };
    match outcome {
        OrderbookAuthoritativeTransactionOutcomeV1::Applied => {
            OrderbookEnvelopeReconciliationV1::Applied {
                transaction_digest,
                finalized_cursor,
            }
        }
        OrderbookAuthoritativeTransactionOutcomeV1::Rejected => {
            OrderbookEnvelopeReconciliationV1::Rejected {
                transaction_digest,
                finalized_cursor,
            }
        }
        OrderbookAuthoritativeTransactionOutcomeV1::Absent
            if local_evidence_blocks_absence_retry =>
        {
            OrderbookEnvelopeReconciliationV1::Pending {
                transaction_digest,
                finalized_cursor,
            }
        }
        OrderbookAuthoritativeTransactionOutcomeV1::Absent => {
            OrderbookEnvelopeReconciliationV1::Absent {
                transaction_digest,
                finalized_cursor,
            }
        }
        OrderbookAuthoritativeTransactionOutcomeV1::Unavailable => {
            OrderbookEnvelopeReconciliationV1::Unavailable
        }
    }
}

fn classify_exact_orderbook_entrypoint_outcome(
    expected_hash: &HashOf<SignedTransaction>,
    block_available: bool,
    results: impl IntoIterator<
        Item = (
            HashOf<SignedTransaction>,
            OrderbookCommittedExternalOutcomeV1,
        ),
    >,
) -> OrderbookAuthoritativeTransactionOutcomeV1 {
    if !block_available {
        return OrderbookAuthoritativeTransactionOutcomeV1::Unavailable;
    }
    let mut exact = results
        .into_iter()
        .filter_map(|(hash, outcome)| (&hash == expected_hash).then_some(outcome));
    let Some(outcome) = exact.next() else {
        return OrderbookAuthoritativeTransactionOutcomeV1::Unavailable;
    };
    if exact.next().is_some() {
        // A transaction hash must have exactly one external entrypoint result.
        return OrderbookAuthoritativeTransactionOutcomeV1::Unavailable;
    }
    match outcome {
        OrderbookCommittedExternalOutcomeV1::Applied => {
            OrderbookAuthoritativeTransactionOutcomeV1::Applied
        }
        OrderbookCommittedExternalOutcomeV1::Rejected => {
            OrderbookAuthoritativeTransactionOutcomeV1::Rejected
        }
    }
}

fn inspect_indexed_orderbook_transaction(
    kura: &iroha_core::kura::Kura,
    transaction_hash: &HashOf<SignedTransaction>,
    block_height: NonZeroUsize,
    expected_block_hash: HashOf<BlockHeader>,
) -> OrderbookAuthoritativeTransactionOutcomeV1 {
    let Some(block) = kura.get_block(block_height) else {
        return classify_exact_orderbook_entrypoint_outcome(
            transaction_hash,
            false,
            std::iter::empty::<(
                HashOf<SignedTransaction>,
                OrderbookCommittedExternalOutcomeV1,
            )>(),
        );
    };
    let Ok(block_height_u64) = u64::try_from(block_height.get()) else {
        return OrderbookAuthoritativeTransactionOutcomeV1::Unavailable;
    };
    if block.header().height().get() != block_height_u64 || block.hash() != expected_block_hash {
        return OrderbookAuthoritativeTransactionOutcomeV1::Unavailable;
    }
    let external_entrypoint_count = block.external_entrypoint_count();
    classify_exact_orderbook_entrypoint_outcome(
        transaction_hash,
        true,
        block
            .entrypoint_results()
            .take(external_entrypoint_count)
            .filter_map(|(_, entrypoint, result)| {
                let TransactionEntrypoint::External(transaction) = entrypoint else {
                    return None;
                };
                let outcome = if result.0.is_ok() {
                    OrderbookCommittedExternalOutcomeV1::Applied
                } else {
                    OrderbookCommittedExternalOutcomeV1::Rejected
                };
                Some((transaction.hash(), outcome))
            }),
    )
}

fn orderbook_finalized_cursor_from_view(
    view: &impl StateReadOnly,
) -> Option<OrderbookFinalizedCursorV1> {
    u64::try_from(view.block_hashes().len())
        .ok()
        .zip(view.block_hashes().last())
        .map(|(height, hash)| OrderbookFinalizedCursorV1 {
            height,
            block_hash: *hash.as_ref(),
        })
        .filter(|cursor| cursor.height != 0 && cursor.block_hash != [0; 32])
}

fn current_orderbook_finalized_cursor(
    state: &SharedAppState,
) -> Option<OrderbookFinalizedCursorV1> {
    let view = state.state.query_view();
    orderbook_finalized_cursor_from_view(&view)
}

fn orderbook_finalized_tip_from_view(
    view: &impl StateReadOnly,
) -> Option<(OrderbookFinalizedCursorV1, u64)> {
    let finalized_cursor = orderbook_finalized_cursor_from_view(view)?;
    let block = view.latest_block()?;
    if block.header().height().get() != finalized_cursor.height
        || block.hash().as_ref() != &finalized_cursor.block_hash
    {
        return None;
    }
    let finalized_at_unix = u64::try_from(block.header().creation_time().as_millis())
        .ok()
        .map(|millis| millis / 1_000)
        .filter(|timestamp| *timestamp != 0)?;
    Some((finalized_cursor, finalized_at_unix))
}

fn orderbook_maintenance_due_in_view(
    view: &impl StateReadOnly,
    finalized_cursor: OrderbookFinalizedCursorV1,
    status: &OrderbookLedgerStatusV1,
    finalized_at_unix: u64,
) -> Result<OrderbookMaintenanceDueV1, ()> {
    let active_orders = status
        .open_orders
        .checked_add(status.partially_filled_orders)
        .ok_or(())?;
    if active_orders > u64::from(ORDERBOOK_MAX_OPEN_ORDERS_V1)
        || status.open_settlement_channels
            > u64::from(ORDERBOOK_MAX_OPEN_SETTLEMENT_CHANNELS_V1)
    {
        return Ok(OrderbookMaintenanceDueV1::Unknown);
    }

    let mut inspected_orders = 0_u64;
    for lifecycle in [
        OrderbookOrderStatusV1::Open,
        OrderbookOrderStatusV1::PartiallyFilled,
    ] {
        let mut after = None;
        loop {
            let page = FindSorafsOrderbookOrders::new(
                Some(finalized_cursor),
                Some(lifecycle),
                after,
                ORDERBOOK_QUERY_MAX_ITEMS_V1,
            )
            .execute(view)
            .map_err(|_| ())?;
            if page.finalized_cursor != finalized_cursor {
                return Err(());
            }
            for record in &page.orders {
                inspected_orders = inspected_orders.checked_add(1).ok_or(())?;
                let order = decode_order_request_v1(&record.canonical_order).map_err(|_| ())?;
                let provider_revoked = order.side == OrderSideV1::Ask
                    && record.provider_id.is_none_or(|provider_id| {
                        view.world().provider_owners().get(&provider_id) != Some(&record.owner)
                            || !FindSorafsReserveProviderById::new(provider_id)
                                .execute(view)
                                .is_ok_and(|account| {
                                    account.terms.provider_account.subject_id()
                                        == record.owner.subject_id()
                                        && account.lifecycle_stage
                                            != ReserveLifecycleStage::Default
                                })
                    });
                if finalized_at_unix >= order.expiry_unix || provider_revoked {
                    return Ok(OrderbookMaintenanceDueV1::Due);
                }
            }
            match (page.has_more, page.next_after_order_id) {
                (true, Some(next)) => after = Some(next),
                (false, None) => break,
                _ => return Err(()),
            }
        }
    }
    if inspected_orders != active_orders {
        return Ok(OrderbookMaintenanceDueV1::Unknown);
    }

    let mut inspected_channels = 0_u64;
    let mut after = None;
    loop {
        let page = FindSorafsOrderbookChannels::new(
            Some(finalized_cursor),
            Some(OrderbookSettlementChannelStatusV1::Open),
            after,
            ORDERBOOK_QUERY_MAX_ITEMS_V1,
        )
        .execute(view)
        .map_err(|_| ())?;
        if page.finalized_cursor != finalized_cursor {
            return Err(());
        }
        for channel in &page.channels {
            inspected_channels = inspected_channels.checked_add(1).ok_or(())?;
            if finalized_at_unix >= channel.expires_at_unix {
                return Ok(OrderbookMaintenanceDueV1::Due);
            }
        }
        match (page.has_more, page.next_after_channel_id) {
            (true, Some(next)) => after = Some(next),
            (false, None) => break,
            _ => return Err(()),
        }
    }
    if inspected_channels != status.open_settlement_channels {
        return Ok(OrderbookMaintenanceDueV1::Unknown);
    }
    Ok(OrderbookMaintenanceDueV1::NotDue)
}

fn generated_orderbook_operation_in_one_finalized_view(
    state: &SharedAppState,
    policy: OrderbookWorkerPolicy,
) -> Result<
    Option<(
        OrderbookOperationV1,
        sorafs_node::orderbook_transaction_forwarder::OrderbookTransactionContextV1,
)>,
    (),
> {
    let view = state.state.view();
    let (finalized_cursor, finalized_at_unix) =
        orderbook_finalized_tip_from_view(&view).ok_or(())?;
    let policy_record = FindSorafsOrderbookPolicy.execute(&view).map_err(|_| ())?;
    if policy_record.activated_at_unix == 0
        || policy_record.activated_at_unix > finalized_at_unix
    {
        return Err(());
    }
    let status = FindSorafsOrderbookStatus.execute(&view).map_err(|_| ())?;
    let maintenance_due = orderbook_maintenance_due_in_view(
        &view,
        finalized_cursor,
        &status,
        finalized_at_unix,
    )?;
    let snapshot = OrderbookGenerationSnapshotV1 {
        finalized_cursor,
        finalized_at_unix,
        policy_record: policy_record.clone(),
        status: status.clone(),
        maintenance_due,
    };
    let operation = plan_orderbook_generation(
        &snapshot,
        policy.match_batch_limit(),
        policy.maintenance_batch_limit(),
    )
    .map_err(|_| ())?;
    Ok(operation.map(|operation| {
        (
            operation,
            sorafs_node::orderbook_transaction_forwarder::OrderbookTransactionContextV1 {
                chain_id: state.chain_id.as_ref().clone(),
                policy_record,
                book_revision: status.book_revision,
                finalized_cursor,
            },
        )
    }))
}

fn receipt_identity(
    retained: &OrderbookTransactionSigningRequestV1,
) -> Option<([u8; 32], [u8; 32])> {
    let OrderbookOperationV1::SettlementReceipt(instruction) = &retained.operation else {
        return None;
    };
    let receipt = decode_settlement_receipt_v1(instruction.receipt_payload()).ok()?;
    Some((receipt.receipt_id, receipt.channel_id))
}

fn orderbook_snapshot_in_view(
    view: &impl StateReadOnly,
    delivery: &OrderbookTransactionPendingV1,
    retained: &OrderbookTransactionSigningRequestV1,
) -> Option<OrderbookFinalizedSnapshotV1> {
    let (finalized_cursor, finalized_at_unix) = orderbook_finalized_tip_from_view(view)?;
    let baseline_block_hash = delivery
        .baseline_finalized_height
        .checked_sub(1)
        .and_then(|height| usize::try_from(height).ok())
        .and_then(|index| view.block_hashes().get(index))
        .map(|hash| *hash.as_ref());

    let policy_record = match FindSorafsOrderbookPolicy.execute(view) {
        Ok(record) => Some(record),
        Err(QueryExecutionFail::Find(FindError::SorafsOrderbookPolicy)) => None,
        Err(_) => return None,
    };
    let status = match FindSorafsOrderbookStatus.execute(view) {
        Ok(status) => Some(status),
        Err(QueryExecutionFail::Find(FindError::SorafsOrderbookStatus)) => None,
        Err(_) => return None,
    };

    let (settlement_receipt, settlement_channel) =
        if let Some((receipt_id, channel_id)) = receipt_identity(retained) {
            let receipt = match FindSorafsOrderbookReceiptById::new(receipt_id).execute(view) {
                Ok(receipt) => Some(receipt),
                Err(QueryExecutionFail::Find(FindError::SorafsOrderbookReceipt(missing)))
                    if missing == receipt_id =>
                {
                    None
                }
                Err(_) => return None,
            };
            let channel = match FindSorafsOrderbookChannelById::new(channel_id).execute(view) {
                Ok(channel) => Some(channel),
                Err(QueryExecutionFail::Find(FindError::SorafsOrderbookChannel(missing)))
                    if missing == channel_id =>
                {
                    None
                }
                Err(_) => return None,
            };
            (receipt, channel)
        } else {
            (None, None)
        };

    Some(OrderbookFinalizedSnapshotV1 {
        finalized_cursor,
        finalized_at_unix,
        baseline_block_hash,
        policy_record,
        status,
        settlement_receipt,
        settlement_channel,
    })
}

fn observe_orderbook_transaction_in_one_finalized_view(
    state: &SharedAppState,
    delivery: &OrderbookTransactionPendingV1,
    retained: &OrderbookTransactionSigningRequestV1,
    transaction_hash: Option<&HashOf<SignedTransaction>>,
) -> Option<OrderbookFinalizedObservationV1> {
    let view = state.state.view();
    let snapshot = orderbook_snapshot_in_view(&view, delivery, retained)?;
    let transaction_outcome =
        transaction_hash.map(
            |transaction_hash| match view.transactions().get(transaction_hash) {
                None => OrderbookAuthoritativeTransactionOutcomeV1::Absent,
                Some(block_height) if block_height.get() > view.block_hashes().len() => {
                    OrderbookAuthoritativeTransactionOutcomeV1::Unavailable
                }
                Some(block_height) => {
                    let Some(expected_block_hash) = view
                        .block_hashes()
                        .get(block_height.get().saturating_sub(1))
                        .copied()
                    else {
                        return OrderbookAuthoritativeTransactionOutcomeV1::Unavailable;
                    };
                    inspect_indexed_orderbook_transaction(
                        view.kura(),
                        transaction_hash,
                        block_height,
                        expected_block_hash,
                    )
                }
            },
        );
    Some(OrderbookFinalizedObservationV1 {
        snapshot,
        transaction_outcome,
    })
}

fn decode_exact_orderbook_signed_transaction(
    bytes: &[u8],
    request: &OrderbookTransactionSigningRequestV1,
) -> Option<SignedTransaction> {
    if bytes.is_empty() || bytes.len() > ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1 {
        return None;
    }
    norito::core::from_bytes_view(bytes).ok()?;
    let total_elements = bytes.len().checked_mul(8)?;
    let total_allocated_bytes = bytes.len().checked_mul(20)?.checked_add(512 * 1024)?;
    let limits = norito::DecodeLimits::new(
        ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1,
        ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1,
        total_elements,
        total_allocated_bytes,
        128,
    );
    let transaction =
        norito::decode_from_bytes_with_limits::<SignedTransaction>(bytes, limits).ok()?;
    if norito::to_bytes(&transaction).ok()?.as_slice() != bytes
        || transaction.verify_signature().is_err()
        || transaction.chain() != &request.chain_id
        || transaction.authority() != &request.authority
    {
        return None;
    }
    let expected_instruction = InstructionBox::from(request.operation.clone());
    match transaction.instructions() {
        Executable::Instructions(instructions)
            if instructions.len() == 1 && instructions.first() == Some(&expected_instruction) =>
        {
            Some(transaction)
        }
        _ => None,
    }
}

/// Start unconditional durable drain/reconciliation.
///
/// `orderbook_worker.enabled` controls only generation of new matcher or
/// maintenance operations. It never suppresses restart recovery for entries
/// already admitted to the durable outbox.
pub(crate) fn spawn_sorafs_orderbook_transaction_forwarder_worker(
    state: SharedAppState,
    shutdown_signal: ShutdownSignal,
) {
    let policy = state.sorafs_node.config().orderbook_worker_policy();
    let supervision = orderbook_worker_supervision(policy);
    debug_assert!(supervision.drain_enabled);
    if !supervision.generation_enabled {
        debug!(
            "SoraFS orderbook generation is disabled; durable drain and finalized reconciliation remain active"
        );
    }
    if state.sorafs_orderbook_transaction_signer.is_none() {
        warn!(
            "SoraFS orderbook forwarder has no runtime signer; durable drain and finalized reconciliation remain active"
        );
    }

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(policy.scan_interval());
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut cursor = SorafsOrderbookTransactionForwarderCursorV1::default();
        loop {
            tokio::select! {
                () = shutdown_signal.receive() => break,
                _ = interval.tick() => {
                    let scan =
                        run_sorafs_orderbook_transaction_forwarder_scan(&state, &mut cursor).await;
                    if scan.generated != 0
                        || scan.generation_replayed != 0
                        || scan.generation_deferred != 0
                        || scan.scanned != 0
                    {
                        debug!(
                            generated = scan.generated,
                            generation_replayed = scan.generation_replayed,
                            generation_deferred = scan.generation_deferred,
                            scanned = scan.scanned,
                            finalized = scan.finalized,
                            signed = scan.signed,
                            submitted = scan.submitted,
                            deferred = scan.deferred,
                            conflicted = scan.conflicted,
                            rejected = scan.rejected,
                            generation_enabled = supervision.generation_enabled,
                            "processed durable native SoraFS orderbook transactions"
                        );
                    }
                }
            }
        }
    });
}

pub(crate) async fn run_sorafs_orderbook_transaction_forwarder_scan(
    state: &SharedAppState,
    cursor: &mut SorafsOrderbookTransactionForwarderCursorV1,
) -> SorafsOrderbookTransactionForwarderScanV1 {
    let mut scan = SorafsOrderbookTransactionForwarderScanV1::default();
    let policy = state.sorafs_node.config().orderbook_worker_policy();
    if policy.enabled() {
        match generated_orderbook_operation_in_one_finalized_view(state, policy) {
            Ok(Some((operation, context))) => {
                match state
                    .sorafs_node
                    .enqueue_orderbook_transaction(operation, &context)
                {
                    Ok(OrderbookTransactionEnqueueResultV1::Inserted { .. }) => {
                        scan.generated = scan.generated.saturating_add(1);
                    }
                    Ok(OrderbookTransactionEnqueueResultV1::Existing { .. }) => {
                        scan.generation_replayed = scan.generation_replayed.saturating_add(1);
                    }
                    Err(_) => {
                        scan.generation_deferred = scan.generation_deferred.saturating_add(1);
                        warn!("failed to durably enqueue generated native SoraFS orderbook work");
                    }
                }
            }
            Ok(None) => {}
            Err(()) => {
                scan.generation_deferred = scan.generation_deferred.saturating_add(1);
                warn!(
                    "failed to derive native SoraFS orderbook work from one finalized ledger view"
                );
            }
        }
    }

    let scan_limit = ORDERBOOK_TRANSACTION_FORWARDER_MAX_SCAN_ITEMS_V1;
    let pending = match state
        .sorafs_node
        .pending_orderbook_transactions_after(cursor.after_sequence, scan_limit)
    {
        Ok(pending) => pending,
        Err(_) => {
            warn!("failed to load durable native SoraFS orderbook transactions");
            return scan;
        }
    };

    for delivery in pending {
        cursor.after_sequence = Some(delivery.sequence);
        scan.scanned = scan.scanned.saturating_add(1);
        if validate_orderbook_pending_delivery_v1(&delivery).is_err() {
            scan.deferred = scan.deferred.saturating_add(1);
            warn!("durable native SoraFS orderbook delivery failed validation");
            continue;
        }

        let retained = match state
            .sorafs_node
            .orderbook_transaction_operation_for_reconciliation(delivery.operation_id)
        {
            Ok(retained)
                if retained.operation_id == delivery.operation_id
                    && retained.chain_id == delivery.chain_id
                    && retained.authority == delivery.authority
                    && validate_orderbook_reconciliation_material_v1(&delivery, &retained)
                        .is_ok() =>
            {
                retained
            }
            Ok(_) | Err(_) => {
                scan.deferred = scan.deferred.saturating_add(1);
                warn!("failed to read exact native SoraFS orderbook semantics");
                continue;
            }
        };

        let exact_transaction = match delivery.signed_transaction_bytes.as_deref() {
            Some(bytes) => {
                if retained_orderbook_transaction_digest(
                    delivery.transaction_digest,
                    Some(bytes),
                )
                .is_none()
                {
                    scan.deferred = scan.deferred.saturating_add(1);
                    warn!("durable native SoraFS orderbook transaction digest is invalid");
                    continue;
                }
                let Some(transaction) =
                    decode_exact_orderbook_signed_transaction(bytes, &retained)
                else {
                    scan.deferred = scan.deferred.saturating_add(1);
                    warn!("durable native SoraFS orderbook transaction bytes failed validation");
                    continue;
                };
                Some(transaction)
            }
            None => None,
        };
        let exact_transaction_hash = exact_transaction.as_ref().map(SignedTransaction::hash);
        let local_evidence_before = exact_transaction_hash.as_ref().is_some_and(|hash| {
            let queue_pending = state
                .queue
                .contains_pending_hash(hash.clone(), &state.state);
            let cache_kind = state
                .pipeline_status_cache
                .lookup(hash)
                .map(|entry| entry.kind);
            local_orderbook_evidence_blocks_absence_retry(queue_pending, cache_kind)
        });

        let Some(observation) = observe_orderbook_transaction_in_one_finalized_view(
            state,
            &delivery,
            &retained,
            exact_transaction_hash.as_ref(),
        ) else {
            scan.deferred = scan.deferred.saturating_add(1);
            warn!("authoritative SoraFS orderbook observation failed");
            continue;
        };
        let semantics = reconcile_orderbook_semantics(
            state.chain_id.as_ref(),
            &delivery,
            &retained,
            &observation.snapshot,
        );
        let local_evidence_after = exact_transaction_hash.as_ref().is_some_and(|hash| {
            let queue_pending = state
                .queue
                .contains_pending_hash(hash.clone(), &state.state);
            let cache_kind = state
                .pipeline_status_cache
                .lookup(hash)
                .map(|entry| entry.kind);
            local_orderbook_evidence_blocks_absence_retry(queue_pending, cache_kind)
        });
        let envelope = match observation.transaction_outcome {
            Some(outcome) => classify_orderbook_envelope(
                delivery.transaction_digest,
                delivery.signed_transaction_bytes.as_deref(),
                observation.snapshot.finalized_cursor,
                outcome,
                local_evidence_before || local_evidence_after,
            ),
            None => OrderbookEnvelopeReconciliationV1::NotSigned,
        };
        let signer_authority = state
            .sorafs_orderbook_transaction_signer
            .as_ref()
            .map(|signer| signer.authority());
        let action = plan_orderbook_worker_action(
            state.chain_id.as_ref(),
            signer_authority.as_ref(),
            &delivery,
            envelope,
            semantics,
        );

        apply_orderbook_worker_action(
            state,
            &delivery,
            &retained,
            exact_transaction,
            action,
            &mut scan,
        )
        .await;
    }
    scan
}

async fn apply_orderbook_worker_action(
    state: &SharedAppState,
    delivery: &OrderbookTransactionPendingV1,
    retained: &OrderbookTransactionSigningRequestV1,
    exact_transaction: Option<SignedTransaction>,
    action: OrderbookWorkerActionV1,
    scan: &mut SorafsOrderbookTransactionForwarderScanV1,
) {
    match action {
        OrderbookWorkerActionV1::FinalizeExact {
            transaction_digest,
            finalized_cursor,
        } => match state.sorafs_node.mark_orderbook_transaction_finalized(
            delivery.operation_id,
            transaction_digest,
            finalized_cursor,
        ) {
            Ok(()) => scan.finalized = scan.finalized.saturating_add(1),
            Err(_) => scan.deferred = scan.deferred.saturating_add(1),
        },
        OrderbookWorkerActionV1::FinalizeSemantic { finalized_cursor } => {
            match state
                .sorafs_node
                .mark_orderbook_transaction_semantic_finalized(
                    delivery.operation_id,
                    finalized_cursor,
                ) {
                Ok(()) => scan.finalized = scan.finalized.saturating_add(1),
                Err(_) => scan.deferred = scan.deferred.saturating_add(1),
            }
        }
        OrderbookWorkerActionV1::DeadLetterConflict { finalized_cursor } => {
            match state
                .sorafs_node
                .mark_orderbook_transaction_finalized_conflict(
                    delivery.operation_id,
                    finalized_cursor,
                ) {
                Ok(()) => scan.conflicted = scan.conflicted.saturating_add(1),
                Err(_) => scan.deferred = scan.deferred.saturating_add(1),
            }
        }
        OrderbookWorkerActionV1::MarkFinalizedAbsent { finalized_cursor } => {
            if state
                .sorafs_node
                .mark_orderbook_transaction_finalized_absent(
                    delivery.operation_id,
                    finalized_cursor,
                )
                .is_err()
            {
                warn!(
                    "failed to checkpoint authoritative absence for a native SoraFS orderbook transaction"
                );
            }
            scan.deferred = scan.deferred.saturating_add(1);
        }
        OrderbookWorkerActionV1::AdoptExactPending => {
            let transitioned = if delivery.state == OrderbookTransactionDeliveryStateV1::Signed {
                state
                    .sorafs_node
                    .begin_orderbook_transaction_submission(delivery.operation_id)
                    .is_ok_and(|bytes| {
                        delivery
                            .signed_transaction_bytes
                            .as_deref()
                            .is_some_and(|expected| bytes.as_slice() == expected)
                    })
            } else {
                true
            };
            if transitioned
                && state
                    .sorafs_node
                    .mark_orderbook_transaction_submitted(delivery.operation_id)
                    .is_ok()
            {
                scan.submitted = scan.submitted.saturating_add(1);
            } else {
                scan.deferred = scan.deferred.saturating_add(1);
            }
        }
        OrderbookWorkerActionV1::MarkTransactionRejected { finalized_cursor } => {
            let transitioned = if delivery.state == OrderbookTransactionDeliveryStateV1::Signed {
                state
                    .sorafs_node
                    .begin_orderbook_transaction_submission(delivery.operation_id)
                    .is_ok_and(|bytes| {
                        delivery
                            .signed_transaction_bytes
                            .as_deref()
                            .is_some_and(|expected| bytes.as_slice() == expected)
                    })
            } else {
                true
            };
            if transitioned
                && state
                    .sorafs_node
                    .mark_orderbook_transaction_rejected(
                        delivery.operation_id,
                        finalized_cursor,
                    )
                    .is_ok()
            {
                scan.rejected = scan.rejected.saturating_add(1);
            } else {
                scan.deferred = scan.deferred.saturating_add(1);
            }
        }
        OrderbookWorkerActionV1::ClaimForSigning => {
            let Some(signer) = state.sorafs_orderbook_transaction_signer.clone() else {
                scan.deferred = scan.deferred.saturating_add(1);
                return;
            };
            let claimed = match state
                .sorafs_node
                .claim_orderbook_transaction_for_signing(delivery.operation_id)
            {
                Ok(claimed) if claimed == *retained => claimed,
                Ok(_) | Err(_) => {
                    let _ = state
                        .sorafs_node
                        .release_orderbook_transaction_signing_claim(delivery.operation_id);
                    scan.deferred = scan.deferred.saturating_add(1);
                    return;
                }
            };
            let Some((transaction, transaction_bytes)) =
                sign_sorafs_orderbook_transaction(state, signer, &claimed).await
            else {
                let _ = state
                    .sorafs_node
                    .release_orderbook_transaction_signing_claim(delivery.operation_id);
                scan.deferred = scan.deferred.saturating_add(1);
                return;
            };
            match state
                .sorafs_node
                .store_signed_orderbook_transaction(delivery.operation_id, &transaction_bytes)
            {
                Ok(transaction_digest)
                    if transaction_digest == *blake3_hash(&transaction_bytes).as_bytes() =>
                {
                    scan.signed = scan.signed.saturating_add(1);
                }
                Ok(_) | Err(_) => {
                    let _ = state
                        .sorafs_node
                        .release_orderbook_transaction_signing_claim(delivery.operation_id);
                    scan.deferred = scan.deferred.saturating_add(1);
                    return;
                }
            }
            match submit_sorafs_orderbook_transaction(
                state,
                delivery.operation_id,
                transaction_bytes,
                transaction,
            )
            .await
            {
                OrderbookTransactionSubmissionResultV1::Submitted => {
                    scan.submitted = scan.submitted.saturating_add(1);
                }
                OrderbookTransactionSubmissionResultV1::Rejected => {
                    scan.rejected = scan.rejected.saturating_add(1);
                }
                OrderbookTransactionSubmissionResultV1::Deferred => {
                    scan.deferred = scan.deferred.saturating_add(1);
                }
            }
        }
        OrderbookWorkerActionV1::SubmitSignedBytes => {
            let (Some(transaction), Some(transaction_bytes)) = (
                exact_transaction,
                delivery.signed_transaction_bytes.clone(),
            ) else {
                scan.deferred = scan.deferred.saturating_add(1);
                return;
            };
            match submit_sorafs_orderbook_transaction(
                state,
                delivery.operation_id,
                transaction_bytes,
                transaction,
            )
            .await
            {
                OrderbookTransactionSubmissionResultV1::Submitted => {
                    scan.submitted = scan.submitted.saturating_add(1);
                }
                OrderbookTransactionSubmissionResultV1::Rejected => {
                    scan.rejected = scan.rejected.saturating_add(1);
                }
                OrderbookTransactionSubmissionResultV1::Deferred => {
                    scan.deferred = scan.deferred.saturating_add(1);
                }
            }
        }
        OrderbookWorkerActionV1::Defer(_) => {
            scan.deferred = scan.deferred.saturating_add(1);
        }
    }
}

async fn sign_sorafs_orderbook_transaction(
    state: &SharedAppState,
    signer: Arc<dyn SoraFsOrderbookTransactionSigner>,
    request: &OrderbookTransactionSigningRequestV1,
) -> Option<(SignedTransaction, Vec<u8>)> {
    if signer.authority() != request.authority || request.chain_id != *state.chain_id {
        return None;
    }
    let mut builder = TransactionBuilder::new(
        request.chain_id.clone(),
        request.authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([InstructionBox::from(request.operation.clone())]);
    builder.set_ttl(Duration::from_secs(300));
    let mut payload = builder.into_payload().ok()?;
    payload.fee_payment = crate::quote_internal_fee_payment(state, &payload).ok()?;
    let expected_payload = payload.clone();
    let transaction = tokio::task::spawn_blocking(move || signer.sign(payload))
        .await
        .ok()?
        .ok()?;
    if transaction.payload() != &expected_payload {
        return None;
    }
    let bytes = norito::to_bytes(&transaction).ok()?;
    if bytes.len() > ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1
        || decode_exact_orderbook_signed_transaction(&bytes, request).is_none()
    {
        return None;
    }
    Some((transaction, bytes))
}

async fn submit_sorafs_orderbook_transaction(
    state: &SharedAppState,
    operation_id: [u8; 32],
    transaction_bytes: Vec<u8>,
    transaction: SignedTransaction,
) -> OrderbookTransactionSubmissionResultV1 {
    if transaction.chain() != state.chain_id.as_ref() {
        return OrderbookTransactionSubmissionResultV1::Deferred;
    }
    let accepted = match crate::routing::accept_transaction_for_ingress(
        state.chain_id.clone(),
        state.state.clone(),
        transaction.clone(),
        &state.telemetry,
    ) {
        Ok(accepted) => accepted,
        Err(_) => {
            let exact = state
                .sorafs_node
                .begin_orderbook_transaction_submission(operation_id);
            if !exact
                .as_ref()
                .is_ok_and(|bytes| bytes == &transaction_bytes)
            {
                return OrderbookTransactionSubmissionResultV1::Deferred;
            }
            let Some(finalized_cursor) = current_orderbook_finalized_cursor(state) else {
                return OrderbookTransactionSubmissionResultV1::Deferred;
            };
            return if state
                .sorafs_node
                .mark_orderbook_transaction_rejected(operation_id, finalized_cursor)
                .is_ok()
            {
                OrderbookTransactionSubmissionResultV1::Rejected
            } else {
                OrderbookTransactionSubmissionResultV1::Deferred
            };
        }
    };
    let durable_retry_claim = match state
        .queue
        .durable_plan_admission_claim_with_state(&accepted, state.state.as_ref())
    {
        Ok(claim) => claim,
        Err(_) => return OrderbookTransactionSubmissionResultV1::Deferred,
    };
    let routing_plan = if let Some(claim) = durable_retry_claim.as_ref() {
        claim.routing_plan.clone()
    } else {
        match state
            .queue
            .route_plan_with_state(&accepted, state.state.as_ref())
        {
            Ok(plan) => plan,
            Err(_) => return OrderbookTransactionSubmissionResultV1::Deferred,
        }
    };
    let routing_decision = routing_plan.coordinator_route();
    let exact_transaction_bytes = match state
        .sorafs_node
        .begin_orderbook_transaction_submission(operation_id)
    {
        Ok(bytes) => bytes,
        Err(_) => return OrderbookTransactionSubmissionResultV1::Deferred,
    };
    if exact_transaction_bytes != transaction_bytes {
        return OrderbookTransactionSubmissionResultV1::Deferred;
    }

    let disposition = if crate::should_execute_route_locally(state.as_ref(), routing_decision) {
        match crate::routing::push_accepted_transaction_for_ingress_with_routing_plan_strict_durable(
            state.queue.clone(),
            state.state.clone(),
            accepted,
            routing_plan,
        ) {
            Ok(_) => OrderbookTransactionSubmissionDispositionV1::Submitted,
            Err(crate::Error::PushIntoQueue { source, .. }) => {
                classify_local_orderbook_transaction_submission(source.as_ref())
            }
            Err(_) => OrderbookTransactionSubmissionDispositionV1::Ambiguous,
        }
    } else {
        let response = crate::execute_torii_transaction_via_proxy(
            state,
            state,
            transaction.into(),
            routing_plan,
            durable_retry_claim,
            true,
            crate::utils::ResponseFormat::Norito,
        )
        .await;
        if response.status() == StatusCode::ACCEPTED {
            OrderbookTransactionSubmissionDispositionV1::Submitted
        } else {
            OrderbookTransactionSubmissionDispositionV1::Ambiguous
        }
    };

    match disposition {
        OrderbookTransactionSubmissionDispositionV1::Submitted => {
            if state
                .sorafs_node
                .mark_orderbook_transaction_submitted(operation_id)
                .is_ok()
            {
                OrderbookTransactionSubmissionResultV1::Submitted
            } else {
                OrderbookTransactionSubmissionResultV1::Deferred
            }
        }
        OrderbookTransactionSubmissionDispositionV1::DefinitelyNotSubmitted => {
            let _ = state
                .sorafs_node
                .mark_orderbook_transaction_not_submitted(operation_id);
            OrderbookTransactionSubmissionResultV1::Deferred
        }
        OrderbookTransactionSubmissionDispositionV1::Rejected => {
            let Some(finalized_cursor) = current_orderbook_finalized_cursor(state) else {
                return OrderbookTransactionSubmissionResultV1::Deferred;
            };
            if state
                .sorafs_node
                .mark_orderbook_transaction_rejected(operation_id, finalized_cursor)
                .is_ok()
            {
                OrderbookTransactionSubmissionResultV1::Rejected
            } else {
                OrderbookTransactionSubmissionResultV1::Deferred
            }
        }
        OrderbookTransactionSubmissionDispositionV1::Ambiguous => {
            OrderbookTransactionSubmissionResultV1::Deferred
        }
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, HashOf};

    use super::*;

    fn cursor(height: u64, seed: u8) -> OrderbookFinalizedCursorV1 {
        OrderbookFinalizedCursorV1 {
            height,
            block_hash: [seed; 32],
        }
    }

    fn transaction_hash(seed: u8) -> HashOf<SignedTransaction> {
        HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32]))
    }

    #[test]
    fn disabled_generation_never_disables_durable_drain() {
        let supervision = orderbook_worker_supervision(OrderbookWorkerPolicy::default());
        assert!(supervision.drain_enabled);
    }

    #[test]
    fn local_pending_or_committed_evidence_blocks_absence_retry() {
        assert!(local_orderbook_evidence_blocks_absence_retry(true, None));
        for kind in [
            crate::PipelineStatusKind::Queued,
            crate::PipelineStatusKind::Approved,
            crate::PipelineStatusKind::Committed,
            crate::PipelineStatusKind::Applied,
        ] {
            assert!(local_orderbook_evidence_blocks_absence_retry(
                false,
                Some(kind),
            ));
        }
        assert!(!local_orderbook_evidence_blocks_absence_retry(
            false, None,
        ));
    }

    #[test]
    fn finalized_entrypoint_lookup_requires_exactly_one_result() {
        let expected = transaction_hash(1);
        let other = transaction_hash(2);
        assert_eq!(
            classify_exact_orderbook_entrypoint_outcome(
                &expected,
                true,
                [(other, OrderbookCommittedExternalOutcomeV1::Applied)],
            ),
            OrderbookAuthoritativeTransactionOutcomeV1::Unavailable,
        );
        assert_eq!(
            classify_exact_orderbook_entrypoint_outcome(
                &expected,
                true,
                [
                    (
                        expected.clone(),
                        OrderbookCommittedExternalOutcomeV1::Applied,
                    ),
                    (expected, OrderbookCommittedExternalOutcomeV1::Applied),
                ],
            ),
            OrderbookAuthoritativeTransactionOutcomeV1::Unavailable,
        );
    }

    #[test]
    fn envelope_absence_retries_only_without_local_evidence() {
        let bytes = [0xA5; 32];
        let digest = *blake3_hash(&bytes).as_bytes();
        assert!(matches!(
            classify_orderbook_envelope(
                Some(digest),
                Some(&bytes),
                cursor(7, 7),
                OrderbookAuthoritativeTransactionOutcomeV1::Absent,
                true,
            ),
            OrderbookEnvelopeReconciliationV1::Pending { .. }
        ));
        assert!(matches!(
            classify_orderbook_envelope(
                Some(digest),
                Some(&bytes),
                cursor(7, 7),
                OrderbookAuthoritativeTransactionOutcomeV1::Absent,
                false,
            ),
            OrderbookEnvelopeReconciliationV1::Absent { .. }
        ));
    }
}
