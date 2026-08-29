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
    fn lane_teu_status_uses_one_complete_v1_binary_layout() {
        let binding = NexusLaneManifestValidatorBindingStatus {
            validator: "validator-account".to_owned(),
            peer_id: "peer-id".to_owned(),
            torii_url: Some("https://validator.example".to_owned()),
        };
        let status = NexusLaneTeuStatus {
            lane_id: 1,
            capacity: 2,
            committed: 3,
            buckets: crate::metrics::NexusLaneTeuBuckets {
                floor: 4,
                headroom: 5,
                must_serve: 6,
                circuit_breaker: 7,
            },
            deferrals: crate::metrics::NexusLaneTeuDeferrals {
                cap_exceeded: 8,
                envelope_limit: 9,
                quota: 10,
                circuit_breaker: 11,
            },
            must_serve_truncations: 12,
            trigger_level: 13,
            starvation_bound_slots: 14,
            block_height: 15,
            finality_lag_slots: 16,
            settlement_backlog_xor_micro: 17,
            tx_vertices: 18,
            tx_edges: 19,
            overlay_count: 20,
            overlay_instr_total: 21,
            overlay_bytes_total: 22,
            rbc_chunks: 23,
            rbc_bytes_total: 24,
            peak_layer_width: 25,
            layer_count: 26,
            avg_layer_width: 27,
            median_layer_width: 28,
            scheduler_utilization_pct: 29,
            layer_width_buckets: crate::metrics::SchedulerLayerWidthBuckets::new([
                30, 31, 32, 33, 34, 35, 36, 37,
            ]),
            detached_prepared: 38,
            detached_merged: 39,
            detached_fallback: 40,
            quarantine_executed: 41,
            manifest_required: true,
            manifest_ready: true,
            alias: "lane-one".to_owned(),
            dataspace_id: 42,
            dataspace_alias: Some("dataspace-one".to_owned()),
            visibility: Some("private".to_owned()),
            storage_profile: "archive".to_owned(),
            lane_type: Some("governed".to_owned()),
            governance: Some("council".to_owned()),
            settlement: Some("instant".to_owned()),
            scheduler_teu_capacity_override: Some(43),
            scheduler_starvation_bound_override: Some(44),
            manifest_path: Some("manifests/lane-one.json".to_owned()),
            manifest_validators: vec!["validator-account".to_owned()],
            manifest_validator_bindings: vec![binding.clone()],
            manifest_quorum: Some(1),
            manifest_protected_namespaces: vec!["protected".to_owned()],
            manifest_runtime_upgrade: Some(NexusLaneRuntimeUpgradeHookStatus {
                allow: true,
                require_metadata: true,
                metadata_key: Some("runtime_id".to_owned()),
                allowed_ids: vec!["runtime-v1".to_owned()],
            }),
        };

        let expected_json = norito::json::to_json(&status).expect("serialize lane status JSON");
        let bytes = to_bytes(&status).expect("serialize lane status");
        let archived = from_bytes(&bytes).expect("deserialize lane status");
        let decoded = NexusLaneTeuStatus::deserialize(archived);

        assert_eq!(decoded.manifest_validator_bindings, vec![binding]);
        assert_eq!(
            norito::json::to_json(&decoded).expect("serialize decoded lane status JSON"),
            expected_json
        );

        let truncated = &bytes[..bytes.len() / 2];
        assert!(
            from_bytes::<NexusLaneTeuStatus>(truncated).is_err(),
            "the V1 decoder must reject partial layouts"
        );
    }
}
