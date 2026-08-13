//! Strict, route-aware validation for SoraFS orderbook transaction submission.

use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey};
use iroha_version::codec::{DecodeVersioned as _, EncodeVersioned as _};
use thiserror::Error;

use crate::{
    NetworkId,
    account::AccountId,
    isi::sorafs::{
        CancelSorafsOrderbookOrder, RecordSorafsOrderbookSettlementReceipt,
        SubmitSorafsOrderbookOrder,
    },
    transaction::{
        Executable, SignedTransaction, TransactionEntrypoint, TransactionSubmissionReceipt,
    },
};
use sorafs_manifest::{
    OrderbookSignatureV1, decode_order_cancel_v1, decode_order_request_v1,
    decode_settlement_receipt_v1, verify_order_cancel_signature_v1,
    verify_order_request_signature_v1, verify_settlement_receipt_signature_v1,
};

/// Hard ceiling for one exact canonical versioned signed orderbook transaction.
pub const ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1: usize = 2 * 1024 * 1024;
/// Hard ceiling for one canonical transaction-submission receipt.
pub const ORDERBOOK_SUBMISSION_RECEIPT_MAX_CANONICAL_BYTES_V1: usize = 1024 * 1024;

/// The native instruction admitted by one orderbook submission route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SorafsOrderbookSubmissionRouteV1 {
    /// `POST /v1/sorafs/orderbook/orders`.
    SubmitOrder,
    /// `POST /v1/sorafs/orderbook/cancel`.
    CancelOrder,
    /// `POST /v1/sorafs/orderbook/receipts`.
    RecordReceipt,
}

impl SorafsOrderbookSubmissionRouteV1 {
    /// Parse the stable SDK route label.
    pub fn parse_sdk_label(label: &str) -> Result<Self, SorafsOrderbookSubmissionValidationError> {
        match label {
            "order" => Ok(Self::SubmitOrder),
            "cancel" => Ok(Self::CancelOrder),
            "receipt" => Ok(Self::RecordReceipt),
            _ => Err(SorafsOrderbookSubmissionValidationError::UnsupportedRoute),
        }
    }
}

/// Identities Torii places in the signed receipt and response headers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SorafsOrderbookSubmissionIdentityV1 {
    /// Legacy transaction identity, typed as a signed transaction hash.
    pub tx_hash: HashOf<SignedTransaction>,
    /// Canonical entrypoint identity.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Canonical signed-transaction identity.
    pub signed_transaction_hash: HashOf<SignedTransaction>,
}

/// Strict orderbook submission or receipt validation failure.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SorafsOrderbookSubmissionValidationError {
    /// The caller supplied an empty transaction wire.
    #[error("signed orderbook transaction must not be empty")]
    EmptyTransaction,
    /// The caller supplied a transaction larger than the V1 ingress ceiling.
    #[error("signed orderbook transaction exceeds the canonical V1 byte bound")]
    TransactionTooLarge,
    /// The bytes are not one current versioned signed transaction.
    #[error("signed orderbook transaction is not a valid current versioned wire")]
    InvalidTransactionEncoding,
    /// Re-encoding did not reproduce the caller's exact bytes.
    #[error("signed orderbook transaction is not the exact canonical versioned wire")]
    NonCanonicalTransaction,
    /// The authority signature is missing or invalid.
    #[error("signed orderbook transaction has an invalid authority signature")]
    InvalidTransactionSignature,
    /// The transaction belongs to a different application-pinned network.
    #[error("signed orderbook transaction network does not match the expected network")]
    NetworkMismatch,
    /// The executable is not a native instruction list.
    #[error("signed orderbook transaction must contain one native instruction")]
    NonInstructionExecutable,
    /// The native instruction list does not contain exactly one item.
    #[error("signed orderbook transaction must contain exactly one native instruction")]
    NonSingletonInstruction,
    /// The sole instruction does not match the selected endpoint.
    #[error("signed orderbook transaction instruction does not match the selected route")]
    RouteMismatch,
    /// The embedded order, cancellation, or settlement receipt is invalid.
    #[error("signed orderbook transaction contains an invalid embedded payload")]
    InvalidEmbeddedPayload,
    /// The embedded order/cancellation owner or signer differs from transaction authority.
    #[error("signed orderbook payload owner or signer does not match transaction authority")]
    EmbeddedPayloadAuthorityMismatch,
    /// The SDK route label is not one of the three stable V1 labels.
    #[error("unsupported SoraFS orderbook submission route")]
    UnsupportedRoute,
    /// The caller supplied an empty receipt wire.
    #[error("transaction submission receipt must not be empty")]
    EmptyReceipt,
    /// The receipt exceeds the SDK's bounded response ceiling.
    #[error("transaction submission receipt exceeds the canonical V1 byte bound")]
    ReceiptTooLarge,
    /// The body is not a transaction submission receipt.
    #[error("invalid transaction submission receipt encoding")]
    InvalidReceiptEncoding,
    /// Re-encoding did not reproduce the exact response bytes.
    #[error("transaction submission receipt is not the exact canonical wire")]
    NonCanonicalReceipt,
    /// The receipt signature is invalid for its embedded signer.
    #[error("transaction submission receipt signature is invalid")]
    InvalidReceiptSignature,
    /// The embedded signer does not match the caller's trust anchor.
    #[error("transaction submission receipt signer does not match the expected signer")]
    ReceiptSignerMismatch,
    /// The legacy transaction identity does not match the submitted wire.
    #[error("transaction submission receipt tx_hash does not match the submitted transaction")]
    ReceiptTransactionHashMismatch,
    /// The entrypoint identity does not match the submitted wire.
    #[error(
        "transaction submission receipt entrypoint_hash does not match the submitted transaction"
    )]
    ReceiptEntrypointHashMismatch,
    /// The signed-transaction identity is absent or does not match the submitted wire.
    #[error(
        "transaction submission receipt signed_transaction_hash does not match the submitted transaction"
    )]
    ReceiptSignedTransactionHashMismatch,
}

/// Decode and validate one exact caller-signed orderbook transaction before HTTP.
pub fn inspect_sorafs_orderbook_submission_v1(
    bytes: &[u8],
    route: SorafsOrderbookSubmissionRouteV1,
    expected_network: &NetworkId,
) -> Result<SorafsOrderbookSubmissionIdentityV1, SorafsOrderbookSubmissionValidationError> {
    if bytes.is_empty() {
        return Err(SorafsOrderbookSubmissionValidationError::EmptyTransaction);
    }
    if bytes.len() > ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1 {
        return Err(SorafsOrderbookSubmissionValidationError::TransactionTooLarge);
    }
    let transaction = SignedTransaction::decode_all_versioned(bytes)
        .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidTransactionEncoding)?;
    if transaction.encode_versioned() != bytes {
        return Err(SorafsOrderbookSubmissionValidationError::NonCanonicalTransaction);
    }
    if transaction.network_id() != Some(expected_network) {
        return Err(SorafsOrderbookSubmissionValidationError::NetworkMismatch);
    }
    let canonical_framed = norito::to_bytes(&transaction)
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
    match route {
        SorafsOrderbookSubmissionRouteV1::SubmitOrder => {
            let submit = instruction
                .as_any()
                .downcast_ref::<SubmitSorafsOrderbookOrder>()
                .ok_or(SorafsOrderbookSubmissionValidationError::RouteMismatch)?;
            let order = decode_order_request_v1(&submit.order_payload)
                .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload)?;
            verify_order_request_signature_v1(&order)
                .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload)?;
            require_owner_authority(&transaction, &order.owner_account, &order.signature)?;
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
            require_owner_authority(&transaction, &payload.owner_account, &payload.signature)?;
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
        }
    }

    let entrypoint_hash = transaction.hash_as_entrypoint();
    let tx_hash =
        HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::from(entrypoint_hash.clone()));
    let signed_transaction_hash = transaction.hash();
    Ok(SorafsOrderbookSubmissionIdentityV1 {
        tx_hash,
        entrypoint_hash,
        signed_transaction_hash,
    })
}

fn require_owner_authority(
    transaction: &SignedTransaction,
    owner_account: &[u8],
    signature: &OrderbookSignatureV1,
) -> Result<(), SorafsOrderbookSubmissionValidationError> {
    let owner_literal = std::str::from_utf8(owner_account)
        .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload)?;
    let parsed = AccountId::parse_encoded(owner_literal)
        .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidEmbeddedPayload)?;
    if parsed.canonical().as_bytes() != owner_account
        || parsed.account_id().subject_id() != transaction.authority().subject_id()
    {
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
    let receipt: TransactionSubmissionReceipt = norito::decode_from_bytes(bytes)
        .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidReceiptEncoding)?;
    if norito::to_bytes(&receipt)
        .map_err(|_| SorafsOrderbookSubmissionValidationError::InvalidReceiptEncoding)?
        != bytes
    {
        return Err(SorafsOrderbookSubmissionValidationError::NonCanonicalReceipt);
    }
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
