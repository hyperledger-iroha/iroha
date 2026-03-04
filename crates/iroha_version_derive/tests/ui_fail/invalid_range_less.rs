use iroha_version_derive::{declare_versioned, version};
use norito::{Decode as NoritoDecode, Encode as NoritoEncode};

declare_versioned!(VersionedMessage 2..1);

#[version(version = 1, versioned_alias = "VersionedMessage")]
#[derive(Debug, Clone, NoritoDecode, NoritoEncode)]
struct Message;

pub fn main() {}
