//! Local SoraFS orderbook runtime mirror.

use std::collections::BTreeMap;

use blake3::Hasher;
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::orderbook::OrderbookOwnerNonceHighWaterV1;
use sorafs_manifest::{
    OrderBookEntryV1, OrderCancelReasonV1, OrderCancelV1, OrderFillOutcomeV1, OrderRequestV1,
    OrderSideV1, OrderbookRuntimeSnapshotV1, OrderbookValidationError, SettlementChannelV1,
    SettlementReceiptV1, TradeEventV1, XorQuantity, apply_settlement_receipt_v1,
    match_order_book_v1, open_settlement_channel_for_trade_v1, verify_order_cancel_signature_v1,
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
    /// The owner nonce does not advance the last durably committed operation.
    #[error(
        "owner `{owner_account_hex}` nonce {nonce} is stale or replayed (highest committed nonce {highest_nonce})"
    )]
    StaleOwnerNonce {
        /// Hex-encoded canonical owner account bytes.
        owner_account_hex: String,
        /// Rejected order or cancellation nonce.
        nonce: u64,
        /// Highest nonce already committed for the owner.
        highest_nonce: u64,
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
    /// The monotonic local event sequence overflowed.
    #[error("orderbook event sequence overflow")]
    EventSequenceOverflow,
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
    #[error("order price {price} XOR/GiB is not aligned to configured tick {tick} XOR")]
    OrderPriceTickMismatch {
        /// Exact submitted XOR price per GiB.
        price: XorQuantity,
        /// Exact configured XOR price tick per GiB.
        tick: XorQuantity,
    },
    /// The configured price tick violates the canonical XOR precision bound.
    #[error("configured orderbook price tick `{tick}` is invalid: {reason}")]
    InvalidPriceTick {
        /// Canonical decimal spelling of the rejected tick.
        tick: String,
        /// Stable validation failure description.
        reason: String,
    },
    /// Exact settlement-ledger aggregation exceeded the bounded quantity domain.
    #[error("orderbook settlement ledger arithmetic overflow")]
    SettlementLedgerOverflow,
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
    /// A configured authoritative-state ceiling was reached.
    #[error("orderbook resource `{resource}` exhausted (limit {limit})")]
    ResourceExhausted {
        /// Bounded resource label.
        resource: &'static str,
        /// Configured entry ceiling.
        limit: usize,
    },
    /// A durable orderbook snapshot contained duplicate or inconsistent indexes.
    #[error("invalid orderbook runtime snapshot: {0}")]
    InvalidSnapshot(String),
    /// The durable orderbook state-and-event checkpoint could not be committed.
    #[error("orderbook checkpoint failed: {0}")]
    Checkpoint(String),
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
    /// Exact XOR debited from this buyer's local orderbook escrow.
    pub debited: XorQuantity,
    /// Exact XOR still locked for this buyer across open or closing channels.
    pub remaining_locked: XorQuantity,
}

/// Provider-side settlement ledger totals derived from accepted orderbook receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderbookProviderSettlementLedgerEntry {
    /// Governance-controlled provider identifier.
    pub provider_id: [u8; 32],
    /// Exact XOR credited to this provider.
    pub credited: XorQuantity,
    /// Exact XOR retained as settlement fees for this provider's receipts.
    pub fee_retained: XorQuantity,
    /// Exact XOR still locked for this provider across open or closing channels.
    pub remaining_locked: XorQuantity,
}

/// Deterministic local settlement ledger derived from orderbook channels and receipts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderbookSettlementLedger {
    /// Exact total XOR debited from buyers.
    pub total_buyer_debited: XorQuantity,
    /// Exact total XOR credited to providers.
    pub total_provider_credited: XorQuantity,
    /// Exact total XOR retained as fees.
    pub total_fee_retained: XorQuantity,
    /// Exact total XOR still locked across all local settlement channels.
    pub total_remaining_locked: XorQuantity,
    /// Buyer-side ledger rows sorted by account bytes.
    pub buyers: Vec<OrderbookBuyerSettlementLedgerEntry>,
    /// Provider-side ledger rows sorted by provider id.
    pub providers: Vec<OrderbookProviderSettlementLedgerEntry>,
}

/// Event kind emitted by the local orderbook mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum OrderbookEventKind {
    /// An order was accepted by the local mirror.
    OrderAccepted,
    /// An open order was cancelled by the local mirror.
    OrderCancelled,
    /// A settlement receipt was accepted by the local mirror.
    SettlementReceiptAccepted,
}

/// Sequenced event emitted by the local orderbook mirror.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
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

#[derive(Debug, Clone)]
pub(crate) struct OrderbookRuntime {
    next_sequence: u64,
    owner_nonce_high_waters: BTreeMap<Vec<u8>, u64>,
    open_orders: BTreeMap<[u8; 32], OrderBookEntryV1>,
    trades: Vec<TradeEventV1>,
    settlement_channels: BTreeMap<[u8; 32], SettlementChannelV1>,
    settlement_receipts: BTreeMap<[u8; 32], SettlementReceiptV1>,
    expired_order_ids: Vec<[u8; 32]>,
    entry_limit: usize,
    owner_nonce_limit: usize,
}

impl Default for OrderbookRuntime {
    fn default() -> Self {
        Self::with_entry_limit(65_536)
    }
}

impl OrderbookRuntime {
    pub(crate) fn with_entry_limit(entry_limit: usize) -> Self {
        Self::with_limits(entry_limit, entry_limit)
    }

    fn with_limits(entry_limit: usize, owner_nonce_limit: usize) -> Self {
        Self {
            next_sequence: 0,
            owner_nonce_high_waters: BTreeMap::new(),
            open_orders: BTreeMap::new(),
            trades: Vec::new(),
            settlement_channels: BTreeMap::new(),
            settlement_receipts: BTreeMap::new(),
            expired_order_ids: Vec::new(),
            entry_limit: entry_limit.max(1),
            owner_nonce_limit: owner_nonce_limit.max(1),
        }
    }

    pub(crate) fn submit_order(
        &mut self,
        order: OrderRequestV1,
        now_unix: u64,
    ) -> Result<OrderbookSubmitOutcome, OrderbookRuntimeError> {
        let mut candidate = self.clone();
        let outcome = candidate.submit_order_inner(order, now_unix)?;
        candidate.ensure_collection_limits()?;
        *self = candidate;
        Ok(outcome)
    }

    fn submit_order_inner(
        &mut self,
        order: OrderRequestV1,
        now_unix: u64,
    ) -> Result<OrderbookSubmitOutcome, OrderbookRuntimeError> {
        verify_order_request_signature_v1(&order)?;
        self.record_owner_nonce(&order.owner_account, order.nonce)?;
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
        let mut candidate = self.clone();
        let outcome = candidate.cancel_order_inner(cancel)?;
        candidate.ensure_collection_limits()?;
        *self = candidate;
        Ok(outcome)
    }

    fn cancel_order_inner(
        &mut self,
        cancel: OrderCancelV1,
    ) -> Result<OrderbookCancelOutcome, OrderbookRuntimeError> {
        verify_order_cancel_signature_v1(&cancel)?;
        self.record_owner_nonce(&cancel.owner_account, cancel.nonce)?;
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

    fn record_owner_nonce(
        &mut self,
        owner_account: &[u8],
        nonce: u64,
    ) -> Result<(), OrderbookRuntimeError> {
        if let Some(highest_nonce) = self.owner_nonce_high_waters.get(owner_account) {
            if nonce <= *highest_nonce {
                return Err(OrderbookRuntimeError::StaleOwnerNonce {
                    owner_account_hex: hex::encode(owner_account),
                    nonce,
                    highest_nonce: *highest_nonce,
                });
            }
        } else if self.owner_nonce_high_waters.len() >= self.owner_nonce_limit {
            return Err(OrderbookRuntimeError::ResourceExhausted {
                resource: "owner_nonce_high_waters",
                limit: self.owner_nonce_limit,
            });
        }
        self.owner_nonce_high_waters
            .insert(owner_account.to_vec(), nonce);
        Ok(())
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
        if self.settlement_receipts.len() >= self.entry_limit {
            return Err(OrderbookRuntimeError::ResourceExhausted {
                resource: "settlement_receipts",
                limit: self.entry_limit,
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

    pub(crate) fn snapshot(
        &self,
        generated_at_unix: u64,
    ) -> Result<OrderbookSnapshot, OrderbookRuntimeError> {
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
        let settlement_ledger = settlement_ledger_from(&settlement_channels, &settlement_receipts)?;
        Ok(OrderbookSnapshot {
            next_sequence: self.next_sequence,
            generated_at_unix,
            open_orders,
            trades: self.trades.clone(),
            settlement_channels,
            settlement_receipts,
            settlement_ledger,
            expired_order_ids: self.expired_order_ids.clone(),
        })
    }

    pub(crate) fn runtime_snapshot(&self, generated_at_unix: u64) -> OrderbookRuntimeSnapshotV1 {
        let mut open_orders = self.open_orders.values().cloned().collect::<Vec<_>>();
        open_orders.sort_by(|lhs, rhs| {
            lhs.sequence
                .cmp(&rhs.sequence)
                .then_with(|| lhs.order.order_id.cmp(&rhs.order.order_id))
        });
        OrderbookRuntimeSnapshotV1 {
            version: sorafs_manifest::ORDERBOOK_RUNTIME_SNAPSHOT_VERSION_V1,
            next_sequence: self.next_sequence,
            generated_at_unix,
            owner_nonce_high_waters: self
                .owner_nonce_high_waters
                .iter()
                .map(
                    |(owner_account, highest_nonce)| OrderbookOwnerNonceHighWaterV1 {
                        owner_account: owner_account.clone(),
                        highest_nonce: *highest_nonce,
                    },
                )
                .collect(),
            open_orders,
            trades: self.trades.clone(),
            settlement_channels: self.settlement_channels.values().cloned().collect(),
            settlement_receipts: self.settlement_receipts.values().cloned().collect(),
            expired_order_ids: self.expired_order_ids.clone(),
        }
    }

    fn ensure_collection_limits(&self) -> Result<(), OrderbookRuntimeError> {
        if self.owner_nonce_high_waters.len() > self.owner_nonce_limit {
            return Err(OrderbookRuntimeError::ResourceExhausted {
                resource: "owner_nonce_high_waters",
                limit: self.owner_nonce_limit,
            });
        }
        for (resource, count) in [
            ("open_orders", self.open_orders.len()),
            ("trades", self.trades.len()),
            ("settlement_channels", self.settlement_channels.len()),
            ("settlement_receipts", self.settlement_receipts.len()),
            ("expired_order_ids", self.expired_order_ids.len()),
        ] {
            if count > self.entry_limit {
                return Err(OrderbookRuntimeError::ResourceExhausted {
                    resource,
                    limit: self.entry_limit,
                });
            }
        }
        Ok(())
    }

    pub(crate) fn restore_runtime_snapshot(
        &mut self,
        snapshot: OrderbookRuntimeSnapshotV1,
    ) -> Result<(), OrderbookRuntimeError> {
        snapshot.validate()?;
        if snapshot.owner_nonce_high_waters.len() > self.owner_nonce_limit {
            return Err(OrderbookRuntimeError::ResourceExhausted {
                resource: "owner_nonce_high_waters",
                limit: self.owner_nonce_limit,
            });
        }
        for (resource, count) in [
            ("open_orders", snapshot.open_orders.len()),
            ("trades", snapshot.trades.len()),
            ("settlement_channels", snapshot.settlement_channels.len()),
            ("settlement_receipts", snapshot.settlement_receipts.len()),
            ("expired_order_ids", snapshot.expired_order_ids.len()),
        ] {
            if count > self.entry_limit {
                return Err(OrderbookRuntimeError::ResourceExhausted {
                    resource,
                    limit: self.entry_limit,
                });
            }
        }
        let mut open_orders = BTreeMap::new();
        for entry in snapshot.open_orders {
            let order_id = entry.order.order_id;
            if open_orders.insert(order_id, entry).is_some() {
                return Err(OrderbookRuntimeError::InvalidSnapshot(format!(
                    "duplicate order id {}",
                    hex::encode(order_id)
                )));
            }
        }
        let owner_nonce_high_waters = snapshot
            .owner_nonce_high_waters
            .into_iter()
            .map(|entry| (entry.owner_account, entry.highest_nonce))
            .collect();
        let mut settlement_channels = BTreeMap::new();
        for channel in snapshot.settlement_channels {
            let channel_id = channel.channel_id;
            if settlement_channels.insert(channel_id, channel).is_some() {
                return Err(OrderbookRuntimeError::InvalidSnapshot(format!(
                    "duplicate settlement channel id {}",
                    hex::encode(channel_id)
                )));
            }
        }
        let mut settlement_receipts = BTreeMap::new();
        for receipt in snapshot.settlement_receipts {
            let receipt_id = receipt.receipt_id;
            if settlement_receipts.insert(receipt_id, receipt).is_some() {
                return Err(OrderbookRuntimeError::InvalidSnapshot(format!(
                    "duplicate settlement receipt id {}",
                    hex::encode(receipt_id)
                )));
            }
        }
        let entry_limit = self.entry_limit;
        let owner_nonce_limit = self.owner_nonce_limit;
        *self = Self {
            next_sequence: snapshot.next_sequence,
            owner_nonce_high_waters,
            open_orders,
            trades: snapshot.trades,
            settlement_channels,
            settlement_receipts,
            expired_order_ids: snapshot.expired_order_ids,
            entry_limit,
            owner_nonce_limit,
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
    debited: XorQuantity,
    remaining_locked: XorQuantity,
}

#[derive(Debug, Default)]
struct ProviderLedgerTotals {
    credited: XorQuantity,
    fee_retained: XorQuantity,
    remaining_locked: XorQuantity,
}

fn add_ledger_amount(
    total: &mut XorQuantity,
    amount: &XorQuantity,
) -> Result<(), OrderbookRuntimeError> {
    *total = total
        .checked_add(amount)
        .map_err(|_| OrderbookRuntimeError::SettlementLedgerOverflow)?;
    Ok(())
}

fn settlement_ledger_from(
    settlement_channels: &[SettlementChannelV1],
    settlement_receipts: &[SettlementReceiptV1],
) -> Result<OrderbookSettlementLedger, OrderbookRuntimeError> {
    let mut channel_index = BTreeMap::<[u8; 32], (&Vec<u8>, [u8; 32])>::new();
    let mut buyer_totals = BTreeMap::<Vec<u8>, BuyerLedgerTotals>::new();
    let mut provider_totals = BTreeMap::<[u8; 32], ProviderLedgerTotals>::new();
    let mut total_remaining_locked = XorQuantity::zero();

    for channel in settlement_channels {
        channel_index.insert(
            channel.channel_id,
            (&channel.buyer_account, channel.provider_id),
        );
        let locked = &channel.xor_locked;
        add_ledger_amount(&mut total_remaining_locked, locked)?;
        let buyer_entry = buyer_totals
            .entry(channel.buyer_account.clone())
            .or_default();
        add_ledger_amount(&mut buyer_entry.remaining_locked, locked)?;
        let provider_entry = provider_totals.entry(channel.provider_id).or_default();
        add_ledger_amount(&mut provider_entry.remaining_locked, locked)?;
    }

    let mut total_buyer_debited = XorQuantity::zero();
    let mut total_provider_credited = XorQuantity::zero();
    let mut total_fee_retained = XorQuantity::zero();
    for receipt in settlement_receipts {
        let Some((buyer_account, provider_id)) = channel_index.get(&receipt.channel_id) else {
            continue;
        };
        let debited = &receipt.xor_debited;
        let credited = &receipt.provider_credit;
        let fee = &receipt.fee_amount;
        add_ledger_amount(&mut total_buyer_debited, debited)?;
        add_ledger_amount(&mut total_provider_credited, credited)?;
        add_ledger_amount(&mut total_fee_retained, fee)?;
        let buyer_entry = buyer_totals.entry((*buyer_account).clone()).or_default();
        add_ledger_amount(&mut buyer_entry.debited, debited)?;
        let provider_entry = provider_totals.entry(*provider_id).or_default();
        add_ledger_amount(&mut provider_entry.credited, credited)?;
        add_ledger_amount(&mut provider_entry.fee_retained, fee)?;
    }

    Ok(OrderbookSettlementLedger {
        total_buyer_debited,
        total_provider_credited,
        total_fee_retained,
        total_remaining_locked,
        buyers: buyer_totals
            .into_iter()
            .map(
                |(buyer_account, totals)| OrderbookBuyerSettlementLedgerEntry {
                    buyer_account,
                    debited: totals.debited,
                    remaining_locked: totals.remaining_locked,
                },
            )
            .collect(),
        providers: provider_totals
            .into_iter()
            .map(
                |(provider_id, totals)| OrderbookProviderSettlementLedgerEntry {
                    provider_id,
                    credited: totals.credited,
                    fee_retained: totals.fee_retained,
                    remaining_locked: totals.remaining_locked,
                },
            )
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, Signature as IrohaSignature};
    use sorafs_manifest::{
        BYTES_PER_GIB, ByteRangeV1, ORDERBOOK_ORDER_VERSION_V1, OrderTierV1, OrderbookSignatureV1,
        SETTLEMENT_RECEIPT_VERSION_V1, deal::XorQuantity, derive_orderbook_order_id_v1,
        order_cancel_signature_digest_v1, order_request_signature_digest_v1,
        provider_advert::SignatureAlgorithm, settlement_receipt_signature_digest_v1,
    };

    use super::*;

    fn xor(value: &str) -> XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }

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
        let owner_account = owner.to_vec();
        let nonce = u64::from(id);
        sign_order(
            OrderRequestV1 {
                version: ORDERBOOK_ORDER_VERSION_V1,
                order_id: derive_orderbook_order_id_v1(&owner_account, nonce),
                side,
                tier: OrderTierV1::Hot,
                price_per_gib: XorQuantity::try_from_micro(price_micro)
                    .expect("legacy micro-XOR value is representable"),
                quantity_gib: 4,
                remaining_gib: 4,
                owner_account,
                expiry_unix: 1_800_000_100,
                nonce,
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

        let snapshot = runtime.snapshot(1_800_000_000).expect("orderbook snapshot");
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
        ask.price_per_gib = XorQuantity::try_from_micro(1_400_000)
            .expect("legacy micro-XOR value is representable");

        assert!(matches!(
            runtime.submit_order(ask, 1_800_000_000),
            Err(OrderbookRuntimeError::Validation(
                OrderbookValidationError::SignatureVerification { .. }
            ))
        ));
    }

    #[test]
    fn runtime_rejects_exact_order_replay_after_fill() {
        let mut runtime = OrderbookRuntime::default();
        let ask = order(1, OrderSideV1::Ask, 1_500_000, b"provider");
        let replay = ask.clone();
        runtime
            .submit_order(ask, 1_800_000_000)
            .expect("accept ask");
        runtime
            .submit_order(
                order(2, OrderSideV1::Bid, 1_600_000, b"buyer"),
                1_800_000_000,
            )
            .expect("fill ask");

        assert_eq!(
            runtime
                .submit_order(replay, 1_800_000_001)
                .expect_err("filled order replay must be rejected"),
            OrderbookRuntimeError::StaleOwnerNonce {
                owner_account_hex: hex::encode(b"provider"),
                nonce: 1,
                highest_nonce: 1,
            }
        );
        assert_eq!(
            runtime
                .snapshot(1_800_000_001)
                .expect("orderbook snapshot")
                .trades
                .len(),
            1
        );
    }

    #[test]
    fn runtime_rejects_same_and_cross_owner_retired_order_id_reuse() {
        let mut runtime = OrderbookRuntime::default();
        let ask = order(1, OrderSideV1::Ask, 1_500_000, b"provider");
        let retired_order_id = ask.order_id;
        runtime
            .submit_order(ask, 1_800_000_000)
            .expect("accept ask");
        runtime
            .submit_order(
                order(2, OrderSideV1::Bid, 1_600_000, b"buyer"),
                1_800_000_000,
            )
            .expect("retire ask through fill");

        for (owner, nonce, signing_seed) in [
            (b"provider".as_slice(), 3, 0x33),
            (b"other-owner".as_slice(), 1, 0x44),
        ] {
            let mut reused = order(9, OrderSideV1::Ask, 1_500_000, owner);
            reused.nonce = nonce;
            reused.order_id = retired_order_id;
            let expected_order_id = derive_orderbook_order_id_v1(owner, nonce);
            let reused = sign_order(reused, signing_seed);

            assert_eq!(
                runtime
                    .submit_order(reused, 1_800_000_001)
                    .expect_err("retired order id reuse must be rejected"),
                OrderbookRuntimeError::Validation(
                    OrderbookValidationError::OrderIdDerivationMismatch {
                        order_id: retired_order_id,
                        expected_order_id,
                    }
                )
            );
        }
        assert_eq!(
            runtime
                .snapshot(1_800_000_001)
                .expect("orderbook snapshot")
                .trades
                .len(),
            1
        );
    }

    #[test]
    fn runtime_rejects_exact_order_replay_after_expiry() {
        let mut runtime = OrderbookRuntime::default();
        let expired = order(1, OrderSideV1::Ask, 1_500_000, b"provider");
        let expired_order_id = expired.order_id;
        let replay = expired.clone();
        let first = runtime
            .submit_order(expired, 1_800_000_101)
            .expect("accept and expire order");
        assert_eq!(first.expired_order_ids, vec![expired_order_id]);
        assert_eq!(first.open_order_count, 0);

        assert!(matches!(
            runtime.submit_order(replay, 1_800_000_102),
            Err(OrderbookRuntimeError::StaleOwnerNonce {
                nonce: 1,
                highest_nonce: 1,
                ..
            })
        ));
        assert_eq!(
            runtime
                .snapshot(1_800_000_102)
                .expect("orderbook snapshot")
                .expired_order_ids,
            vec![expired_order_id]
        );
    }

    #[test]
    fn owner_nonce_high_water_is_scoped_to_canonical_owner() {
        let mut runtime = OrderbookRuntime::default();
        runtime
            .submit_order(
                order(1, OrderSideV1::Ask, 1_500_000, b"provider"),
                1_800_000_000,
            )
            .expect("accept first owner's nonce");
        let mut buyer = order(2, OrderSideV1::Bid, 1_600_000, b"buyer");
        buyer.nonce = 1;
        buyer.order_id = derive_orderbook_order_id_v1(&buyer.owner_account, buyer.nonce);
        let buyer = sign_order(buyer, 0x22);
        runtime
            .submit_order(buyer, 1_800_000_000)
            .expect("same nonce remains valid for a distinct owner");

        assert_eq!(
            runtime
                .runtime_snapshot(1_800_000_001)
                .owner_nonce_high_waters,
            vec![
                OrderbookOwnerNonceHighWaterV1 {
                    owner_account: b"buyer".to_vec(),
                    highest_nonce: 1,
                },
                OrderbookOwnerNonceHighWaterV1 {
                    owner_account: b"provider".to_vec(),
                    highest_nonce: 1,
                },
            ]
        );
    }

    #[test]
    fn runtime_cancels_only_matching_owner_order() {
        let mut runtime = OrderbookRuntime::default();
        let ask = order(1, OrderSideV1::Ask, 1_500_000, b"provider");
        let order_id = ask.order_id;
        runtime
            .submit_order(ask, 1_800_000_000)
            .expect("accept ask");

        let wrong_owner = sign_cancel(
            OrderCancelV1 {
                version: sorafs_manifest::ORDERBOOK_CANCEL_VERSION_V1,
                order_id,
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
        assert_eq!(outcome.cancelled_order.order_id, order_id);
    }

    #[test]
    fn runtime_cancel_advances_shared_owner_nonce_and_rejects_replay() {
        let mut runtime = OrderbookRuntime::default();
        runtime
            .submit_order(
                order(1, OrderSideV1::Ask, 1_500_000, b"provider"),
                1_800_000_000,
            )
            .expect("accept ask");
        let cancel = wrong_owner_fixture();
        let replay = cancel.clone();
        runtime.cancel_order(cancel).expect("cancel ask");

        assert!(matches!(
            runtime.cancel_order(replay),
            Err(OrderbookRuntimeError::StaleOwnerNonce {
                nonce: 2,
                highest_nonce: 2,
                ..
            })
        ));

        for stale_nonce in [1, 2] {
            let mut stale_order = order(3, OrderSideV1::Ask, 1_500_000, b"provider");
            stale_order.nonce = stale_nonce;
            stale_order.order_id =
                derive_orderbook_order_id_v1(&stale_order.owner_account, stale_nonce);
            let stale_order = sign_order(stale_order, 0x33);
            assert!(matches!(
                runtime.submit_order(stale_order, 1_800_000_001),
                Err(OrderbookRuntimeError::StaleOwnerNonce {
                    nonce,
                    highest_nonce: 2,
                    ..
                }) if nonce == stale_nonce
            ));
        }

        runtime
            .submit_order(
                order(3, OrderSideV1::Ask, 1_500_000, b"provider"),
                1_800_000_001,
            )
            .expect("higher owner nonce remains admissible");
        assert_eq!(
            runtime
                .runtime_snapshot(1_800_000_001)
                .owner_nonce_high_waters,
            vec![OrderbookOwnerNonceHighWaterV1 {
                owner_account: b"provider".to_vec(),
                highest_nonce: 3,
            }]
        );
    }

    #[test]
    fn runtime_snapshot_restore_preserves_owner_nonce_replay_protection() {
        let mut source = OrderbookRuntime::default();
        let order = order(7, OrderSideV1::Ask, 1_500_000, b"provider");
        let replay = order.clone();
        source
            .submit_order(order, 1_800_000_000)
            .expect("accept order");
        let snapshot = source.runtime_snapshot(1_800_000_001);

        let mut restored = OrderbookRuntime::default();
        restored
            .restore_runtime_snapshot(snapshot)
            .expect("restore runtime snapshot");
        assert!(matches!(
            restored.submit_order(replay, 1_800_000_002),
            Err(OrderbookRuntimeError::StaleOwnerNonce {
                nonce: 7,
                highest_nonce: 7,
                ..
            })
        ));
        assert_eq!(
            restored
                .snapshot(1_800_000_002)
                .expect("orderbook snapshot")
                .open_orders
                .len(),
            1
        );
    }

    #[test]
    fn runtime_restore_rejects_forged_owner_nonce_state_without_mutation() {
        let mut source = OrderbookRuntime::default();
        source
            .submit_order(
                order(7, OrderSideV1::Ask, 1_500_000, b"provider"),
                1_800_000_000,
            )
            .expect("accept order");
        let mut forged = source.runtime_snapshot(1_800_000_001);
        forged.owner_nonce_high_waters.clear();

        let mut target = OrderbookRuntime::default();
        let before = target.runtime_snapshot(1_800_000_001);
        assert!(matches!(
            target.restore_runtime_snapshot(forged),
            Err(OrderbookRuntimeError::Validation(
                OrderbookValidationError::SnapshotOpenOrderNonceMissing { .. }
            ))
        ));
        assert_eq!(target.runtime_snapshot(1_800_000_001), before);
    }

    #[test]
    fn owner_nonce_capacity_is_bounded_and_failure_is_atomic() {
        let mut runtime = OrderbookRuntime::with_entry_limit(1);
        runtime
            .submit_order(
                order(1, OrderSideV1::Ask, 1_500_000, b"provider-a"),
                1_800_000_000,
            )
            .expect("accept first owner");
        let before = runtime.runtime_snapshot(1_800_000_001);

        assert_eq!(
            runtime
                .submit_order(
                    order(2, OrderSideV1::Ask, 1_500_000, b"provider-b"),
                    1_800_000_001,
                )
                .expect_err("new owner past nonce capacity must be rejected"),
            OrderbookRuntimeError::ResourceExhausted {
                resource: "owner_nonce_high_waters",
                limit: 1,
            }
        );
        assert_eq!(runtime.runtime_snapshot(1_800_000_001), before);
    }

    #[test]
    fn restore_rejects_owner_nonce_state_above_configured_capacity() {
        let mut source = OrderbookRuntime::with_entry_limit(2);
        source
            .submit_order(
                order(1, OrderSideV1::Ask, 1_500_000, b"provider-a"),
                1_800_000_000,
            )
            .expect("accept first owner");
        source
            .submit_order(
                order(2, OrderSideV1::Ask, 1_500_000, b"provider-b"),
                1_800_000_000,
            )
            .expect("accept second owner");
        let snapshot = source.runtime_snapshot(1_800_000_001);

        let mut target = OrderbookRuntime::with_entry_limit(1);
        assert_eq!(
            target
                .restore_runtime_snapshot(snapshot)
                .expect_err("oversized owner nonce state must be rejected"),
            OrderbookRuntimeError::ResourceExhausted {
                resource: "owner_nonce_high_waters",
                limit: 1,
            }
        );
        assert!(
            target
                .runtime_snapshot(1_800_000_001)
                .owner_nonce_high_waters
                .is_empty()
        );
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
        let channel = runtime
            .snapshot(1_800_000_000)
            .expect("orderbook snapshot")
            .settlement_channels[0]
            .clone();
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
        let snapshot = runtime.snapshot(1_800_000_020).expect("orderbook snapshot");
        assert_eq!(snapshot.settlement_receipts.len(), 1);
        assert_eq!(
            snapshot.settlement_ledger.total_buyer_debited,
            xor("0.0001")
        );
        assert_eq!(
            snapshot.settlement_ledger.total_provider_credited,
            xor("0.00009")
        );
        assert_eq!(
            snapshot.settlement_ledger.total_fee_retained,
            xor("0.00001")
        );
        assert_eq!(
            snapshot.settlement_ledger.total_remaining_locked,
            outcome.updated_channel.xor_locked
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

    #[test]
    fn settlement_ledger_preserves_sub_micro_and_wide_values_and_rejects_overflow() {
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
        let channel = runtime
            .snapshot(1_800_000_000)
            .expect("orderbook snapshot")
            .settlement_channels[0]
            .clone();

        let mut exact_receipt = receipt(9, &channel, 0, 1, 1_800_000_010, 100);
        exact_receipt.xor_debited = xor("340282366920938463463374607431768211456.000000001");
        exact_receipt.provider_credit = xor("340282366920938463463374607431768211456");
        exact_receipt.fee_amount = xor("0.000000001");
        let ledger = settlement_ledger_from(
            std::slice::from_ref(&channel),
            std::slice::from_ref(&exact_receipt),
        )
        .expect("exact ledger");
        assert_eq!(ledger.total_buyer_debited, exact_receipt.xor_debited);
        assert_eq!(
            ledger.total_provider_credited,
            exact_receipt.provider_credit
        );
        assert_eq!(ledger.total_fee_retained, exact_receipt.fee_amount);

        let huge = xor(&format!("4{}", "0".repeat(153)));
        let mut first = exact_receipt.clone();
        first.receipt_id = [0xA1; 32];
        first.xor_debited = huge.clone();
        let mut second = first.clone();
        second.receipt_id = [0xA2; 32];
        assert_eq!(
            settlement_ledger_from(&[channel], &[first, second]),
            Err(OrderbookRuntimeError::SettlementLedgerOverflow),
            "bounded exact aggregation must fail closed instead of wrapping"
        );
    }

    fn wrong_owner_fixture() -> OrderCancelV1 {
        let owner_account = b"provider".to_vec();
        sign_cancel(
            OrderCancelV1 {
                version: sorafs_manifest::ORDERBOOK_CANCEL_VERSION_V1,
                order_id: derive_orderbook_order_id_v1(&owner_account, 1),
                owner_account,
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
                xor_debited: XorQuantity::try_from_micro(debited_micro)
                    .expect("legacy micro-XOR value is representable"),
                provider_credit: XorQuantity::try_from_micro(
                    debited_micro
                        .checked_sub(10)
                        .expect("fixture debit covers its fee"),
                )
                .expect("legacy micro-XOR value is representable"),
                fee_amount: XorQuantity::try_from_micro(10)
                    .expect("legacy micro-XOR value is representable"),
                issued_at_unix,
                settlement_signature: signature(),
            },
            id.saturating_add(0x40),
        )
    }

    #[test]
    fn configured_limits_refuse_growth_without_partial_orderbook_mutation() {
        let mut runtime = OrderbookRuntime::with_limits(1, 4);
        runtime
            .submit_order(
                order(1, OrderSideV1::Ask, 1_500_000, b"provider-a"),
                1_800_000_000,
            )
            .expect("accept first ask");
        assert!(matches!(
            runtime
                .submit_order(
                    order(9, OrderSideV1::Ask, 1_500_000, b"provider-extra"),
                    1_800_000_000,
                )
                .expect_err("second open order must be refused"),
            OrderbookRuntimeError::ResourceExhausted {
                resource: "open_orders",
                limit: 1
            }
        ));
        assert_eq!(
            runtime
                .snapshot(1_800_000_000)
                .expect("orderbook snapshot")
                .next_sequence,
            1
        );
        runtime
            .submit_order(
                order(2, OrderSideV1::Bid, 1_600_000, b"buyer-a"),
                1_800_000_000,
            )
            .expect("open first channel");
        runtime
            .submit_order(
                order(3, OrderSideV1::Ask, 1_500_000, b"provider-b"),
                1_800_000_000,
            )
            .expect("accept next ask");

        assert!(matches!(
            runtime
                .submit_order(
                    order(4, OrderSideV1::Bid, 1_600_000, b"buyer-b"),
                    1_800_000_000,
                )
                .expect_err("second trade history entry must be refused"),
            OrderbookRuntimeError::ResourceExhausted {
                resource: "trades",
                limit: 1
            }
        ));
        let snapshot = runtime.snapshot(1_800_000_000).expect("orderbook snapshot");
        assert_eq!(snapshot.next_sequence, 3);
        assert_eq!(snapshot.open_orders.len(), 1);
        assert_eq!(
            snapshot.open_orders[0].order.order_id,
            derive_orderbook_order_id_v1(b"provider-b", 3)
        );
        assert_eq!(snapshot.trades.len(), 1);
        assert_eq!(snapshot.settlement_channels.len(), 1);

        let channel = snapshot.settlement_channels[0].clone();
        runtime
            .submit_receipt(receipt(7, &channel, 0, BYTES_PER_GIB, 1_800_000_010, 100))
            .expect("accept first receipt");
        assert!(matches!(
            runtime
                .submit_receipt(receipt(
                    8,
                    &channel,
                    BYTES_PER_GIB,
                    2 * BYTES_PER_GIB,
                    1_800_000_011,
                    100,
                ))
                .expect_err("second receipt history entry must be refused"),
            OrderbookRuntimeError::ResourceExhausted {
                resource: "settlement_receipts",
                limit: 1
            }
        ));
        assert_eq!(
            runtime
                .snapshot(1_800_000_020)
                .expect("orderbook snapshot")
                .settlement_receipts
                .len(),
            1
        );
    }
}
