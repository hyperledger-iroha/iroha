//! Transaction structures and related implementations.

/// Error types surfaced by transaction validation and execution.
pub mod error;
/// Executable payloads backing transactions and triggers.
pub mod executable;
/// Signed transaction forms and helpers.
pub mod signed;

pub use crate::trigger::{DataTriggerSequence, DataTriggerStep, TimeTriggerEntrypoint};
pub use executable::{Executable, IvmBytecode};
pub use signed::{
    ExecutionStep, SignedTransaction, TransactionBuilder, TransactionEntrypoint, TransactionResult,
    TransactionResultInner, TransactionSignature,
};

/// The prelude re-exports most commonly used traits, structs and macros from this module.
pub mod prelude {
    pub use super::{
        DataTriggerSequence, DataTriggerStep, Executable, ExecutionStep, IvmBytecode,
        SignedTransaction, TimeTriggerEntrypoint, TransactionBuilder, TransactionEntrypoint,
        TransactionResult, TransactionResultInner, TransactionSignature, error::prelude::*,
    };
}
