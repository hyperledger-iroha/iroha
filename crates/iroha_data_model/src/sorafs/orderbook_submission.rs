//! Strict, route-aware validation for SoraFS orderbook transaction submission.
use crate::{
    NetworkId,
    account::AccountAddress,
    isi::sorafs::{
        CancelSorafsOrderbookOrder, RecordSorafsOrderbookSettlementReceipt,
        SubmitSorafsOrderbookOrder,
    },
    transaction::{
        Executable, SignedTransaction, TransactionEntrypoint, TransactionSubmissionReceipt,
    },
};
use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey};
use iroha_version::codec::DecodeVersioned as _;
use sorafs_manifest::{
    ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1, OrderCancelReasonV1, OrderCancelV1, OrderRequestV1,
    OrderSideV1, OrderTierV1, OrderbookSignatureV1, OrderbookValidationPayloadKindV1,
    SettlementReceiptV1, XorQuantity, decode_order_cancel_v1, decode_order_request_v1,
    decode_settlement_receipt_v1, verify_order_cancel_signature_v1,
    verify_order_request_signature_v1, verify_settlement_receipt_signature_v1,
};
use std::str::FromStr;
use thiserror::Error;
/// Hard ceiling for one exact canonical versioned signed orderbook transaction.
pub const ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1: usize = 2 * 1024 * 1024;
/// Hard ceiling for one canonical transaction-submission receipt.
pub const ORDERBOOK_SUBMISSION_RECEIPT_MAX_CANONICAL_BYTES_V1: usize = 1024 * 1024;
/// The native instruction admitted by one orderbook submission route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SorafsOrderbookSubmissionRouteV1 {
    /// Admit a signed order request.
    SubmitOrder,
    /// Admit a signed order cancellation.
    CancelOrder,
    /// Admit a signed settlement receipt.
    RecordReceipt,
}
impl SorafsOrderbookSubmissionRouteV1 {
    /// Parse the stable SDK route label.
    #[rustfmt::skip]
    pub fn parse_sdk_label(label: &str) -> Result<Self, SorafsOrderbookSubmissionValidationError> {
        match label { "order" => Ok(Self::SubmitOrder), "cancel" => Ok(Self::CancelOrder), "receipt" => Ok(Self::RecordReceipt), _ => Err(SorafsOrderbookSubmissionValidationError::UnsupportedRoute) }
    }
}
/// Identities Torii places in the signed receipt and response headers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SorafsOrderbookSubmissionIdentityV1 {
    /// Transaction identity exposed by the submission endpoint.
    pub tx_hash: HashOf<SignedTransaction>,
    /// Hash of the canonical transaction entrypoint.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Hash of the complete signed transaction.
    pub signed_transaction_hash: HashOf<SignedTransaction>,
}
fn parse_exact<T: FromStr + ToString>(literal: &str) -> Option<T> {
    let parsed = literal.parse::<T>().ok()?;
    (parsed.to_string() == literal).then_some(parsed)
}
/// Parse the three exact checksummed identity literals accepted from SDK boundaries.
#[rustfmt::skip]
pub fn parse_sorafs_orderbook_submission_identity_v1(
    tx_hash: &str,
    entrypoint_hash: &str,
    signed_transaction_hash: &str,
) -> Option<SorafsOrderbookSubmissionIdentityV1> {
    Some(SorafsOrderbookSubmissionIdentityV1 { tx_hash: parse_exact(tx_hash)?, entrypoint_hash: parse_exact(entrypoint_hash)?, signed_transaction_hash: parse_exact(signed_transaction_hash)? })
}
/// Parse one exact checksummed receipt signer literal accepted from SDK boundaries.
pub fn parse_sorafs_orderbook_receipt_signer_v1(literal: &str) -> Option<PublicKey> {
    parse_exact(literal)
}
/// Parse one exact reference-SDK payload label.
#[rustfmt::skip]
pub fn parse_sorafs_orderbook_payload_kind_v1(
    label: &str,
) -> Option<OrderbookValidationPayloadKindV1> {
    Some(match label {
        "order-request" => OrderbookValidationPayloadKindV1::OrderRequest, "order-cancel" => OrderbookValidationPayloadKindV1::OrderCancel,
        "trade-event" => OrderbookValidationPayloadKindV1::TradeEvent, "settlement-channel" => OrderbookValidationPayloadKindV1::SettlementChannel,
        "settlement-receipt" => OrderbookValidationPayloadKindV1::SettlementReceipt,
        _ => return None,
    })
}
/// Parse one exact order side label.
#[rustfmt::skip]
pub fn parse_sorafs_orderbook_side_v1(label: &str) -> Option<OrderSideV1> {
    match label { "bid" => Some(OrderSideV1::Bid), "ask" => Some(OrderSideV1::Ask), _ => None }
}
/// Parse one exact storage tier label.
#[rustfmt::skip]
pub fn parse_sorafs_orderbook_tier_v1(label: &str) -> Option<OrderTierV1> {
    match label { "hot" => Some(OrderTierV1::Hot), "warm" => Some(OrderTierV1::Warm), "archive" => Some(OrderTierV1::Archive), _ => None }
}
/// Parse a cancellation label with the binding's pinned owner-requested spelling.
#[rustfmt::skip]
pub fn parse_sorafs_orderbook_cancel_reason_v1(
    label: &str,
    owner_requested_label: &str,
) -> Option<OrderCancelReasonV1> {
    if label == owner_requested_label {
        return Some(OrderCancelReasonV1::OwnerRequested);
    }
    match label { "expired" => Some(OrderCancelReasonV1::Expired), "governance" => Some(OrderCancelReasonV1::Governance), "replaced" => Some(OrderCancelReasonV1::Replaced), _ => None }
}
/// Parse bounded unsigned decimal text for native SDK bindings.
#[rustfmt::skip]
pub fn parse_sorafs_orderbook_decimal_u64_v1(value: &str, context: &str) -> Result<u64, String> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) || (value.len() > 1 && value.starts_with('0')) {
        return Err(format!("{context} must use canonical unsigned decimal spelling"));
    }
    value.parse().map_err(|error| format!("{context} must be an unsigned 64-bit decimal integer: {error}"))
}
/// Parse exact bounded XOR quantity text for native SDK bindings.
#[rustfmt::skip]
pub fn parse_sorafs_orderbook_xor_quantity_v1(
    value: &str,
    context: &str,
) -> Result<XorQuantity, String> {
    if value.len() > 155 {
        return Err(format!("{context} exceeds the canonical XOR quantity text bound"));
    }
    let quantity: XorQuantity = value.parse().map_err(|error| format!("{context} must be a canonical non-negative XOR quantity: {error}"))?;
    if quantity.to_string() != value {
        return Err(format!("{context} must use canonical XOR quantity spelling"));
    }
    Ok(quantity)
}
/// Narrow one SDK integer to canonical fee basis points.
#[rustfmt::skip]
pub fn parse_sorafs_orderbook_fee_bps_v1(value: u32, context: &str) -> Result<u16, String> {
    value.try_into().map_err(|_| format!("{context} must fit in u16 basis points"))
}
/// Enforce the shared SDK owner byte bound before derivation or signing.
pub fn validate_sorafs_orderbook_owner_account_v1(owner_account: &[u8]) -> Result<(), String> {
    if owner_account.is_empty() {
        return Err("owner_account must not be empty".to_owned());
    }
    if owner_account.len() > ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 {
        return Err(format!(
            "owner_account must be at most {ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1} bytes"
        ));
    }
    Ok(())
}
/// Authenticated route command retained for finalized-state checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidatedSorafsOrderbookSubmissionCommandV1 {
    /// A validated order request and the policy digest it targets.
    SubmitOrder {
        /// Digest of the finalized orderbook policy targeted by the request.
        policy_digest: [u8; 32],
        /// Decoded and signature-verified order request.
        order: OrderRequestV1,
    },
    /// A validated cancellation and the policy digest it targets.
    CancelOrder {
        /// Digest of the finalized orderbook policy targeted by the cancellation.
        policy_digest: [u8; 32],
        /// Decoded and signature-verified cancellation.
        cancellation: OrderCancelV1,
    },
    /// A validated receipt and the policy digest it targets.
    RecordReceipt {
        /// Digest of the finalized orderbook policy targeted by the receipt.
        policy_digest: [u8; 32],
        /// Decoded and signature-verified settlement receipt.
        receipt: SettlementReceiptV1,
    },
}
/// Stateless validation output shared by Torii and strict SDK bindings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedSorafsOrderbookSubmissionV1 {
    /// Canonical identities derived from the submitted transaction.
    pub identity: SorafsOrderbookSubmissionIdentityV1,
    /// Validated route command and decoded embedded payload.
    pub command: ValidatedSorafsOrderbookSubmissionCommandV1,
}
/// Strict orderbook submission or receipt validation failure.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[rustfmt::skip]
pub enum SorafsOrderbookSubmissionValidationError {
    /// The caller supplied no transaction bytes.
    #[error("signed orderbook transaction must not be empty")] EmptyTransaction,
    /// The transaction exceeds the V1 ingress ceiling.
    #[error("signed orderbook transaction exceeds the canonical V1 byte bound")] TransactionTooLarge,
    /// The bytes do not contain a current versioned signed transaction.
    #[error("signed orderbook transaction is not a valid current versioned wire")] InvalidTransactionEncoding,
    /// The transaction bytes are not the exact canonical wire encoding.
    #[error("signed orderbook transaction is not the exact canonical versioned wire")] NonCanonicalTransaction,
    /// The transaction authority signature is invalid.
    #[error("signed orderbook transaction has an invalid authority signature")] InvalidTransactionSignature,
    /// The transaction targets a different network.
    #[error("signed orderbook transaction network does not match the expected network")] NetworkMismatch,
    /// The transaction executable is not a native instruction list.
    #[error("signed orderbook transaction must contain one native instruction")] NonInstructionExecutable,
    /// The transaction contains other than one instruction.
    #[error("signed orderbook transaction must contain exactly one native instruction")] NonSingletonInstruction,
    /// The instruction does not match the selected submission route.
    #[error("signed orderbook transaction instruction does not match the selected route")] RouteMismatch,
    /// The embedded orderbook payload is malformed or non-canonical.
    #[error("signed orderbook transaction contains an invalid embedded payload")] InvalidEmbeddedPayload,
    /// The embedded owner or signer differs from the transaction authority.
    #[error("signed orderbook payload owner or signer does not match transaction authority")] EmbeddedPayloadAuthorityMismatch,
    /// The SDK route label is not part of the V1 route inventory.
    #[error("unsupported SoraFS orderbook submission route")] UnsupportedRoute,
    /// The caller supplied no receipt bytes.
    #[error("transaction submission receipt must not be empty")] EmptyReceipt,
    /// The receipt exceeds the V1 response ceiling.
    #[error("transaction submission receipt exceeds the canonical V1 byte bound")] ReceiptTooLarge,
    /// The bytes do not contain a transaction-submission receipt.
    #[error("invalid transaction submission receipt encoding")] InvalidReceiptEncoding,
    /// The receipt bytes are not the exact canonical encoding.
    #[error("transaction submission receipt is not the exact canonical wire")] NonCanonicalReceipt,
    /// The receipt signature is invalid.
    #[error("transaction submission receipt signature is invalid")] InvalidReceiptSignature,
    /// The receipt signer differs from the expected trust anchor.
    #[error("transaction submission receipt signer does not match the expected signer")] ReceiptSignerMismatch,
    /// The receipt's transaction hash differs from the submitted transaction.
    #[error("transaction submission receipt tx_hash does not match the submitted transaction")] ReceiptTransactionHashMismatch,
    /// The receipt's entrypoint hash differs from the submitted transaction.
    #[error("transaction submission receipt entrypoint_hash does not match the submitted transaction")] ReceiptEntrypointHashMismatch,
    /// The receipt's signed-transaction hash differs from the submitted transaction.
    #[error("transaction submission receipt signed_transaction_hash does not match the submitted transaction")] ReceiptSignedTransactionHashMismatch,
}
/// Decode and validate one exact caller-signed orderbook transaction before HTTP.
#[deprecated(note = "use inspect_sorafs_orderbook_submission_for_discriminant_v1")]
#[rustfmt::skip]
pub fn inspect_sorafs_orderbook_submission_v1(
    bytes: &[u8], route: SorafsOrderbookSubmissionRouteV1, expected_network: &NetworkId,
) -> Result<SorafsOrderbookSubmissionIdentityV1, SorafsOrderbookSubmissionValidationError> {
    Ok(inspect_sorafs_orderbook_submission_for_discriminant_v1(bytes, route, expected_network, crate::account::address::chain_discriminant())?.identity)
}
/// Decode and validate using the deployment's explicit I105 discriminant.
pub fn inspect_sorafs_orderbook_submission_for_discriminant_v1(
    bytes: &[u8],
    route: SorafsOrderbookSubmissionRouteV1,
    expected_network: &NetworkId,
    expected_chain_discriminant: u16,
) -> Result<ValidatedSorafsOrderbookSubmissionV1, SorafsOrderbookSubmissionValidationError> {
    if bytes.is_empty() {
        return Err(SorafsOrderbookSubmissionValidationError::EmptyTransaction);
    }
    if bytes.len() > ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1 {
        return Err(SorafsOrderbookSubmissionValidationError::TransactionTooLarge);
    }
    let transaction = SignedTransaction::decode_all_versioned(bytes)
        .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidTransactionEncoding)?;
    if transaction
        .encode_wire_v1()
        .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidTransactionEncoding)?
        != bytes
    {
        return Err(SorafsOrderbookSubmissionValidationError::NonCanonicalTransaction);
    }
    validate_sorafs_orderbook_submission_transaction_v1(
        &transaction,
        route,
        expected_network,
        expected_chain_discriminant,
    )
}
/// Validate one decoded transaction without narrowing Torii's JSON ingress.
pub fn validate_sorafs_orderbook_submission_transaction_v1(
    transaction: &SignedTransaction,
    route: SorafsOrderbookSubmissionRouteV1,
    expected_network: &NetworkId,
    expected_chain_discriminant: u16,
) -> Result<ValidatedSorafsOrderbookSubmissionV1, SorafsOrderbookSubmissionValidationError> {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    if transaction.network_id() != Some(expected_network) {
        return Err(SorafsOrderbookSubmissionValidationError::NetworkMismatch);
    }
    let canonical_framed = norito::to_bytes(transaction)
        .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidTransactionEncoding)?;
    if canonical_framed.len() > ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1 {
        return Err(SorafsOrderbookSubmissionValidationError::TransactionTooLarge);
    }
    transaction
        .verify_signature()
        .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidTransactionSignature)?;
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(SorafsOrderbookSubmissionValidationError::NonInstructionExecutable);
    };
    if instructions.len() != 1 {
        return Err(SorafsOrderbookSubmissionValidationError::NonSingletonInstruction);
    }
    let instruction = &instructions[0];
    let command = match route {
        SorafsOrderbookSubmissionRouteV1::SubmitOrder => {
            let submit = instruction
                .as_any()
                .downcast_ref::<SubmitSorafsOrderbookOrder>()
                .ok_or(SorafsOrderbookSubmissionValidationError::RouteMismatch)?;
            let order = decode_order_request_v1(&submit.order_payload)
                .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload)?;
            verify_order_request_signature_v1(&order)
                .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload)?;
            require_owner_authority(
                &transaction,
                &order.owner_account,
                &order.signature,
                expected_chain_discriminant,
            )?;
            ValidatedSorafsOrderbookSubmissionCommandV1::SubmitOrder {
                policy_digest: submit.policy_digest,
                order,
            }
        }
        SorafsOrderbookSubmissionRouteV1::CancelOrder => {
            let cancel = instruction
                .as_any()
                .downcast_ref::<CancelSorafsOrderbookOrder>()
                .ok_or(SorafsOrderbookSubmissionValidationError::RouteMismatch)?;
            let payload = decode_order_cancel_v1(&cancel.cancel_payload)
                .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload)?;
            verify_order_cancel_signature_v1(&payload)
                .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload)?;
            require_owner_authority(
                &transaction,
                &payload.owner_account,
                &payload.signature,
                expected_chain_discriminant,
            )?;
            ValidatedSorafsOrderbookSubmissionCommandV1::CancelOrder {
                policy_digest: cancel.policy_digest,
                cancellation: payload,
            }
        }
        SorafsOrderbookSubmissionRouteV1::RecordReceipt => {
            let record = instruction
                .as_any()
                .downcast_ref::<RecordSorafsOrderbookSettlementReceipt>()
                .ok_or(SorafsOrderbookSubmissionValidationError::RouteMismatch)?;
            let payload = decode_settlement_receipt_v1(&record.receipt_payload)
                .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload)?;
            verify_settlement_receipt_signature_v1(&payload)
                .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload)?;
            ValidatedSorafsOrderbookSubmissionCommandV1::RecordReceipt {
                policy_digest: record.policy_digest,
                receipt: payload,
            }
        }
    };
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let tx_hash =
        HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::from(entrypoint_hash.clone()));
    let signed_transaction_hash = transaction.hash();
    Ok(ValidatedSorafsOrderbookSubmissionV1 {
        identity: SorafsOrderbookSubmissionIdentityV1 {
            tx_hash,
            entrypoint_hash,
            signed_transaction_hash,
        },
        command,
    })
}
fn require_owner_authority(
    transaction: &SignedTransaction,
    owner_account: &[u8],
    signature: &OrderbookSignatureV1,
    expected_chain_discriminant: u16,
) -> Result<(), SorafsOrderbookSubmissionValidationError> {
    let owner_literal = std::str::from_utf8(owner_account)
        .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload)?;
    let parsed = AccountAddress::parse_encoded(owner_literal, Some(expected_chain_discriminant))
        .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload)?;
    let owner = parsed
        .to_account_id()
        .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload)?;
    if owner
        .to_i105_for_discriminant(expected_chain_discriminant)
        .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload)?
        .as_bytes()
        != owner_account
    {
        return Err(SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload);
    }
    if owner.subject_id() != transaction.authority().subject_id() {
        return Err(SorafsOrderbookSubmissionValidationError::EmbeddedPayloadAuthorityMismatch);
    }
    let public_key = transaction
        .authority()
        .try_signatory()
        .ok_or(SorafsOrderbookSubmissionValidationError::EmbeddedPayloadAuthorityMismatch)?;
    let (algorithm, bytes) = public_key
        .try_to_bytes()
        .map_err(|_| SorafsOrderbookSubmissionValidationError::EmbeddedPayloadAuthorityMismatch)?;
    if algorithm != Algorithm::Ed25519 || bytes != signature.public_key.as_slice() {
        return Err(SorafsOrderbookSubmissionValidationError::EmbeddedPayloadAuthorityMismatch);
    }
    Ok(())
}
/// Decode, authenticate, trust-anchor, and identity-bind one exact receipt body.
pub fn decode_and_verify_sorafs_orderbook_submission_receipt_v1(
    bytes: &[u8],
    expected_identity: &SorafsOrderbookSubmissionIdentityV1,
    expected_receipt_signer: &PublicKey,
) -> Result<TransactionSubmissionReceipt, SorafsOrderbookSubmissionValidationError> {
    if bytes.is_empty() {
        return Err(SorafsOrderbookSubmissionValidationError::EmptyReceipt);
    }
    if bytes.len() > ORDERBOOK_SUBMISSION_RECEIPT_MAX_CANONICAL_BYTES_V1 {
        return Err(SorafsOrderbookSubmissionValidationError::ReceiptTooLarge);
    }
    let receipt: TransactionSubmissionReceipt =
        norito::decode_canonical(bytes).map_err(|error| {
            if matches!(error, norito::Error::NonCanonicalEncoding) {
                SorafsOrderbookSubmissionValidationError::NonCanonicalReceipt
            } else {
                SorafsOrderbookSubmissionValidationError::InvalidReceiptEncoding
            }
        })?;
    receipt
        .verify()
        .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidReceiptSignature)?;
    if &receipt.payload.signer != expected_receipt_signer {
        return Err(SorafsOrderbookSubmissionValidationError::ReceiptSignerMismatch);
    }
    if receipt.payload.tx_hash != expected_identity.tx_hash {
        return Err(SorafsOrderbookSubmissionValidationError::ReceiptTransactionHashMismatch);
    }
    if receipt.payload.entrypoint_hash != expected_identity.entrypoint_hash {
        return Err(SorafsOrderbookSubmissionValidationError::ReceiptEntrypointHashMismatch);
    }
    if receipt.payload.signed_transaction_hash.as_ref()
        != Some(&expected_identity.signed_transaction_hash)
    {
        return Err(SorafsOrderbookSubmissionValidationError::ReceiptSignedTransactionHashMismatch);
    }
    Ok(receipt)
}
#[cfg(test)]
#[path = "orderbook_submission_tests.rs"]
mod tests;
