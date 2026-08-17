use iroha_schema::IntoSchema;
use norito::core::DecodeFromSlice;

/// Schema-closed manifest validator binding exposed through `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
pub struct NexusLaneManifestValidatorBindingStatus {
    /// Canonical validator authority account.
    pub validator: String,
    /// Canonical consensus and routed-traffic peer identity.
    pub peer_id: String,
    /// Torii base URL declared for authoritative HTTP routing.
    #[norito(required)]
    pub torii_url: Option<String>,
}

impl norito::core::NoritoSerialize for NexusLaneManifestValidatorBindingStatus {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.validator.clone(),
            self.peer_id.clone(),
            self.torii_url.clone(),
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for NexusLaneManifestValidatorBindingStatus {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (validator, peer_id, torii_url) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            validator,
            peer_id,
            torii_url,
        }
    }
}

impl<'a> DecodeFromSlice<'a> for NexusLaneManifestValidatorBindingStatus {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((validator, peer_id, torii_url), used) =
            <(String, String, Option<String>)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                validator,
                peer_id,
                torii_url,
            },
            used,
        ))
    }
}

/// Snapshot of the runtime-upgrade governance hook declared in a lane manifest.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
pub struct NexusLaneRuntimeUpgradeHookStatus {
    /// Whether runtime-upgrade instructions are permitted.
    pub allow: bool,
    /// Whether runtime-upgrade instructions must include manifest metadata.
    pub require_metadata: bool,
    /// Metadata key enforced by the manifest.
    #[norito(default)]
    pub metadata_key: Option<String>,
    /// Allowed metadata identifiers declared by the manifest.
    pub allowed_ids: Vec<String>,
}

impl norito::core::NoritoSerialize for NexusLaneRuntimeUpgradeHookStatus {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (
            self.allow,
            self.require_metadata,
            self.metadata_key.clone(),
            self.allowed_ids.clone(),
        );
        norito::core::NoritoSerialize::serialize(&payload, writer)
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for NexusLaneRuntimeUpgradeHookStatus {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        let (allow, require_metadata, metadata_key, allowed_ids) =
            norito::core::NoritoDeserialize::deserialize(archived.cast());
        Self {
            allow,
            require_metadata,
            metadata_key,
            allowed_ids,
        }
    }
}

impl<'a> DecodeFromSlice<'a> for NexusLaneRuntimeUpgradeHookStatus {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((allow, require_metadata, metadata_key, allowed_ids), used) =
            <(bool, bool, Option<String>, Vec<String>)>::decode_from_slice(bytes)?;
        Ok((
            Self {
                allow,
                require_metadata,
                metadata_key,
                allowed_ids,
            },
            used,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::NexusLaneTeuStatus;
    use norito::{NoritoDeserialize, from_bytes, to_bytes};

    #[test]
    fn manifest_validator_binding_status_has_exact_json_schema() {
        let binding = NexusLaneManifestValidatorBindingStatus {
            validator: "validator-account".to_owned(),
            peer_id: "peer-id".to_owned(),
            torii_url: Some("https://validator.example".to_owned()),
        };
        let encoded = norito::json::to_json(&binding).expect("serialize manifest binding");
        let decoded: NexusLaneManifestValidatorBindingStatus =
            norito::json::from_json(&encoded).expect("deserialize manifest binding");
        assert_eq!(decoded, binding);

        let unknown = r#"{"validator":"validator-account","peer_id":"peer-id","torii_url":"https://validator.example","unexpected":true}"#;
        assert!(
            norito::json::from_json::<NexusLaneManifestValidatorBindingStatus>(unknown).is_err(),
            "manifest binding rows must reject unknown JSON fields"
        );
        let missing = r#"{"validator":"validator-account","peer_id":"peer-id"}"#;
        assert!(
            norito::json::from_json::<NexusLaneManifestValidatorBindingStatus>(missing).is_err(),
            "manifest binding rows must require the exact torii_url field"
        );
    }

    #[test]
    fn lane_teu_status_roundtrips_manifest_validator_binding() {
        let binding = NexusLaneManifestValidatorBindingStatus {
            validator: "validator-account".to_owned(),
            peer_id: "peer-id".to_owned(),
            torii_url: Some("https://validator.example".to_owned()),
        };
        let status = NexusLaneTeuStatus {
            manifest_validator_bindings: vec![binding.clone()],
            ..NexusLaneTeuStatus::default()
        };

        let bytes = to_bytes(&status).expect("serialize lane status");
        let archived = from_bytes(&bytes).expect("deserialize lane status");
        let decoded = NexusLaneTeuStatus::deserialize(archived);

        assert_eq!(decoded.manifest_validator_bindings, vec![binding]);
    }
}
