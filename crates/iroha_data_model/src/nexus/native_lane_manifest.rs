//! Canonical JSON source schema for native lane governance manifests.
//!
//! These source descriptors are distinct from Space Directory manifests and
//! from the authenticated runtime catalog envelope carrying a manifest. Native
//! producers and consumers use one closed schema without importing node logic.
use std::collections::BTreeMap;

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use norito::json::Value as JsonValue;

/// First-release native lane governance manifest JSON source.
///
/// This descriptor is shared by manifest producers and the node loader. Decoding
/// validates the closed JSON shape; the node additionally checks source budgets,
/// lane/catalog binding, governance rules, validator identities and commitments.
/// Optional fields preserve absent declarations for that semantic validation;
/// successfully decoding a descriptor does not establish that it is admissible.
#[derive(Debug, Clone, PartialEq, Eq, DeriveJsonSerialize, DeriveJsonDeserialize, Default)]
#[norito(deny_unknown_fields)]
pub struct NativeLaneManifestV1 {
    /// Lane alias the manifest targets.
    pub lane: Option<String>,
    /// Governance module identifier asserted by the manifest.
    pub governance: Option<String>,
    /// Semantic version (major); the node accepts only [`Self::VERSION`], or omission.
    pub version: Option<u32>,
    /// Committee members or validator bindings (human readable).
    #[norito(default)]
    pub validators: Option<Vec<NativeLaneValidatorBindingV1>>,
    /// Quorum threshold applied to the validator set.
    pub quorum: Option<u32>,
    /// Namespaces protected by governance (transactions require explicit approval).
    #[norito(default)]
    pub protected_namespaces: Option<Vec<String>>,
    /// Optional map of governance hooks (module-specific).
    #[norito(default)]
    pub hooks: Option<BTreeMap<String, JsonValue>>,
    /// Optional privacy commitment descriptors consumed by private lanes.
    #[norito(default)]
    pub privacy_commitments: Option<Vec<NativeLanePrivacyCommitmentV1>>,
}
/// Manifest-level validator binding descriptor.
#[derive(Debug, Clone, PartialEq, Eq, DeriveJsonSerialize, DeriveJsonDeserialize, Default)]
#[norito(deny_unknown_fields)]
pub struct NativeLaneValidatorBindingV1 {
    /// Validator authority account literal.
    pub validator: Option<String>,
    /// Consensus/transport peer identity literal.
    pub peer_id: Option<String>,
    /// Optional Torii base URL used when authoritative routing must bridge over HTTP.
    #[norito(default)]
    pub torii_url: Option<String>,
}
/// Manifest-level privacy commitment descriptor.
#[derive(Debug, Clone, PartialEq, Eq, DeriveJsonSerialize, DeriveJsonDeserialize, Default)]
#[norito(deny_unknown_fields)]
pub struct NativeLanePrivacyCommitmentV1 {
    /// Registry identifier assigned to the commitment entry.
    pub id: Option<u16>,
    /// Commitment scheme. The first release accepts only `merkle`.
    pub scheme: Option<String>,
    /// Merkle-specific parameters.
    #[norito(default)]
    pub merkle: Option<NativeLaneMerkleCommitmentV1>,
}
/// Merkle commitment parameters advertised in manifests.
#[derive(Debug, Clone, PartialEq, Eq, DeriveJsonSerialize, DeriveJsonDeserialize, Default)]
#[norito(deny_unknown_fields)]
pub struct NativeLaneMerkleCommitmentV1 {
    /// Canonical 32-byte root digest encoded as hex.
    pub root: Option<String>,
    /// Maximum allowed audit-path depth.
    pub max_depth: Option<u8>,
}
impl NativeLaneManifestV1 {
    /// Supported native lane manifest semantic version.
    pub const VERSION: u32 = 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::json;

    #[test]
    fn native_lane_manifest_json_roundtrip_preserves_complete_source() {
        let raw = r#"{
            "lane": "dpn",
            "governance": "parliament",
            "version": 1,
            "validators": [{
                "validator": "validator-account",
                "peer_id": "validator-peer",
                "torii_url": "https://validator.example"
            }],
            "quorum": 1,
            "protected_namespaces": ["dpn"],
            "hooks": {"runtime_upgrade": {"allow": false, "allowed_ids": ["upgrade-1"]}},
            "privacy_commitments": [{
                "id": 3,
                "scheme": "merkle",
                "merkle": {
                    "root": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                    "max_depth": 32
                }
            }]
        }"#;
        let decoded: NativeLaneManifestV1 = json::from_str(raw).expect("typed manifest");
        assert_eq!(decoded.version, Some(NativeLaneManifestV1::VERSION));
        let encoded = json::to_json(&decoded).expect("encode native manifest");
        let roundtrip: NativeLaneManifestV1 = json::from_str(&encoded).expect("decode manifest");
        assert_eq!(roundtrip, decoded);
        let source_value: JsonValue = json::from_str(raw).expect("original source object");
        let encoded_value: JsonValue = json::from_str(&encoded).expect("encoded source object");
        assert_eq!(encoded_value, source_value);
    }

    #[test]
    fn native_lane_manifest_json_preserves_absent_declarations() {
        let decoded: NativeLaneManifestV1 = json::from_str("{}").expect("optional source fields");
        assert_eq!(decoded, NativeLaneManifestV1::default());
        let encoded = json::to_json(&decoded).expect("encode absent declarations");
        let roundtrip: NativeLaneManifestV1 =
            json::from_str(&encoded).expect("decode absent declarations");
        assert_eq!(roundtrip, decoded);
    }

    #[test]
    fn native_lane_manifest_json_rejects_unknown_fields_at_every_layer() {
        for (field, raw) in [
            ("validatorz", r#"{"validatorz":[]}"#),
            ("weight", r#"{"validators":[{"weight":1}]}"#),
            ("snark", r#"{"privacy_commitments":[{"snark":{}}]}"#),
            (
                "depth",
                r#"{"privacy_commitments":[{"merkle":{"depth":32}}]}"#,
            ),
        ] {
            let error = json::from_str::<NativeLaneManifestV1>(raw)
                .expect_err("unknown source fields must fail closed");
            assert!(error.to_string().contains(field), "field {field}: {error}");
        }
    }

    #[test]
    fn native_lane_manifest_json_rejects_untyped_or_overflowing_fields() {
        for raw in [
            r#"{"version":"1"}"#,
            r#"{"validators":["validator-account"]}"#,
            r#"{"validators":[{"validator":7}]}"#,
            r#"{"validators":[{"peer_id":false}]}"#,
            r#"{"quorum":4294967296}"#,
            r#"{"protected_namespaces":"dpn"}"#,
            r#"{"hooks":[]}"#,
            r#"{"privacy_commitments":[{"id":65536}]}"#,
            r#"{"privacy_commitments":[{"merkle":{"max_depth":256}}]}"#,
        ] {
            assert!(
                json::from_str::<NativeLaneManifestV1>(raw).is_err(),
                "accepted {raw}"
            );
        }
    }
}
