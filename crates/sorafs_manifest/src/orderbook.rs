#![allow(unexpected_cfgs)]
//! Orderbook and streaming-settlement payload schemas for SoraFS (SFM-2).
//!
//! These Norito payloads provide the deterministic data-model foundation for
//! the SoraFS XOR orderbook. The pure helpers in this module cover deterministic
//! pair and full-book matching, fee calculation, settlement-channel opening,
//! receipt application, and payload signature verification. Authoritative
//! sequencing, lifecycle state, and escrow mutation are committed ledger state.
use std::collections::BTreeSet;
use blake3::Hasher;
use ed25519_dalek::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, Signer, SigningKey};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;
use crate::{
    deal::{BASIS_POINTS_PER_UNIT, DealAmountError, XorQuantity},
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
/// Domain separator for canonical V1 order identifiers.
pub const ORDERBOOK_ORDER_ID_DOMAIN_V1: &[u8] = b"sorafs.orderbook.order-id.v1";
/// Maximum canonical owner-account byte length accepted by V1 orderbook payloads.
///
/// This protocol ceiling bounds the durable owner-nonce high-water key space and
/// must be enforced before hashing or signing owner-controlled input.
pub const ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1: usize = 256;
/// Maximum exact canonical size of a single V1 orderbook payload.
///
/// Orders, cancellations, trades, channels, and receipts are deliberately
/// small. Keeping a common ceiling prevents forged Norito length prefixes from
/// turning validator or HTTP ingress into an unbounded allocation surface.
pub const ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1: usize = 64 * 1024;
const ORDERBOOK_DECODE_MAX_DEPTH_V1: usize = 64;
/// Production resource limits for decoding one canonical V1 orderbook payload.
///
/// Callers may intersect this budget with a tighter request-scoped budget via
/// the `*_with_limits` decoders. No caller-provided budget can loosen these
/// protocol ceilings.
pub const ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1: norito::DecodeLimits = norito::DecodeLimits::new(
    512,
    ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1,
    ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1 * 2,
    ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1 * 4,
    ORDERBOOK_DECODE_MAX_DEPTH_V1,
);
const ORDERBOOK_TRADE_ID_DOMAIN_V1: &[u8] = b"sorafs.orderbook.trade-id.v1";
/// Domain separator for settlement-channel identifiers derived from trades.
pub const ORDERBOOK_SETTLEMENT_CHANNEL_ID_DOMAIN_V1: &[u8] =
    b"sorafs.orderbook.settlement-channel-id.v1";
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
        if self.algorithm != SignatureAlgorithm::Ed25519 {
            return Err(OrderbookValidationError::UnsupportedSignatureAlgorithm {
                algorithm: self.algorithm,
            });
        }
        if self.public_key.is_empty()
            || crate::inert_bytes(&self.public_key)
            || self.signature.is_empty()
            || crate::inert_bytes(&self.signature)
        {
            return Err(OrderbookValidationError::InvalidSignature);
        }
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
    pub price_per_gib: XorQuantity,
    /// Total GiB requested/offered.
    pub quantity_gib: u64,
    /// Remaining GiB after partial fills.
    pub remaining_gib: u64,
    /// Canonical owner account bytes.
    pub owner_account: Vec<u8>,
    /// Exact provider registry identity sold by an ask; absent for bids.
    pub provider_id: Option<[u8; 32]>,
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
        validate_owner_account_v1(&self.owner_account)?;
        validate_digest(self.order_id, OrderbookValidationError::InvalidOrderId)?;
        if self.nonce == 0 {
            return Err(OrderbookValidationError::ZeroNonce);
        }
        let expected_order_id = derive_orderbook_order_id_v1(&self.owner_account, self.nonce);
        if self.order_id != expected_order_id {
            return Err(OrderbookValidationError::OrderIdDerivationMismatch {
                order_id: self.order_id,
                expected_order_id,
            });
        }
        match (self.side, self.provider_id) {
            (OrderSideV1::Bid, None) => {}
            (OrderSideV1::Bid, Some(_)) => {
                return Err(OrderbookValidationError::BidProviderBindingForbidden);
            }
            (OrderSideV1::Ask, None) => {
                return Err(OrderbookValidationError::AskProviderBindingRequired);
            }
            (OrderSideV1::Ask, Some(provider_id)) => {
                validate_digest(provider_id, OrderbookValidationError::InvalidProviderId)?;
            }
        }
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
        if self.expiry_unix == 0 {
            return Err(OrderbookValidationError::InvalidTimestamp);
        }
        validate_fee_bps(self.maker_fee_bps)?;
        validate_fee_bps(self.taker_fee_bps)?;
        self.signature.validate()
    }
}
/// Derive the canonical identifier for an orderbook order.
///
/// V1 hashes the SoraFS order-id domain separator, the nonce in little-endian
/// form, and the canonical owner-account bytes. The V1 payload has no chain-id
/// or market-domain field, so this identifier is intentionally not chain
/// bound; authenticated request and ledger admission layers must provide that
/// domain separation. Binding the account rather than its current controller
/// key preserves the identifier across account-key rotation.
#[must_use]
pub fn derive_orderbook_order_id_v1(owner_account: &[u8], nonce: u64) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(ORDERBOOK_ORDER_ID_DOMAIN_V1);
    hasher.update(&nonce.to_le_bytes());
    hasher.update(owner_account);
    *hasher.finalize().as_bytes()
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
        validate_owner_account_v1(&self.owner_account)?;
        validate_digest(self.order_id, OrderbookValidationError::InvalidOrderId)?;
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
    validate_owner_account_v1(&order.owner_account)?;
    let signable = OrderRequestSigningViewV1::from_order(order);
    preflight_orderbook_payload_len(&signable, ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1)?;
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
/// Sign an order submission with the canonical SFM-2 Ed25519 digest.
///
/// The helper replaces the payload's signature material with the signing key's
/// public key and the signature over [`order_request_signature_digest_v1`], then
/// verifies the resulting payload before returning it.
pub fn sign_order_request_ed25519_v1(
    mut order: OrderRequestV1,
    signing_key: &SigningKey,
) -> Result<OrderRequestV1, OrderbookValidationError> {
    order.signature = empty_ed25519_orderbook_signature(signing_key);
    let digest = order_request_signature_digest_v1(&order)?;
    order.signature.signature = signing_key.sign(&digest).to_bytes().to_vec();
    verify_order_request_signature_v1(&order)?;
    Ok(order)
}
/// Derive the canonical Ed25519 message digest for an order cancellation.
///
/// The digest is BLAKE3 over a domain separator plus the canonical Norito cancel
/// bytes with only `signature.signature` cleared.
pub fn order_cancel_signature_digest_v1(
    cancel: &OrderCancelV1,
) -> Result<[u8; 32], OrderbookValidationError> {
    validate_owner_account_v1(&cancel.owner_account)?;
    let signable = OrderCancelSigningViewV1::from_cancel(cancel);
    preflight_orderbook_payload_len(&signable, ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1)?;
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
/// Sign an order cancellation with the canonical SFM-2 Ed25519 digest.
///
/// The helper replaces the payload's signature material with the signing key's
/// public key and the signature over [`order_cancel_signature_digest_v1`], then
/// verifies the resulting payload before returning it.
pub fn sign_order_cancel_ed25519_v1(
    mut cancel: OrderCancelV1,
    signing_key: &SigningKey,
) -> Result<OrderCancelV1, OrderbookValidationError> {
    cancel.signature = empty_ed25519_orderbook_signature(signing_key);
    let digest = order_cancel_signature_digest_v1(&cancel)?;
    cancel.signature.signature = signing_key.sign(&digest).to_bytes().to_vec();
    verify_order_cancel_signature_v1(&cancel)?;
    Ok(cancel)
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
    pub price_per_gib: XorQuantity,
    /// Filled GiB.
    pub filled_gib: u64,
    /// Maker fee charged for the fill.
    pub maker_fee: XorQuantity,
    /// Taker fee charged for the fill.
    pub taker_fee: XorQuantity,
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
    pub gross_value: XorQuantity,
}
/// Order plus canonical admission sequence used for price-time priority.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
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
/// Decode an exact canonical V1 order request under production resource limits.
pub fn decode_order_request_v1(
    bytes: &[u8],
) -> Result<OrderRequestV1, OrderbookPayloadDecodeError> {
    decode_order_request_v1_with_limits(bytes, ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1)
}
/// Decode an exact canonical V1 order request under caller-composed limits.
pub fn decode_order_request_v1_with_limits(
    bytes: &[u8],
    limits: norito::DecodeLimits,
) -> Result<OrderRequestV1, OrderbookPayloadDecodeError> {
    decode_orderbook_payload_v1(bytes, limits)
}
/// Decode an exact canonical V1 order cancellation under production resource limits.
pub fn decode_order_cancel_v1(bytes: &[u8]) -> Result<OrderCancelV1, OrderbookPayloadDecodeError> {
    decode_order_cancel_v1_with_limits(bytes, ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1)
}
/// Decode an exact canonical V1 order cancellation under caller-composed limits.
pub fn decode_order_cancel_v1_with_limits(
    bytes: &[u8],
    limits: norito::DecodeLimits,
) -> Result<OrderCancelV1, OrderbookPayloadDecodeError> {
    decode_orderbook_payload_v1(bytes, limits)
}
/// Decode an exact canonical V1 trade event under production resource limits.
pub fn decode_trade_event_v1(bytes: &[u8]) -> Result<TradeEventV1, OrderbookPayloadDecodeError> {
    decode_trade_event_v1_with_limits(bytes, ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1)
}
/// Decode an exact canonical V1 trade event under caller-composed limits.
pub fn decode_trade_event_v1_with_limits(
    bytes: &[u8],
    limits: norito::DecodeLimits,
) -> Result<TradeEventV1, OrderbookPayloadDecodeError> {
    decode_orderbook_payload_v1(bytes, limits)
}
/// Decode an exact canonical V1 settlement channel under production resource limits.
pub fn decode_settlement_channel_v1(
    bytes: &[u8],
) -> Result<SettlementChannelV1, OrderbookPayloadDecodeError> {
    decode_settlement_channel_v1_with_limits(bytes, ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1)
}
/// Decode an exact canonical V1 settlement channel under caller-composed limits.
pub fn decode_settlement_channel_v1_with_limits(
    bytes: &[u8],
    limits: norito::DecodeLimits,
) -> Result<SettlementChannelV1, OrderbookPayloadDecodeError> {
    decode_orderbook_payload_v1(bytes, limits)
}
/// Decode an exact canonical V1 settlement receipt under production resource limits.
pub fn decode_settlement_receipt_v1(
    bytes: &[u8],
) -> Result<SettlementReceiptV1, OrderbookPayloadDecodeError> {
    decode_settlement_receipt_v1_with_limits(bytes, ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1)
}
/// Decode an exact canonical V1 settlement receipt under caller-composed limits.
pub fn decode_settlement_receipt_v1_with_limits(
    bytes: &[u8],
    limits: norito::DecodeLimits,
) -> Result<SettlementReceiptV1, OrderbookPayloadDecodeError> {
    decode_orderbook_payload_v1(bytes, limits)
}
fn decode_orderbook_payload_v1<T>(
    bytes: &[u8],
    limits: norito::DecodeLimits,
) -> Result<T, OrderbookPayloadDecodeError>
where
    for<'de> T: norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    if bytes.len() > ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1 {
        return Err(OrderbookPayloadDecodeError::PayloadTooLarge {
            length: bytes.len(),
            maximum: ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1,
        });
    }
    let limits = intersect_orderbook_decode_limits(limits);
    norito::decode_canonical_with_limits(bytes, limits).map_err(|error| {
        if error.is_decode_resource_limit() {
            OrderbookPayloadDecodeError::DecodeResourceLimit
        } else if matches!(error, norito::Error::NonCanonicalEncoding) {
            OrderbookPayloadDecodeError::NonCanonicalEncoding
        } else {
            OrderbookPayloadDecodeError::Decode {
                reason: error.to_string(),
            }
        }
    })
}
fn intersect_orderbook_decode_limits(limits: norito::DecodeLimits) -> norito::DecodeLimits {
    norito::DecodeLimits::new(
        limits
            .max_sequence_elements()
            .min(ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1.max_sequence_elements()),
        limits
            .max_field_bytes()
            .min(ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1.max_field_bytes()),
        limits
            .max_total_elements()
            .min(ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1.max_total_elements()),
        limits
            .max_total_allocated_bytes()
            .min(ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1.max_total_allocated_bytes()),
        limits
            .max_nesting_depth()
            .min(ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1.max_nesting_depth()),
    )
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
    if bid.price_per_gib < ask.price_per_gib {
        return Err(OrderbookValidationError::PriceDoesNotCross {
            bid_price: bid.price_per_gib.clone(),
            ask_price: ask.price_per_gib.clone(),
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
        price_per_gib: maker.price_per_gib.clone(),
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
/// expired orders into sequence-ordered `expired_order_ids`, then matches
/// independently by tier: bids sort by highest price, asks by lowest price, and
/// both sides use lower sequence as the time-priority tiebreaker.
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
            expired_order_ids.push((entry.sequence, entry.order.order_id));
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
            if tier_bids[0].order.price_per_gib < tier_asks[0].order.price_per_gib {
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
    expired_order_ids.sort_by(|(lhs_sequence, lhs_id), (rhs_sequence, rhs_id)| {
        lhs_sequence
            .cmp(rhs_sequence)
            .then_with(|| lhs_id.cmp(rhs_id))
    });
    Ok(OrderBookMatchOutcomeV1 {
        fills,
        remaining_orders: remaining.into_iter().map(|entry| entry.order).collect(),
        expired_order_ids: expired_order_ids
            .into_iter()
            .map(|(_, order_id)| order_id)
            .collect(),
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
/// Derive the canonical settlement-channel identifier for a trade.
///
/// A valid trade has exactly one first-release channel. Keeping this derivation
/// in the shared payload crate ensures local mirrors, ledger execution, SDKs,
/// and reconciliation workers cannot choose different channel identifiers for
/// the same fill.
pub fn derive_orderbook_settlement_channel_id_v1(
    trade: &TradeEventV1,
) -> Result<[u8; 32], OrderbookValidationError> {
    trade.validate()?;
    let mut hasher = Hasher::new();
    hasher.update(ORDERBOOK_SETTLEMENT_CHANNEL_ID_DOMAIN_V1);
    hasher.update(&trade.trade_id);
    let mut channel_id = *hasher.finalize().as_bytes();
    if channel_id.iter().all(|byte| *byte == 0) {
        channel_id[31] = 1;
    }
    Ok(channel_id)
}
fn sort_bids_by_price_time(entries: &mut [WorkingOrderV1]) {
    entries.sort_by(|lhs, rhs| {
        rhs.order
            .price_per_gib
            .cmp(&lhs.order.price_per_gib)
            .then_with(|| lhs.sequence.cmp(&rhs.sequence))
            .then_with(|| lhs.order.order_id.cmp(&rhs.order.order_id))
    });
}
fn sort_asks_by_price_time(entries: &mut [WorkingOrderV1]) {
    entries.sort_by(|lhs, rhs| {
        lhs.order
            .price_per_gib
            .cmp(&rhs.order.price_per_gib)
            .then_with(|| lhs.sequence.cmp(&rhs.sequence))
            .then_with(|| lhs.order.order_id.cmp(&rhs.order.order_id))
    });
}
/// Return the gross value represented by a trade event.
pub fn trade_gross_value_v1(trade: &TradeEventV1) -> Result<XorQuantity, OrderbookValidationError> {
    trade.validate()?;
    trade
        .price_per_gib
        .checked_mul_u64(trade.filled_gib)
        .map_err(OrderbookValidationError::Amount)
}
/// Return the escrow amount needed to cover a trade value and both fee fields.
pub fn trade_escrow_requirement_v1(
    trade: &TradeEventV1,
) -> Result<XorQuantity, OrderbookValidationError> {
    trade_gross_value_v1(trade)?
        .checked_add(&trade.maker_fee)
        .and_then(|amount| amount.checked_add(&trade.taker_fee))
        .map_err(OrderbookValidationError::Amount)
}
/// Return the immutable maker-plus-taker fee custody for a trade.
pub fn trade_fee_requirement_v1(
    trade: &TradeEventV1,
) -> Result<XorQuantity, OrderbookValidationError> {
    trade.validate()?;
    trade
        .maker_fee
        .checked_add(&trade.taker_fee)
        .map_err(OrderbookValidationError::Amount)
}
/// Return the conservative native custody required to admit one full bid.
///
/// The buyer can become either maker or taker. The counterparty's applicable
/// fee is not known at bid admission, so the bound combines the signed bid fee
/// for each role with the governed maximum for the opposite role, then selects
/// the larger exact basis-point charge. The entire signed quantity is priced
/// at the bid's limit price; execution at any lower crossing price therefore
/// cannot exceed this lock.
pub fn bid_order_escrow_requirement_v1(
    bid: &OrderRequestV1,
    governed_max_maker_fee_bps: u16,
    governed_max_taker_fee_bps: u16,
) -> Result<XorQuantity, OrderbookValidationError> {
    bid.validate()?;
    if bid.side != OrderSideV1::Bid {
        return Err(OrderbookValidationError::BidEscrowRequiresBid { side: bid.side });
    }
    validate_fee_bps(governed_max_maker_fee_bps)?;
    validate_fee_bps(governed_max_taker_fee_bps)?;
    let gross = bid
        .price_per_gib
        .checked_mul_u64(bid.quantity_gib)
        .map_err(OrderbookValidationError::Amount)?;
    let bid_as_maker_bps = u32::from(bid.maker_fee_bps) + u32::from(governed_max_taker_fee_bps);
    let bid_as_taker_bps = u32::from(bid.taker_fee_bps) + u32::from(governed_max_maker_fee_bps);
    let maximum_fee = gross
        .checked_mul_basis_points_u32(bid_as_maker_bps.max(bid_as_taker_bps))
        .map_err(OrderbookValidationError::Amount)?;
    gross
        .checked_add(&maximum_fee)
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
    /// Return true when the range covers no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.end <= self.start
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
    pub xor_locked: XorQuantity,
    /// Initial total XOR partitioned for the channel.
    pub initial_xor_locked: XorQuantity,
    /// Initial immutable maker-plus-taker fee custody.
    pub initial_fee_xor_locked: XorQuantity,
    /// Maker-plus-taker fee custody not yet settled.
    pub remaining_fee_xor_locked: XorQuantity,
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
        validate_owner_account_v1(&self.buyer_account)?;
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
        if self.status == SettlementChannelStatusV1::Closed && self.remaining_bytes != 0 {
            return Err(OrderbookValidationError::ClosedChannelHasRemainingBytes {
                remaining_bytes: self.remaining_bytes,
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
        if self.xor_locked > self.initial_xor_locked
            || self.initial_fee_xor_locked > self.initial_xor_locked
            || self.remaining_fee_xor_locked > self.initial_fee_xor_locked
            || self.remaining_fee_xor_locked > self.xor_locked
        {
            return Err(OrderbookValidationError::InvalidChannelFeeCustody);
        }
        if matches!(
            self.status,
            SettlementChannelStatusV1::Closed | SettlementChannelStatusV1::Refunded
        ) && (!self.xor_locked.is_zero() || !self.remaining_fee_xor_locked.is_zero())
        {
            return Err(OrderbookValidationError::TerminalChannelHasCustody);
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
    let initial_xor_locked = trade_escrow_requirement_v1(trade)?;
    let initial_fee_xor_locked = trade_fee_requirement_v1(trade)?;
    let channel = SettlementChannelV1 {
        version: SETTLEMENT_CHANNEL_VERSION_V1,
        channel_id,
        trade_id: trade.trade_id,
        buyer_account,
        provider_id,
        total_bytes,
        remaining_bytes: total_bytes,
        xor_locked: initial_xor_locked.clone(),
        initial_xor_locked,
        initial_fee_xor_locked: initial_fee_xor_locked.clone(),
        remaining_fee_xor_locked: initial_fee_xor_locked,
        status: SettlementChannelStatusV1::Open,
        opened_at_unix,
        updated_at_unix: opened_at_unix,
    };
    channel.validate()?;
    Ok(channel)
}
/// Ledger-derived economic split for one streaming receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementSplitV1 {
    /// Total custody debited for the delivered bytes.
    pub xor_debited: XorQuantity,
    /// Amount credited to the provider.
    pub provider_credit: XorQuantity,
    /// Immutable trade-fee portion credited to treasury.
    pub fee_amount: XorQuantity,
}
/// Prorate remaining total and fee custody toward zero for delivered bytes.
///
/// Both ratios use the same remaining-byte denominator. A final receipt
/// (`bytes_delivered == remaining_bytes`) therefore consumes all residual
/// custody exactly, including rounding dust from earlier chunks.
pub fn deterministic_settlement_split_v1(
    remaining_xor_locked: &XorQuantity,
    remaining_fee_xor_locked: &XorQuantity,
    bytes_delivered: u64,
    remaining_bytes: u64,
) -> Result<SettlementSplitV1, OrderbookValidationError> {
    let denominator =
        core::num::NonZeroU64::new(remaining_bytes).ok_or(OrderbookValidationError::ZeroBytes)?;
    if bytes_delivered == 0 || bytes_delivered > remaining_bytes {
        return Err(OrderbookValidationError::ReceiptExceedsRemainingBytes {
            delivered: bytes_delivered,
            remaining_bytes,
        });
    }
    if remaining_fee_xor_locked > remaining_xor_locked {
        return Err(OrderbookValidationError::InvalidChannelFeeCustody);
    }
    let xor_debited = remaining_xor_locked
        .checked_mul_ratio(bytes_delivered, denominator)
        .map_err(OrderbookValidationError::Amount)?;
    let fee_amount = remaining_fee_xor_locked
        .checked_mul_ratio(bytes_delivered, denominator)
        .map_err(OrderbookValidationError::Amount)?;
    let provider_credit = xor_debited
        .checked_sub(&fee_amount)
        .map_err(OrderbookValidationError::Amount)?;
    Ok(SettlementSplitV1 {
        xor_debited,
        provider_credit,
        fee_amount,
    })
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
    pub xor_debited: XorQuantity,
    /// XOR credited to the provider.
    pub provider_credit: XorQuantity,
    /// XOR retained as fee.
    pub fee_amount: XorQuantity,
    /// Unix timestamp (seconds) when the receipt was issued.
    pub issued_at_unix: u64,
    /// Signature over the canonical settlement receipt bytes.
    pub settlement_signature: OrderbookSignatureV1,
}
mod borrowed_norito {
    use norito::core::NoritoSerialize;
    /// Borrowed value that delegates canonical Norito serialization.
    pub(super) struct Value<'a, T>(pub(super) &'a T);
    impl<T: NoritoSerialize> NoritoSerialize for Value<'_, T> {
        fn schema_hash() -> [u8; 16] {
            T::schema_hash()
        }
        fn serialize(
            &self,
            writer: &mut norito::core::Encoder<'_>,
        ) -> Result<(), norito::core::Error> {
            self.0.serialize(writer)
        }
        fn encoded_len_hint(&self) -> Option<usize> {
            self.0.encoded_len_hint()
        }
        fn encoded_len_exact(&self) -> Option<usize> {
            self.0.encoded_len_exact()
        }
    }
    /// Borrowed vector that preserves the owned `Vec<T>` wire representation.
    pub(super) struct Vec<'a, T>(std::option::Option<&'a std::vec::Vec<T>>);
    impl<'a, T> Vec<'a, T> {
        /// Wrap an existing owned vector without cloning it.
        pub(super) fn borrowed(value: &'a std::vec::Vec<T>) -> Self {
            Self(Some(value))
        }
        /// Represent the canonical empty owned vector without allocating it.
        pub(super) fn empty() -> Self {
            Self(None)
        }
    }
    impl<T: NoritoSerialize> NoritoSerialize for Vec<'_, T> {
        fn schema_hash() -> [u8; 16] {
            <std::vec::Vec<T>>::schema_hash()
        }
        fn serialize(
            &self,
            writer: &mut norito::core::Encoder<'_>,
        ) -> Result<(), norito::core::Error> {
            match self.0 {
                Some(value) => value.serialize(writer),
                None => std::vec::Vec::<T>::new().serialize(writer),
            }
        }
        fn encoded_len_hint(&self) -> Option<usize> {
            match self.0 {
                Some(value) => value.encoded_len_hint(),
                None => std::vec::Vec::<T>::new().encoded_len_hint(),
            }
        }
        fn encoded_len_exact(&self) -> Option<usize> {
            match self.0 {
                Some(value) => value.encoded_len_exact(),
                None => std::vec::Vec::<T>::new().encoded_len_exact(),
            }
        }
    }
}
#[derive(NoritoSerialize)]
struct OrderbookSignatureSigningViewWireV1<'a> {
    algorithm: SignatureAlgorithm,
    public_key: borrowed_norito::Vec<'a, u8>,
    signature: borrowed_norito::Vec<'a, u8>,
}
struct OrderbookSignatureSigningViewV1<'a>(OrderbookSignatureSigningViewWireV1<'a>);
impl<'a> OrderbookSignatureSigningViewV1<'a> {
    fn from_signature(signature: &'a OrderbookSignatureV1) -> Self {
        Self(OrderbookSignatureSigningViewWireV1 {
            algorithm: signature.algorithm,
            public_key: borrowed_norito::Vec::borrowed(&signature.public_key),
            signature: borrowed_norito::Vec::empty(),
        })
    }
}
impl norito::core::NoritoSerialize for OrderbookSignatureSigningViewV1<'_> {
    fn schema_hash() -> [u8; 16] {
        OrderbookSignatureV1::schema_hash()
    }
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}
#[derive(NoritoSerialize)]
struct OrderRequestSigningViewWireV1<'a> {
    version: u8,
    order_id: [u8; 32],
    side: OrderSideV1,
    tier: OrderTierV1,
    price_per_gib: borrowed_norito::Value<'a, XorQuantity>,
    quantity_gib: u64,
    remaining_gib: u64,
    owner_account: borrowed_norito::Vec<'a, u8>,
    provider_id: Option<[u8; 32]>,
    expiry_unix: u64,
    nonce: u64,
    maker_fee_bps: u16,
    taker_fee_bps: u16,
    signature: OrderbookSignatureSigningViewV1<'a>,
}
struct OrderRequestSigningViewV1<'a>(OrderRequestSigningViewWireV1<'a>);
impl<'a> OrderRequestSigningViewV1<'a> {
    fn from_order(order: &'a OrderRequestV1) -> Self {
        Self(OrderRequestSigningViewWireV1 {
            version: order.version,
            order_id: order.order_id,
            side: order.side,
            tier: order.tier,
            price_per_gib: borrowed_norito::Value(&order.price_per_gib),
            quantity_gib: order.quantity_gib,
            remaining_gib: order.remaining_gib,
            owner_account: borrowed_norito::Vec::borrowed(&order.owner_account),
            provider_id: order.provider_id,
            expiry_unix: order.expiry_unix,
            nonce: order.nonce,
            maker_fee_bps: order.maker_fee_bps,
            taker_fee_bps: order.taker_fee_bps,
            signature: OrderbookSignatureSigningViewV1::from_signature(&order.signature),
        })
    }
}
impl norito::core::NoritoSerialize for OrderRequestSigningViewV1<'_> {
    fn schema_hash() -> [u8; 16] {
        OrderRequestV1::schema_hash()
    }
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}
#[derive(NoritoSerialize)]
struct OrderCancelSigningViewWireV1<'a> {
    version: u8,
    order_id: [u8; 32],
    owner_account: borrowed_norito::Vec<'a, u8>,
    reason: OrderCancelReasonV1,
    nonce: u64,
    signature: OrderbookSignatureSigningViewV1<'a>,
}
struct OrderCancelSigningViewV1<'a>(OrderCancelSigningViewWireV1<'a>);
impl<'a> OrderCancelSigningViewV1<'a> {
    fn from_cancel(cancel: &'a OrderCancelV1) -> Self {
        Self(OrderCancelSigningViewWireV1 {
            version: cancel.version,
            order_id: cancel.order_id,
            owner_account: borrowed_norito::Vec::borrowed(&cancel.owner_account),
            reason: cancel.reason,
            nonce: cancel.nonce,
            signature: OrderbookSignatureSigningViewV1::from_signature(&cancel.signature),
        })
    }
}
impl norito::core::NoritoSerialize for OrderCancelSigningViewV1<'_> {
    fn schema_hash() -> [u8; 16] {
        OrderCancelV1::schema_hash()
    }
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}
#[derive(NoritoSerialize)]
struct SettlementReceiptSigningViewWireV1<'a> {
    version: u8,
    receipt_id: [u8; 32],
    channel_id: [u8; 32],
    trade_id: [u8; 32],
    range: ByteRangeV1,
    chunk_hash: [u8; 32],
    bytes_delivered: u64,
    xor_debited: borrowed_norito::Value<'a, XorQuantity>,
    provider_credit: borrowed_norito::Value<'a, XorQuantity>,
    fee_amount: borrowed_norito::Value<'a, XorQuantity>,
    issued_at_unix: u64,
    settlement_signature: OrderbookSignatureSigningViewV1<'a>,
}
struct SettlementReceiptSigningViewV1<'a>(SettlementReceiptSigningViewWireV1<'a>);
impl<'a> SettlementReceiptSigningViewV1<'a> {
    fn from_receipt(receipt: &'a SettlementReceiptV1) -> Self {
        Self(SettlementReceiptSigningViewWireV1 {
            version: receipt.version,
            receipt_id: receipt.receipt_id,
            channel_id: receipt.channel_id,
            trade_id: receipt.trade_id,
            range: receipt.range,
            chunk_hash: receipt.chunk_hash,
            bytes_delivered: receipt.bytes_delivered,
            xor_debited: borrowed_norito::Value(&receipt.xor_debited),
            provider_credit: borrowed_norito::Value(&receipt.provider_credit),
            fee_amount: borrowed_norito::Value(&receipt.fee_amount),
            issued_at_unix: receipt.issued_at_unix,
            settlement_signature: OrderbookSignatureSigningViewV1::from_signature(
                &receipt.settlement_signature,
            ),
        })
    }
}
impl norito::core::NoritoSerialize for SettlementReceiptSigningViewV1<'_> {
    fn schema_hash() -> [u8; 16] {
        SettlementReceiptV1::schema_hash()
    }
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}
impl SettlementReceiptV1 {
    /// Validate structural and accounting constraints.
    pub fn validate(&self) -> Result<(), OrderbookValidationError> {
        preflight_orderbook_payload_len(self, ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1)?;
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
            .checked_add(&self.fee_amount)
            .map_err(OrderbookValidationError::Amount)?;
        if total_credit != self.xor_debited {
            return Err(OrderbookValidationError::SettlementImbalance {
                debited: self.xor_debited.clone(),
                credited_plus_fees: total_credit,
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
    preflight_orderbook_payload_len(receipt, ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1)?;
    let signable = SettlementReceiptSigningViewV1::from_receipt(receipt);
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
/// Sign a settlement receipt with the canonical SFM-2 Ed25519 digest.
///
/// The helper replaces the receipt's settlement signature material with the
/// signing key's public key and the signature over
/// [`settlement_receipt_signature_digest_v1`], then verifies the resulting
/// payload before returning it.
pub fn sign_settlement_receipt_ed25519_v1(
    mut receipt: SettlementReceiptV1,
    signing_key: &SigningKey,
) -> Result<SettlementReceiptV1, OrderbookValidationError> {
    receipt.settlement_signature = empty_ed25519_orderbook_signature(signing_key);
    let digest = settlement_receipt_signature_digest_v1(&receipt)?;
    receipt.settlement_signature.signature = signing_key.sign(&digest).to_bytes().to_vec();
    verify_settlement_receipt_signature_v1(&receipt)?;
    Ok(receipt)
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
    if receipt.xor_debited > channel.xor_locked {
        return Err(OrderbookValidationError::ReceiptExceedsEscrow {
            debited: receipt.xor_debited.clone(),
            escrow: channel.xor_locked.clone(),
        });
    }
    let expected_split = deterministic_settlement_split_v1(
        &channel.xor_locked,
        &channel.remaining_fee_xor_locked,
        delivered,
        channel.remaining_bytes,
    )?;
    if receipt.xor_debited != expected_split.xor_debited
        || receipt.provider_credit != expected_split.provider_credit
        || receipt.fee_amount != expected_split.fee_amount
    {
        return Err(OrderbookValidationError::SettlementSplitMismatch {
            expected_debit: expected_split.xor_debited,
            expected_provider_credit: expected_split.provider_credit,
            expected_fee: expected_split.fee_amount,
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
            .checked_sub(&receipt.xor_debited)
            .map_err(OrderbookValidationError::Amount)?,
        remaining_fee_xor_locked: channel
            .remaining_fee_xor_locked
            .checked_sub(&receipt.fee_amount)
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
    struct Blake3Writer<'a>(&'a mut Hasher);
    impl std::io::Write for Blake3Writer<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut hasher = Hasher::new();
    hasher.update(domain);
    norito::core::write_frame_to_writer(payload, &mut Blake3Writer(&mut hasher)).map_err(
        |err| OrderbookValidationError::SignaturePayloadEncoding {
            reason: err.to_string(),
        },
    )?;
    Ok(*hasher.finalize().as_bytes())
}
fn preflight_orderbook_payload_len<T: norito::core::NoritoSerialize>(
    payload: &T,
    maximum: usize,
) -> Result<usize, OrderbookValidationError> {
    if let Some(length) = payload.encoded_len_exact()
        && length > maximum
    {
        return Err(OrderbookValidationError::PayloadTooLarge { length, maximum });
    }
    let length = norito::core::encoded_payload_len(payload)
        .map_err(|_| OrderbookValidationError::CanonicalLengthUnavailable)?;
    if length > maximum {
        return Err(OrderbookValidationError::PayloadTooLarge { length, maximum });
    }
    Ok(length)
}
fn empty_ed25519_orderbook_signature(signing_key: &SigningKey) -> OrderbookSignatureV1 {
    OrderbookSignatureV1 {
        algorithm: SignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: Vec::new(),
    }
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
    let verifying_key = crate::checked_ed25519_verifying_key_from_bytes(&public_key)
        .map_err(|err| OrderbookValidationError::InvalidPublicKey { reason: err })?;
    let mut signature_bytes = [0u8; SIGNATURE_LENGTH];
    signature_bytes.copy_from_slice(&signature.signature);
    let signature = crate::checked_ed25519_signature_from_bytes(&signature_bytes)
        .map_err(|reason| OrderbookValidationError::SignatureVerification { reason })?;
    verifying_key
        .verify_strict(digest, &signature)
        .map_err(|err| OrderbookValidationError::SignatureVerification {
            reason: err.to_string(),
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
pub(crate) fn validate_owner_account_v1(
    owner_account: &[u8],
) -> Result<(), OrderbookValidationError> {
    if owner_account.is_empty() {
        return Err(OrderbookValidationError::EmptyOwnerAccount);
    }
    if owner_account.len() > ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 {
        return Err(OrderbookValidationError::OwnerAccountTooLong {
            length: owner_account.len(),
            max: ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1,
        });
    }
    if crate::inert_bytes(owner_account) {
        return Err(OrderbookValidationError::NonCanonicalOwnerAccount);
    }
    Ok(())
}
fn validate_fee_bps(fee_bps: u16) -> Result<(), OrderbookValidationError> {
    if fee_bps > BASIS_POINTS_PER_UNIT {
        return Err(OrderbookValidationError::InvalidFeeBps { fee_bps });
    }
    Ok(())
}
/// Failure to decode an attacker-controlled orderbook archive canonically.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OrderbookPayloadDecodeError {
    /// The archive exceeds the payload-kind byte ceiling.
    #[error("orderbook payload is {length} bytes; maximum canonical size is {maximum}")]
    PayloadTooLarge {
        /// Supplied archive length.
        length: usize,
        /// Maximum accepted canonical length.
        maximum: usize,
    },
    /// Decoding reached a caller or protocol resource boundary.
    #[error("orderbook payload exceeded its decode resource limit")]
    DecodeResourceLimit,
    /// Norito rejected the archive under the bounded decode budget.
    #[error("failed to decode orderbook payload: {reason}")]
    Decode {
        /// Codec diagnostic.
        reason: String,
    },
    /// Re-encoding the decoded value failed.
    #[error("failed to encode canonical orderbook payload: {reason}")]
    CanonicalEncoding {
        /// Codec diagnostic.
        reason: String,
    },
    /// The archive decoded but was not the exact canonical Norito encoding.
    #[error("orderbook payload is not the exact canonical Norito encoding")]
    NonCanonicalEncoding,
}
impl OrderbookPayloadDecodeError {
    /// Return whether decoding stopped at a caller-provided resource boundary.
    #[must_use]
    pub const fn is_decode_resource_limit(&self) -> bool {
        matches!(self, Self::DecodeResourceLimit)
    }
}
/// Validation errors for SoraFS orderbook payloads.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OrderbookValidationError {
    /// The canonical encoder cannot provide an allocation-free exact length.
    #[error("orderbook payload does not expose an exact canonical encoded length")]
    CanonicalLengthUnavailable,
    /// A directly constructed payload exceeds the canonical V1 byte ceiling.
    #[error("orderbook payload is {length} bytes; maximum canonical size is {maximum}")]
    PayloadTooLarge {
        /// Exact canonical payload length.
        length: usize,
        /// Maximum accepted canonical payload length.
        maximum: usize,
    },
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
    /// Order identifier does not match the canonical owner-and-nonce derivation.
    #[error(
        "order id {order_id:02x?} does not match canonical owner-and-nonce id {expected_order_id:02x?}"
    )]
    OrderIdDerivationMismatch {
        /// Identifier carried by the order.
        order_id: [u8; 32],
        /// Canonical identifier derived from owner account and nonce.
        expected_order_id: [u8; 32],
    },
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
    /// A bid carried an ask-only provider binding.
    #[error("bid orders must not carry a provider id")]
    BidProviderBindingForbidden,
    /// An ask omitted its exact provider binding.
    #[error("ask orders must carry an exact non-zero provider id")]
    AskProviderBindingRequired,
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
    /// Owner account exceeds the canonical V1 byte ceiling.
    #[error("owner account length {length} exceeds maximum {max} bytes")]
    OwnerAccountTooLong {
        /// Observed owner-account byte length.
        length: usize,
        /// Canonical maximum owner-account byte length.
        max: usize,
    },
    /// Owner/buyer account bytes are inert rather than a canonical identity.
    #[error("owner account bytes must not be inert")]
    NonCanonicalOwnerAccount,
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
    /// Bid custody was requested for a non-bid order.
    #[error("native order escrow requires a bid order, found {side:?}")]
    BidEscrowRequiresBid {
        /// Supplied order side.
        side: OrderSideV1,
    },
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
    #[error("bid price {bid_price} does not cross ask price {ask_price}")]
    PriceDoesNotCross {
        /// Exact bid price in XOR per GiB.
        bid_price: XorQuantity,
        /// Exact ask price in XOR per GiB.
        ask_price: XorQuantity,
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
        /// Exact debit amount.
        debited: XorQuantity,
        /// Exact remaining escrow.
        escrow: XorQuantity,
    },
    /// Channel total or fee custody violates immutable accounting bounds.
    #[error("settlement channel fee custody is outside total-custody bounds")]
    InvalidChannelFeeCustody,
    /// A closed or refunded channel retained total or fee custody.
    #[error("terminal settlement channel retains custody")]
    TerminalChannelHasCustody,
    /// Receipt split differs from the deterministic trade-derived split.
    #[error(
        "settlement split mismatch: expected debit {expected_debit}, provider credit {expected_provider_credit}, fee {expected_fee}"
    )]
    SettlementSplitMismatch {
        /// Expected total debit.
        expected_debit: XorQuantity,
        /// Expected provider credit.
        expected_provider_credit: XorQuantity,
        /// Expected fee amount.
        expected_fee: XorQuantity,
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
    /// Closed channel still has unsettled bytes.
    #[error("closed settlement channel still has {remaining_bytes} remaining bytes")]
    ClosedChannelHasRemainingBytes {
        /// Remaining bytes.
        remaining_bytes: u64,
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
        /// Exact debit amount.
        debited: XorQuantity,
        /// Exact provider credit plus fee amount.
        credited_plus_fees: XorQuantity,
    },
    /// Amount arithmetic failed.
    #[error(transparent)]
    Amount(DealAmountError),
}
#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use super::*;
    use ed25519_dalek::SigningKey;
    use norito::core::NoritoSerialize as _;
    const SMALL_ORDER_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
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
    fn encode_bare_with_flags<T: norito::core::NoritoSerialize>(value: &T, flags: u8) -> Vec<u8> {
        let _guard = norito::core::DecodeFlagsGuard::enter_with_hint(flags, flags);
        let mut bytes = Vec::new();
        norito::core::serialize_to_buffer(value, &mut bytes).expect("serialize explicit layout");
        bytes
    }
    fn encode_frame_with_flags<T: norito::core::NoritoSerialize>(value: &T, flags: u8) -> Vec<u8> {
        let _guard = norito::core::DecodeFlagsGuard::enter_with_hint(flags, flags);
        norito::to_bytes(value).expect("serialize explicit canonical frame")
    }
    fn supported_layouts() -> [u8; 8] {
        use norito::core::header_flags::{COMPACT_LEN, FIELD_BITSET, PACKED_SEQ, PACKED_STRUCT};
        [
            0,
            COMPACT_LEN,
            PACKED_SEQ,
            PACKED_SEQ | COMPACT_LEN,
            PACKED_STRUCT,
            PACKED_STRUCT | COMPACT_LEN,
            PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
            PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
        ]
    }
    fn historical_signature_digest<T: norito::core::NoritoSerialize>(
        domain: &[u8],
        value: &T,
    ) -> [u8; 32] {
        let bytes = norito::to_bytes(value).expect("encode historical signature preimage");
        let mut hasher = Hasher::new();
        hasher.update(domain);
        hasher.update(&bytes);
        *hasher.finalize().as_bytes()
    }
    fn sign_order(order: OrderRequestV1, seed: u8) -> OrderRequestV1 {
        let key = signing_key(seed);
        sign_order_request_ed25519_v1(order, &key).expect("signed order")
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
    fn sign_cancel(cancel: OrderCancelV1, seed: u8) -> OrderCancelV1 {
        let key = signing_key(seed);
        sign_order_cancel_ed25519_v1(cancel, &key).expect("signed cancel")
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
            xor_debited: XorQuantity::try_from_micro(100)
                .expect("legacy micro-XOR value is representable"),
            provider_credit: XorQuantity::try_from_micro(90)
                .expect("legacy micro-XOR value is representable"),
            fee_amount: XorQuantity::try_from_micro(10)
                .expect("legacy micro-XOR value is representable"),
            issued_at_unix: 1_800_000_200,
            settlement_signature: signature(),
        }
    }
    fn sign_receipt(receipt: SettlementReceiptV1, seed: u8) -> SettlementReceiptV1 {
        let key = signing_key(seed);
        sign_settlement_receipt_ed25519_v1(receipt, &key).expect("signed receipt")
    }
    fn order() -> OrderRequestV1 {
        let owner_account = account(3);
        let nonce = 1;
        OrderRequestV1 {
            version: ORDERBOOK_ORDER_VERSION_V1,
            order_id: derive_orderbook_order_id_v1(&owner_account, nonce),
            side: OrderSideV1::Bid,
            tier: OrderTierV1::Hot,
            price_per_gib: XorQuantity::try_from_micro(1_500_000)
                .expect("legacy micro-XOR value is representable"),
            quantity_gib: 10,
            remaining_gib: 10,
            owner_account,
            provider_id: None,
            expiry_unix: 1_800_000_000,
            nonce,
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
        order.side = side;
        order.price_per_gib = XorQuantity::try_from_micro(price_per_gib_micro)
            .expect("legacy micro-XOR value is representable");
        order.quantity_gib = quantity_gib;
        order.remaining_gib = quantity_gib;
        order.owner_account = account(seed);
        order.provider_id = (side == OrderSideV1::Ask).then_some([seed.max(1); 32]);
        order.nonce = u64::from(seed);
        refresh_order_id(&mut order);
        order
    }
    fn refresh_order_id(order: &mut OrderRequestV1) {
        order.order_id = derive_orderbook_order_id_v1(&order.owner_account, order.nonce);
    }
    fn book_entry(order: OrderRequestV1, sequence: u64) -> OrderBookEntryV1 {
        OrderBookEntryV1 { order, sequence }
    }
    fn snapshot_trade() -> TradeEventV1 {
        TradeEventV1 {
            version: ORDERBOOK_TRADE_EVENT_VERSION_V1,
            trade_id: id(4),
            maker_order_id: id(2),
            taker_order_id: id(3),
            tier: OrderTierV1::Hot,
            price_per_gib: XorQuantity::try_from_micro(1_400_000)
                .expect("legacy micro-XOR value is representable"),
            filled_gib: 2,
            maker_fee: XorQuantity::try_from_micro(14_000)
                .expect("legacy micro-XOR value is representable"),
            taker_fee: XorQuantity::try_from_micro(28_000)
                .expect("legacy micro-XOR value is representable"),
            timestamp_unix: 1_800_000_100,
        }
    }
    fn snapshot_channel(trade: &TradeEventV1) -> SettlementChannelV1 {
        open_settlement_channel_for_trade_v1(trade, id(5), account(9), id(6), 1_800_000_100)
            .expect("snapshot channel should open")
    }
    #[derive(Debug)]
    struct DeterministicRng {
        state: u64,
    }
    impl DeterministicRng {
        fn new(seed: u64) -> Self {
            Self { state: seed }
        }
        fn next_u64(&mut self) -> u64 {
            let mut value = self.state;
            value ^= value << 13;
            value ^= value >> 7;
            value ^= value << 17;
            self.state = value;
            value
        }
        fn range(&mut self, upper_exclusive: u64) -> u64 {
            self.next_u64() % upper_exclusive
        }
    }
    fn generated_account(scenario: u8, index: u8) -> Vec<u8> {
        let mut account = vec![0u8; 33];
        account[0] = 0xA0;
        account[1] = scenario;
        account[2] = index;
        account
    }
    fn generated_tier(value: u64) -> OrderTierV1 {
        match value % 3 {
            0 => OrderTierV1::Hot,
            1 => OrderTierV1::Warm,
            _ => OrderTierV1::Archive,
        }
    }
    fn generated_side(value: u64) -> OrderSideV1 {
        if value.is_multiple_of(2) {
            OrderSideV1::Bid
        } else {
            OrderSideV1::Ask
        }
    }
    fn tier_index(tier: OrderTierV1) -> usize {
        match tier {
            OrderTierV1::Hot => 0,
            OrderTierV1::Warm => 1,
            OrderTierV1::Archive => 2,
        }
    }
    fn side_index(side: OrderSideV1) -> usize {
        match side {
            OrderSideV1::Bid => 0,
            OrderSideV1::Ask => 1,
        }
    }
    fn add_quantity(totals: &mut [[u64; 2]; 3], tier: OrderTierV1, side: OrderSideV1, value: u64) {
        totals[tier_index(tier)][side_index(side)] =
            totals[tier_index(tier)][side_index(side)].saturating_add(value);
    }
    fn assert_match_invariants(
        entries: &[OrderBookEntryV1],
        outcome: &OrderBookMatchOutcomeV1,
        now_unix: u64,
    ) {
        let mut original = BTreeMap::new();
        let mut live_totals = [[0u64; 2]; 3];
        let mut expected_expired = BTreeSet::new();
        for entry in entries {
            let order = &entry.order;
            assert!(
                original.insert(order.order_id, order.clone()).is_none(),
                "generated fixture must keep order ids unique"
            );
            if now_unix > order.expiry_unix {
                expected_expired.insert(order.order_id);
            } else {
                add_quantity(
                    &mut live_totals,
                    order.tier,
                    order.side,
                    order.remaining_gib,
                );
            }
        }
        let actual_expired = outcome
            .expired_order_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_expired, expected_expired);
        let mut remaining_totals = [[0u64; 2]; 3];
        let mut remaining_ids = BTreeSet::new();
        let mut max_remaining_bid: [Option<u128>; 3] = [None, None, None];
        let mut min_remaining_ask: [Option<u128>; 3] = [None, None, None];
        for order in &outcome.remaining_orders {
            assert!(remaining_ids.insert(order.order_id));
            assert!(!expected_expired.contains(&order.order_id));
            let original_order = original
                .get(&order.order_id)
                .expect("remaining order must originate from input");
            assert_eq!(order.side, original_order.side);
            assert_eq!(order.tier, original_order.tier);
            assert!(order.remaining_gib > 0);
            assert!(order.remaining_gib <= original_order.remaining_gib);
            add_quantity(
                &mut remaining_totals,
                order.tier,
                order.side,
                order.remaining_gib,
            );
            let tier = tier_index(order.tier);
            let price = order
                .price_per_gib
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation");
            match order.side {
                OrderSideV1::Bid => {
                    max_remaining_bid[tier] =
                        Some(max_remaining_bid[tier].map_or(price, |current| current.max(price)));
                }
                OrderSideV1::Ask => {
                    min_remaining_ask[tier] =
                        Some(min_remaining_ask[tier].map_or(price, |current| current.min(price)));
                }
            }
        }
        let mut filled_totals = [[0u64; 2]; 3];
        let mut trade_ids = BTreeSet::new();
        for fill in &outcome.fills {
            assert!(trade_ids.insert(fill.trade.trade_id));
            assert!(fill.trade.filled_gib > 0);
            let maker = original
                .get(&fill.trade.maker_order_id)
                .expect("maker must originate from input");
            let taker = original
                .get(&fill.trade.taker_order_id)
                .expect("taker must originate from input");
            assert!(!expected_expired.contains(&maker.order_id));
            assert!(!expected_expired.contains(&taker.order_id));
            assert_ne!(maker.side, taker.side);
            assert_eq!(maker.tier, taker.tier);
            let (bid, ask) = if maker.side == OrderSideV1::Bid {
                (maker, taker)
            } else {
                (taker, maker)
            };
            assert!(
                bid.price_per_gib
                    .try_to_micro()
                    .expect("XOR quantity has exact legacy micro representation")
                    >= ask
                        .price_per_gib
                        .try_to_micro()
                        .expect("XOR quantity has exact legacy micro representation"),
                "filled orders must cross"
            );
            add_quantity(
                &mut filled_totals,
                fill.trade.tier,
                OrderSideV1::Bid,
                fill.trade.filled_gib,
            );
            add_quantity(
                &mut filled_totals,
                fill.trade.tier,
                OrderSideV1::Ask,
                fill.trade.filled_gib,
            );
            assert_eq!(
                fill.gross_value,
                fill.trade
                    .price_per_gib
                    .checked_mul_u64(fill.trade.filled_gib)
                    .expect("gross value should fit fixture limits")
            );
            assert_eq!(
                fill.trade.maker_fee,
                fill.gross_value
                    .checked_mul_basis_points(maker.maker_fee_bps)
                    .expect("maker fee should fit fixture limits")
            );
            assert_eq!(
                fill.trade.taker_fee,
                fill.gross_value
                    .checked_mul_basis_points(taker.taker_fee_bps)
                    .expect("taker fee should fit fixture limits")
            );
        }
        for tier in 0..3 {
            for side in 0..2 {
                assert_eq!(
                    live_totals[tier][side],
                    remaining_totals[tier][side].saturating_add(filled_totals[tier][side]),
                    "live quantity must equal remaining plus filled for tier {tier} side {side}"
                );
            }
            if let (Some(bid), Some(ask)) = (max_remaining_bid[tier], min_remaining_ask[tier]) {
                assert!(
                    bid < ask,
                    "remaining book must not contain a crossing bid/ask for tier {tier}"
                );
            }
        }
    }
    fn shuffle_entries(entries: &mut [OrderBookEntryV1], rng: &mut DeterministicRng) {
        for index in (1..entries.len()).rev() {
            let swap_with = rng.range((index + 1) as u64) as usize;
            entries.swap(index, swap_with);
        }
    }
    #[test]
    fn order_accepts_valid_payload() {
        assert_eq!(order().validate(), Ok(()));
    }
    #[test]
    fn order_requires_exact_provider_binding_only_for_asks() {
        let mut bid_with_provider = order();
        bid_with_provider.provider_id = Some(id(9));
        assert_eq!(
            bid_with_provider.validate(),
            Err(OrderbookValidationError::BidProviderBindingForbidden),
        );
        let mut ask_without_provider = order();
        ask_without_provider.side = OrderSideV1::Ask;
        assert_eq!(
            ask_without_provider.validate(),
            Err(OrderbookValidationError::AskProviderBindingRequired),
        );
        ask_without_provider.provider_id = Some([0; 32]);
        assert_eq!(
            ask_without_provider.validate(),
            Err(OrderbookValidationError::InvalidProviderId),
        );
        ask_without_provider.provider_id = Some(id(9));
        assert_eq!(ask_without_provider.validate(), Ok(()));
    }
    #[test]
    fn order_accepts_owner_account_at_v1_byte_ceiling() {
        let mut bounded = order();
        bounded.owner_account = vec![0x42; ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1];
        refresh_order_id(&mut bounded);
        assert_eq!(bounded.validate(), Ok(()));
        let signed = sign_order(bounded, 0x41);
        assert_eq!(verify_order_request_signature_v1(&signed), Ok(()));
    }
    #[test]
    fn order_rejects_owner_account_above_v1_byte_ceiling_before_id_or_signature_use() {
        let mut oversized = order();
        oversized.owner_account = vec![0x42; ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 + 1];
        oversized.order_id = [0xAA; 32];
        oversized.signature.signature.clear();
        let expected = OrderbookValidationError::OwnerAccountTooLong {
            length: ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 + 1,
            max: ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1,
        };
        assert_eq!(oversized.validate(), Err(expected.clone()));
        assert_eq!(order_request_signature_digest_v1(&oversized), Err(expected));
    }
    #[test]
    fn cancel_accepts_owner_account_at_v1_byte_ceiling() {
        let mut bounded = cancel();
        bounded.owner_account = vec![0x43; ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1];
        assert_eq!(bounded.validate(), Ok(()));
        let signed = sign_cancel(bounded, 0x42);
        assert_eq!(verify_order_cancel_signature_v1(&signed), Ok(()));
    }
    #[test]
    fn cancel_rejects_owner_account_above_v1_byte_ceiling_before_signature_use() {
        let mut oversized = cancel();
        oversized.owner_account = vec![0x43; ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 + 1];
        oversized.signature.signature.clear();
        let expected = OrderbookValidationError::OwnerAccountTooLong {
            length: ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 + 1,
            max: ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1,
        };
        assert_eq!(oversized.validate(), Err(expected.clone()));
        assert_eq!(order_cancel_signature_digest_v1(&oversized), Err(expected));
    }
    #[test]
    fn order_and_cancel_reject_inert_owner_accounts() {
        let mut invalid_order = order();
        invalid_order.owner_account = vec![0; 33];
        refresh_order_id(&mut invalid_order);
        assert_eq!(
            invalid_order.validate(),
            Err(OrderbookValidationError::NonCanonicalOwnerAccount)
        );
        let mut invalid_cancel = cancel();
        invalid_cancel.owner_account = vec![0; 33];
        assert_eq!(
            invalid_cancel.validate(),
            Err(OrderbookValidationError::NonCanonicalOwnerAccount)
        );
    }
    #[test]
    fn order_id_derivation_binds_owner_and_nonce() {
        let owner = account(3);
        let order_id = derive_orderbook_order_id_v1(&owner, 1);
        assert_eq!(order_id, order().order_id);
        assert!(order_id.iter().any(|byte| *byte != 0));
        assert_ne!(order_id, derive_orderbook_order_id_v1(&owner, 2));
        assert_ne!(order_id, derive_orderbook_order_id_v1(&account(4), 1));
    }
    #[test]
    fn order_id_derivation_matches_cross_sdk_golden_vector() {
        assert_eq!(
            hex::encode(derive_orderbook_order_id_v1(b"buyer@sora", 7)),
            "9d91ad7700ca0c4762e031f9231aa38dd4502c6048c6ffa31d365e3c4e080b69"
        );
    }
    #[test]
    fn order_rejects_same_owner_retired_id_reuse_at_higher_nonce() {
        let original = order();
        let mut reused = original.clone();
        reused.nonce = original.nonce + 1;
        let expected_order_id = derive_orderbook_order_id_v1(&reused.owner_account, reused.nonce);
        assert_eq!(
            reused.validate(),
            Err(OrderbookValidationError::OrderIdDerivationMismatch {
                order_id: original.order_id,
                expected_order_id,
            })
        );
    }
    #[test]
    fn order_rejects_cross_owner_retired_id_reuse() {
        let original = order();
        let mut reused = original.clone();
        reused.owner_account = account(4);
        let expected_order_id = derive_orderbook_order_id_v1(&reused.owner_account, reused.nonce);
        assert_eq!(
            reused.validate(),
            Err(OrderbookValidationError::OrderIdDerivationMismatch {
                order_id: original.order_id,
                expected_order_id,
            })
        );
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
    fn order_and_cancel_reject_zero_nonce() {
        let mut invalid_order = order();
        invalid_order.nonce = 0;
        assert_eq!(
            invalid_order.validate(),
            Err(OrderbookValidationError::ZeroNonce)
        );
        let mut invalid_cancel = cancel();
        invalid_cancel.nonce = 0;
        assert_eq!(
            invalid_cancel.validate(),
            Err(OrderbookValidationError::ZeroNonce)
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
    fn orderbook_signatures_reject_reserved_multisig_material() {
        let mut receipt = receipt();
        receipt.settlement_signature.algorithm = SignatureAlgorithm::MultiSig;
        assert_eq!(
            receipt.validate(),
            Err(OrderbookValidationError::UnsupportedSignatureAlgorithm {
                algorithm: SignatureAlgorithm::MultiSig,
            })
        );
    }
    #[test]
    fn settlement_receipt_size_preflight_accepts_boundary_and_rejects_one_over() {
        let receipt = receipt();
        let exact = norito::core::encoded_payload_len(&receipt)
            .expect("settlement receipt canonical length must be countable");
        assert_eq!(preflight_orderbook_payload_len(&receipt, exact), Ok(exact));
        assert_eq!(
            preflight_orderbook_payload_len(&receipt, exact.saturating_sub(1)),
            Err(OrderbookValidationError::PayloadTooLarge {
                length: exact,
                maximum: exact.saturating_sub(1),
            })
        );
        let mut oversized = receipt;
        oversized.settlement_signature.signature =
            vec![9; ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1];
        assert!(matches!(
            oversized.validate(),
            Err(OrderbookValidationError::PayloadTooLarge {
                maximum: ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1,
                ..
            })
        ));
        assert!(matches!(
            settlement_receipt_signature_digest_v1(&oversized),
            Err(OrderbookValidationError::PayloadTooLarge {
                maximum: ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1,
                ..
            })
        ));
    }
    #[test]
    fn borrowed_order_and_cancel_signing_views_preserve_historical_frames_and_digests() {
        let order = sign_order(order(), 0x11);
        let mut owned_order = order.clone();
        owned_order.signature.signature.clear();
        let borrowed_order = OrderRequestSigningViewV1::from_order(&order);
        let cancel = sign_cancel(cancel(), 0x12);
        let mut owned_cancel = cancel.clone();
        owned_cancel.signature.signature.clear();
        let borrowed_cancel = OrderCancelSigningViewV1::from_cancel(&cancel);
        for flags in supported_layouts() {
            assert_eq!(
                encode_frame_with_flags(&borrowed_order, flags),
                encode_frame_with_flags(&owned_order, flags),
                "borrowed order signing frame changed for flags 0x{flags:02x}"
            );
            assert_eq!(
                encode_frame_with_flags(&borrowed_cancel, flags),
                encode_frame_with_flags(&owned_cancel, flags),
                "borrowed cancellation signing frame changed for flags 0x{flags:02x}"
            );
            let _guard = norito::core::DecodeFlagsGuard::enter_with_hint(flags, flags);
            assert_eq!(
                order_request_signature_digest_v1(&order).expect("stream order digest"),
                historical_signature_digest(ORDERBOOK_ORDER_SIGNATURE_DOMAIN_V1, &owned_order,),
                "streamed order digest changed for flags 0x{flags:02x}"
            );
            assert_eq!(
                order_cancel_signature_digest_v1(&cancel).expect("stream cancellation digest"),
                historical_signature_digest(ORDERBOOK_CANCEL_SIGNATURE_DOMAIN_V1, &owned_cancel,),
                "streamed cancellation digest changed for flags 0x{flags:02x}"
            );
        }
    }
    #[test]
    fn borrowed_settlement_receipt_signing_view_is_byte_exact_for_every_layout() {
        let receipt = sign_receipt(receipt(), 0x14);
        let mut owned = receipt.clone();
        owned.settlement_signature.signature.clear();
        let borrowed = SettlementReceiptSigningViewV1::from_receipt(&receipt);
        assert_eq!(
            <SettlementReceiptSigningViewV1<'_> as norito::core::NoritoSerialize>::schema_hash(),
            SettlementReceiptV1::schema_hash()
        );
        assert_eq!(
            norito::to_bytes(&borrowed).expect("encode borrowed signing view"),
            norito::to_bytes(&owned).expect("encode historical owned signing payload")
        );
        for flags in supported_layouts() {
            let owned_bytes = encode_bare_with_flags(&owned, flags);
            let borrowed_bytes = encode_bare_with_flags(&borrowed, flags);
            assert_eq!(
                borrowed_bytes, owned_bytes,
                "borrowed settlement signing bytes changed for flags 0x{flags:02x}"
            );
            let _guard = norito::core::DecodeFlagsGuard::enter_with_hint(flags, flags);
            assert_eq!(
                borrowed.encoded_len_exact(),
                owned.encoded_len_exact(),
                "borrowed settlement signing size changed for flags 0x{flags:02x}"
            );
            assert_eq!(
                norito::core::encoded_payload_len(&borrowed)
                    .expect("borrowed settlement receipt length must be countable"),
                borrowed_bytes.len()
            );
            assert_eq!(
                encode_frame_with_flags(&borrowed, flags),
                encode_frame_with_flags(&owned, flags),
                "borrowed settlement canonical frame or layout flags changed for flags 0x{flags:02x}"
            );
            assert_eq!(
                settlement_receipt_signature_digest_v1(&receipt)
                    .expect("digest borrowed settlement signing view"),
                historical_signature_digest(SETTLEMENT_RECEIPT_SIGNATURE_DOMAIN_V1, &owned),
                "settlement signature digest changed for flags 0x{flags:02x}"
            );
        }
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
    fn ed25519_signing_helpers_attach_public_key_and_verify_payloads() {
        let key = signing_key(0x41);
        let public_key = key.verifying_key().to_bytes().to_vec();
        let signed_order = sign_order_request_ed25519_v1(order(), &key).expect("signed order");
        assert_eq!(signed_order.signature.public_key, public_key);
        assert_eq!(signed_order.signature.signature.len(), SIGNATURE_LENGTH);
        assert_eq!(verify_order_request_signature_v1(&signed_order), Ok(()));
        let signed_cancel = sign_order_cancel_ed25519_v1(cancel(), &key).expect("signed cancel");
        assert_eq!(signed_cancel.signature.public_key, public_key);
        assert_eq!(signed_cancel.signature.signature.len(), SIGNATURE_LENGTH);
        assert_eq!(verify_order_cancel_signature_v1(&signed_cancel), Ok(()));
        let signed_receipt =
            sign_settlement_receipt_ed25519_v1(receipt(), &key).expect("signed receipt");
        assert_eq!(signed_receipt.settlement_signature.public_key, public_key);
        assert_eq!(
            signed_receipt.settlement_signature.signature.len(),
            SIGNATURE_LENGTH
        );
        assert_eq!(
            verify_settlement_receipt_signature_v1(&signed_receipt),
            Ok(())
        );
    }
    #[test]
    fn verify_order_signature_accepts_valid_payload_and_rejects_tamper() {
        let signed = sign_order(order(), 0x15);
        assert_eq!(verify_order_request_signature_v1(&signed), Ok(()));
        let mut tampered = signed;
        tampered.price_per_gib = XorQuantity::try_from_micro(9_999_999)
            .expect("legacy micro-XOR value is representable");
        assert!(matches!(
            verify_order_request_signature_v1(&tampered),
            Err(OrderbookValidationError::SignatureVerification { .. })
        ));
        let mut nonce_tampered = sign_order(order(), 0x15);
        nonce_tampered.nonce += 1;
        nonce_tampered.order_id =
            derive_orderbook_order_id_v1(&nonce_tampered.owner_account, nonce_tampered.nonce);
        assert!(matches!(
            verify_order_request_signature_v1(&nonce_tampered),
            Err(OrderbookValidationError::SignatureVerification { .. })
        ));
    }
    #[test]
    fn verify_order_signature_rejects_all_zero_signature_material() {
        let mut signed = sign_order(order(), 0x18);
        signed.signature.signature.fill(0);
        let err = verify_order_request_signature_v1(&signed)
            .expect_err("all-zero order signature must be rejected");
        assert!(matches!(err, OrderbookValidationError::InvalidSignature));
    }
    #[test]
    fn verify_order_signature_rejects_all_zero_public_key_material() {
        let mut signed = sign_order(order(), 0x19);
        signed.signature.public_key = vec![0; PUBLIC_KEY_LENGTH];
        let err = verify_order_request_signature_v1(&signed)
            .expect_err("all-zero order public key must be rejected");
        assert!(matches!(err, OrderbookValidationError::InvalidSignature));
    }
    #[test]
    fn verify_order_signature_rejects_malformed_ed25519_signature_r() {
        for (label, replacement_r, expected_reason) in [
            ("small-order", SMALL_ORDER_R, "small-order"),
            ("noncanonical", NONCANONICAL_R, "not a canonical"),
        ] {
            let mut signed = sign_order(order(), 0x1A);
            signed.signature.signature[..PUBLIC_KEY_LENGTH].copy_from_slice(&replacement_r);
            let err = verify_order_request_signature_v1(&signed)
                .expect_err("malformed order signature R must be rejected");
            assert!(
                matches!(
                    &err,
                    OrderbookValidationError::SignatureVerification { reason }
                        if reason.contains(expected_reason)
                ),
                "{label} signature R produced unexpected error: {err}"
            );
        }
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
            price_per_gib: XorQuantity::try_from_micro(2_000_000)
                .expect("legacy micro-XOR value is representable"),
            filled_gib: 5,
            maker_fee: XorQuantity::try_from_micro(1_000)
                .expect("legacy micro-XOR value is representable"),
            taker_fee: XorQuantity::try_from_micro(2_000)
                .expect("legacy micro-XOR value is representable"),
            timestamp_unix: 1_800_000_100,
        };
        assert_eq!(trade.validate(), Err(OrderbookValidationError::SelfTrade));
    }
    #[test]
    fn bid_order_escrow_covers_full_limit_value_and_worst_role_fee() {
        let bid = order();
        let required = bid_order_escrow_requirement_v1(&bid, 100, 200)
            .expect("derive conservative bid custody");
        assert_eq!(
            required,
            XorQuantity::try_from_micro(15_307_500).expect("expected custody"),
        );
        let mut ask = bid;
        ask.side = OrderSideV1::Ask;
        ask.provider_id = Some(id(9));
        assert_eq!(
            bid_order_escrow_requirement_v1(&ask, 100, 200),
            Err(OrderbookValidationError::BidEscrowRequiresBid {
                side: OrderSideV1::Ask,
            }),
        );
    }
    #[test]
    fn match_orders_creates_trade_and_remaining_quantities() {
        let mut maker = order();
        maker.side = OrderSideV1::Ask;
        maker.provider_id = Some(id(11));
        maker.owner_account = account(11);
        refresh_order_id(&mut maker);
        maker.price_per_gib = XorQuantity::try_from_micro(1_500_000)
            .expect("legacy micro-XOR value is representable");
        maker.remaining_gib = 10;
        maker.maker_fee_bps = 5;
        let mut taker = order();
        taker.side = OrderSideV1::Bid;
        taker.owner_account = account(12);
        refresh_order_id(&mut taker);
        taker.price_per_gib = XorQuantity::try_from_micro(1_600_000)
            .expect("legacy micro-XOR value is representable");
        taker.quantity_gib = 4;
        taker.remaining_gib = 4;
        taker.taker_fee_bps = 10;
        let outcome =
            match_orders_v1(&maker, &taker, id(13), 1_700_000_000).expect("orders should match");
        assert_eq!(outcome.trade.maker_order_id, maker.order_id);
        assert_eq!(outcome.trade.taker_order_id, taker.order_id);
        assert_eq!(outcome.trade.price_per_gib, maker.price_per_gib);
        assert_eq!(outcome.trade.filled_gib, 4);
        assert_eq!(
            outcome.gross_value,
            XorQuantity::try_from_micro(6_000_000)
                .expect("legacy micro-XOR value is representable")
        );
        assert_eq!(
            outcome.trade.maker_fee,
            XorQuantity::try_from_micro(3_000).expect("legacy micro-XOR value is representable")
        );
        assert_eq!(
            outcome.trade.taker_fee,
            XorQuantity::try_from_micro(6_000).expect("legacy micro-XOR value is representable")
        );
        assert_eq!(outcome.maker_remaining_gib, 6);
        assert_eq!(outcome.taker_remaining_gib, 0);
        assert_eq!(outcome.trade.validate(), Ok(()));
    }
    #[test]
    fn match_orders_rejects_non_crossing_prices() {
        let mut maker = order();
        maker.side = OrderSideV1::Ask;
        maker.provider_id = Some(id(11));
        maker.owner_account = account(11);
        refresh_order_id(&mut maker);
        maker.price_per_gib = XorQuantity::try_from_micro(1_500_000)
            .expect("legacy micro-XOR value is representable");
        let mut taker = order();
        taker.side = OrderSideV1::Bid;
        taker.owner_account = account(12);
        refresh_order_id(&mut taker);
        taker.price_per_gib = XorQuantity::try_from_micro(1_400_000)
            .expect("legacy micro-XOR value is representable");
        assert_eq!(
            match_orders_v1(&maker, &taker, id(13), 1_700_000_000),
            Err(OrderbookValidationError::PriceDoesNotCross {
                bid_price: XorQuantity::try_from_micro(1_400_000)
                    .expect("legacy micro-XOR value is representable"),
                ask_price: XorQuantity::try_from_micro(1_500_000)
                    .expect("legacy micro-XOR value is representable"),
            })
        );
    }
    #[test]
    fn match_orders_compares_sub_micro_prices_exactly() {
        let ask_price: XorQuantity = "0.0000002".parse().expect("canonical sub-micro XOR price");
        let bid_price: XorQuantity = "0.0000001".parse().expect("canonical sub-micro XOR price");
        let mut maker = order();
        maker.side = OrderSideV1::Ask;
        maker.provider_id = Some(id(21));
        maker.owner_account = account(21);
        maker.price_per_gib = ask_price.clone();
        refresh_order_id(&mut maker);
        let mut taker = order();
        taker.side = OrderSideV1::Bid;
        taker.owner_account = account(22);
        taker.price_per_gib = bid_price.clone();
        refresh_order_id(&mut taker);
        assert_eq!(
            match_orders_v1(&maker, &taker, id(23), 1_700_000_000),
            Err(OrderbookValidationError::PriceDoesNotCross {
                bid_price,
                ask_price,
            })
        );
        maker.price_per_gib = "0.0000001".parse().expect("canonical sub-micro XOR price");
        taker.price_per_gib = "0.0000002".parse().expect("canonical sub-micro XOR price");
        let outcome = match_orders_v1(&maker, &taker, id(24), 1_700_000_000)
            .expect("sub-micro crossing prices must match without legacy projection");
        assert_eq!(outcome.trade.price_per_gib.to_string(), "0.0000001");
    }
    #[test]
    fn match_order_book_uses_price_time_priority_and_partial_fills() {
        let bid = book_order(21, OrderSideV1::Bid, 2_000_000, 10);
        let low_ask = book_order(22, OrderSideV1::Ask, 900_000, 4);
        let high_ask = book_order(23, OrderSideV1::Ask, 1_000_000, 3);
        let bid_id = bid.order_id;
        let low_ask_id = low_ask.order_id;
        let high_ask_id = high_ask.order_id;
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
        assert_eq!(outcome.fills[0].trade.maker_order_id, low_ask_id);
        assert_eq!(outcome.fills[0].trade.taker_order_id, bid_id);
        assert_eq!(
            outcome.fills[0].trade.price_per_gib,
            XorQuantity::try_from_micro(900_000).expect("legacy micro-XOR value is representable")
        );
        assert_eq!(outcome.fills[0].trade.filled_gib, 4);
        assert_eq!(outcome.fills[0].maker_remaining_gib, 0);
        assert_eq!(outcome.fills[0].taker_remaining_gib, 6);
        assert_eq!(outcome.fills[1].trade.maker_order_id, high_ask_id);
        assert_eq!(outcome.fills[1].trade.taker_order_id, bid_id);
        assert_eq!(
            outcome.fills[1].trade.price_per_gib,
            XorQuantity::try_from_micro(1_000_000)
                .expect("legacy micro-XOR value is representable")
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
        assert_eq!(outcome.remaining_orders[0].order_id, bid_id);
        assert_eq!(outcome.remaining_orders[0].remaining_gib, 3);
    }
    #[test]
    fn match_order_book_generated_streams_preserve_balance_and_no_crossing_remainder() {
        let now_unix = 1_700_000_000;
        let mut rng = DeterministicRng::new(0x5eed_f00d_cafe_babe);
        for scenario in 0..48u8 {
            let order_count = 8 + rng.range(20) as usize;
            let mut entries = Vec::with_capacity(order_count);
            for index in 0..order_count {
                let index_u8 = index as u8;
                let side = generated_side(rng.range(2));
                let tier = generated_tier(rng.range(3));
                let price_step = rng.range(14);
                let price_micro = match side {
                    OrderSideV1::Bid => 900_000 + price_step * 90_000,
                    OrderSideV1::Ask => 850_000 + price_step * 80_000,
                };
                let quantity_gib = 1 + rng.range(12);
                let owner_account = generated_account(scenario, index_u8);
                let nonce = u64::from(scenario) * 1_000 + index as u64 + 1;
                let mut order = OrderRequestV1 {
                    version: ORDERBOOK_ORDER_VERSION_V1,
                    order_id: derive_orderbook_order_id_v1(&owner_account, nonce),
                    side,
                    tier,
                    price_per_gib: XorQuantity::try_from_micro(u128::from(price_micro))
                        .expect("legacy micro-XOR value is representable"),
                    quantity_gib,
                    remaining_gib: quantity_gib,
                    owner_account,
                    provider_id: (side == OrderSideV1::Ask)
                        .then_some([index_u8.wrapping_add(1); 32]),
                    expiry_unix: now_unix + 1 + rng.range(600),
                    nonce,
                    maker_fee_bps: rng.range(40) as u16,
                    taker_fee_bps: rng.range(40) as u16,
                    signature: signature(),
                };
                if rng.range(11) == 0 {
                    order.expiry_unix = now_unix.saturating_sub(1);
                }
                entries.push(book_entry(order, index as u64));
            }
            let outcome =
                match_order_book_v1(&entries, now_unix).expect("generated book should match");
            assert_match_invariants(&entries, &outcome, now_unix);
        }
    }
    #[test]
    fn match_order_book_generated_streams_are_permutation_invariant() {
        let now_unix = 1_700_000_000;
        let mut rng = DeterministicRng::new(0x0123_4567_89ab_cdef);
        for scenario in 0..32u8 {
            let order_count = 18 + rng.range(18) as usize;
            let mut entries = Vec::with_capacity(order_count);
            for index in 0..order_count {
                let index_u8 = index as u8;
                let side = generated_side((index as u64) ^ rng.range(4));
                let tier = generated_tier((index as u64) + rng.range(5));
                let price_step = rng.range(18);
                let price_micro = match side {
                    OrderSideV1::Bid => 1_000_000 + price_step * 70_000,
                    OrderSideV1::Ask => 900_000 + price_step * 65_000,
                };
                let quantity_gib = 1 + rng.range(16);
                let owner_account = generated_account(scenario.wrapping_add(80), index_u8);
                let nonce = u64::from(scenario) * 10_000 + index as u64 + 1;
                let mut order = OrderRequestV1 {
                    version: ORDERBOOK_ORDER_VERSION_V1,
                    order_id: derive_orderbook_order_id_v1(&owner_account, nonce),
                    side,
                    tier,
                    price_per_gib: XorQuantity::try_from_micro(u128::from(price_micro))
                        .expect("legacy micro-XOR value is representable"),
                    quantity_gib,
                    remaining_gib: quantity_gib,
                    owner_account,
                    provider_id: (side == OrderSideV1::Ask)
                        .then_some([index_u8.wrapping_add(1); 32]),
                    expiry_unix: now_unix + 10 + rng.range(900),
                    nonce,
                    maker_fee_bps: rng.range(60) as u16,
                    taker_fee_bps: rng.range(60) as u16,
                    signature: signature(),
                };
                if index % 7 == 0 || index % 11 == 0 {
                    order.expiry_unix = now_unix.saturating_sub(1);
                }
                entries.push(book_entry(order, index as u64));
            }
            let expected =
                match_order_book_v1(&entries, now_unix).expect("canonical book should match");
            assert_match_invariants(&entries, &expected, now_unix);
            let mut shuffled = entries.clone();
            shuffle_entries(&mut shuffled, &mut rng);
            let actual =
                match_order_book_v1(&shuffled, now_unix).expect("shuffled book should match");
            assert_eq!(
                actual, expected,
                "matching must depend on canonical sequence, not input order, in scenario {scenario}"
            );
            assert_match_invariants(&shuffled, &actual, now_unix);
        }
    }
    #[test]
    fn match_order_book_skips_expired_orders() {
        let mut expired_ask = book_order(31, OrderSideV1::Ask, 1_000_000, 5);
        expired_ask.expiry_unix = 1_699_999_999;
        let live_bid = book_order(32, OrderSideV1::Bid, 2_000_000, 5);
        let expired_ask_id = expired_ask.order_id;
        let live_bid_id = live_bid.order_id;
        let outcome = match_order_book_v1(
            &[book_entry(live_bid, 2), book_entry(expired_ask, 1)],
            1_700_000_000,
        )
        .expect("expired orders should be skipped");
        assert!(outcome.fills.is_empty());
        assert_eq!(outcome.expired_order_ids, vec![expired_ask_id]);
        assert_eq!(outcome.remaining_orders.len(), 1);
        assert_eq!(outcome.remaining_orders[0].order_id, live_bid_id);
        assert_eq!(outcome.remaining_orders[0].remaining_gib, 5);
    }
    #[test]
    fn match_order_book_rejects_duplicate_order_ids() {
        let bid = book_order(41, OrderSideV1::Bid, 2_000_000, 5);
        let duplicate_id = bid.order_id;
        let ask = bid.clone();
        assert_eq!(
            match_order_book_v1(&[book_entry(bid, 1), book_entry(ask, 2)], 1_700_000_000),
            Err(OrderbookValidationError::DuplicateOrderId {
                order_id: duplicate_id
            })
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
        maker.provider_id = Some(id(11));
        maker.owner_account = account(11);
        refresh_order_id(&mut maker);
        maker.price_per_gib = XorQuantity::try_from_micro(1_500_000)
            .expect("legacy micro-XOR value is representable");
        maker.remaining_gib = 4;
        maker.maker_fee_bps = 5;
        let mut taker = order();
        taker.side = OrderSideV1::Bid;
        taker.owner_account = account(12);
        refresh_order_id(&mut taker);
        taker.price_per_gib = XorQuantity::try_from_micro(1_600_000)
            .expect("legacy micro-XOR value is representable");
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
        assert_eq!(
            channel.xor_locked,
            XorQuantity::try_from_micro(6_009_000)
                .expect("legacy micro-XOR value is representable")
        );
        assert_eq!(channel.status, SettlementChannelStatusV1::Open);
        assert_eq!(channel.validate(), Ok(()));
    }
    #[test]
    fn settlement_channel_id_is_canonical_and_trade_bound() {
        let trade = snapshot_trade();
        let channel_id = derive_orderbook_settlement_channel_id_v1(&trade)
            .expect("valid trade derives a channel id");
        assert_ne!(channel_id, [0; 32]);
        assert_eq!(
            derive_orderbook_settlement_channel_id_v1(&trade),
            Ok(channel_id)
        );
        let mut other = trade.clone();
        other.trade_id[0] ^= 1;
        assert_ne!(
            derive_orderbook_settlement_channel_id_v1(&other)
                .expect("other valid trade derives a channel id"),
            channel_id
        );
        let mut invalid = trade;
        invalid.trade_id = [0; 32];
        assert_eq!(
            derive_orderbook_settlement_channel_id_v1(&invalid),
            Err(OrderbookValidationError::InvalidTradeId)
        );
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
            xor_locked: XorQuantity::try_from_micro(3_000_000)
                .expect("legacy micro-XOR value is representable"),
            initial_xor_locked: XorQuantity::try_from_micro(3_000_000)
                .expect("legacy micro-XOR value is representable"),
            initial_fee_xor_locked: XorQuantity::zero(),
            remaining_fee_xor_locked: XorQuantity::zero(),
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
    fn settlement_channel_enforces_canonical_buyer_account_boundaries() {
        let trade = snapshot_trade();
        let mut channel = snapshot_channel(&trade);
        channel.buyer_account = vec![0x42; ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1];
        channel
            .validate()
            .expect("buyer account at exact byte ceiling validates");
        channel.buyer_account.push(0x42);
        assert_eq!(
            channel.validate(),
            Err(OrderbookValidationError::OwnerAccountTooLong {
                length: ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 + 1,
                max: ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1,
            })
        );
        channel.buyer_account = vec![0; 33];
        assert_eq!(
            channel.validate(),
            Err(OrderbookValidationError::NonCanonicalOwnerAccount)
        );
        channel.buyer_account.clear();
        assert_eq!(
            channel.validate(),
            Err(OrderbookValidationError::EmptyOwnerAccount)
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
            xor_debited: XorQuantity::try_from_micro(100)
                .expect("legacy micro-XOR value is representable"),
            provider_credit: XorQuantity::try_from_micro(91)
                .expect("legacy micro-XOR value is representable"),
            fee_amount: XorQuantity::try_from_micro(10)
                .expect("legacy micro-XOR value is representable"),
            issued_at_unix: 1_800_000_200,
            settlement_signature: signature(),
        };
        assert_eq!(
            receipt.validate(),
            Err(OrderbookValidationError::SettlementImbalance {
                debited: XorQuantity::try_from_micro(100)
                    .expect("legacy micro-XOR value is representable"),
                credited_plus_fees: XorQuantity::try_from_micro(101)
                    .expect("legacy micro-XOR value is representable"),
            })
        );
    }
    #[test]
    fn settlement_accounting_preserves_sub_micro_amounts() {
        let one: XorQuantity = "0.0000001"
            .parse()
            .expect("canonical sub-micro XOR quantity");
        let two: XorQuantity = "0.0000002"
            .parse()
            .expect("canonical sub-micro XOR quantity");
        let three: XorQuantity = "0.0000003"
            .parse()
            .expect("canonical sub-micro XOR quantity");
        let mut imbalanced = receipt();
        imbalanced.xor_debited = two.clone();
        imbalanced.provider_credit = two.clone();
        imbalanced.fee_amount = one.clone();
        assert_eq!(
            imbalanced.validate(),
            Err(OrderbookValidationError::SettlementImbalance {
                debited: two.clone(),
                credited_plus_fees: three.clone(),
            })
        );
        let trade = snapshot_trade();
        let mut channel = snapshot_channel(&trade);
        channel.xor_locked = two.clone();
        channel.initial_xor_locked = two.clone();
        channel.initial_fee_xor_locked = XorQuantity::zero();
        channel.remaining_fee_xor_locked = XorQuantity::zero();
        let mut overdraw = receipt();
        overdraw.xor_debited = three.clone();
        overdraw.provider_credit = two;
        overdraw.fee_amount = one;
        assert_eq!(
            apply_settlement_receipt_v1(&channel, &overdraw),
            Err(OrderbookValidationError::ReceiptExceedsEscrow {
                debited: three,
                escrow: channel.xor_locked,
            })
        );
    }
    #[test]
    fn channel_rejects_closed_state_with_remaining_bytes() {
        let mut channel = snapshot_channel(&snapshot_trade());
        channel.status = SettlementChannelStatusV1::Closed;
        assert_eq!(
            channel.validate(),
            Err(OrderbookValidationError::ClosedChannelHasRemainingBytes {
                remaining_bytes: 2 * BYTES_PER_GIB,
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
            xor_locked: XorQuantity::try_from_micro(100)
                .expect("legacy micro-XOR value is representable"),
            initial_xor_locked: XorQuantity::try_from_micro(100)
                .expect("legacy micro-XOR value is representable"),
            initial_fee_xor_locked: XorQuantity::try_from_micro(10)
                .expect("legacy micro-XOR value is representable"),
            remaining_fee_xor_locked: XorQuantity::try_from_micro(10)
                .expect("legacy micro-XOR value is representable"),
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
            xor_debited: XorQuantity::try_from_micro(100)
                .expect("legacy micro-XOR value is representable"),
            provider_credit: XorQuantity::try_from_micro(90)
                .expect("legacy micro-XOR value is representable"),
            fee_amount: XorQuantity::try_from_micro(10)
                .expect("legacy micro-XOR value is representable"),
            issued_at_unix: 1_800_000_200,
            settlement_signature: signature(),
        };
        let updated =
            apply_settlement_receipt_v1(&channel, &receipt).expect("receipt should apply");
        assert_eq!(updated.remaining_bytes, 0);
        assert_eq!(updated.xor_locked, XorQuantity::zero());
        assert_eq!(updated.status, SettlementChannelStatusV1::Closed);
        assert_eq!(updated.updated_at_unix, receipt.issued_at_unix);
        assert_eq!(updated.validate(), Ok(()));
    }
    #[test]
    fn deterministic_settlement_split_preserves_zero_fee_channels() {
        let total = XorQuantity::try_from_micro(101).expect("channel total");
        let split = deterministic_settlement_split_v1(&total, &XorQuantity::zero(), 3, 10)
            .expect("zero-fee split");
        assert_eq!(split.fee_amount, XorQuantity::zero());
        assert_eq!(split.provider_credit, split.xor_debited);
        assert!(split.xor_debited < total);
    }
    #[test]
    fn chunked_settlement_prorates_fee_and_consumes_final_rounding_dust_exactly() {
        let initial_total = XorQuantity::try_from_micro(101).expect("channel total");
        let initial_fee = XorQuantity::try_from_micro(10).expect("channel fee");
        let mut channel = SettlementChannelV1 {
            version: SETTLEMENT_CHANNEL_VERSION_V1,
            channel_id: id(5),
            trade_id: id(4),
            buyer_account: account(8),
            provider_id: id(6),
            total_bytes: 3,
            remaining_bytes: 3,
            xor_locked: initial_total.clone(),
            initial_xor_locked: initial_total.clone(),
            initial_fee_xor_locked: initial_fee.clone(),
            remaining_fee_xor_locked: initial_fee.clone(),
            status: SettlementChannelStatusV1::Open,
            opened_at_unix: 1_800_000_100,
            updated_at_unix: 1_800_000_100,
        };
        let mut total_debited = XorQuantity::zero();
        let mut total_provider_credit = XorQuantity::zero();
        let mut total_fees = XorQuantity::zero();
        for index in 0_u64..3 {
            let split = deterministic_settlement_split_v1(
                &channel.xor_locked,
                &channel.remaining_fee_xor_locked,
                1,
                channel.remaining_bytes,
            )
            .expect("derive sequential split");
            let receipt = SettlementReceiptV1 {
                version: SETTLEMENT_RECEIPT_VERSION_V1,
                receipt_id: id(u8::try_from(20 + index).expect("receipt id seed")),
                channel_id: channel.channel_id,
                trade_id: channel.trade_id,
                range: ByteRangeV1 {
                    start: index,
                    end: index + 1,
                },
                chunk_hash: id(u8::try_from(30 + index).expect("chunk id seed")),
                bytes_delivered: 1,
                xor_debited: split.xor_debited.clone(),
                provider_credit: split.provider_credit.clone(),
                fee_amount: split.fee_amount.clone(),
                issued_at_unix: 1_800_000_200 + index,
                settlement_signature: signature(),
            };
            total_debited = total_debited
                .checked_add(&split.xor_debited)
                .expect("sum debits");
            total_provider_credit = total_provider_credit
                .checked_add(&split.provider_credit)
                .expect("sum provider credits");
            total_fees = total_fees.checked_add(&split.fee_amount).expect("sum fees");
            channel =
                apply_settlement_receipt_v1(&channel, &receipt).expect("apply sequential receipt");
        }
        assert_eq!(total_debited, initial_total);
        assert_eq!(total_fees, initial_fee);
        assert_eq!(
            total_provider_credit
                .checked_add(&total_fees)
                .expect("provider plus fees"),
            total_debited,
        );
        assert_eq!(channel.remaining_bytes, 0);
        assert_eq!(channel.xor_locked, XorQuantity::zero());
        assert_eq!(channel.remaining_fee_xor_locked, XorQuantity::zero());
        assert_eq!(channel.status, SettlementChannelStatusV1::Closed);
    }
    #[test]
    fn settlement_rejects_balanced_but_inflated_receipt_amounts() {
        let channel = SettlementChannelV1 {
            version: SETTLEMENT_CHANNEL_VERSION_V1,
            channel_id: id(5),
            trade_id: id(4),
            buyer_account: account(8),
            provider_id: id(6),
            total_bytes: 10,
            remaining_bytes: 10,
            xor_locked: XorQuantity::try_from_micro(100).expect("channel total"),
            initial_xor_locked: XorQuantity::try_from_micro(100).expect("channel total"),
            initial_fee_xor_locked: XorQuantity::try_from_micro(10).expect("channel fee"),
            remaining_fee_xor_locked: XorQuantity::try_from_micro(10).expect("channel fee"),
            status: SettlementChannelStatusV1::Open,
            opened_at_unix: 1_800_000_100,
            updated_at_unix: 1_800_000_100,
        };
        let expected = deterministic_settlement_split_v1(
            &channel.xor_locked,
            &channel.remaining_fee_xor_locked,
            5,
            channel.remaining_bytes,
        )
        .expect("expected split");
        let inflation = XorQuantity::try_from_micro(1).expect("inflation");
        let inflated_debit = expected
            .xor_debited
            .checked_add(&inflation)
            .expect("inflated debit");
        let inflated_provider_credit = expected
            .provider_credit
            .checked_add(&inflation)
            .expect("inflated provider credit");
        let receipt = SettlementReceiptV1 {
            version: SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id: id(7),
            channel_id: channel.channel_id,
            trade_id: channel.trade_id,
            range: ByteRangeV1 { start: 0, end: 5 },
            chunk_hash: id(8),
            bytes_delivered: 5,
            xor_debited: inflated_debit,
            provider_credit: inflated_provider_credit,
            fee_amount: expected.fee_amount,
            issued_at_unix: 1_800_000_200,
            settlement_signature: signature(),
        };
        assert!(matches!(
            apply_settlement_receipt_v1(&channel, &receipt),
            Err(OrderbookValidationError::SettlementSplitMismatch { .. })
        ));
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
            xor_locked: XorQuantity::try_from_micro(100)
                .expect("legacy micro-XOR value is representable"),
            initial_xor_locked: XorQuantity::try_from_micro(100)
                .expect("legacy micro-XOR value is representable"),
            initial_fee_xor_locked: XorQuantity::try_from_micro(10)
                .expect("legacy micro-XOR value is representable"),
            remaining_fee_xor_locked: XorQuantity::try_from_micro(10)
                .expect("legacy micro-XOR value is representable"),
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
            xor_debited: XorQuantity::try_from_micro(100)
                .expect("legacy micro-XOR value is representable"),
            provider_credit: XorQuantity::try_from_micro(90)
                .expect("legacy micro-XOR value is representable"),
            fee_amount: XorQuantity::try_from_micro(10)
                .expect("legacy micro-XOR value is representable"),
            issued_at_unix: 1_800_000_200,
            settlement_signature: signature(),
        };
        assert_eq!(
            apply_settlement_receipt_v1(&channel, &receipt),
            Err(OrderbookValidationError::SettlementChannelMismatch)
        );
    }
    #[test]
    fn bounded_decoders_accept_exact_canonical_orderbook_archives() {
        let order = order();
        let encoded = norito::to_bytes(&order).expect("encode order");
        assert_eq!(
            decode_order_request_v1(&encoded).expect("decode canonical order"),
            order
        );
    }
    #[test]
    fn bounded_decoder_rejects_oversized_archive_before_decode() {
        let archive = vec![0_u8; ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1 + 1];
        assert_eq!(
            decode_order_request_v1(&archive),
            Err(OrderbookPayloadDecodeError::PayloadTooLarge {
                length: archive.len(),
                maximum: ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1,
            })
        );
    }
    #[test]
    fn bounded_decoder_rejects_noncanonical_trailing_bytes() {
        let mut encoded = norito::to_bytes(&order()).expect("encode order");
        encoded.push(0);
        assert_eq!(
            decode_order_request_v1(&encoded),
            Err(OrderbookPayloadDecodeError::NonCanonicalEncoding)
        );
    }
    #[test]
    fn caller_decode_budget_cannot_be_loosened_or_bypassed() {
        let encoded = norito::to_bytes(&order()).expect("encode order");
        let no_allocation = norito::DecodeLimits::new(
            ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1.max_sequence_elements(),
            ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1.max_field_bytes(),
            ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1.max_total_elements(),
            0,
            ORDERBOOK_PAYLOAD_DECODE_LIMITS_V1.max_nesting_depth(),
        );
        assert_eq!(
            decode_order_request_v1_with_limits(&encoded, no_allocation),
            Err(OrderbookPayloadDecodeError::DecodeResourceLimit)
        );
    }
}
