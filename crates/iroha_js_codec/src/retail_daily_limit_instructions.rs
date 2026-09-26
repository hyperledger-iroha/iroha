//! Exact first-release JSON contracts for native retail policy instructions.
//!
//! The typed data model owns the Norito payload. This adapter admits only the
//! canonical JSON projection of each type, then retains the native instruction
//! identity through both framed and transaction-archive codec paths.

use iroha_data_model::isi::{
    Instruction, InstructionBox,
    retail_daily_limit::{
        ActivateRetailDailyLimitV1, BindRetailIdentityV1, RetailMonetaryMovementV1,
    },
};
use norito::json::{self, Value};

use crate::{CodecError, CodecResult};

macro_rules! retail_contracts {
    ($($name:literal => $ty:ty),+ $(,)?) => {
        pub(super) fn is_retail_instruction(instruction: &InstructionBox) -> bool {
            let typed: &dyn Instruction = &**instruction;
            let any = typed.as_any();
            $(any.is::<$ty>())||+
        }

        pub(super) fn from_json(value: &Value) -> Option<CodecResult<InstructionBox>> {
            let Value::Object(envelope) = value else { return None; };
            $(if let Some(payload) = envelope.get($name) {
                return Some((|| {
                    crate::require_exact_json_fields(
                        envelope,
                        &[$name],
                        "retail instruction envelope",
                    )?;
                    let typed: $ty = crate::strict_typed_instruction(payload, $name)?;
                    Ok(typed.into())
                })());
            })+
            None
        }

        pub(super) fn to_json(instruction: &InstructionBox) -> Option<CodecResult<Value>> {
            let typed: &dyn Instruction = &**instruction;
            let any = typed.as_any();
            $(if let Some(value) = any.downcast_ref::<$ty>() {
                return Some((|| {
                    let payload = json::to_value(value).map_err(crate::codec_error)?;
                    let reconstructed: $ty = crate::strict_typed_instruction(&payload, $name)?;
                    if norito::encode_canonical(value).map_err(crate::codec_error)?
                        != norito::encode_canonical(&reconstructed).map_err(crate::codec_error)?
                    {
                        return Err(CodecError::failure(
                            "retail instruction JSON changes canonical Norito bytes",
                        ));
                    }
                    Ok(crate::instruction_envelope($name, payload))
                })());
            })+
            None
        }
    };
}

retail_contracts! {
    "ActivateRetailDailyLimitV1" => ActivateRetailDailyLimitV1,
    "BindRetailIdentityV1" => BindRetailIdentityV1,
    "RetailMonetaryMovementV1" => RetailMonetaryMovementV1,
}

#[cfg(test)]
#[path = "retail_daily_limit_instruction_tests.rs"]
mod tests;
