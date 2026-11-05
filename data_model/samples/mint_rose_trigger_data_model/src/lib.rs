//! Arguments to mint rose with args trigger
use iroha_data_model::prelude::Json;
use norito::{
    Error as NoritoError,
    core::{NoritoDeserialize, NoritoSerialize},
    json::{self, JsonDeserialize, Parser},
};
/// Arguments to mint rose with args trigger
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::derive::JsonSerialize,
)]
pub struct MintRoseArgs {
    /// Whole rose units, encoded in JSON as the contract's canonical quantity string.
    #[norito(json = "whole_quantity")]
    pub val: u32,
}
mod whole_quantity {
    use norito::json::{self, BoundedJsonError, JsonSerialize, JsonWriteSink};

    pub(super) fn serialize(value: &u32, out: &mut String) {
        out.push('"');
        value.json_serialize(out);
        out.push('"');
    }

    pub(super) fn serialize_bounded(
        value: &u32,
        out: &mut dyn JsonWriteSink,
    ) -> Result<(), BoundedJsonError> {
        json::write_json_display_to(value, out)
    }
}

impl json::JsonDeserialize for MintRoseArgs {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        parser.skip_ws();
        parser.expect(b'{')?;
        parser.skip_ws();
        let mut val: Option<u32> = None;
        while !parser.try_consume_char(b'}')? {
            let key = parser.parse_string()?;
            parser.expect(b':')?;
            match key.as_str() {
                "val" => {
                    // At most ten decimal digits fit in the sample's u32 amount.
                    let mut preflight = *parser;
                    preflight.skip_string_bounded(10)?;
                    let text = parser.parse_string()?;
                    if !text.bytes().all(|byte| byte.is_ascii_digit())
                        || (text.len() > 1 && text.starts_with('0'))
                    {
                        return Err(json::Error::Message("noncanonical `val` quantity".into()));
                    }
                    let parsed = text
                        .parse::<u32>()
                        .map_err(|_| json::Error::Message("`val` out of range".into()))?;
                    if val.replace(parsed).is_some() {
                        return Err(json::Error::Message("duplicate field `val`".into()));
                    }
                }
                _ => parser.skip_value()?,
            }
            parser.skip_ws();
            if !parser.try_consume_char(b',')? {
                parser.expect(b'}')?;
                break;
            }
            parser.skip_ws();
        }
        let val = val.ok_or_else(|| json::Error::Message("missing field `val`".into()))?;
        Ok(Self { val })
    }
}
impl From<MintRoseArgs> for Json {
    fn from(details: MintRoseArgs) -> Self {
        let json =
            json::to_json(&details).expect("MintRoseArgs JSON serialization should not fail");
        Json::from_raw_json(json).expect("MintRoseArgs serializer must emit valid bounded JSON")
    }
}
impl TryFrom<&Json> for MintRoseArgs {
    type Error = NoritoError;
    fn try_from(payload: &Json) -> Result<Self, Self::Error> {
        let mut parser = Parser::new(payload.as_ref());
        let parsed = MintRoseArgs::json_deserialize(&mut parser)?;
        parser.skip_ws();
        if !parser.eof() {
            return Err(NoritoError::from(
                "trailing characters after MintRoseArgs JSON",
            ));
        }
        Ok(parsed)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mint_rose_args_roundtrip() {
        let args = MintRoseArgs { val: 42 };
        let json = Json::from(args.clone());
        let decoded = MintRoseArgs::try_from(&json).expect("decode");
        assert_eq!(args, decoded);
    }

    #[test]
    fn mint_rose_args_support_checked_json_with_exact_bounds() {
        for val in [0, 1, 42, u32::MAX] {
            let args = MintRoseArgs { val };
            let expected = format!("{{\"val\":\"{val}\"}}");
            assert_eq!(json::to_json(&args).unwrap(), expected);
            assert_eq!(
                json::to_json_bounded(&args, expected.len()).unwrap(),
                expected
            );
            assert!(json::to_json_bounded(&args, expected.len() - 1).is_err());
            // ExecuteTrigger::with_args uses this checked constructor directly.
            let payload = Json::try_new(&args).expect("bounded trigger arguments");
            assert_eq!(payload.as_ref(), expected);
            assert_eq!(MintRoseArgs::try_from(&payload).unwrap(), args);
        }
    }

    #[test]
    fn mint_rose_args_reject_noncanonical_or_out_of_range_quantities() {
        for input in [
            r#"{"val":42}"#,
            r#"{"val":""}"#,
            r#"{"val":"01"}"#,
            r#"{"val":"+1"}"#,
            r#"{"val":"-1"}"#,
            r#"{"val":"1.0"}"#,
            r#"{"val":"4294967296"}"#,
            r#"{"val":"10000000000"}"#,
            r#"{"val":"1","val":"2"}"#,
        ] {
            assert!(json::from_str::<MintRoseArgs>(input).is_err(), "{input}");
        }
    }
}
