#![allow(unexpected_cfgs)]

//! Orderbook and streaming-settlement payload schemas for SoraFS (SFM-2).
//!
//! These Norito payloads provide the deterministic data-model foundation for
//! the future SoraFS XOR orderbook. The pure helpers in this module cover
//! deterministic pair and full-book snapshot matching, fee calculation,
//! settlement-channel opening, receipt application, and payload signature
//! verification. Runtime account authorization, service-side sequencing,
//! contract submission, and durable escrow mutation still belong to the runtime
//! layers that consume these payloads.

use std::collections::BTreeSet;

use blake3::Hasher;
use ed25519_dalek::{
    PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, Signature as DalekSignature, Verifier, VerifyingKey,
};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

use crate::{
    deal::{BASIS_POINTS_PER_UNIT, DealAmountError, XorAmount},
    provider_advert::SignatureAlgorithm,
};

/// Schema version for [`OrderRequestV1`].
pub const ORDERBOOK_ORDER_VERSION_V1: u8 = 1;
/// Schema version for [`OrderCancelV1`].
pub const ORDERBOOK_CANCEL_VERSION_V1: u8 = 1;
/// Schema version for [`TradeEventV1`].
pub const ORDERBOOK_TRADE_EVENT_VERSION_V1: u8 = 1;
/// Schema version for [`SettlementChannelV1`].
pub const SETTLEMENT_CHANNEL_VERSION_V1: u8 = 1;
/// Schema version for [`SettlementReceiptV1`].
pub const SETTLEMENT_RECEIPT_VERSION_V1: u8 = 1;
/// Number of bytes in one GiB.
pub const BYTES_PER_GIB: u64 = 1_073_741_824;
const ORDERBOOK_TRADE_ID_DOMAIN_V1: &[u8] = b"sorafs.orderbook.trade-id.v1";
const ORDERBOOK_ORDER_SIGNATURE_DOMAIN_V1: &[u8] = b"sorafs.orderbook.order-signature.v1";
const ORDERBOOK_CANCEL_SIGNATURE_DOMAIN_V1: &[u8] = b"sorafs.orderbook.cancel-signature.v1";
const SETTLEMENT_RECEIPT_SIGNATURE_DOMAIN_V1: &[u8] =
    b"sorafs.orderbook.settlement-receipt-signature.v1";

/// Order side in the XOR orderbook.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum OrderSideV1 {
    /// Buyer bids for capacity/egress.
    Bid = 1,
    /// Provider asks to sell capacity/egress.
    Ask = 2,
}

/// SoraFS storage tier used by orderbook pricing.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum OrderTierV1 {
    /// Hot storage tier.
    Hot = 1,
    /// Warm storage tier.
    Warm = 2,
    /// Archive storage tier.
    Archive = 3,
}

/// Reason attached to an order-cancel request.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum OrderCancelReasonV1 {
    /// Owner-requested cancellation.
    OwnerRequested = 1,
    /// Order expired.
    Expired = 2,
    /// Governance pause or policy change.
    Governance = 3,
    /// Order was replaced by a newer nonce/order.
    Replaced = 4,
}

/// Settlement channel lifecycle status.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum SettlementChannelStatusV1 {
    /// Channel is open and can accept receipts.
    Open = 1,
    /// Channel is closing after final receipt/reconciliation.
    Closing = 2,
    /// Channel closed successfully.
    Closed = 3,
    /// Channel breached policy or delivery SLA.
    Breached = 4,
    /// Channel was refunded.
    Refunded = 5,
}

/// Signature material attached to orderbook payloads.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct OrderbookSignatureV1 {
    /// Signature algorithm identifier.
    pub algorithm: SignatureAlgorithm,
    /// Public key bytes.
    pub public_key: Vec<u8>,
    /// Raw signature bytes.
    pub signature: Vec<u8>,
}

impl OrderbookSignatureV1 {
    fn validate(&self) -> Result<(), OrderbookValidationError> {
        if self.public_key.is_empty() || self.signature.is_empty() {
            return Err(OrderbookValidationError::InvalidSignature);
        }
        if matches!(self.algorithm, SignatureAlgorithm::Ed25519) {
            if self.public_key.len() != PUBLIC_KEY_LENGTH {
                return Err(OrderbookValidationError::InvalidPublicKeyLength {
                    length: self.public_key.len(),
                });
            }
            if self.signature.len() != SIGNATURE_LENGTH {
                return Err(OrderbookValidationError::InvalidSignatureLength {
                    length: self.signature.len(),
                });
            }
        }
        Ok(())
    }
}

/// Canonical order-submission payload.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct OrderRequestV1 {
    /// Schema version (`ORDERBOOK_ORDER_VERSION_V1`).
    pub version: u8,
    /// Unique order identifier.
    pub order_id: [u8; 32],
    /// Bid or ask.
    pub side: OrderSideV1,
    /// Storage tier priced by the order.
    pub tier: OrderTierV1,
    /// Price in micro-XOR per GiB.
    pub price_per_gib: XorAmount,
    /// Total GiB requested/offered.
    pub quantity_gib: u64,
    /// Remaining GiB after partial fills.
    pub remaining_gib: u64,
    /// Canonical owner account bytes.
    pub owner_account: Vec<u8>,
    /// Unix timestamp (seconds) after which the order expires.
    pub expiry_unix: u64,
    /// Owner nonce used to prevent replay.
    pub nonce: u64,
    /// Maker fee in basis points.
    pub maker_fee_bps: u16,
    /// Taker fee in basis points.
    pub taker_fee_bps: u16,
    /// Signature over the canonical order submission bytes.
    pub signature: OrderbookSignatureV1,
}

impl OrderRequestV1 {
    /// Validate structural and policy constraints.
    pub fn validate(&self) -> Result<(), OrderbookValidationError> {
        if self.version != ORDERBOOK_ORDER_VERSION_V1 {
            return Err(OrderbookValidationError::UnsupportedOrderVersion {
                found: self.version,
            });
        }
        validate_digest(self.order_id, OrderbookValidationError::InvalidOrderId)?;
        if self.price_per_gib.is_zero() {
            return Err(OrderbookValidationError::ZeroPrice);
        }
        if self.quantity_gib == 0 {
            return Err(OrderbookValidationError::ZeroQuantity);
        }
        if self.remaining_gib == 0 || self.remaining_gib > self.quantity_gib {
            return Err(OrderbookValidationError::InvalidRemainingQuantity {
                remaining_gib: self.remaining_gib,
                quantity_gib: self.quantity_gib,
            });
        }
        if self.owner_account.is_empty() {
            return Err(OrderbookValidationError::EmptyOwnerAccount);
        }
        if self.expiry_unix == 0 {
            return Err(OrderbookValidationError::InvalidTimestamp);
        }
        if self.nonce == 0 {
            return Err(OrderbookValidationError::ZeroNonce);
        }
        validate_fee_bps(self.maker_fee_bps)?;
        validate_fee_bps(self.taker_fee_bps)?;
        self.signature.validate()
    }
}

/// Canonical order-cancel payload.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct OrderCancelV1 {
    /// Schema version (`ORDERBOOK_CANCEL_VERSION_V1`).
    pub version: u8,
    /// Order being cancelled.
    pub order_id: [u8; 32],
    /// Canonical owner account bytes.
    pub owner_account: Vec<u8>,
    /// Cancellation reason.
    pub reason: OrderCancelReasonV1,
    /// Owner nonce used to prevent replay.
    pub nonce: u64,
    /// Signature over the canonical cancellation bytes.
    pub signature: OrderbookSignatureV1,
}

impl OrderCancelV1 {
    /// Validate structural and policy constraints.
    pub fn validate(&self) -> Result<(), OrderbookValidationError> {
        if self.version != ORDERBOOK_CANCEL_VERSION_V1 {
            return Err(OrderbookValidationError::UnsupportedCancelVersion {
                found: self.version,
            });
        }
        validate_digest(self.order_id, OrderbookValidationError::InvalidOrderId)?;
        if self.owner_account.is_empty() {
            return Err(OrderbookValidationError::EmptyOwnerAccount);
        }
        if self.nonce == 0 {
            return Err(OrderbookValidationError::ZeroNonce);
        }
        self.signature.validate()
    }
}

/// Derive the canonical Ed25519 message digest for an order submission.
///
/// The digest is BLAKE3 over a domain separator plus the canonical Norito order
/// bytes with only `signature.signature` cleared. This avoids a circular
/// signature while binding the algorithm, signer public key, order owner,
/// nonce, pricing, and quantity fields.
pub fn order_request_signature_digest_v1(
    order: &OrderRequestV1,
) -> Result<[u8; 32], OrderbookValidationError> {
    let mut signable = order.clone();
    signable.signature.signature.clear();
    orderbook_signature_digest(ORDERBOOK_ORDER_SIGNATURE_DOMAIN_V1, &signable)
}

/// Verify the signature attached to an order submission.
pub fn verify_order_request_signature_v1(
    order: &OrderRequestV1,
) -> Result<(), OrderbookValidationError> {
    order.validate()?;
    let digest = order_request_signature_digest_v1(order)?;
    verify_orderbook_signature_v1(&order.signature, &digest)
}

/// Derive the canonical Ed25519 message digest for an order cancellation.
///
/// The digest is BLAKE3 over a domain separator plus the canonical Norito cancel
/// bytes with only `signature.signature` cleared.
pub fn order_cancel_signature_digest_v1(
    cancel: &OrderCancelV1,
) -> Result<[u8; 32], OrderbookValidationError> {
    let mut signable = cancel.clone();
    signable.signature.signature.clear();
    orderbook_signature_digest(ORDERBOOK_CANCEL_SIGNATURE_DOMAIN_V1, &signable)
}

/// Verify the signature attached to an order cancellation.
pub fn verify_order_cancel_signature_v1(
    cancel: &OrderCancelV1,
) -> Result<(), OrderbookValidationError> {
    cancel.validate()?;
    let digest = order_cancel_signature_digest_v1(cancel)?;
    verify_orderbook_signature_v1(&cancel.signature, &digest)
}

/// Trade fill event emitted by a deterministic matcher/contract.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct TradeEventV1 {
    /// Schema version (`ORDERBOOK_TRADE_EVENT_VERSION_V1`).
    pub version: u8,
    /// Unique trade identifier.
    pub trade_id: [u8; 32],
    /// Maker order identifier.
    pub maker_order_id: [u8; 32],
    /// Taker order identifier.
    pub taker_order_id: [u8; 32],
    /// Storage tier filled by the trade.
    pub tier: OrderTierV1,
    /// Fill price in micro-XOR per GiB.
    pub price_per_gib: XorAmount,
    /// Filled GiB.
    pub filled_gib: u64,
    /// Maker fee charged for the fill.
    pub maker_fee: XorAmount,
    /// Taker fee charged for the fill.
    pub taker_fee: XorAmount,
    /// Unix timestamp (seconds) when the fill was recorded.
    pub timestamp_unix: u64,
}

impl TradeEventV1 {
    /// Validate structural and accounting constraints.
    pub fn validate(&self) -> Result<(), OrderbookValidationError> {
        if self.version != ORDERBOOK_TRADE_EVENT_VERSION_V1 {
            return Err(OrderbookValidationError::UnsupportedTradeVersion {
                found: self.version,
            });
        }
        validate_digest(self.trade_id, OrderbookValidationError::InvalidTradeId)?;
        validate_digest(
            self.maker_order_id,
            OrderbookValidationError::InvalidMakerOrderId,
        )?;
        validate_digest(
            self.taker_order_id,
            OrderbookValidationError::InvalidTakerOrderId,
        )?;
        if self.maker_order_id == self.taker_order_id {
            return Err(OrderbookValidationError::SelfTrade);
        }
        if self.price_per_gib.is_zero() {
            return Err(OrderbookValidationError::ZeroPrice);
        }
        if self.filled_gib == 0 {
            return Err(OrderbookValidationError::ZeroQuantity);
        }
        if self.timestamp_unix == 0 {
            return Err(OrderbookValidationError::InvalidTimestamp);
        }
        Ok(())
    }
}

/// Deterministic result of matching one maker order with one taker order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderFillOutcomeV1 {
    /// Trade event emitted for the fill.
    pub trade: TradeEventV1,
    /// Maker remaining GiB after the fill.
    pub maker_remaining_gib: u64,
    /// Taker remaining GiB after the fill.
    pub taker_remaining_gib: u64,
    /// Gross fill value before maker/taker fees.
    pub gross_value: XorAmount,
}

/// Order plus canonical admission sequence used for price-time priority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderBookEntryV1 {
    /// Canonical order payload.
    pub order: OrderRequestV1,
    /// Monotonic admission sequence assigned by the caller/runtime.
    pub sequence: u64,
}

/// Deterministic result of matching a full order-book snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderBookMatchOutcomeV1 {
    /// Fill outcomes in deterministic execution order.
    pub fills: Vec<OrderFillOutcomeV1>,
    /// Non-expired orders left after matching, sorted by admission sequence.
    pub remaining_orders: Vec<OrderRequestV1>,
    /// Expired order identifiers skipped before matching.
    pub expired_order_ids: Vec<[u8; 32]>,
}

#[derive(Debug, Clone)]
struct WorkingOrderV1 {
    order: OrderRequestV1,
    sequence: u64,
}

/// Match one maker order with one taker order and return the deterministic fill.
///
/// The fill uses the maker price, the minimum remaining quantity, and the maker
/// and taker fee basis points attached to the respective orders. This function
/// does not authorize signatures or mutate persistent order state.
pub fn match_orders_v1(
    maker: &OrderRequestV1,
    taker: &OrderRequestV1,
    trade_id: [u8; 32],
    timestamp_unix: u64,
) -> Result<OrderFillOutcomeV1, OrderbookValidationError> {
    maker.validate()?;
    taker.validate()?;
    validate_digest(trade_id, OrderbookValidationError::InvalidTradeId)?;
    if timestamp_unix == 0 {
        return Err(OrderbookValidationError::InvalidTimestamp);
    }
    if timestamp_unix > maker.expiry_unix {
        return Err(OrderbookValidationError::ExpiredOrder {
            order_id: maker.order_id,
            expiry_unix: maker.expiry_unix,
            now_unix: timestamp_unix,
        });
    }
    if timestamp_unix > taker.expiry_unix {
        return Err(OrderbookValidationError::ExpiredOrder {
            order_id: taker.order_id,
            expiry_unix: taker.expiry_unix,
            now_unix: timestamp_unix,
        });
    }
    if maker.order_id == taker.order_id || maker.owner_account == taker.owner_account {
        return Err(OrderbookValidationError::SelfTrade);
    }
    if maker.side == taker.side {
        return Err(OrderbookValidationError::SameOrderSide { side: maker.side });
    }
    if maker.tier != taker.tier {
        return Err(OrderbookValidationError::TierMismatch {
            maker_tier: maker.tier,
            taker_tier: taker.tier,
        });
    }

    let (bid, ask) = if maker.side == OrderSideV1::Bid {
        (maker, taker)
    } else {
        (taker, maker)
    };
    if bid.price_per_gib.as_micro() < ask.price_per_gib.as_micro() {
        return Err(OrderbookValidationError::PriceDoesNotCross {
            bid_price_micro: bid.price_per_gib.as_micro(),
            ask_price_micro: ask.price_per_gib.as_micro(),
        });
    }

    let filled_gib = maker.remaining_gib.min(taker.remaining_gib);
    let gross_value = maker
        .price_per_gib
        .checked_mul_u64(filled_gib)
        .map_err(OrderbookValidationError::Amount)?;
    let maker_fee = gross_value
        .checked_mul_basis_points(maker.maker_fee_bps)
        .map_err(OrderbookValidationError::Amount)?;
    let taker_fee = gross_value
        .checked_mul_basis_points(taker.taker_fee_bps)
        .map_err(OrderbookValidationError::Amount)?;
    let trade = TradeEventV1 {
        version: ORDERBOOK_TRADE_EVENT_VERSION_V1,
        trade_id,
        maker_order_id: maker.order_id,
        taker_order_id: taker.order_id,
        tier: maker.tier,
        price_per_gib: maker.price_per_gib,
        filled_gib,
        maker_fee,
        taker_fee,
        timestamp_unix,
    };
    trade.validate()?;

    Ok(OrderFillOutcomeV1 {
        trade,
        maker_remaining_gib: maker.remaining_gib - filled_gib,
        taker_remaining_gib: taker.remaining_gib - filled_gib,
        gross_value,
    })
}

/// Match an order-book snapshot using deterministic price-time priority.
///
/// Callers must supply entries in any order together with unique monotonic
/// `sequence` values from the canonical order-admission log. The helper filters
/// expired orders into `expired_order_ids`, then matches independently by tier:
/// bids sort by highest price, asks by lowest price, and both sides use lower
/// sequence as the time-priority tiebreaker.
pub fn match_order_book_v1(
    entries: &[OrderBookEntryV1],
    timestamp_unix: u64,
) -> Result<OrderBookMatchOutcomeV1, OrderbookValidationError> {
    if timestamp_unix == 0 {
        return Err(OrderbookValidationError::InvalidTimestamp);
    }

    let mut seen_order_ids = BTreeSet::new();
    let mut seen_sequences = BTreeSet::new();
    let mut expired_order_ids = Vec::new();
    let mut bids = Vec::new();
    let mut asks = Vec::new();

    for entry in entries {
        entry.order.validate()?;
        if !seen_order_ids.insert(entry.order.order_id) {
            return Err(OrderbookValidationError::DuplicateOrderId {
                order_id: entry.order.order_id,
            });
        }
        if !seen_sequences.insert(entry.sequence) {
            return Err(OrderbookValidationError::DuplicateOrderSequence {
                sequence: entry.sequence,
            });
        }
        if timestamp_unix > entry.order.expiry_unix {
            expired_order_ids.push(entry.order.order_id);
            continue;
        }

        let working = WorkingOrderV1 {
            order: entry.order.clone(),
            sequence: entry.sequence,
        };
        match working.order.side {
            OrderSideV1::Bid => bids.push(working),
            OrderSideV1::Ask => asks.push(working),
        }
    }

    let mut fills = Vec::new();
    let mut remaining = Vec::new();
    for tier in [OrderTierV1::Hot, OrderTierV1::Warm, OrderTierV1::Archive] {
        let mut tier_bids = bids
            .iter()
            .filter(|entry| entry.order.tier == tier)
            .cloned()
            .collect::<Vec<_>>();
        let mut tier_asks = asks
            .iter()
            .filter(|entry| entry.order.tier == tier)
            .cloned()
            .collect::<Vec<_>>();
        sort_bids_by_price_time(&mut tier_bids);
        sort_asks_by_price_time(&mut tier_asks);

        while !tier_bids.is_empty() && !tier_asks.is_empty() {
            if tier_bids[0].order.price_per_gib.as_micro()
                < tier_asks[0].order.price_per_gib.as_micro()
            {
                break;
            }

            let maker_is_bid = tier_bids[0].sequence < tier_asks[0].sequence;
            let (maker, taker) = if maker_is_bid {
                (&tier_bids[0].order, &tier_asks[0].order)
            } else {
                (&tier_asks[0].order, &tier_bids[0].order)
            };
            let trade_id =
                derive_orderbook_trade_id_v1(fills.len() as u64, maker, taker, timestamp_unix);
            let fill = match_orders_v1(maker, taker, trade_id, timestamp_unix)?;

            if maker_is_bid {
                tier_bids[0].order.remaining_gib = fill.maker_remaining_gib;
                tier_asks[0].order.remaining_gib = fill.taker_remaining_gib;
            } else {
                tier_asks[0].order.remaining_gib = fill.maker_remaining_gib;
                tier_bids[0].order.remaining_gib = fill.taker_remaining_gib;
            }
            fills.push(fill);

            if tier_bids[0].order.remaining_gib == 0 {
                tier_bids.remove(0);
            }
            if !tier_asks.is_empty() && tier_asks[0].order.remaining_gib == 0 {
                tier_asks.remove(0);
            }
        }

        remaining.extend(
            tier_bids
                .into_iter()
                .chain(tier_asks.into_iter())
                .filter(|entry| entry.order.remaining_gib > 0),
        );
    }
    remaining.sort_by(|lhs, rhs| {
        lhs.sequence
            .cmp(&rhs.sequence)
            .then_with(|| lhs.order.order_id.cmp(&rhs.order.order_id))
    });

    Ok(OrderBookMatchOutcomeV1 {
        fills,
        remaining_orders: remaining.into_iter().map(|entry| entry.order).collect(),
        expired_order_ids,
    })
}

/// Derive a deterministic trade id for full-book matching.
#[must_use]
pub fn derive_orderbook_trade_id_v1(
    fill_index: u64,
    maker: &OrderRequestV1,
    taker: &OrderRequestV1,
    timestamp_unix: u64,
) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(ORDERBOOK_TRADE_ID_DOMAIN_V1);
    hasher.update(&fill_index.to_le_bytes());
    hasher.update(&timestamp_unix.to_le_bytes());
    hasher.update(&maker.order_id);
    hasher.update(&taker.order_id);
    hasher.update(&maker.remaining_gib.to_le_bytes());
    hasher.update(&taker.remaining_gib.to_le_bytes());
    let mut trade_id = *hasher.finalize().as_bytes();
    if trade_id.iter().all(|byte| *byte == 0) {
        trade_id[31] = 1;
    }
    trade_id
}

fn sort_bids_by_price_time(entries: &mut [WorkingOrderV1]) {
    entries.sort_by(|lhs, rhs| {
        rhs.order
            .price_per_gib
            .as_micro()
            .cmp(&lhs.order.price_per_gib.as_micro())
            .then_with(|| lhs.sequence.cmp(&rhs.sequence))
            .then_with(|| lhs.order.order_id.cmp(&rhs.order.order_id))
    });
}

fn sort_asks_by_price_time(entries: &mut [WorkingOrderV1]) {
    entries.sort_by(|lhs, rhs| {
        lhs.order
            .price_per_gib
            .as_micro()
            .cmp(&rhs.order.price_per_gib.as_micro())
            .then_with(|| lhs.sequence.cmp(&rhs.sequence))
            .then_with(|| lhs.order.order_id.cmp(&rhs.order.order_id))
    });
}

/// Return the gross value represented by a trade event.
pub fn trade_gross_value_v1(trade: &TradeEventV1) -> Result<XorAmount, OrderbookValidationError> {
    trade.validate()?;
    trade
        .price_per_gib
        .checked_mul_u64(trade.filled_gib)
        .map_err(OrderbookValidationError::Amount)
}

/// Return the escrow amount needed to cover a trade value and both fee fields.
pub fn trade_escrow_requirement_v1(
    trade: &TradeEventV1,
) -> Result<XorAmount, OrderbookValidationError> {
    trade_gross_value_v1(trade)?
        .checked_add(trade.maker_fee)
        .and_then(|amount| amount.checked_add(trade.taker_fee))
        .map_err(OrderbookValidationError::Amount)
}

/// Half-open byte range `[start, end)` covered by a settlement receipt.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct ByteRangeV1 {
    /// Start offset, inclusive.
    pub start: u64,
    /// End offset, exclusive.
    pub end: u64,
}

impl ByteRangeV1 {
    /// Return the byte length represented by the range.
    pub fn len(self) -> Result<u64, OrderbookValidationError> {
        self.end
            .checked_sub(self.start)
            .filter(|length| *length > 0)
            .ok_or(OrderbookValidationError::InvalidByteRange {
                start: self.start,
                end: self.end,
            })
    }
}

/// Streaming settlement channel state.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct SettlementChannelV1 {
    /// Schema version (`SETTLEMENT_CHANNEL_VERSION_V1`).
    pub version: u8,
    /// Unique settlement channel identifier.
    pub channel_id: [u8; 32],
    /// Trade associated with the channel.
    pub trade_id: [u8; 32],
    /// Canonical buyer account bytes.
    pub buyer_account: Vec<u8>,
    /// Governance-controlled provider identifier.
    pub provider_id: [u8; 32],
    /// Total bytes covered by the channel.
    pub total_bytes: u64,
    /// Bytes not yet settled.
    pub remaining_bytes: u64,
    /// XOR locked in escrow for the channel.
    pub xor_locked: XorAmount,
    /// Channel status.
    pub status: SettlementChannelStatusV1,
    /// Unix timestamp (seconds) when the channel opened.
    pub opened_at_unix: u64,
    /// Unix timestamp (seconds) when the channel last changed.
    pub updated_at_unix: u64,
}

impl SettlementChannelV1 {
    /// Validate structural and accounting constraints.
    pub fn validate(&self) -> Result<(), OrderbookValidationError> {
        if self.version != SETTLEMENT_CHANNEL_VERSION_V1 {
            return Err(OrderbookValidationError::UnsupportedChannelVersion {
                found: self.version,
            });
        }
        validate_digest(self.channel_id, OrderbookValidationError::InvalidChannelId)?;
        validate_digest(self.trade_id, OrderbookValidationError::InvalidTradeId)?;
        if self.buyer_account.is_empty() {
            return Err(OrderbookValidationError::EmptyBuyerAccount);
        }
        validate_digest(
            self.provider_id,
            OrderbookValidationError::InvalidProviderId,
        )?;
        if self.total_bytes == 0 {
            return Err(OrderbookValidationError::ZeroBytes);
        }
        if self.remaining_bytes > self.total_bytes {
            return Err(OrderbookValidationError::InvalidRemainingBytes {
                remaining_bytes: self.remaining_bytes,
                total_bytes: self.total_bytes,
            });
        }
        if self.xor_locked.is_zero()
            && !matches!(
                self.status,
                SettlementChannelStatusV1::Closed | SettlementChannelStatusV1::Refunded
            )
        {
            return Err(OrderbookValidationError::ZeroEscrow);
        }
        if self.opened_at_unix == 0 || self.updated_at_unix < self.opened_at_unix {
            return Err(OrderbookValidationError::InvalidTimestamp);
        }
        Ok(())
    }
}

/// Open a settlement channel for a trade using the canonical GiB-to-byte mapping.
///
/// The channel locks the gross trade value plus both fee fields, providing a
/// deterministic escrow floor for the future runtime.
pub fn open_settlement_channel_for_trade_v1(
    trade: &TradeEventV1,
    channel_id: [u8; 32],
    buyer_account: Vec<u8>,
    provider_id: [u8; 32],
    opened_at_unix: u64,
) -> Result<SettlementChannelV1, OrderbookValidationError> {
    trade.validate()?;
    let total_bytes = trade
        .filled_gib
        .checked_mul(BYTES_PER_GIB)
        .ok_or(OrderbookValidationError::ByteCountOverflow)?;
    let channel = SettlementChannelV1 {
        version: SETTLEMENT_CHANNEL_VERSION_V1,
        channel_id,
        trade_id: trade.trade_id,
        buyer_account,
        provider_id,
        total_bytes,
        remaining_bytes: total_bytes,
        xor_locked: trade_escrow_requirement_v1(trade)?,
        status: SettlementChannelStatusV1::Open,
        opened_at_unix,
        updated_at_unix: opened_at_unix,
    };
    channel.validate()?;
    Ok(channel)
}

/// Signed streaming-settlement receipt for delivered bytes.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct SettlementReceiptV1 {
    /// Schema version (`SETTLEMENT_RECEIPT_VERSION_V1`).
    pub version: u8,
    /// Unique receipt identifier.
    pub receipt_id: [u8; 32],
    /// Channel being settled.
    pub channel_id: [u8; 32],
    /// Trade associated with the channel.
    pub trade_id: [u8; 32],
    /// Delivered byte range.
    pub range: ByteRangeV1,
    /// Chunk/content digest for the delivered range.
    pub chunk_hash: [u8; 32],
    /// Delivered byte count.
    pub bytes_delivered: u64,
    /// XOR debited from buyer escrow.
    pub xor_debited: XorAmount,
    /// XOR credited to the provider.
    pub provider_credit: XorAmount,
    /// XOR retained as fee.
    pub fee_amount: XorAmount,
    /// Unix timestamp (seconds) when the receipt was issued.
    pub issued_at_unix: u64,
    /// Signature over the canonical settlement receipt bytes.
    pub settlement_signature: OrderbookSignatureV1,
}

impl SettlementReceiptV1 {
    /// Validate structural and accounting constraints.
    pub fn validate(&self) -> Result<(), OrderbookValidationError> {
        if self.version != SETTLEMENT_RECEIPT_VERSION_V1 {
            return Err(OrderbookValidationError::UnsupportedReceiptVersion {
                found: self.version,
            });
        }
        validate_digest(self.receipt_id, OrderbookValidationError::InvalidReceiptId)?;
        validate_digest(self.channel_id, OrderbookValidationError::InvalidChannelId)?;
        validate_digest(self.trade_id, OrderbookValidationError::InvalidTradeId)?;
        validate_digest(self.chunk_hash, OrderbookValidationError::InvalidChunkHash)?;
        let range_len = self.range.len()?;
        if self.bytes_delivered == 0 || self.bytes_delivered != range_len {
            return Err(OrderbookValidationError::InvalidDeliveredBytes {
                bytes_delivered: self.bytes_delivered,
                range_len,
            });
        }
        if self.xor_debited.is_zero() {
            return Err(OrderbookValidationError::ZeroDebit);
        }
        let total_credit = self
            .provider_credit
            .checked_add(self.fee_amount)
            .map_err(OrderbookValidationError::Amount)?;
        if total_credit != self.xor_debited {
            return Err(OrderbookValidationError::SettlementImbalance {
                debited: self.xor_debited.as_micro(),
                credited_plus_fees: total_credit.as_micro(),
            });
        }
        if self.issued_at_unix == 0 {
            return Err(OrderbookValidationError::InvalidTimestamp);
        }
        self.settlement_signature.validate()
    }
}

/// Derive the canonical Ed25519 message digest for a settlement receipt.
///
/// The digest is BLAKE3 over a domain separator plus the canonical Norito
/// receipt bytes with only `settlement_signature.signature` cleared.
pub fn settlement_receipt_signature_digest_v1(
    receipt: &SettlementReceiptV1,
) -> Result<[u8; 32], OrderbookValidationError> {
    let mut signable = receipt.clone();
    signable.settlement_signature.signature.clear();
    orderbook_signature_digest(SETTLEMENT_RECEIPT_SIGNATURE_DOMAIN_V1, &signable)
}

/// Verify the signature attached to a settlement receipt.
pub fn verify_settlement_receipt_signature_v1(
    receipt: &SettlementReceiptV1,
) -> Result<(), OrderbookValidationError> {
    receipt.validate()?;
    let digest = settlement_receipt_signature_digest_v1(receipt)?;
    verify_orderbook_signature_v1(&receipt.settlement_signature, &digest)
}

/// Apply a validated settlement receipt to a settlement channel.
///
/// This helper enforces channel/trade binding, monotonic receipt time, remaining
/// byte coverage, and escrow sufficiency. It returns the next channel snapshot
/// without mutating persistent runtime state.
pub fn apply_settlement_receipt_v1(
    channel: &SettlementChannelV1,
    receipt: &SettlementReceiptV1,
) -> Result<SettlementChannelV1, OrderbookValidationError> {
    channel.validate()?;
    receipt.validate()?;
    if !matches!(
        channel.status,
        SettlementChannelStatusV1::Open | SettlementChannelStatusV1::Closing
    ) {
        return Err(OrderbookValidationError::SettlementChannelNotOpen {
            status: channel.status,
        });
    }
    if channel.channel_id != receipt.channel_id || channel.trade_id != receipt.trade_id {
        return Err(OrderbookValidationError::SettlementChannelMismatch);
    }
    if receipt.issued_at_unix < channel.updated_at_unix {
        return Err(OrderbookValidationError::InvalidTimestamp);
    }
    let delivered = receipt.range.len()?;
    if receipt.range.end > channel.total_bytes {
        return Err(OrderbookValidationError::ReceiptExceedsChannelBytes {
            range_end: receipt.range.end,
            total_bytes: channel.total_bytes,
        });
    }
    if delivered > channel.remaining_bytes {
        return Err(OrderbookValidationError::ReceiptExceedsRemainingBytes {
            delivered,
            remaining_bytes: channel.remaining_bytes,
        });
    }
    if receipt.xor_debited.as_micro() > channel.xor_locked.as_micro() {
        return Err(OrderbookValidationError::ReceiptExceedsEscrow {
            debited: receipt.xor_debited.as_micro(),
            escrow: channel.xor_locked.as_micro(),
        });
    }

    let remaining_bytes = channel.remaining_bytes - delivered;
    let status = if remaining_bytes == 0 {
        SettlementChannelStatusV1::Closed
    } else {
        channel.status
    };
    let next = SettlementChannelV1 {
        remaining_bytes,
        xor_locked: channel
            .xor_locked
            .checked_sub(receipt.xor_debited)
            .map_err(OrderbookValidationError::Amount)?,
        status,
        updated_at_unix: receipt.issued_at_unix,
        ..channel.clone()
    };
    next.validate()?;
    Ok(next)
}

fn orderbook_signature_digest<T: norito::core::NoritoSerialize>(
    domain: &[u8],
    payload: &T,
) -> Result<[u8; 32], OrderbookValidationError> {
    let payload_bytes = norito::to_bytes(payload).map_err(|err| {
        OrderbookValidationError::SignaturePayloadEncoding {
            reason: err.to_string(),
        }
    })?;
    let mut hasher = Hasher::new();
    hasher.update(domain);
    hasher.update(&payload_bytes);
    Ok(*hasher.finalize().as_bytes())
}

fn verify_orderbook_signature_v1(
    signature: &OrderbookSignatureV1,
    digest: &[u8; 32],
) -> Result<(), OrderbookValidationError> {
    signature.validate()?;
    match signature.algorithm {
        SignatureAlgorithm::Ed25519 => verify_ed25519_orderbook_signature(signature, digest),
        SignatureAlgorithm::MultiSig => {
            Err(OrderbookValidationError::UnsupportedSignatureAlgorithm {
                algorithm: signature.algorithm,
            })
        }
    }
}

fn verify_ed25519_orderbook_signature(
    signature: &OrderbookSignatureV1,
    digest: &[u8; 32],
) -> Result<(), OrderbookValidationError> {
    let mut public_key = [0u8; PUBLIC_KEY_LENGTH];
    public_key.copy_from_slice(&signature.public_key);
    let verifying_key = VerifyingKey::from_bytes(&public_key).map_err(|err| {
        OrderbookValidationError::InvalidPublicKey {
            reason: err.to_string(),
        }
    })?;

    let mut signature_bytes = [0u8; SIGNATURE_LENGTH];
    signature_bytes.copy_from_slice(&signature.signature);
    let signature = DalekSignature::from_bytes(&signature_bytes);

    verifying_key.verify(digest, &signature).map_err(|err| {
        OrderbookValidationError::SignatureVerification {
            reason: err.to_string(),
        }
    })
}

fn validate_digest(
    digest: [u8; 32],
    error: OrderbookValidationError,
) -> Result<(), OrderbookValidationError> {
    if digest.iter().all(|byte| *byte == 0) {
        return Err(error);
    }
    Ok(())
}

fn validate_fee_bps(fee_bps: u16) -> Result<(), OrderbookValidationError> {
    if fee_bps > BASIS_POINTS_PER_UNIT {
        return Err(OrderbookValidationError::InvalidFeeBps { fee_bps });
    }
    Ok(())
}

/// Validation errors for SoraFS orderbook payloads.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OrderbookValidationError {
    /// Unsupported order version.
    #[error("unsupported order version {found}")]
    UnsupportedOrderVersion { found: u8 },
    /// Unsupported cancel version.
    #[error("unsupported cancel version {found}")]
    UnsupportedCancelVersion { found: u8 },
    /// Unsupported trade event version.
    #[error("unsupported trade event version {found}")]
    UnsupportedTradeVersion { found: u8 },
    /// Unsupported settlement channel version.
    #[error("unsupported settlement channel version {found}")]
    UnsupportedChannelVersion { found: u8 },
    /// Unsupported settlement receipt version.
    #[error("unsupported settlement receipt version {found}")]
    UnsupportedReceiptVersion { found: u8 },
    /// Order identifier is all zeroes.
    #[error("order id must not be zero")]
    InvalidOrderId,
    /// Maker order identifier is all zeroes.
    #[error("maker order id must not be zero")]
    InvalidMakerOrderId,
    /// Taker order identifier is all zeroes.
    #[error("taker order id must not be zero")]
    InvalidTakerOrderId,
    /// Trade identifier is all zeroes.
    #[error("trade id must not be zero")]
    InvalidTradeId,
    /// Settlement channel identifier is all zeroes.
    #[error("settlement channel id must not be zero")]
    InvalidChannelId,
    /// Settlement receipt identifier is all zeroes.
    #[error("settlement receipt id must not be zero")]
    InvalidReceiptId,
    /// Chunk hash is all zeroes.
    #[error("chunk hash must not be zero")]
    InvalidChunkHash,
    /// Provider identifier is all zeroes.
    #[error("provider id must not be zero")]
    InvalidProviderId,
    /// Price is zero.
    #[error("price must be positive")]
    ZeroPrice,
    /// Quantity is zero.
    #[error("quantity must be positive")]
    ZeroQuantity,
    /// Remaining quantity is zero or exceeds total quantity.
    #[error("remaining quantity {remaining_gib} must be within 1..={quantity_gib}")]
    InvalidRemainingQuantity {
        /// Remaining GiB.
        remaining_gib: u64,
        /// Total GiB.
        quantity_gib: u64,
    },
    /// Owner account bytes are empty.
    #[error("owner account must not be empty")]
    EmptyOwnerAccount,
    /// Buyer account bytes are empty.
    #[error("buyer account must not be empty")]
    EmptyBuyerAccount,
    /// Timestamp is missing or out of order.
    #[error("timestamp is missing or out of order")]
    InvalidTimestamp,
    /// Nonce is zero.
    #[error("nonce must be non-zero")]
    ZeroNonce,
    /// Fee exceeds 100%.
    #[error("fee basis points {fee_bps} exceed 100%")]
    InvalidFeeBps {
        /// Fee basis points.
        fee_bps: u16,
    },
    /// Signature material is missing.
    #[error("signature material must not be empty")]
    InvalidSignature,
    /// Ed25519 public key length is invalid.
    #[error("invalid Ed25519 public key length {length}")]
    InvalidPublicKeyLength {
        /// Observed public key byte length.
        length: usize,
    },
    /// Ed25519 signature length is invalid.
    #[error("invalid Ed25519 signature length {length}")]
    InvalidSignatureLength {
        /// Observed signature byte length.
        length: usize,
    },
    /// Signature algorithm is not supported for orderbook payload verification.
    #[error("unsupported orderbook signature algorithm {algorithm:?}")]
    UnsupportedSignatureAlgorithm {
        /// Unsupported algorithm.
        algorithm: SignatureAlgorithm,
    },
    /// Public key bytes could not be parsed.
    #[error("invalid orderbook public key: {reason}")]
    InvalidPublicKey {
        /// Verification backend reason.
        reason: String,
    },
    /// Canonical signable payload encoding failed.
    #[error("failed to encode orderbook signature payload: {reason}")]
    SignaturePayloadEncoding {
        /// Norito encoding failure reason.
        reason: String,
    },
    /// Signature verification failed.
    #[error("orderbook signature verification failed: {reason}")]
    SignatureVerification {
        /// Verification backend reason.
        reason: String,
    },
    /// Maker and taker order ids are identical.
    #[error("maker and taker order ids must differ")]
    SelfTrade,
    /// Order expired before the match timestamp.
    #[error("order expired at {expiry_unix}, match timestamp {now_unix}")]
    ExpiredOrder {
        /// Expired order identifier.
        order_id: [u8; 32],
        /// Order expiry timestamp.
        expiry_unix: u64,
        /// Match timestamp.
        now_unix: u64,
    },
    /// Maker and taker use the same side.
    #[error("maker and taker must have opposite sides, both were {side:?}")]
    SameOrderSide {
        /// Duplicate side.
        side: OrderSideV1,
    },
    /// Maker and taker tiers differ.
    #[error("maker tier {maker_tier:?} does not match taker tier {taker_tier:?}")]
    TierMismatch {
        /// Maker tier.
        maker_tier: OrderTierV1,
        /// Taker tier.
        taker_tier: OrderTierV1,
    },
    /// Bid price does not cross ask price.
    #[error("bid price {bid_price_micro} does not cross ask price {ask_price_micro}")]
    PriceDoesNotCross {
        /// Bid price in micro-XOR per GiB.
        bid_price_micro: u128,
        /// Ask price in micro-XOR per GiB.
        ask_price_micro: u128,
    },
    /// Duplicate order id in a book snapshot.
    #[error("duplicate order id {order_id:02x?}")]
    DuplicateOrderId {
        /// Repeated order identifier.
        order_id: [u8; 32],
    },
    /// Duplicate admission sequence in a book snapshot.
    #[error("duplicate order sequence {sequence}")]
    DuplicateOrderSequence {
        /// Repeated sequence.
        sequence: u64,
    },
    /// GiB-to-byte expansion overflowed.
    #[error("byte count overflow")]
    ByteCountOverflow,
    /// Settlement channel is not open for receipts.
    #[error("settlement channel status {status:?} cannot accept receipts")]
    SettlementChannelNotOpen {
        /// Current channel status.
        status: SettlementChannelStatusV1,
    },
    /// Receipt does not bind to the channel.
    #[error("settlement receipt does not match channel id or trade id")]
    SettlementChannelMismatch,
    /// Receipt range exceeds channel byte coverage.
    #[error("receipt range end {range_end} exceeds channel total bytes {total_bytes}")]
    ReceiptExceedsChannelBytes {
        /// Receipt range end.
        range_end: u64,
        /// Channel total byte coverage.
        total_bytes: u64,
    },
    /// Receipt delivered bytes exceed remaining channel bytes.
    #[error("receipt delivered {delivered} exceeds remaining channel bytes {remaining_bytes}")]
    ReceiptExceedsRemainingBytes {
        /// Delivered bytes.
        delivered: u64,
        /// Remaining bytes.
        remaining_bytes: u64,
    },
    /// Receipt debit exceeds remaining channel escrow.
    #[error("receipt debit {debited} exceeds channel escrow {escrow}")]
    ReceiptExceedsEscrow {
        /// Debited micro-XOR.
        debited: u128,
        /// Remaining escrow micro-XOR.
        escrow: u128,
    },
    /// Byte count is zero.
    #[error("byte count must be positive")]
    ZeroBytes,
    /// Remaining bytes exceed total bytes.
    #[error("remaining bytes {remaining_bytes} exceed total bytes {total_bytes}")]
    InvalidRemainingBytes {
        /// Remaining bytes.
        remaining_bytes: u64,
        /// Total bytes.
        total_bytes: u64,
    },
    /// Escrow is zero.
    #[error("escrow amount must be positive")]
    ZeroEscrow,
    /// Byte range is empty or inverted.
    #[error("invalid byte range {start}..{end}")]
    InvalidByteRange {
        /// Start offset.
        start: u64,
        /// End offset.
        end: u64,
    },
    /// Delivered bytes do not match the receipt range.
    #[error("delivered bytes {bytes_delivered} do not match range length {range_len}")]
    InvalidDeliveredBytes {
        /// Delivered bytes.
        bytes_delivered: u64,
        /// Range length.
        range_len: u64,
    },
    /// Buyer debit is zero.
    #[error("debit amount must be positive")]
    ZeroDebit,
    /// Settlement credits do not balance against the debit.
    #[error("settlement imbalance: debit {debited}, credited plus fees {credited_plus_fees}")]
    SettlementImbalance {
        /// Debited micro-XOR.
        debited: u128,
        /// Credited plus fee micro-XOR.
        credited_plus_fees: u128,
    },
    /// Amount arithmetic failed.
    #[error(transparent)]
    Amount(DealAmountError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn id(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn account(seed: u8) -> Vec<u8> {
        vec![seed; 33]
    }

    fn signature() -> OrderbookSignatureV1 {
        OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: vec![7; PUBLIC_KEY_LENGTH],
            signature: vec![9; SIGNATURE_LENGTH],
        }
    }

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn sign_order(mut order: OrderRequestV1, seed: u8) -> OrderRequestV1 {
        let key = signing_key(seed);
        order.signature = OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: key.verifying_key().to_bytes().to_vec(),
            signature: Vec::new(),
        };
        let digest = order_request_signature_digest_v1(&order).expect("order digest");
        order.signature.signature = key.sign(&digest).to_bytes().to_vec();
        order
    }

    fn cancel() -> OrderCancelV1 {
        OrderCancelV1 {
            version: ORDERBOOK_CANCEL_VERSION_V1,
            order_id: id(1),
            owner_account: account(3),
            reason: OrderCancelReasonV1::OwnerRequested,
            nonce: 2,
            signature: signature(),
        }
    }

    fn sign_cancel(mut cancel: OrderCancelV1, seed: u8) -> OrderCancelV1 {
        let key = signing_key(seed);
        cancel.signature = OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: key.verifying_key().to_bytes().to_vec(),
            signature: Vec::new(),
        };
        let digest = order_cancel_signature_digest_v1(&cancel).expect("cancel digest");
        cancel.signature.signature = key.sign(&digest).to_bytes().to_vec();
        cancel
    }

    fn receipt() -> SettlementReceiptV1 {
        SettlementReceiptV1 {
            version: SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id: id(7),
            channel_id: id(5),
            trade_id: id(4),
            range: ByteRangeV1 { start: 10, end: 42 },
            chunk_hash: id(8),
            bytes_delivered: 32,
            xor_debited: XorAmount::from_micro(100),
            provider_credit: XorAmount::from_micro(90),
            fee_amount: XorAmount::from_micro(10),
            issued_at_unix: 1_800_000_200,
            settlement_signature: signature(),
        }
    }

    fn sign_receipt(mut receipt: SettlementReceiptV1, seed: u8) -> SettlementReceiptV1 {
        let key = signing_key(seed);
        receipt.settlement_signature = OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: key.verifying_key().to_bytes().to_vec(),
            signature: Vec::new(),
        };
        let digest =
            settlement_receipt_signature_digest_v1(&receipt).expect("settlement receipt digest");
        receipt.settlement_signature.signature = key.sign(&digest).to_bytes().to_vec();
        receipt
    }

    fn order() -> OrderRequestV1 {
        OrderRequestV1 {
            version: ORDERBOOK_ORDER_VERSION_V1,
            order_id: id(1),
            side: OrderSideV1::Bid,
            tier: OrderTierV1::Hot,
            price_per_gib: XorAmount::from_micro(1_500_000),
            quantity_gib: 10,
            remaining_gib: 10,
            owner_account: account(3),
            expiry_unix: 1_800_000_000,
            nonce: 1,
            maker_fee_bps: 5,
            taker_fee_bps: 10,
            signature: signature(),
        }
    }

    fn book_order(
        seed: u8,
        side: OrderSideV1,
        price_per_gib_micro: u128,
        quantity_gib: u64,
    ) -> OrderRequestV1 {
        let mut order = order();
        order.order_id = id(seed);
        order.side = side;
        order.price_per_gib = XorAmount::from_micro(price_per_gib_micro);
        order.quantity_gib = quantity_gib;
        order.remaining_gib = quantity_gib;
        order.owner_account = account(seed);
        order.nonce = u64::from(seed);
        order
    }

    fn book_entry(order: OrderRequestV1, sequence: u64) -> OrderBookEntryV1 {
        OrderBookEntryV1 { order, sequence }
    }

    #[test]
    fn order_accepts_valid_payload() {
        assert_eq!(order().validate(), Ok(()));
    }

    #[test]
    fn order_rejects_invalid_remaining_quantity() {
        let mut order = order();
        order.remaining_gib = 11;
        assert_eq!(
            order.validate(),
            Err(OrderbookValidationError::InvalidRemainingQuantity {
                remaining_gib: 11,
                quantity_gib: 10,
            })
        );
    }

    #[test]
    fn order_rejects_bad_ed25519_signature_lengths() {
        let mut order = order();
        order.signature.public_key.pop();
        assert_eq!(
            order.validate(),
            Err(OrderbookValidationError::InvalidPublicKeyLength {
                length: PUBLIC_KEY_LENGTH - 1,
            })
        );
    }

    #[test]
    fn cancel_accepts_valid_payload() {
        assert_eq!(cancel().validate(), Ok(()));
    }

    #[test]
    fn signature_digests_ignore_signature_bytes_but_bind_public_key() {
        let signed = sign_order(order(), 0x11);
        let mut replaced_signature = signed.clone();
        replaced_signature.signature.signature = vec![0xAA; SIGNATURE_LENGTH];
        assert_eq!(
            order_request_signature_digest_v1(&signed),
            order_request_signature_digest_v1(&replaced_signature)
        );

        let mut replaced_key = signed.clone();
        replaced_key.signature.public_key = signing_key(0x12).verifying_key().to_bytes().to_vec();
        assert_ne!(
            order_request_signature_digest_v1(&signed),
            order_request_signature_digest_v1(&replaced_key)
        );

        let cancel = sign_cancel(cancel(), 0x13);
        assert!(order_cancel_signature_digest_v1(&cancel).is_ok());

        let receipt = sign_receipt(receipt(), 0x14);
        assert!(settlement_receipt_signature_digest_v1(&receipt).is_ok());
    }

    #[test]
    fn verify_order_signature_accepts_valid_payload_and_rejects_tamper() {
        let signed = sign_order(order(), 0x15);
        assert_eq!(verify_order_request_signature_v1(&signed), Ok(()));

        let mut tampered = signed;
        tampered.price_per_gib = XorAmount::from_micro(9_999_999);
        assert!(matches!(
            verify_order_request_signature_v1(&tampered),
            Err(OrderbookValidationError::SignatureVerification { .. })
        ));
    }

    #[test]
    fn verify_cancel_signature_accepts_valid_payload() {
        let signed = sign_cancel(cancel(), 0x16);
        assert_eq!(verify_order_cancel_signature_v1(&signed), Ok(()));
    }

    #[test]
    fn verify_settlement_receipt_signature_accepts_valid_payload() {
        let signed = sign_receipt(receipt(), 0x17);
        assert_eq!(verify_settlement_receipt_signature_v1(&signed), Ok(()));
    }

    #[test]
    fn trade_rejects_self_trade() {
        let trade = TradeEventV1 {
            version: ORDERBOOK_TRADE_EVENT_VERSION_V1,
            trade_id: id(4),
            maker_order_id: id(1),
            taker_order_id: id(1),
            tier: OrderTierV1::Warm,
            price_per_gib: XorAmount::from_micro(2_000_000),
            filled_gib: 5,
            maker_fee: XorAmount::from_micro(1_000),
            taker_fee: XorAmount::from_micro(2_000),
            timestamp_unix: 1_800_000_100,
        };
        assert_eq!(trade.validate(), Err(OrderbookValidationError::SelfTrade));
    }

    #[test]
    fn match_orders_creates_trade_and_remaining_quantities() {
        let mut maker = order();
        maker.side = OrderSideV1::Ask;
        maker.order_id = id(11);
        maker.owner_account = account(11);
        maker.price_per_gib = XorAmount::from_micro(1_500_000);
        maker.remaining_gib = 10;
        maker.maker_fee_bps = 5;
        let mut taker = order();
        taker.side = OrderSideV1::Bid;
        taker.order_id = id(12);
        taker.owner_account = account(12);
        taker.price_per_gib = XorAmount::from_micro(1_600_000);
        taker.quantity_gib = 4;
        taker.remaining_gib = 4;
        taker.taker_fee_bps = 10;

        let outcome =
            match_orders_v1(&maker, &taker, id(13), 1_700_000_000).expect("orders should match");

        assert_eq!(outcome.trade.maker_order_id, maker.order_id);
        assert_eq!(outcome.trade.taker_order_id, taker.order_id);
        assert_eq!(outcome.trade.price_per_gib, maker.price_per_gib);
        assert_eq!(outcome.trade.filled_gib, 4);
        assert_eq!(outcome.gross_value, XorAmount::from_micro(6_000_000));
        assert_eq!(outcome.trade.maker_fee, XorAmount::from_micro(3_000));
        assert_eq!(outcome.trade.taker_fee, XorAmount::from_micro(6_000));
        assert_eq!(outcome.maker_remaining_gib, 6);
        assert_eq!(outcome.taker_remaining_gib, 0);
        assert_eq!(outcome.trade.validate(), Ok(()));
    }

    #[test]
    fn match_orders_rejects_non_crossing_prices() {
        let mut maker = order();
        maker.side = OrderSideV1::Ask;
        maker.order_id = id(11);
        maker.owner_account = account(11);
        maker.price_per_gib = XorAmount::from_micro(1_500_000);
        let mut taker = order();
        taker.side = OrderSideV1::Bid;
        taker.order_id = id(12);
        taker.owner_account = account(12);
        taker.price_per_gib = XorAmount::from_micro(1_400_000);

        assert_eq!(
            match_orders_v1(&maker, &taker, id(13), 1_700_000_000),
            Err(OrderbookValidationError::PriceDoesNotCross {
                bid_price_micro: 1_400_000,
                ask_price_micro: 1_500_000,
            })
        );
    }

    #[test]
    fn match_order_book_uses_price_time_priority_and_partial_fills() {
        let bid = book_order(21, OrderSideV1::Bid, 2_000_000, 10);
        let low_ask = book_order(22, OrderSideV1::Ask, 900_000, 4);
        let high_ask = book_order(23, OrderSideV1::Ask, 1_000_000, 3);
        let outcome = match_order_book_v1(
            &[
                book_entry(high_ask, 1),
                book_entry(bid, 4),
                book_entry(low_ask, 3),
            ],
            1_700_000_000,
        )
        .expect("book should match");

        assert_eq!(outcome.fills.len(), 2);
        assert_eq!(outcome.fills[0].trade.maker_order_id, id(22));
        assert_eq!(outcome.fills[0].trade.taker_order_id, id(21));
        assert_eq!(
            outcome.fills[0].trade.price_per_gib,
            XorAmount::from_micro(900_000)
        );
        assert_eq!(outcome.fills[0].trade.filled_gib, 4);
        assert_eq!(outcome.fills[0].maker_remaining_gib, 0);
        assert_eq!(outcome.fills[0].taker_remaining_gib, 6);
        assert_eq!(outcome.fills[1].trade.maker_order_id, id(23));
        assert_eq!(outcome.fills[1].trade.taker_order_id, id(21));
        assert_eq!(
            outcome.fills[1].trade.price_per_gib,
            XorAmount::from_micro(1_000_000)
        );
        assert_eq!(outcome.fills[1].trade.filled_gib, 3);
        assert_eq!(outcome.fills[1].maker_remaining_gib, 0);
        assert_eq!(outcome.fills[1].taker_remaining_gib, 3);
        assert_ne!(
            outcome.fills[0].trade.trade_id,
            outcome.fills[1].trade.trade_id
        );
        assert!(
            outcome.fills[0]
                .trade
                .trade_id
                .iter()
                .any(|byte| *byte != 0)
        );
        assert_eq!(outcome.expired_order_ids, Vec::<[u8; 32]>::new());
        assert_eq!(outcome.remaining_orders.len(), 1);
        assert_eq!(outcome.remaining_orders[0].order_id, id(21));
        assert_eq!(outcome.remaining_orders[0].remaining_gib, 3);
    }

    #[test]
    fn match_order_book_skips_expired_orders() {
        let mut expired_ask = book_order(31, OrderSideV1::Ask, 1_000_000, 5);
        expired_ask.expiry_unix = 1_699_999_999;
        let live_bid = book_order(32, OrderSideV1::Bid, 2_000_000, 5);

        let outcome = match_order_book_v1(
            &[book_entry(live_bid, 2), book_entry(expired_ask, 1)],
            1_700_000_000,
        )
        .expect("expired orders should be skipped");

        assert!(outcome.fills.is_empty());
        assert_eq!(outcome.expired_order_ids, vec![id(31)]);
        assert_eq!(outcome.remaining_orders.len(), 1);
        assert_eq!(outcome.remaining_orders[0].order_id, id(32));
        assert_eq!(outcome.remaining_orders[0].remaining_gib, 5);
    }

    #[test]
    fn match_order_book_rejects_duplicate_order_ids() {
        let bid = book_order(41, OrderSideV1::Bid, 2_000_000, 5);
        let mut ask = book_order(42, OrderSideV1::Ask, 1_000_000, 5);
        ask.order_id = bid.order_id;

        assert_eq!(
            match_order_book_v1(&[book_entry(bid, 1), book_entry(ask, 2)], 1_700_000_000),
            Err(OrderbookValidationError::DuplicateOrderId { order_id: id(41) })
        );
    }

    #[test]
    fn match_order_book_rejects_duplicate_sequences() {
        let bid = book_order(51, OrderSideV1::Bid, 2_000_000, 5);
        let ask = book_order(52, OrderSideV1::Ask, 1_000_000, 5);

        assert_eq!(
            match_order_book_v1(&[book_entry(bid, 7), book_entry(ask, 7)], 1_700_000_000),
            Err(OrderbookValidationError::DuplicateOrderSequence { sequence: 7 })
        );
    }

    #[test]
    fn open_settlement_channel_for_trade_locks_trade_value_and_fees() {
        let mut maker = order();
        maker.side = OrderSideV1::Ask;
        maker.order_id = id(11);
        maker.owner_account = account(11);
        maker.price_per_gib = XorAmount::from_micro(1_500_000);
        maker.remaining_gib = 4;
        maker.maker_fee_bps = 5;
        let mut taker = order();
        taker.side = OrderSideV1::Bid;
        taker.order_id = id(12);
        taker.owner_account = account(12);
        taker.price_per_gib = XorAmount::from_micro(1_600_000);
        taker.remaining_gib = 4;
        taker.taker_fee_bps = 10;
        let trade = match_orders_v1(&maker, &taker, id(13), 1_700_000_000)
            .expect("orders should match")
            .trade;

        let channel = open_settlement_channel_for_trade_v1(
            &trade,
            id(14),
            b"buyer@sora".to_vec(),
            id(15),
            1_700_000_001,
        )
        .expect("channel should open");

        assert_eq!(channel.trade_id, trade.trade_id);
        assert_eq!(channel.total_bytes, 4 * BYTES_PER_GIB);
        assert_eq!(channel.remaining_bytes, channel.total_bytes);
        assert_eq!(channel.xor_locked, XorAmount::from_micro(6_009_000));
        assert_eq!(channel.status, SettlementChannelStatusV1::Open);
        assert_eq!(channel.validate(), Ok(()));
    }

    #[test]
    fn channel_rejects_remaining_bytes_over_total() {
        let channel = SettlementChannelV1 {
            version: SETTLEMENT_CHANNEL_VERSION_V1,
            channel_id: id(5),
            trade_id: id(4),
            buyer_account: account(8),
            provider_id: id(6),
            total_bytes: 1_024,
            remaining_bytes: 1_025,
            xor_locked: XorAmount::from_micro(3_000_000),
            status: SettlementChannelStatusV1::Open,
            opened_at_unix: 1_800_000_100,
            updated_at_unix: 1_800_000_100,
        };
        assert_eq!(
            channel.validate(),
            Err(OrderbookValidationError::InvalidRemainingBytes {
                remaining_bytes: 1_025,
                total_bytes: 1_024,
            })
        );
    }

    #[test]
    fn settlement_receipt_accepts_balanced_receipt() {
        assert_eq!(receipt().validate(), Ok(()));
    }

    #[test]
    fn settlement_receipt_rejects_imbalanced_receipt() {
        let receipt = SettlementReceiptV1 {
            version: SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id: id(7),
            channel_id: id(5),
            trade_id: id(4),
            range: ByteRangeV1 { start: 10, end: 42 },
            chunk_hash: id(8),
            bytes_delivered: 32,
            xor_debited: XorAmount::from_micro(100),
            provider_credit: XorAmount::from_micro(91),
            fee_amount: XorAmount::from_micro(10),
            issued_at_unix: 1_800_000_200,
            settlement_signature: signature(),
        };
        assert_eq!(
            receipt.validate(),
            Err(OrderbookValidationError::SettlementImbalance {
                debited: 100,
                credited_plus_fees: 101,
            })
        );
    }

    #[test]
    fn apply_settlement_receipt_closes_channel_when_fully_delivered() {
        let channel = SettlementChannelV1 {
            version: SETTLEMENT_CHANNEL_VERSION_V1,
            channel_id: id(5),
            trade_id: id(4),
            buyer_account: account(8),
            provider_id: id(6),
            total_bytes: 32,
            remaining_bytes: 32,
            xor_locked: XorAmount::from_micro(100),
            status: SettlementChannelStatusV1::Open,
            opened_at_unix: 1_800_000_100,
            updated_at_unix: 1_800_000_100,
        };
        let receipt = SettlementReceiptV1 {
            version: SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id: id(7),
            channel_id: id(5),
            trade_id: id(4),
            range: ByteRangeV1 { start: 0, end: 32 },
            chunk_hash: id(8),
            bytes_delivered: 32,
            xor_debited: XorAmount::from_micro(100),
            provider_credit: XorAmount::from_micro(90),
            fee_amount: XorAmount::from_micro(10),
            issued_at_unix: 1_800_000_200,
            settlement_signature: signature(),
        };

        let updated =
            apply_settlement_receipt_v1(&channel, &receipt).expect("receipt should apply");

        assert_eq!(updated.remaining_bytes, 0);
        assert_eq!(updated.xor_locked, XorAmount::zero());
        assert_eq!(updated.status, SettlementChannelStatusV1::Closed);
        assert_eq!(updated.updated_at_unix, receipt.issued_at_unix);
        assert_eq!(updated.validate(), Ok(()));
    }

    #[test]
    fn apply_settlement_receipt_rejects_channel_mismatch() {
        let channel = SettlementChannelV1 {
            version: SETTLEMENT_CHANNEL_VERSION_V1,
            channel_id: id(5),
            trade_id: id(4),
            buyer_account: account(8),
            provider_id: id(6),
            total_bytes: 32,
            remaining_bytes: 32,
            xor_locked: XorAmount::from_micro(100),
            status: SettlementChannelStatusV1::Open,
            opened_at_unix: 1_800_000_100,
            updated_at_unix: 1_800_000_100,
        };
        let receipt = SettlementReceiptV1 {
            version: SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id: id(7),
            channel_id: id(55),
            trade_id: id(4),
            range: ByteRangeV1 { start: 0, end: 32 },
            chunk_hash: id(8),
            bytes_delivered: 32,
            xor_debited: XorAmount::from_micro(100),
            provider_credit: XorAmount::from_micro(90),
            fee_amount: XorAmount::from_micro(10),
            issued_at_unix: 1_800_000_200,
            settlement_signature: signature(),
        };

        assert_eq!(
            apply_settlement_receipt_v1(&channel, &receipt),
            Err(OrderbookValidationError::SettlementChannelMismatch)
        );
    }
}
