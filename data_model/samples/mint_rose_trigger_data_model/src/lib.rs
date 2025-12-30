//! Arguments to mint rose with args trigger
#[allow(unused_imports)]
use std::eprintln;
use std::string::String;

use iroha_data_model::prelude::Json;
use norito::{
    Error as NoritoError,
    core::{NoritoDeserialize, NoritoSerialize},
    json::{self, JsonDeserialize, JsonSerialize, Parser},
};

/// Arguments to mint rose with args trigger
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct MintRoseArgs {
    /// Amount to mint
    pub val: u32,
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
                    let raw = parser.parse_u64()?;
                    let parsed = u32::try_from(raw)
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

impl JsonSerialize for MintRoseArgs {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"val\":");
        json::JsonSerialize::json_serialize(&self.val, out);
        out.push('}');
    }
}

impl From<MintRoseArgs> for Json {
    fn from(details: MintRoseArgs) -> Self {
        let json =
            json::to_json(&details).expect("MintRoseArgs JSON serialization should not fail");
        Json::from_string_unchecked(json)
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
        // ensure canonical normalization (parser ensures val fits u32)
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern crate std;

    #[test]
    fn mint_rose_args_roundtrip() {
        let args = MintRoseArgs { val: 42 };
        let json = Json::from(args.clone());
        let decoded = MintRoseArgs::try_from(&json).expect("decode");
        assert_eq!(args, decoded);
    }
}
