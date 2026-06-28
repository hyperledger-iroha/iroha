//! Local SoraFS orderbook runtime mirror.

use std::collections::BTreeMap;

use blake3::Hasher;
use sorafs_manifest::{
    OrderBookEntryV1, OrderCancelReasonV1, OrderCancelV1, OrderFillOutcomeV1, OrderRequestV1,
    OrderSideV1, OrderbookRuntimeSnapshotV1, OrderbookValidationError, SettlementChannelV1,
    SettlementReceiptV1, TradeEventV1, apply_settlement_receipt_v1, match_order_book_v1,
    open_settlement_channel_for_trade_v1, verify_order_cancel_signature_v1,
    verify_order_request_signature_v1, verify_settlement_receipt_signature_v1,
};
use thiserror::Error;

const ORDERBOOK_CHANNEL_ID_DOMAIN_V1: &[u8] = b"sorafs.orderbook.local.channel-id.v1";
const ORDERBOOK_PROVIDER_ID_DOMAIN_V1: &[u8] = b"sorafs.orderbook.local.provider-id.v1";

/// Derive the local orderbook provider id from canonical provider owner-account bytes.
///
/// The local mirror uses this deterministic id in settlement-channel snapshots
/// until the durable provider registry/contract owns provider-account binding.
#[must_use]
pub fn local_orderbook_provider_id_for_owner_account(owner_account: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(ORDERBOOK_PROVIDER_ID_DOMAIN_V1);
    hasher.update(owner_account);
    nonzero_digest(*hasher.finalize().as_bytes())
}

/// Error raised by the local orderbook runtime mirror.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OrderbookRuntimeError {
    /// The submitted orderbook payload failed canonical validation.
    #[error(transparent)]
    Validation(#[from] OrderbookValidationError),
    /// The order id is already present in the local open-order book.
    #[error("order id `{order_id_hex}` is already open")]
    DuplicateOrderId {
        /// Hex-encoded duplicate order id.
        order_id_hex: String,
    },
    /// The requested order id is not open in the local book.
    #[error("order id `{order_id_hex}` is not open")]
    OrderNotFound {
        /// Hex-encoded missing order id.
        order_id_hex: String,
    },
    /// The requested settlement channel is not known to the local mirror.
    #[error("settlement channel `{channel_id_hex}` is not open or known")]
    SettlementChannelNotFound {
        /// Hex-encoded missing settlement channel id.
        channel_id_hex: String,
    },
    /// The receipt id was already accepted by the local mirror.
    #[error("settlement receipt `{receipt_id_hex}` was already accepted")]
    DuplicateReceiptId {
        /// Hex-encoded duplicate receipt id.
        receipt_id_hex: String,
    },
    /// The receipt byte range overlaps a previously accepted receipt.
    #[error(
        "settlement receipt `{receipt_id_hex}` overlaps previously accepted receipt `{existing_receipt_id_hex}`"
    )]
    ReceiptRangeOverlap {
        /// Hex-encoded candidate receipt id.
        receipt_id_hex: String,
        /// Hex-encoded existing receipt id with overlapping range.
        existing_receipt_id_hex: String,
    },
    /// A cancel payload was signed for a different owner than the open order.
    #[error("cancel owner does not match open order owner")]
    CancelOwnerMismatch,
    /// The monotonic local admission sequence overflowed.
    #[error("orderbook admission sequence overflow")]
    SequenceOverflow,
    /// The deterministic matcher referenced an order missing from the local pre-match snapshot.
    #[error("matcher output referenced an unknown order")]
    MissingMatchedOrder,
    /// A matched pair did not contain exactly one bid and one ask.
    #[error("matcher output did not contain one bid and one ask")]
    InvalidMatchedSides,
    /// The order quantity is below the configured minimum.
    #[error("order quantity {quantity_gib} GiB is below configured minimum {min_order_gib} GiB")]
    OrderBelowMinimum {
        /// Submitted order quantity.
        quantity_gib: u64,
        /// Configured minimum order quantity.
        min_order_gib: u64,
    },
    /// The order price is not aligned to the configured tick.
    #[error(
        "order price {price_micro_xor} micro-XOR/GiB is not aligned to configured tick {tick_micro_xor} micro-XOR"
    )]
    OrderPriceTickMismatch {
        /// Submitted order price in micro-XOR per GiB.
        price_micro_xor: u128,
        /// Configured price tick in micro-XOR per GiB.
        tick_micro_xor: u64,
    },
    /// The provider's current reserve lifecycle state disables new adverts.
    #[error(
        "reserve lifecycle stage `{stage}` disables orderbook adverts for provider `{provider_id_hex}`"
    )]
    ReserveLifecycleAdvertDisabled {
        /// Hex-encoded provider identifier derived from the order owner account.
        provider_id_hex: String,
        /// Stable reserve lifecycle stage label that triggered advert disablement.
        stage: String,
    },
    /// The local orderbook lock was poisoned.
    #[error("orderbook state lock poisoned")]
    StateLockPoisoned,
}

/// Result of accepting an order into the local orderbook mirror.
#[derive(Debug, Clone)]
pub struct OrderbookSubmitOutcome {
    /// Accepted order payload.
    pub accepted_order: OrderRequestV1,
    /// Local admission sequence assigned to the order.
    pub sequence: u64,
    /// Deterministic fills produced after the order was accepted.
    pub fills: Vec<OrderFillOutcomeV1>,
    /// Settlement channels opened for the produced fills.
    pub settlement_channels_opened: Vec<SettlementChannelV1>,
    /// Order ids expired during the matching pass.
    pub expired_order_ids: Vec<[u8; 32]>,
    /// Number of open orders left after the matching pass.
    pub open_order_count: usize,
}

/// Result of cancelling an open order in the local orderbook mirror.
#[derive(Debug, Clone)]
pub struct OrderbookCancelOutcome {
    /// Cancelled order payload.
    pub cancelled_order: OrderRequestV1,
    /// Cancellation reason supplied by the owner.
    pub reason: OrderCancelReasonV1,
    /// Number of open orders left after cancellation.
    pub open_order_count: usize,
}

/// Result of applying a streaming-settlement receipt to a local channel.
#[derive(Debug, Clone)]
pub struct OrderbookReceiptOutcome {
    /// Accepted settlement receipt payload.
    pub accepted_receipt: SettlementReceiptV1,
    /// Channel snapshot after the receipt was applied.
    pub updated_channel: SettlementChannelV1,
    /// Number of receipts accepted by the local mirror.
    pub settlement_receipt_count: usize,
    /// Number of settlement channels that remain open after the update.
    pub open_settlement_channel_count: usize,
}

/// Buyer-side settlement ledger totals derived from accepted orderbook receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderbookBuyerSettlementLedgerEntry {
    /// Canonical buyer account bytes.
    pub buyer_account: Vec<u8>,
    /// Total micro-XOR debited from this buyer's local orderbook escrow.
    pub debited_micro_xor: u128,
    /// Micro-XOR still locked for this buyer across open or closing channels.
    pub remaining_locked_micro_xor: u128,
}

/// Provider-side settlement ledger totals derived from accepted orderbook receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderbookProviderSettlementLedgerEntry {
    /// Governance-controlled provider identifier.
    pub provider_id: [u8; 32],
    /// Total micro-XOR credited to this provider.
    pub credited_micro_xor: u128,
    /// Total micro-XOR retained as settlement fees for this provider's receipts.
    pub fee_retained_micro_xor: u128,
    /// Micro-XOR still locked for this provider across open or closing channels.
    pub remaining_locked_micro_xor: u128,
}

/// Deterministic local settlement ledger derived from orderbook channels and receipts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderbookSettlementLedger {
    /// Total micro-XOR debited from buyers.
    pub total_buyer_debited_micro_xor: u128,
    /// Total micro-XOR credited to providers.
    pub total_provider_credited_micro_xor: u128,
    /// Total micro-XOR retained as fees.
    pub total_fee_retained_micro_xor: u128,
    /// Total micro-XOR still locked across all local settlement channels.
    pub total_remaining_locked_micro_xor: u128,
    /// Buyer-side ledger rows sorted by account bytes.
    pub buyers: Vec<OrderbookBuyerSettlementLedgerEntry>,
    /// Provider-side ledger rows sorted by provider id.
    pub providers: Vec<OrderbookProviderSettlementLedgerEntry>,
}

/// Event kind emitted by the local orderbook mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderbookEventKind {
    /// An order was accepted by the local mirror.
    OrderAccepted,
    /// An open order was cancelled by the local mirror.
    OrderCancelled,
    /// A settlement receipt was accepted by the local mirror.
    SettlementReceiptAccepted,
}

/// Sequenced event emitted by the local orderbook mirror.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderbookEvent {
    /// Monotonic local event sequence.
    pub sequence: u64,
    /// Event kind.
    pub kind: OrderbookEventKind,
    /// Unix timestamp when the event was generated.
    pub generated_at_unix: u64,
    /// Order id associated with the event, when present.
    pub order_id: Option<[u8; 32]>,
    /// Trade ids emitted by this event.
    pub trade_ids: Vec<[u8; 32]>,
    /// Settlement channel ids opened or updated by this event.
    pub settlement_channel_ids: Vec<[u8; 32]>,
    /// Settlement receipt id associated with this event, when present.
    pub receipt_id: Option<[u8; 32]>,
    /// Order ids expired by the matching pass.
    pub expired_order_ids: Vec<[u8; 32]>,
    /// Open-order count after the event.
    pub open_order_count: u64,
    /// Open settlement channel count after the event.
    pub open_settlement_channel_count: u64,
    /// Accepted settlement receipt count after the event.
    pub settlement_receipt_count: u64,
}

/// Snapshot of the local orderbook mirror.
#[derive(Debug, Clone)]
pub struct OrderbookSnapshot {
    /// Next admission sequence that will be assigned by the local mirror.
    pub next_sequence: u64,
    /// Unix timestamp used when the snapshot was produced.
    pub generated_at_unix: u64,
    /// Open orders with their local admission sequence.
    pub open_orders: Vec<OrderBookEntryV1>,
    /// Trade events emitted by the local matcher.
    pub trades: Vec<TradeEventV1>,
    /// Settlement channels opened for local trade events.
    pub settlement_channels: Vec<SettlementChannelV1>,
    /// Settlement receipts accepted for local settlement channels.
    pub settlement_receipts: Vec<SettlementReceiptV1>,
    /// Derived local escrow/debit/credit ledger summary.
    pub settlement_ledger: OrderbookSettlementLedger,
    /// Order ids expired by local matching passes.
    pub expired_order_ids: Vec<[u8; 32]>,
}

#[derive(Debug, Default)]
pub(crate) struct OrderbookRuntime {
    next_sequence: u64,
    open_orders: BTreeMap<[u8; 32], OrderBookEntryV1>,
    trades: Vec<TradeEventV1>,
    settlement_channels: BTreeMap<[u8; 32], SettlementChannelV1>,
    settlement_receipts: BTreeMap<[u8; 32], SettlementReceiptV1>,
    expired_order_ids: Vec<[u8; 32]>,
}

impl OrderbookRuntime {
    pub(crate) fn submit_order(
        &mut self,
        order: OrderRequestV1,
        now_unix: u64,
    ) -> Result<OrderbookSubmitOutcome, OrderbookRuntimeError> {
        verify_order_request_signature_v1(&order)?;
        if self.open_orders.contains_key(&order.order_id) {
            return Err(OrderbookRuntimeError::DuplicateOrderId {
                order_id_hex: hex::encode(order.order_id),
            });
        }

        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(OrderbookRuntimeError::SequenceOverflow)?;
        self.open_orders.insert(
            order.order_id,
            OrderBookEntryV1 {
                order: order.clone(),
                sequence,
            },
        );

        let match_outcome = self.match_open_orders(now_unix)?;
        Ok(OrderbookSubmitOutcome {
            accepted_order: order,
            sequence,
            open_order_count: self.open_orders.len(),
            fills: match_outcome.fills,
            settlement_channels_opened: match_outcome.settlement_channels_opened,
            expired_order_ids: match_outcome.expired_order_ids,
        })
    }

    pub(crate) fn cancel_order(
        &mut self,
        cancel: OrderCancelV1,
    ) -> Result<OrderbookCancelOutcome, OrderbookRuntimeError> {
        verify_order_cancel_signature_v1(&cancel)?;
        let Some(entry) = self.open_orders.get(&cancel.order_id) else {
            return Err(OrderbookRuntimeError::OrderNotFound {
                order_id_hex: hex::encode(cancel.order_id),
            });
        };
        if entry.order.owner_account != cancel.owner_account {
            return Err(OrderbookRuntimeError::CancelOwnerMismatch);
        }
        let cancelled = self
            .open_orders
            .remove(&cancel.order_id)
            .expect("checked open order exists")
            .order;
        Ok(OrderbookCancelOutcome {
            cancelled_order: cancelled,
            reason: cancel.reason,
            open_order_count: self.open_orders.len(),
        })
    }

    pub(crate) fn submit_receipt(
        &mut self,
        receipt: SettlementReceiptV1,
    ) -> Result<OrderbookReceiptOutcome, OrderbookRuntimeError> {
        verify_settlement_receipt_signature_v1(&receipt)?;
        if self.settlement_receipts.contains_key(&receipt.receipt_id) {
            return Err(OrderbookRuntimeError::DuplicateReceiptId {
                receipt_id_hex: hex::encode(receipt.receipt_id),
            });
        }
        let channel = self
            .settlement_channels
            .get(&receipt.channel_id)
            .ok_or_else(|| OrderbookRuntimeError::SettlementChannelNotFound {
                channel_id_hex: hex::encode(receipt.channel_id),
            })?;
        if let Some(existing) = self.settlement_receipts.values().find(|existing| {
            existing.channel_id == receipt.channel_id && byte_ranges_overlap(existing, &receipt)
        }) {
            return Err(OrderbookRuntimeError::ReceiptRangeOverlap {
                receipt_id_hex: hex::encode(receipt.receipt_id),
                existing_receipt_id_hex: hex::encode(existing.receipt_id),
            });
        }

        let updated_channel = apply_settlement_receipt_v1(channel, &receipt)?;
        self.settlement_channels
            .insert(updated_channel.channel_id, updated_channel.clone());
        self.settlement_receipts
            .insert(receipt.receipt_id, receipt.clone());
        let open_settlement_channel_count = self
            .settlement_channels
            .values()
            .filter(|channel| {
                matches!(
                    channel.status,
                    sorafs_manifest::SettlementChannelStatusV1::Open
                )
            })
            .count();
        Ok(OrderbookReceiptOutcome {
            accepted_receipt: receipt,
            updated_channel,
            settlement_receipt_count: self.settlement_receipts.len(),
            open_settlement_channel_count,
        })
    }

    pub(crate) fn snapshot(&self, generated_at_unix: u64) -> OrderbookSnapshot {
        let mut open_orders = self.open_orders.values().cloned().collect::<Vec<_>>();
        open_orders.sort_by(|lhs, rhs| {
            lhs.sequence
                .cmp(&rhs.sequence)
                .then_with(|| lhs.order.order_id.cmp(&rhs.order.order_id))
        });
        let settlement_channels = self
            .settlement_channels
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let settlement_receipts = self
            .settlement_receipts
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let settlement_ledger = settlement_ledger_from(&settlement_channels, &settlement_receipts);
        OrderbookSnapshot {
            next_sequence: self.next_sequence,
            generated_at_unix,
            open_orders,
            trades: self.trades.clone(),
            settlement_channels,
            settlement_receipts,
            settlement_ledger,
            expired_order_ids: self.expired_order_ids.clone(),
        }
    }

    pub(crate) fn runtime_snapshot(&self, generated_at_unix: u64) -> OrderbookRuntimeSnapshotV1 {
        let snapshot = self.snapshot(generated_at_unix);
        OrderbookRuntimeSnapshotV1 {
            version: sorafs_manifest::ORDERBOOK_RUNTIME_SNAPSHOT_VERSION_V1,
            next_sequence: snapshot.next_sequence,
            generated_at_unix: snapshot.generated_at_unix,
            open_orders: snapshot.open_orders,
            trades: snapshot.trades,
            settlement_channels: snapshot.settlement_channels,
            settlement_receipts: snapshot.settlement_receipts,
            expired_order_ids: snapshot.expired_order_ids,
        }
    }

    pub(crate) fn restore_runtime_snapshot(
        &mut self,
        snapshot: OrderbookRuntimeSnapshotV1,
    ) -> Result<(), OrderbookRuntimeError> {
        snapshot.validate()?;
        let mut open_orders = BTreeMap::new();
        for entry in snapshot.open_orders {
            open_orders.insert(entry.order.order_id, entry);
        }
        let settlement_channels = snapshot
            .settlement_channels
            .into_iter()
            .map(|channel| (channel.channel_id, channel))
            .collect::<BTreeMap<_, _>>();
        let settlement_receipts = snapshot
            .settlement_receipts
            .into_iter()
            .map(|receipt| (receipt.receipt_id, receipt))
            .collect::<BTreeMap<_, _>>();
        *self = Self {
            next_sequence: snapshot.next_sequence,
            open_orders,
            trades: snapshot.trades,
            settlement_channels,
            settlement_receipts,
            expired_order_ids: snapshot.expired_order_ids,
        };
        Ok(())
    }

    fn match_open_orders(
        &mut self,
        now_unix: u64,
    ) -> Result<RuntimeMatchOutcome, OrderbookRuntimeError> {
        let pre_match_orders = self
            .open_orders
            .iter()
            .map(|(order_id, entry)| (*order_id, entry.order.clone()))
            .collect::<BTreeMap<_, _>>();
        let sequence_by_order = self
            .open_orders
            .iter()
            .map(|(order_id, entry)| (*order_id, entry.sequence))
            .collect::<BTreeMap<_, _>>();
        let entries = self.open_orders.values().cloned().collect::<Vec<_>>();
        let outcome = match_order_book_v1(&entries, now_unix)?;

        let mut remaining = BTreeMap::new();
        for order in outcome.remaining_orders {
            let sequence = *sequence_by_order
                .get(&order.order_id)
                .ok_or(OrderbookRuntimeError::MissingMatchedOrder)?;
            remaining.insert(order.order_id, OrderBookEntryV1 { order, sequence });
        }
        self.open_orders = remaining;

        let mut channels = Vec::new();
        for fill in &outcome.fills {
            let channel = settlement_channel_for_fill(fill, &pre_match_orders, now_unix)?;
            self.settlement_channels
                .insert(channel.channel_id, channel.clone());
            channels.push(channel);
            self.trades.push(fill.trade.clone());
        }
        self.expired_order_ids
            .extend(outcome.expired_order_ids.iter().copied());

        Ok(RuntimeMatchOutcome {
            fills: outcome.fills,
            settlement_channels_opened: channels,
            expired_order_ids: outcome.expired_order_ids,
        })
    }
}

#[derive(Debug)]
struct RuntimeMatchOutcome {
    fills: Vec<OrderFillOutcomeV1>,
    settlement_channels_opened: Vec<SettlementChannelV1>,
    expired_order_ids: Vec<[u8; 32]>,
}

fn settlement_channel_for_fill(
    fill: &OrderFillOutcomeV1,
    orders: &BTreeMap<[u8; 32], OrderRequestV1>,
    now_unix: u64,
) -> Result<SettlementChannelV1, OrderbookRuntimeError> {
    let maker = orders
        .get(&fill.trade.maker_order_id)
        .ok_or(OrderbookRuntimeError::MissingMatchedOrder)?;
    let taker = orders
        .get(&fill.trade.taker_order_id)
        .ok_or(OrderbookRuntimeError::MissingMatchedOrder)?;
    let (buyer, provider) = match (maker.side, taker.side) {
        (OrderSideV1::Bid, OrderSideV1::Ask) => (maker, taker),
        (OrderSideV1::Ask, OrderSideV1::Bid) => (taker, maker),
        _ => return Err(OrderbookRuntimeError::InvalidMatchedSides),
    };
    open_settlement_channel_for_trade_v1(
        &fill.trade,
        channel_id_for_trade(&fill.trade),
        buyer.owner_account.clone(),
        provider_id_for_order(provider),
        now_unix,
    )
    .map_err(OrderbookRuntimeError::Validation)
}

fn channel_id_for_trade(trade: &TradeEventV1) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(ORDERBOOK_CHANNEL_ID_DOMAIN_V1);
    hasher.update(&trade.trade_id);
    nonzero_digest(*hasher.finalize().as_bytes())
}

fn provider_id_for_order(order: &OrderRequestV1) -> [u8; 32] {
    local_orderbook_provider_id_for_owner_account(&order.owner_account)
}

fn nonzero_digest(mut digest: [u8; 32]) -> [u8; 32] {
    if digest.iter().all(|byte| *byte == 0) {
        digest[31] = 1;
    }
    digest
}

fn byte_ranges_overlap(existing: &SettlementReceiptV1, candidate: &SettlementReceiptV1) -> bool {
    existing.range.start < candidate.range.end && candidate.range.start < existing.range.end
}

#[derive(Debug, Default)]
struct BuyerLedgerTotals {
    debited_micro_xor: u128,
    remaining_locked_micro_xor: u128,
}

#[derive(Debug, Default)]
struct ProviderLedgerTotals {
    credited_micro_xor: u128,
    fee_retained_micro_xor: u128,
    remaining_locked_micro_xor: u128,
}

fn settlement_ledger_from(
    settlement_channels: &[SettlementChannelV1],
    settlement_receipts: &[SettlementReceiptV1],
) -> OrderbookSettlementLedger {
    let mut channel_index = BTreeMap::<[u8; 32], (&Vec<u8>, [u8; 32])>::new();
    let mut buyer_totals = BTreeMap::<Vec<u8>, BuyerLedgerTotals>::new();
    let mut provider_totals = BTreeMap::<[u8; 32], ProviderLedgerTotals>::new();
    let mut total_remaining_locked_micro_xor = 0u128;

    for channel in settlement_channels {
        channel_index.insert(
            channel.channel_id,
            (&channel.buyer_account, channel.provider_id),
        );
        let locked = channel.xor_locked.as_micro();
        total_remaining_locked_micro_xor = total_remaining_locked_micro_xor.saturating_add(locked);
        let buyer_entry = buyer_totals
            .entry(channel.buyer_account.clone())
            .or_default();
        buyer_entry.remaining_locked_micro_xor = buyer_entry
            .remaining_locked_micro_xor
            .saturating_add(locked);
        let provider_entry = provider_totals.entry(channel.provider_id).or_default();
        provider_entry.remaining_locked_micro_xor = provider_entry
            .remaining_locked_micro_xor
            .saturating_add(locked);
    }

    let mut total_buyer_debited_micro_xor = 0u128;
    let mut total_provider_credited_micro_xor = 0u128;
    let mut total_fee_retained_micro_xor = 0u128;
    for receipt in settlement_receipts {
        let Some((buyer_account, provider_id)) = channel_index.get(&receipt.channel_id) else {
            continue;
        };
        let debited = receipt.xor_debited.as_micro();
        let credited = receipt.provider_credit.as_micro();
        let fee = receipt.fee_amount.as_micro();
        total_buyer_debited_micro_xor = total_buyer_debited_micro_xor.saturating_add(debited);
        total_provider_credited_micro_xor =
            total_provider_credited_micro_xor.saturating_add(credited);
        total_fee_retained_micro_xor = total_fee_retained_micro_xor.saturating_add(fee);
        let buyer_entry = buyer_totals.entry((*buyer_account).clone()).or_default();
        buyer_entry.debited_micro_xor = buyer_entry.debited_micro_xor.saturating_add(debited);
        let provider_entry = provider_totals.entry(*provider_id).or_default();
        provider_entry.credited_micro_xor =
            provider_entry.credited_micro_xor.saturating_add(credited);
        provider_entry.fee_retained_micro_xor =
            provider_entry.fee_retained_micro_xor.saturating_add(fee);
    }

    OrderbookSettlementLedger {
        total_buyer_debited_micro_xor,
        total_provider_credited_micro_xor,
        total_fee_retained_micro_xor,
        total_remaining_locked_micro_xor,
        buyers: buyer_totals
            .into_iter()
            .map(
                |(buyer_account, totals)| OrderbookBuyerSettlementLedgerEntry {
                    buyer_account,
                    debited_micro_xor: totals.debited_micro_xor,
                    remaining_locked_micro_xor: totals.remaining_locked_micro_xor,
                },
            )
            .collect(),
        providers: provider_totals
            .into_iter()
            .map(
                |(provider_id, totals)| OrderbookProviderSettlementLedgerEntry {
                    provider_id,
                    credited_micro_xor: totals.credited_micro_xor,
                    fee_retained_micro_xor: totals.fee_retained_micro_xor,
                    remaining_locked_micro_xor: totals.remaining_locked_micro_xor,
                },
            )
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, Signature as IrohaSignature};
    use sorafs_manifest::{
        BYTES_PER_GIB, ByteRangeV1, ORDERBOOK_ORDER_VERSION_V1, OrderTierV1, OrderbookSignatureV1,
        SETTLEMENT_RECEIPT_VERSION_V1, deal::XorAmount, order_cancel_signature_digest_v1,
        order_request_signature_digest_v1, provider_advert::SignatureAlgorithm,
        settlement_receipt_signature_digest_v1,
    };

    use super::*;

    fn signature() -> OrderbookSignatureV1 {
        OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: Vec::new(),
            signature: Vec::new(),
        }
    }

    fn signing_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("orderbook fixture seed must derive keypair")
    }

    fn signature_for_digest(keypair: &KeyPair, digest: &[u8; 32]) -> OrderbookSignatureV1 {
        let (algorithm, public_key) = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must expose bytes");
        assert_eq!(algorithm, Algorithm::Ed25519);
        let signature = IrohaSignature::try_new(keypair.private_key(), digest)
            .expect("fixture signature must be produced");
        OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: public_key.to_vec(),
            signature: signature.payload().to_vec(),
        }
    }

    fn sign_order(mut order: OrderRequestV1, seed: u8) -> OrderRequestV1 {
        let keypair = signing_keypair(seed);
        let (algorithm, public_key) = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must expose bytes");
        assert_eq!(algorithm, Algorithm::Ed25519);
        order.signature = OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: public_key.to_vec(),
            signature: Vec::new(),
        };
        let digest = order_request_signature_digest_v1(&order).expect("order digest");
        order.signature = signature_for_digest(&keypair, &digest);
        order
    }

    fn sign_cancel(mut cancel: OrderCancelV1, seed: u8) -> OrderCancelV1 {
        let keypair = signing_keypair(seed);
        let (algorithm, public_key) = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must expose bytes");
        assert_eq!(algorithm, Algorithm::Ed25519);
        cancel.signature = OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: public_key.to_vec(),
            signature: Vec::new(),
        };
        let digest = order_cancel_signature_digest_v1(&cancel).expect("cancel digest");
        cancel.signature = signature_for_digest(&keypair, &digest);
        cancel
    }

    fn sign_receipt(mut receipt: SettlementReceiptV1, seed: u8) -> SettlementReceiptV1 {
        let keypair = signing_keypair(seed);
        let (algorithm, public_key) = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must expose bytes");
        assert_eq!(algorithm, Algorithm::Ed25519);
        receipt.settlement_signature = OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: public_key.to_vec(),
            signature: Vec::new(),
        };
        let digest =
            settlement_receipt_signature_digest_v1(&receipt).expect("settlement receipt digest");
        receipt.settlement_signature = signature_for_digest(&keypair, &digest);
        receipt
    }

    fn order(id: u8, side: OrderSideV1, price_micro: u128, owner: &[u8]) -> OrderRequestV1 {
        sign_order(
            OrderRequestV1 {
                version: ORDERBOOK_ORDER_VERSION_V1,
                order_id: [id; 32],
                side,
                tier: OrderTierV1::Hot,
                price_per_gib: XorAmount::from_micro(price_micro),
                quantity_gib: 4,
                remaining_gib: 4,
                owner_account: owner.to_vec(),
                expiry_unix: 1_800_000_100,
                nonce: u64::from(id),
                maker_fee_bps: 10,
                taker_fee_bps: 20,
                signature: signature(),
            },
            id.saturating_add(0x10),
        )
    }

    #[test]
    fn runtime_matches_crossing_orders_and_opens_settlement_channel() {
        let mut runtime = OrderbookRuntime::default();
        let ask = order(1, OrderSideV1::Ask, 1_500_000, b"provider");
        let bid = order(2, OrderSideV1::Bid, 1_600_000, b"buyer");

        let first = runtime
            .submit_order(ask, 1_800_000_000)
            .expect("accept ask");
        assert!(first.fills.is_empty());
        assert_eq!(first.open_order_count, 1);

        let second = runtime
            .submit_order(bid, 1_800_000_000)
            .expect("accept and match bid");
        assert_eq!(second.fills.len(), 1);
        assert_eq!(second.settlement_channels_opened.len(), 1);
        assert_eq!(second.open_order_count, 0);

        let snapshot = runtime.snapshot(1_800_000_000);
        assert!(snapshot.open_orders.is_empty());
        assert_eq!(snapshot.trades.len(), 1);
        assert_eq!(snapshot.settlement_channels.len(), 1);
        assert_eq!(
            snapshot.settlement_channels[0].buyer_account,
            b"buyer".to_vec()
        );
    }

    #[test]
    fn local_provider_id_for_owner_account_matches_channel_derivation() {
        let provider_id = local_orderbook_provider_id_for_owner_account(b"provider");
        assert!(provider_id.iter().any(|byte| *byte != 0));
        assert_eq!(
            provider_id,
            provider_id_for_order(&order(1, OrderSideV1::Ask, 1_500_000, b"provider"))
        );
        assert_ne!(
            provider_id,
            local_orderbook_provider_id_for_owner_account(b"buyer")
        );
    }

    #[test]
    fn runtime_rejects_tampered_order_signature() {
        let mut runtime = OrderbookRuntime::default();
        let mut ask = order(9, OrderSideV1::Ask, 1_500_000, b"provider");
        ask.price_per_gib = XorAmount::from_micro(1_400_000);

        assert!(matches!(
            runtime.submit_order(ask, 1_800_000_000),
            Err(OrderbookRuntimeError::Validation(
                OrderbookValidationError::SignatureVerification { .. }
            ))
        ));
    }

    #[test]
    fn runtime_cancels_only_matching_owner_order() {
        let mut runtime = OrderbookRuntime::default();
        let ask = order(1, OrderSideV1::Ask, 1_500_000, b"provider");
        runtime
            .submit_order(ask, 1_800_000_000)
            .expect("accept ask");

        let wrong_owner = sign_cancel(
            OrderCancelV1 {
                version: sorafs_manifest::ORDERBOOK_CANCEL_VERSION_V1,
                order_id: [1; 32],
                owner_account: b"other".to_vec(),
                reason: OrderCancelReasonV1::OwnerRequested,
                nonce: 1,
                signature: signature(),
            },
            0x31,
        );
        assert!(matches!(
            runtime.cancel_order(wrong_owner),
            Err(OrderbookRuntimeError::CancelOwnerMismatch)
        ));

        let cancel = OrderCancelV1 {
            owner_account: b"provider".to_vec(),
            ..wrong_owner_fixture()
        };
        let outcome = runtime.cancel_order(cancel).expect("cancel order");
        assert_eq!(outcome.open_order_count, 0);
        assert_eq!(outcome.cancelled_order.order_id, [1; 32]);
    }

    #[test]
    fn runtime_applies_receipts_and_rejects_overlapping_ranges() {
        let mut runtime = OrderbookRuntime::default();
        runtime
            .submit_order(
                order(1, OrderSideV1::Ask, 1_500_000, b"provider"),
                1_800_000_000,
            )
            .expect("accept ask");
        runtime
            .submit_order(
                order(2, OrderSideV1::Bid, 1_600_000, b"buyer"),
                1_800_000_000,
            )
            .expect("accept bid");
        let channel = runtime.snapshot(1_800_000_000).settlement_channels[0].clone();
        let first_receipt = receipt(7, &channel, 0, BYTES_PER_GIB, 1_800_000_010, 100);

        let outcome = runtime
            .submit_receipt(first_receipt.clone())
            .expect("apply receipt");

        assert_eq!(
            outcome.accepted_receipt.receipt_id,
            first_receipt.receipt_id
        );
        assert_eq!(outcome.settlement_receipt_count, 1);
        assert_eq!(
            outcome.updated_channel.remaining_bytes,
            channel.remaining_bytes - BYTES_PER_GIB
        );
        let snapshot = runtime.snapshot(1_800_000_020);
        assert_eq!(snapshot.settlement_receipts.len(), 1);
        assert_eq!(
            snapshot.settlement_ledger.total_buyer_debited_micro_xor,
            100
        );
        assert_eq!(
            snapshot.settlement_ledger.total_provider_credited_micro_xor,
            90
        );
        assert_eq!(snapshot.settlement_ledger.total_fee_retained_micro_xor, 10);
        assert_eq!(
            snapshot.settlement_ledger.total_remaining_locked_micro_xor,
            outcome.updated_channel.xor_locked.as_micro()
        );
        assert_eq!(snapshot.settlement_ledger.buyers.len(), 1);
        assert_eq!(
            snapshot.settlement_ledger.buyers[0].buyer_account,
            b"buyer".to_vec()
        );
        assert_eq!(snapshot.settlement_ledger.providers.len(), 1);
        assert_eq!(
            snapshot.settlement_ledger.providers[0].provider_id,
            channel.provider_id
        );

        let overlapping = receipt(8, &channel, 512, BYTES_PER_GIB + 512, 1_800_000_011, 100);
        assert!(matches!(
            runtime.submit_receipt(overlapping),
            Err(OrderbookRuntimeError::ReceiptRangeOverlap { .. })
        ));
    }

    fn wrong_owner_fixture() -> OrderCancelV1 {
        sign_cancel(
            OrderCancelV1 {
                version: sorafs_manifest::ORDERBOOK_CANCEL_VERSION_V1,
                order_id: [1; 32],
                owner_account: b"provider".to_vec(),
                reason: OrderCancelReasonV1::OwnerRequested,
                nonce: 2,
                signature: signature(),
            },
            0x32,
        )
    }

    fn receipt(
        id: u8,
        channel: &SettlementChannelV1,
        start: u64,
        end: u64,
        issued_at_unix: u64,
        debited_micro: u128,
    ) -> SettlementReceiptV1 {
        sign_receipt(
            SettlementReceiptV1 {
                version: SETTLEMENT_RECEIPT_VERSION_V1,
                receipt_id: [id; 32],
                channel_id: channel.channel_id,
                trade_id: channel.trade_id,
                range: ByteRangeV1 { start, end },
                chunk_hash: [id.saturating_add(90); 32],
                bytes_delivered: end - start,
                xor_debited: XorAmount::from_micro(debited_micro),
                provider_credit: XorAmount::from_micro(debited_micro.saturating_sub(10)),
                fee_amount: XorAmount::from_micro(10),
                issued_at_unix,
                settlement_signature: signature(),
            },
            id.saturating_add(0x40),
        )
    }
}
