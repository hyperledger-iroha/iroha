//! One lossless JSON u64 projection at the native JavaScript SDK boundary.
//!
//! Safe integers use JSON numbers; larger values use exact decimal strings.
//! Native models and canonical Norito payloads retain their unsigned 64-bit fields.

use norito::json::Value;

use crate::{CodecError, CodecErrorKind, CodecResult};

pub(super) const MAX_SAFE_INTEGER: u64 = (1_u64 << 53) - 1;

pub(super) fn parse_u64(value: Value, label: &str) -> CodecResult<u64> {
    let parsed = match value {
        Value::Number(number) => number.as_u64().filter(|number| *number <= MAX_SAFE_INTEGER),
        Value::String(text) => {
            // Bounds also prevent unbounded integer parsing at this SDK boundary.
            if text.len() <= 20
                && text
                    .as_bytes()
                    .first()
                    .is_some_and(|byte| matches!(byte, b'1'..=b'9'))
                && text.bytes().all(|byte| byte.is_ascii_digit())
            {
                text.parse::<u64>()
                    .ok()
                    .filter(|number| *number > MAX_SAFE_INTEGER)
            } else {
                None
            }
        }
        _ => None,
    };
    parsed.ok_or_else(|| CodecError::new(
        CodecErrorKind::InvalidArgument,
        format!("{label} must be a JSON safe unsigned integer or an exact decimal u64 string above 9007199254740991"),
    ))
}

pub(super) fn u64_json(value: u64) -> Value {
    if value <= MAX_SAFE_INTEGER {
        Value::Number(value.into())
    } else {
        Value::String(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_sdk_projection_has_one_exact_representation() {
        for number in [
            0,
            1,
            MAX_SAFE_INTEGER - 1,
            MAX_SAFE_INTEGER,
            MAX_SAFE_INTEGER + 1,
            u64::MAX,
        ] {
            let value = u64_json(number);
            assert_eq!(parse_u64(value.clone(), "u64").unwrap(), number);
            let wrong_shape = match value {
                Value::Number(_) => Value::String(number.to_string()),
                Value::String(_) => Value::Number(number.into()),
                _ => unreachable!("u64 projection"),
            };
            assert!(parse_u64(wrong_shape, "u64").is_err());
        }
        for text in [
            "",
            "+9007199254740992",
            "09007199254740992",
            "9007199254740992.0",
            "9e15",
            "9007199254740992 ",
            "-1",
            "18446744073709551616",
        ] {
            assert!(parse_u64(Value::String(text.to_owned()), "u64").is_err());
        }
        for value in [
            Value::Null,
            Value::Bool(true),
            Value::Number(norito::json::Number::F64(1.5)),
        ] {
            assert!(parse_u64(value, "u64").is_err());
        }
    }
}
