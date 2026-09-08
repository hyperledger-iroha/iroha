//! Canonical structured errors returned by the Rust SDK.

use iroha_crypto::PublicKey;
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    transaction::{TransactionDomain, signed::TransactionSignatureError},
};
use thiserror::Error;

/// Canonical result returned by structured Rust SDK operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Canonical structured Rust SDK error family.
///
/// Operations that have not yet migrated to this family remain an explicit
/// first-release redesign gap; new structured operation errors belong here.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum Error {
    /// A request cannot be represented or violates a local operation contract.
    #[error("{operation} request is invalid: {details}")]
    InvalidRequest {
        /// Canonical operation whose request failed.
        operation: &'static str,
        /// Local validation diagnostic.
        details: String,
    },
    /// Canonical HTTP request authentication could not be produced.
    #[error("{operation} request signing failed: {details}")]
    RequestSigning {
        /// Canonical operation whose authentication failed.
        operation: &'static str,
        /// Signing diagnostic without private key material.
        details: String,
    },
    /// One HTTP dispatch or response read failed.
    #[error("{operation} transport failed: {details}")]
    Transport {
        /// Canonical operation whose dispatch failed.
        operation: &'static str,
        /// Transport diagnostic.
        details: String,
    },
    /// The operation deadline expired.
    #[error("{operation} timed out")]
    Timeout {
        /// Canonical operation whose deadline expired.
        operation: &'static str,
    },
    /// Torii returned a non-successful HTTP response.
    #[error("{operation} returned HTTP {status}")]
    Http {
        /// Canonical operation rejected by Torii.
        operation: &'static str,
        /// HTTP status code.
        status: u16,
        /// Exact bounded API response body, including machine-readable error fields.
        body: Vec<u8>,
    },
    /// The response cannot be decoded under the operation's canonical schema.
    #[error("{operation} response decoding failed: {details}")]
    Decode {
        /// Canonical operation whose response failed.
        operation: &'static str,
        /// Codec or content-negotiation diagnostic.
        details: String,
    },
    /// A returned draft or read result does not match the exact request.
    #[error("{operation} response does not bind {field}")]
    ResponseBinding {
        /// Canonical operation whose response is inconsistent.
        operation: &'static str,
        /// Exact field or invariant that failed verification.
        field: &'static str,
    },
    /// A transport attempted to retain more than the configured response bound.
    #[error("HTTP response exceeds the {maximum} byte limit")]
    ResponseTooLarge {
        /// Maximum permitted response bytes.
        maximum: usize,
        /// Observed length, when representable and available.
        actual: Option<usize>,
    },
    /// The explicit blocking facade cannot run in the caller's runtime.
    #[error(transparent)]
    Blocking(#[from] crate::blocking::BlockingCallError),
    /// The owned blocking runtime could not be constructed.
    #[error("blocking runtime construction failed: {details}")]
    BlockingRuntimeConstruction {
        /// Runtime builder diagnostic.
        details: String,
    },
    /// Local transaction preparation failed.
    #[error("transaction preparation failed: {0}")]
    TransactionPreparation(#[from] TransactionPreparationError),
    /// Local transaction signing failed.
    #[error("transaction signing failed: {0}")]
    TransactionSigning(#[from] TransactionSigningError),
}

/// Typed cause of a local account transaction preparation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TransactionPreparationError {
    /// The operating system entropy source failed while producing a nonce.
    #[error("nonce entropy source failed: {details}")]
    NonceEntropy {
        /// Complete entropy-source diagnostic.
        details: String,
    },
    /// Sixteen consecutive entropy samples produced the invalid zero nonce.
    #[error("nonce entropy source returned zero repeatedly")]
    NonceEntropyExhausted,
    /// The prepared payload violated a signature-bound transaction invariant.
    #[error("payload invariant failed: {source}")]
    InvalidPayload {
        /// Canonical data-model validation cause.
        #[source]
        source: TransactionSignatureError,
    },
}

/// Typed cause of a local account transaction signing failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TransactionSigningError {
    /// A multisignature member context cannot create a complete direct signature.
    #[error("multisignature account `{authority}` requires an external threshold signature")]
    DirectSigningUnavailable {
        /// Bound multisignature authority.
        authority: AccountId,
    },
    /// The payload belongs to a different network or to the genesis-only domain.
    #[error("signing domain `{actual:?}` does not match network `{expected}`")]
    NetworkMismatch {
        /// Network bound to the immutable account context.
        expected: NetworkId,
        /// Domain carried by the exact payload.
        actual: TransactionDomain,
    },
    /// The payload authority differs from the immutable account context.
    #[error("authority `{actual}` does not match bound account `{expected}`")]
    AuthorityMismatch {
        /// Authority bound to the immutable account context.
        expected: AccountId,
        /// Authority carried by the exact payload.
        actual: AccountId,
    },
    /// Direct signing requires a single-key payload authority.
    #[error("authority `{authority}` cannot receive a direct signature")]
    AuthorityNotDirect {
        /// Non-direct authority carried by the payload.
        authority: AccountId,
    },
    /// The payload's direct controller differs from the context signing key.
    #[error("signing key does not control the exact payload authority")]
    KeyMismatch {
        /// Public key required by the payload authority.
        expected: PublicKey,
        /// Public key owned by the immutable account context.
        actual: PublicKey,
    },
    /// The supplied exact payload could not be reconstructed for signing.
    #[error("payload is not signable: {source}")]
    InvalidPayload {
        /// Canonical data-model validation cause.
        #[source]
        source: TransactionSignatureError,
    },
    /// The cryptographic signing backend rejected the payload.
    #[error("signature backend failed: {source}")]
    Signature {
        /// Canonical data-model signing cause.
        #[source]
        source: TransactionSignatureError,
    },
}
