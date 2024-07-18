use iroha_version_derive::{declare_versioned_with_json, version_with_json};
use serde::{Deserialize, Serialize};

declare_versioned_with_json!(VersionedMessage 1..3, Debug, Clone, iroha_macro::FromVariant);

#[version_with_json(version = 1, versioned_alias = "VersionedMessage")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message;

impl Message {
    pub fn handle(&self) {}
}

#[version_with_json(version = 2, versioned_alias = "VersionedMessage")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message2;

impl Message2 {
    pub fn handle(&self) {
        panic!("Should have been message version 1.")
    }
}

pub fn main() {
    match Message.into() {
        VersionedMessage::V1(message) => message.handle(),
        VersionedMessage::V2(message) => message.handle(),
    }
}
