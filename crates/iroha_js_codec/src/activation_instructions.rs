//! Closed native JSON contracts for contract activation and lifecycle ownership.
//!
//! Each operation retains its mandatory compare-and-swap revision as an exact
//! decimal string. The existing model owns addresses, hashes and account/Parliament
//! ownership. Encoding or decoding confers no lifecycle authority; the executor
//! checks the retained revision, current owner and certified governance corridor.

use iroha_data_model::isi::{
    Instruction, InstructionBox,
    smart_contract_code::{
        AcceptContractOwnership, ActivateContractInstance, CancelContractOwnershipOffer,
        DeactivateContractInstance, OfferContractOwnership, SetContractParliamentDelegation,
    },
};
use norito::json::{self, JsonDeserialize, JsonSerialize, Value};

use crate::{CodecError, CodecErrorKind, CodecResult};

fn invalid(message: impl Into<String>) -> CodecError {
    CodecError::new(CodecErrorKind::InvalidArgument, message)
}

fn fields(value: Value, names: &[&str], context: &str) -> CodecResult<json::Map> {
    let Value::Object(fields) = value else {
        return Err(invalid(format!("{context} must be an object")));
    };
    // A nullable field must still be present. In particular, an omitted CAS
    // revision can never acquire a data-model or adapter default.
    for name in names {
        if !fields.contains_key(*name) {
            return Err(invalid(format!("{context}: missing field {name}")));
        }
    }
    crate::require_exact_json_fields(&fields, names, context)?;
    Ok(fields)
}

fn model<T: JsonDeserialize + JsonSerialize>(value: Value, context: &str) -> CodecResult<T> {
    let parsed: T =
        json::from_value(value.clone()).map_err(|error| invalid(format!("{context}: {error}")))?;
    if render(&parsed)? != value {
        return Err(invalid(format!(
            "{context} must use its exact canonical native JSON spelling"
        )));
    }
    Ok(parsed)
}

fn render<T: JsonSerialize>(value: &T) -> CodecResult<Value> {
    json::to_value(value).map_err(crate::codec_error)
}

fn revision(value: Value, context: &str) -> CodecResult<u64> {
    let Value::String(text) = value else {
        return Err(invalid(format!(
            "{context} must be a canonical u64 decimal string"
        )));
    };
    let number = text
        .parse::<u64>()
        .map_err(|error| invalid(format!("{context}: {error}")))?;
    if number.to_string() != text {
        return Err(invalid(format!(
            "{context} must be a canonical u64 decimal string"
        )));
    }
    Ok(number)
}

fn render_revision(value: &u64) -> CodecResult<Value> {
    Ok(Value::String(value.to_string()))
}

macro_rules! activation_contracts {
    ($($ty:ident { $($field:ident: $read:ident => $write:ident),+ $(,)? }),+ $(,)?) => {
        pub(super) fn is_activation_instruction(instruction: &InstructionBox) -> bool {
            let instruction: &dyn Instruction = &**instruction;
            let value = instruction.as_any();
            $(value.is::<$ty>())||+
        }

        pub(super) fn from_json(value: &Value) -> Option<CodecResult<InstructionBox>> {
            let Value::Object(envelope) = value else { return None; };
            $(if let Some(payload) = envelope.get(stringify!($ty)) {
                return Some((|| {
                    crate::require_exact_json_fields(envelope, &[stringify!($ty)], "instruction envelope")?;
                    let context = stringify!($ty);
                    let mut fields = fields(payload.clone(), &[$(stringify!($field)),+], context)?;
                    Ok($ty {
                        $($field: $read(crate::required_value(&mut fields, stringify!($field), context)?,
                            &format!("{context}.{}", stringify!($field)))?),+
                    }.into())
                })());
            })+
            None
        }

        pub(super) fn to_json(instruction: &InstructionBox) -> Option<CodecResult<Value>> {
            let instruction: &dyn Instruction = &**instruction;
            let value = instruction.as_any();
            $(if let Some(value) = value.downcast_ref::<$ty>() {
                return Some((|| {
                    let mut fields = json::Map::new();
                    $(fields.insert(stringify!($field).to_owned(), $write(&value.$field)?);)+
                    let mut envelope = json::Map::new();
                    envelope.insert(stringify!($ty).to_owned(), Value::Object(fields));
                    Ok(Value::Object(envelope))
                })());
            })+
            None
        }
    };
}

activation_contracts! {
    ActivateContractInstance {
        contract_address: model => render,
        expected_revision: revision => render_revision,
        code_hash: model => render,
    },
    DeactivateContractInstance {
        contract_address: model => render,
        expected_revision: revision => render_revision,
        reason: model => render,
    },
    SetContractParliamentDelegation {
        contract_address: model => render,
        expected_revision: revision => render_revision,
        delegated: model => render,
    },
    OfferContractOwnership {
        contract_address: model => render,
        expected_revision: revision => render_revision,
        new_owner: model => render,
    },
    AcceptContractOwnership {
        contract_address: model => render,
        expected_revision: revision => render_revision,
    },
    CancelContractOwnershipOffer {
        contract_address: model => render,
        expected_revision: revision => render_revision,
    },
}

#[cfg(test)]
#[path = "activation_instruction_tests.rs"]
mod tests;
