//! Transaction structures and related implementations.

/// Error types surfaced by transaction validation and execution.
pub mod error;
/// Executable payloads backing transactions and triggers.
pub mod executable;
/// Authority-free private Kaigi transaction forms.
pub mod private_kaigi;
/// Transaction submission receipts.
pub mod receipt;
/// Signed transaction forms and helpers.
pub mod signed;

pub use executable::{
    Executable, IvmBytecode, IvmProved, TransactionGasLimitError, parse_transaction_gas_limit,
    require_transaction_gas_limit,
};
pub use private_kaigi::{
    PrivateCreateKaigi, PrivateEndKaigi, PrivateJoinKaigi, PrivateKaigiAction,
    PrivateKaigiArtifacts, PrivateKaigiFeeSpend, PrivateKaigiTemplate, PrivateKaigiTransaction,
};
pub use receipt::{
    TX_SUBMISSION_RECEIPT_DOMAIN, TransactionSubmissionReceipt, TransactionSubmissionReceiptPayload,
};
pub use signed::{
    AuthorityFeePayment, ExecutionStep, FeeChargeKind, FeeChargeLimit, FeePaymentIntent,
    FeePaymentIntentError, SignedTransaction, SponsorFeePayment, TransactionBuilder,
    TransactionEntrypoint, TransactionPayload, TransactionResult, TransactionResultInner,
    TransactionSignature,
};

pub use crate::trigger::{DataTriggerSequence, DataTriggerStep, TimeTriggerEntrypoint};

/// The prelude re-exports most commonly used traits, structs and macros from this module.
pub mod prelude {
    pub use super::{
        AuthorityFeePayment, DataTriggerSequence, DataTriggerStep, Executable, ExecutionStep,
        FeeChargeKind, FeeChargeLimit, FeePaymentIntent, FeePaymentIntentError, IvmBytecode,
        IvmProved, PrivateCreateKaigi, PrivateEndKaigi, PrivateJoinKaigi, PrivateKaigiAction,
        PrivateKaigiArtifacts, PrivateKaigiFeeSpend, PrivateKaigiTemplate, PrivateKaigiTransaction,
        SignedTransaction, SponsorFeePayment, TX_SUBMISSION_RECEIPT_DOMAIN, TimeTriggerEntrypoint,
        TransactionBuilder, TransactionEntrypoint, TransactionGasLimitError, TransactionPayload,
        TransactionResult, TransactionResultInner, TransactionSignature,
        TransactionSubmissionReceipt, TransactionSubmissionReceiptPayload, error::prelude::*,
        parse_transaction_gas_limit, require_transaction_gas_limit,
    };
}
