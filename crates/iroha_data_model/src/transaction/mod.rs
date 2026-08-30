//! Transaction structures and related implementations.
/// Error types surfaced by transaction validation and execution.
pub mod error;
/// Executable payloads backing transactions and triggers.
pub mod executable;
/// Transaction submission receipts.
pub mod receipt;
/// Signed transaction forms and helpers.
pub mod signed;
pub use crate::trigger::{DataTriggerSequence, DataTriggerStep, TimeTriggerEntrypoint};
pub use executable::{
    Executable, ExecutableBatchItem, IvmBytecode, TransactionGasLimitError,
    parse_transaction_gas_limit, require_transaction_gas_limit,
};
pub use receipt::{
    TX_SUBMISSION_RECEIPT_DOMAIN, TransactionSubmissionReceipt, TransactionSubmissionReceiptPayload,
};
pub use signed::{
    AuthorityFeePayment, DEFAULT_TRANSACTION_TIME_TO_LIVE, ExecutionStep, FeeChargeKind,
    FeeChargeLimit, FeePaymentIntent, FeePaymentIntentError, SignedTransaction, SponsorFeePayment,
    TransactionAdmissionIntent, TransactionBuilder, TransactionDomain, TransactionEntrypoint,
    TransactionPayload, TransactionResult, TransactionResultInner, TransactionSignature,
};
/// Metadata key enabling consensus-owned consume-once handling for prepared faucet claims.
pub const FAUCET_CLAIM_MARKER_VERSION_METADATA_KEY: &str = "taira_faucet_claim_marker_version";
/// Initial consensus-owned prepared-faucet claim marker version.
pub const FAUCET_CLAIM_MARKER_VERSION_V1: u64 = 1;
/// Metadata key identifying the prepared-operation family.
pub const PREPARED_OPERATION_METADATA_KEY: &str = "taira_prepared_operation";
/// Exact prepared-operation value for faucet transactions.
pub const PREPARED_FAUCET_OPERATION: &str = "faucet";
/// Metadata key carrying the domain-separated semantic faucet claim hash.
pub const PREPARED_SEMANTIC_HASH_METADATA_KEY: &str = "taira_prepared_semantic_hash";
/// The prelude re-exports most commonly used traits, structs and macros from this module.
pub mod prelude {
    pub use super::{
        AuthorityFeePayment, DataTriggerSequence, DataTriggerStep, Executable, ExecutableBatchItem,
        ExecutionStep, FeeChargeKind, FeeChargeLimit, FeePaymentIntent, FeePaymentIntentError,
        IvmBytecode, SignedTransaction, SponsorFeePayment, TX_SUBMISSION_RECEIPT_DOMAIN,
        TimeTriggerEntrypoint, TransactionAdmissionIntent, TransactionBuilder, TransactionDomain,
        TransactionEntrypoint, TransactionGasLimitError, TransactionPayload, TransactionResult,
        TransactionResultInner, TransactionSignature, TransactionSubmissionReceipt,
        TransactionSubmissionReceiptPayload, error::prelude::*, parse_transaction_gas_limit,
        require_transaction_gas_limit,
    };
}
