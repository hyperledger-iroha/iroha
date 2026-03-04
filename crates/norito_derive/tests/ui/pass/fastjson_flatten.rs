//! pass: FastJson derives support flatten and custom helpers

use norito::json::{JsonDeserialize, JsonSerialize, Parser};

mod passthrough {
    use super::*;

    pub fn serialize(value: &String, out: &mut String) {
        JsonSerialize::json_serialize(value, out);
    }

    pub fn deserialize(parser: &mut Parser<'_>) -> Result<String, norito::Error> {
        JsonDeserialize::json_deserialize(parser).map_err(Into::into)
    }
}

#[derive(Default, norito::JsonSerialize, norito::JsonDeserialize)]
struct Extra {
    #[norito(default)]
    depth: u32,
    label: String,
}

#[derive(norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Container {
    id: u32,
    #[norito(with = "passthrough")]
    tag: String,
    #[norito(default)]
    #[norito(flatten)]
    extra: Extra,
}

fn main() {}
