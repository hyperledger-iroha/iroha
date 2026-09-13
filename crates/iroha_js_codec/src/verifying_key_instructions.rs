//! Closed verifying-key instruction JSON projection over native model records.
//!
//! Record activation, commitments and monotonic versions remain Core admission
//! decisions. Encoding a record does not qualify a verifier for production.

use iroha_data_model::{
    isi::{
        Instruction, InstructionBox,
        verifying_keys::{RegisterVerifyingKey, UpdateVerifyingKey},
    },
    proof::{VERIFYING_KEY_BOX_MAX_PAYLOAD_BYTES_V1, VerifyingKeyId, VerifyingKeyRecord},
};
use norito::json::{self, JsonDeserialize, JsonSerialize, Value};

use super::{CodecError, CodecErrorKind, CodecResult, json_u64};

fn invalid(message: impl Into<String>) -> CodecError {
    CodecError::new(CodecErrorKind::InvalidArgument, message)
}

fn object<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn typed<T: JsonDeserialize + JsonSerialize>(value: &Value, context: &str) -> CodecResult<T> {
    let decoded: T =
        json::from_value(value.clone()).map_err(|error| invalid(format!("{context}: {error}")))?;
    if json::to_value(&decoded).map_err(|error| invalid(error.to_string()))? != *value {
        return Err(invalid(format!(
            "{context} has missing or unknown fields or noncanonical values"
        )));
    }
    Ok(decoded)
}

fn project_record_heights(value: &mut Value, to_native: bool, context: &str) -> CodecResult<()> {
    for key in ["activation_height", "withdraw_height"] {
        if let Some(field) = value.as_object_mut().and_then(|fields| fields.get_mut(key)) {
            if *field == Value::Null {
                continue;
            }
            let label = format!("{context}.{key}");
            *field = if to_native {
                Value::Number(json_u64::parse_u64(field.clone(), &label)?.into())
            } else {
                json_u64::u64_json(
                    field
                        .as_u64()
                        .ok_or_else(|| invalid(format!("{label} must be native u64")))?,
                )
            };
        }
    }
    Ok(())
}

fn record_bounds(id: &VerifyingKeyId, record: &VerifyingKeyRecord) -> CodecResult<()> {
    if !id.is_portable_registry_id() {
        return Err(invalid(
            "verifying-key id must use bounded portable registry syntax",
        ));
    }
    if record
        .key
        .as_ref()
        .is_some_and(|key| key.bytes.len() > VERIFYING_KEY_BOX_MAX_PAYLOAD_BYTES_V1)
    {
        return Err(invalid(
            "verifying-key bytes exceed the native V1 payload bound",
        ));
    }
    Ok(())
}

pub(super) fn from_json(value: &Value) -> Option<CodecResult<InstructionBox>> {
    let Value::Object(outer) = value else {
        return None;
    };
    let Some(payload) = outer.get("verifying_keys") else {
        if outer.contains_key("RegisterVerifyingKey") || outer.contains_key("UpdateVerifyingKey") {
            return Some(Err(invalid(
                "verifying-key instructions require the verifying_keys namespace",
            )));
        }
        return None;
    };
    Some((|| {
        if outer.len() != 1 {
            return Err(invalid(
                "verifying_keys instruction envelope must contain exactly one namespace",
            ));
        }
        let Value::Object(variants) = payload else {
            return Err(invalid("verifying_keys must be an object"));
        };
        if variants.len() != 1 {
            return Err(invalid(
                "verifying_keys must contain exactly one instruction variant",
            ));
        }
        let (name, payload) = variants
            .iter()
            .next()
            .ok_or_else(|| invalid("verifying-key instruction missing"))?;
        if !matches!(name.as_str(), "RegisterVerifyingKey" | "UpdateVerifyingKey") {
            return Err(invalid("unknown verifying_keys instruction variant"));
        }
        let Value::Object(fields) = payload else {
            return Err(invalid(format!("{name} must be an object")));
        };
        if fields.len() != 2 || !fields.contains_key("id") || !fields.contains_key("record") {
            return Err(invalid(format!("{name} requires exactly id and record")));
        }
        let id: VerifyingKeyId = typed(&fields["id"], &format!("{name}.id"))?;
        let mut native_record = fields["record"].clone();
        project_record_heights(&mut native_record, true, &format!("{name}.record"))?;
        let record: VerifyingKeyRecord = typed(&native_record, &format!("{name}.record"))?;
        record_bounds(&id, &record)?;
        match name.as_str() {
            "RegisterVerifyingKey" => Ok(RegisterVerifyingKey { id, record }.into()),
            "UpdateVerifyingKey" => Ok(UpdateVerifyingKey { id, record }.into()),
            _ => Err(invalid("unknown verifying_keys instruction variant")),
        }
    })())
}

pub(super) fn is_verifying_key_instruction(instruction: &InstructionBox) -> bool {
    let instruction: &dyn Instruction = &**instruction;
    instruction.as_any().is::<RegisterVerifyingKey>()
        || instruction.as_any().is::<UpdateVerifyingKey>()
}

fn emit(name: &str, id: &VerifyingKeyId, record: &VerifyingKeyRecord) -> CodecResult<Value> {
    record_bounds(id, record)?;
    let id = json::to_value(id).map_err(|error| invalid(error.to_string()))?;
    let mut record = json::to_value(record).map_err(|error| invalid(error.to_string()))?;
    project_record_heights(&mut record, false, &format!("{name}.record"))?;
    Ok(object([(
        "verifying_keys",
        object([(name, object([("id", id), ("record", record)]))]),
    )]))
}

pub(super) fn to_json(instruction: &InstructionBox) -> Option<CodecResult<Value>> {
    let instruction: &dyn Instruction = &**instruction;
    if let Some(value) = instruction.as_any().downcast_ref::<RegisterVerifyingKey>() {
        return Some(emit("RegisterVerifyingKey", &value.id, &value.record));
    }
    if let Some(value) = instruction.as_any().downcast_ref::<UpdateVerifyingKey>() {
        return Some(emit("UpdateVerifyingKey", &value.id, &value.record));
    }
    None
}

#[cfg(test)]
#[path = "verifying_key_instruction_tests.rs"]
mod tests;
