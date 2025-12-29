//! pass: FastJson derives handle tagged enums with custom helpers

use norito::json::{JsonDeserialize, JsonSerialize, Parser};

mod passthrough {
    use super::*;

    pub fn serialize(value: &u32, out: &mut String) {
        JsonSerialize::json_serialize(value, out);
    }

    pub fn deserialize(parser: &mut Parser<'_>) -> Result<u32, norito::Error> {
        JsonDeserialize::json_deserialize(parser).map_err(Into::into)
    }
}

#[derive(norito::derive::FastJson, norito::derive::FastJsonWrite)]
#[norito(tag = "kind", content = "content")]
enum Tagged {
    #[norito(rename = "unit")]
    Unit,
    #[norito(rename = "single")]
    Single(#[norito(with = "passthrough")] u32),
    Pair(u64, String),
    #[norito(rename = "record")]
    Record {
        id: u32,
        #[norito(default)]
        label: Option<String>,
    },
}

fn main() {}
