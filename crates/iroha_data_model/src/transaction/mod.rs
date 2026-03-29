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

pub use executable::{Executable, IvmBytecode, IvmProved};
pub use private_kaigi::{
    PrivateCreateKaigi, PrivateEndKaigi, PrivateJoinKaigi, PrivateKaigiAction,
    PrivateKaigiArtifacts, PrivateKaigiFeeSpend, PrivateKaigiTemplate, PrivateKaigiTransaction,
};
pub use receipt::{
    TX_SUBMISSION_RECEIPT_DOMAIN, TransactionSubmissionReceipt, TransactionSubmissionReceiptPayload,
};
pub use signed::{
    ExecutionStep, SignedTransaction, TransactionBuilder, TransactionEntrypoint, TransactionResult,
    TransactionResultInner, TransactionSignature,
};

pub use crate::trigger::{DataTriggerSequence, DataTriggerStep, TimeTriggerEntrypoint};

/// The prelude re-exports most commonly used traits, structs and macros from this module.
pub mod prelude {
    pub use super::{
        DataTriggerSequence, DataTriggerStep, Executable, ExecutionStep, IvmBytecode, IvmProved,
        PrivateCreateKaigi, PrivateEndKaigi, PrivateJoinKaigi, PrivateKaigiAction,
        PrivateKaigiArtifacts, PrivateKaigiFeeSpend, PrivateKaigiTemplate, PrivateKaigiTransaction,
        SignedTransaction, TX_SUBMISSION_RECEIPT_DOMAIN, TimeTriggerEntrypoint, TransactionBuilder,
        TransactionEntrypoint, TransactionResult, TransactionResultInner, TransactionSignature,
        TransactionSubmissionReceipt, TransactionSubmissionReceiptPayload, error::prelude::*,
    };
}
