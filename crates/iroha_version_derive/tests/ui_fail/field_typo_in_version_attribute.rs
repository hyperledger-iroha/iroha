use iroha_version_derive::{declare_versioned, version};
use norito::{Decode as NoritoDecode, Encode as NoritoEncode};

declare_versioned!(VersionedMessage 1..2);

#[version(version = 1, versiond_alias = "VersionedMessage")]
#[derive(Debug, Clone, NoritoDecode, NoritoEncode)]
struct Message;

pub fn main() {}
