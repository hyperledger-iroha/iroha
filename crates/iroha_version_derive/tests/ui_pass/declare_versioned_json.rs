use iroha_version_derive::{declare_versioned_with_json, version_with_json};
use norito::json::{JsonDeserialize, JsonSerialize};

declare_versioned_with_json!(VersionedMessage 1..3, Debug, Clone, iroha_macro::FromVariant);

#[version_with_json(version = 1, versioned_alias = "VersionedMessage")]
#[derive(Debug, Clone)]
pub struct Message;

impl Message {
    pub fn handle(&self) {}
}

impl JsonSerialize for Message {
    fn json_serialize(&self, out: &mut String) {
        out.push_str("null");
    }
}

impl JsonDeserialize for Message {
    fn json_deserialize(p: &mut norito::json::Parser<'_>) -> Result<Self, norito::json::Error> {
        p.parse_null()?;
        Ok(Self)
    }
}

#[version_with_json(version = 2, versioned_alias = "VersionedMessage")]
#[derive(Debug, Clone)]
pub struct Message2;

impl Message2 {
    pub fn handle(&self) {
        panic!("Should have been message version 1.")
    }
}

impl JsonSerialize for Message2 {
    fn json_serialize(&self, out: &mut String) {
        out.push_str("null");
    }
}

impl JsonDeserialize for Message2 {
    fn json_deserialize(p: &mut norito::json::Parser<'_>) -> Result<Self, norito::json::Error> {
        p.parse_null()?;
        Ok(Self)
    }
}

pub fn main() {
    match Message.into() {
        VersionedMessage::V1(message) => message.handle(),
        VersionedMessage::V2(message) => message.handle(),
    }
}
