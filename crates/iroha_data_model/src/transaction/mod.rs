//! Transaction structures and related implementations.

/// Error types surfaced by transaction validation and execution.
pub mod error;
/// Executable payloads backing transactions and triggers.
pub mod executable;
/// Transaction submission receipts.
pub mod receipt;
/// Signed transaction forms and helpers.
pub mod signed;

pub use executable::{
    Executable, ExecutableBatchItem, IvmBytecode, IvmProved, TransactionGasLimitError,
    parse_transaction_gas_limit, require_transaction_gas_limit,
};
pub use receipt::{
    TX_SUBMISSION_RECEIPT_DOMAIN, TransactionSubmissionReceipt, TransactionSubmissionReceiptPayload,
};
pub use signed::{
    AuthorityFeePayment, DEFAULT_TRANSACTION_TIME_TO_LIVE, ExecutionStep, FeeChargeKind,
    FeeChargeLimit, FeePaymentIntent, FeePaymentIntentError, SignedTransaction, SponsorFeePayment,
    TransactionBuilder, TransactionDomain, TransactionEntrypoint, TransactionPayload,
    TransactionResult, TransactionResultInner, TransactionSignature,
};

pub use crate::trigger::{DataTriggerSequence, DataTriggerStep, TimeTriggerEntrypoint};

/// The prelude re-exports most commonly used traits, structs and macros from this module.
pub mod prelude {
    pub use super::{
        AuthorityFeePayment, DataTriggerSequence, DataTriggerStep, Executable, ExecutableBatchItem,
        ExecutionStep, FeeChargeKind, FeeChargeLimit, FeePaymentIntent, FeePaymentIntentError,
        IvmBytecode, IvmProved, SignedTransaction, SponsorFeePayment, TX_SUBMISSION_RECEIPT_DOMAIN,
        TimeTriggerEntrypoint, TransactionBuilder, TransactionDomain, TransactionEntrypoint,
        TransactionGasLimitError, TransactionPayload, TransactionResult, TransactionResultInner,
        TransactionSignature, TransactionSubmissionReceipt, TransactionSubmissionReceiptPayload,
        error::prelude::*, parse_transaction_gas_limit, require_transaction_gas_limit,
    };
}
