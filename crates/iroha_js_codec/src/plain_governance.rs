//! Exact public ballot and conviction-update projections over native instructions.

use iroha_data_model::isi::{
    InstructionBox,
    governance::{CastPlainBallot, UpdatePlainConviction},
};
use norito::json::{self, Value};

use crate::{CodecError, CodecErrorKind, CodecResult};

const CAST: &str = "CastPlainBallot";
const UPDATE: &str = "UpdatePlainConviction";

pub(super) fn is_plain_instruction(instruction: &InstructionBox) -> bool {
    instruction.as_any().is::<CastPlainBallot>()
        || instruction.as_any().is::<UpdatePlainConviction>()
}

pub(super) fn from_json(value: &Value) -> Option<CodecResult<InstructionBox>> {
    let Value::Object(envelope) = value else {
        return None;
    };
    let name = if envelope.contains_key(CAST) {
        CAST
    } else if envelope.contains_key(UPDATE) {
        UPDATE
    } else {
        return None;
    };
    Some((|| {
        crate::require_exact_json_fields(envelope, &[name], "plain governance envelope")?;
        let Value::Object(fields) = &envelope[name] else {
            return Err(invalid("plain governance payload must be an object"));
        };
        let required: &[&str] = if name == CAST {
            &[
                "referendum_id",
                "owner",
                "amount",
                "duration_blocks",
                "direction",
            ]
        } else {
            &["referendum_id", "owner", "amount", "duration_blocks"]
        };
        crate::require_exact_json_fields(fields, required, name)?;
        let referendum_id = crate::parse_string_value(fields["referendum_id"].clone(), name)?;
        if !iroha_data_model::governance::is_valid_governance_selector_v1(&referendum_id) {
            return Err(invalid(
                "plain governance referendum_id is not a canonical selector",
            ));
        }
        let owner = crate::parse_account_id_value(fields["owner"].clone(), name)?;
        let amount = crate::parse_canonical_quantity_value(fields["amount"].clone(), name)?;
        // JavaScript converts its lossless duration to an exact JSON integer token.
        let duration_blocks = fields["duration_blocks"].as_u64().ok_or_else(|| {
            invalid("plain governance duration_blocks must be a JSON u64 integer")
        })?;
        if name == CAST {
            let direction = fields["direction"]
                .as_u64()
                .filter(|direction| *direction <= 2)
                .ok_or_else(|| invalid("CastPlainBallot.direction must be exactly 0, 1, or 2"))?;
            Ok(CastPlainBallot {
                referendum_id,
                owner,
                amount,
                duration_blocks,
                direction: u8::try_from(direction).expect("validated direction"),
            }
            .into())
        } else {
            Ok(UpdatePlainConviction {
                referendum_id,
                owner,
                amount,
                duration_blocks,
            }
            .into())
        }
    })())
}

fn invalid(reason: &str) -> CodecError {
    CodecError::new(CodecErrorKind::InvalidArgument, reason)
}

pub(super) fn to_json(instruction: &InstructionBox) -> Option<CodecResult<Value>> {
    let (name, referendum_id, owner, amount, duration, direction) =
        if let Some(value) = instruction.as_any().downcast_ref::<CastPlainBallot>() {
            (
                CAST,
                &value.referendum_id,
                &value.owner,
                &value.amount,
                value.duration_blocks,
                Some(value.direction),
            )
        } else if let Some(value) = instruction.as_any().downcast_ref::<UpdatePlainConviction>() {
            (
                UPDATE,
                &value.referendum_id,
                &value.owner,
                &value.amount,
                value.duration_blocks,
                None,
            )
        } else {
            return None;
        };
    Some((|| {
        let mut fields = json::Map::new();
        fields.insert("referendum_id".into(), Value::String(referendum_id.clone()));
        fields.insert(
            "owner".into(),
            json::to_value(owner).map_err(crate::codec_error)?,
        );
        fields.insert("amount".into(), Value::String(amount.to_string()));
        fields.insert("duration_blocks".into(), Value::Number(duration.into()));
        if let Some(direction) = direction {
            fields.insert(
                "direction".into(),
                Value::Number(u64::from(direction).into()),
            );
        }
        let value = crate::instruction_envelope(name, Value::Object(fields));
        let rebuilt = from_json(&value).expect("known plain governance envelope")?;
        if norito::encode_canonical(&rebuilt).map_err(crate::codec_error)?
            != norito::encode_canonical(instruction).map_err(crate::codec_error)?
        {
            return Err(CodecError::failure(
                "plain governance JSON changes canonical Norito bytes",
            ));
        }
        Ok(value)
    })())
}

#[cfg(test)]
#[path = "plain_governance_tests.rs"]
mod tests;
