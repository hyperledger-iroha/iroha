//! Module containing errors that can occur in transaction lifecycle.

use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    string::String,
};

#[cfg(feature = "json")]
use base64::Engine as _;
#[cfg(feature = "json")]
use base64::engine::general_purpose::STANDARD;
use derive_more::Display;
use getset::Getters;
use iroha_data_model_derive::model;
use iroha_macro::FromVariant;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

pub use self::model::*;
use crate::{
    ValidationFail,
    isi::{Instruction, InstructionBox},
};

#[model]
mod model {
    use super::*;

    /// Error which indicates max instruction count was reached
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", norito(transparent))]
    #[repr(transparent)]
    // SAFETY: `TransactionLimitError` has no trap representation in `String`
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
    pub struct TransactionLimitError {
        /// Reason why transaction exceeds limits
        pub reason: String,
    }

    /// Transaction was rejected because of one of its instructions failing.
    #[derive(Getters, Debug, Clone, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct InstructionExecutionFail {
        /// Instruction for which execution failed
        #[getset(get = "pub")]
        pub instruction: InstructionBox,
        /// Error which happened during execution
        pub reason: String,
    }

    /// Transaction was rejected because execution of IVM bytecode failed
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[display("Failed to execute IVM bytecode: {reason}")]
    #[cfg_attr(feature = "json", norito(transparent))]
    #[repr(transparent)]
    // SAFETY: `IvmExecutionFail` has no trap representation in `String`
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
    pub struct IvmExecutionFail {
        /// Error which happened during execution
        pub reason: String,
    }

    /// Possible reasons for trigger-specific execution failure.
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[repr(u32)]
    pub enum TriggerExecutionFail {
        /// Exceeded maximum depth for chained data triggers.
        MaxDepthExceeded,
    }

    #[cfg(feature = "json")]
    impl norito::json::JsonSerialize for TriggerExecutionFail {
        fn json_serialize(&self, out: &mut String) {
            let label = match self {
                TriggerExecutionFail::MaxDepthExceeded => "MaxDepthExceeded",
            };
            norito::json::write_json_string(label, out);
        }
    }

    #[cfg(feature = "json")]
    impl norito::json::JsonDeserialize for TriggerExecutionFail {
        fn json_deserialize(
            parser: &mut norito::json::Parser<'_>,
        ) -> Result<Self, norito::json::Error> {
            let value = parser.parse_string()?;
            match value.as_str() {
                "MaxDepthExceeded" => Ok(TriggerExecutionFail::MaxDepthExceeded),
                other => Err(norito::json::Error::UnknownField {
                    field: other.to_owned(),
                }),
            }
        }
    }

    /// The reason for rejecting transaction which happened because of transaction.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        displaydoc::Display,
        FromVariant,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[ignore_extra_doc_attributes]
    #[derive(thiserror::Error)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub enum TransactionRejectionReason {
        /// Account does not exist
        AccountDoesNotExist(
            #[skip_from]
            #[skip_try_from]
            #[cfg_attr(not(feature = "fast_dsl"), source)]
            crate::query::error::FindError,
        ),
        /// Failed to validate transaction limits
        ///
        /// e.g. number of instructions
        LimitCheck(#[cfg_attr(not(feature = "fast_dsl"), source)] TransactionLimitError),
        /// Validation failed
        Validation(#[cfg_attr(not(feature = "fast_dsl"), source)] ValidationFail),
        /// Failure in instruction execution
        ///
        /// In practice should be fully replaced by [`crate::ValidationFail::InstructionFailed`]
        /// and will be removed soon.
        InstructionExecution(
            #[cfg_attr(not(feature = "fast_dsl"), source)] InstructionExecutionFail,
        ),
        /// Failure in IVM execution
        IvmExecution(#[cfg_attr(not(feature = "fast_dsl"), source)] IvmExecutionFail),
        /// Execution of a time trigger or an invoked data trigger failed.
        TriggerExecution(#[cfg_attr(not(feature = "fast_dsl"), source)] TriggerExecutionFail),
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonSerialize for TransactionRejectionReason {
    fn json_serialize(&self, out: &mut String) {
        let bytes = norito::to_bytes(self)
            .expect("TransactionRejectionReason Norito serialization must succeed");
        let encoded = STANDARD.encode(bytes);
        norito::json::JsonSerialize::json_serialize(&encoded, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for TransactionRejectionReason {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let encoded = parser.parse_string()?;
        let bytes = STANDARD
            .decode(encoded.as_str())
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
        let archived = norito::from_bytes::<TransactionRejectionReason>(&bytes)
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
        norito::core::NoritoDeserialize::try_deserialize(archived)
            .map_err(|err| norito::json::Error::Message(err.to_string()))
    }
}

impl PartialEq for InstructionExecutionFail {
    fn eq(&self, other: &Self) -> bool {
        Instruction::id(&*self.instruction) == Instruction::id(&*other.instruction)
            && self.instruction.dyn_encode() == other.instruction.dyn_encode()
            && self.reason == other.reason
    }
}

impl Display for InstructionExecutionFail {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Failed to execute instruction: {}", self.reason)
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for TransactionLimitError {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.reason, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for TransactionLimitError {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let reason = parser.parse_string()?;
        Ok(Self { reason })
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for IvmExecutionFail {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.reason, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for IvmExecutionFail {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let reason = parser.parse_string()?;
        Ok(Self { reason })
    }
}

impl std::error::Error for TransactionLimitError {}

#[cfg(not(feature = "fast_dsl"))]
impl std::error::Error for InstructionExecutionFail {}

impl std::error::Error for IvmExecutionFail {}

impl std::error::Error for TriggerExecutionFail {}

pub mod prelude {
    //! The prelude re-exports most commonly used traits, structs and macros from this module.

    pub use super::{
        InstructionExecutionFail, IvmExecutionFail, TransactionRejectionReason,
        TriggerExecutionFail,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Level, isi::Log};

    #[test]
    fn instruction_execution_fail_equality() {
        let log = Log::new(Level::INFO, "log".to_string());
        let fail1 = InstructionExecutionFail {
            instruction: InstructionBox::from(log.clone()),
            reason: "reason".to_string(),
        };
        let fail2 = InstructionExecutionFail {
            instruction: InstructionBox::from(log),
            reason: "reason".to_string(),
        };
        assert_eq!(fail1, fail2);

        let different_instruction = InstructionExecutionFail {
            instruction: InstructionBox::from(Log::new(Level::INFO, "other".to_string())),
            reason: "reason".to_string(),
        };
        assert_ne!(fail1, different_instruction);

        let different_reason = InstructionExecutionFail {
            instruction: fail1.instruction.clone(),
            reason: "other".to_string(),
        };
        assert_ne!(fail1, different_reason);
    }
}
