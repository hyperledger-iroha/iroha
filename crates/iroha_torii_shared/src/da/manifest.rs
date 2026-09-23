//! Canonical HTTP projection of a stored data-availability manifest.

use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json::Value,
};

/// Response from `GET /v1/da/manifests/{storage_ticket}`.
///
/// The Norito artifact remains the manifest's authoritative representation.
/// Storage clients validate its length, digest and projections before use.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct DaManifestResponse {
    /// Canonical lowercase hex storage ticket selected by the request.
    pub storage_ticket: String,
    /// Canonical lowercase hex client blob identifier.
    pub client_blob_id: String,
    /// Canonical lowercase hex BLAKE3 payload digest.
    pub blob_hash: String,
    /// Canonical lowercase hex chunk commitment root.
    pub chunk_root: String,
    /// Canonical lowercase hex BLAKE3 digest of the complete Norito artifact.
    pub manifest_hash: String,
    /// Lane that owns the manifest.
    pub lane_id: u32,
    /// Manifest epoch.
    pub epoch: u64,
    /// Exact length of the decoded Norito artifact, in bytes.
    pub manifest_len: u64,
    /// Standard base64 encoding of the complete Norito artifact.
    pub manifest_norito: String,
    /// JSON projection of the same manifest.
    pub manifest: Value,
    /// Canonical versioned chunk fetch plan derived from the manifest.
    pub chunk_plan: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::json;

    fn response() -> DaManifestResponse {
        DaManifestResponse {
            storage_ticket: "11".repeat(32),
            client_blob_id: "22".repeat(32),
            blob_hash: "33".repeat(32),
            chunk_root: "44".repeat(32),
            manifest_hash: "55".repeat(32),
            lane_id: 7,
            epoch: 9,
            manifest_len: 3,
            manifest_norito: "AQID".into(),
            manifest: norito::json!({"version": 1}),
            chunk_plan: norito::json!({"schema": "sorafs.chunk_fetch_plan.v1"}),
        }
    }

    #[test]
    fn response_roundtrip_uses_exact_required_fields() {
        let expected = response();
        let encoded = json::to_vec(&expected).expect("encode response");
        assert_eq!(
            json::from_slice::<DaManifestResponse>(&encoded).unwrap(),
            expected
        );
        let value = json::to_value(&expected).unwrap();
        let fields = value.as_object().unwrap();
        assert_eq!(fields.len(), 11);
        for field in fields.keys() {
            let mut missing = fields.clone();
            missing.remove(field);
            assert!(
                json::from_value::<DaManifestResponse>(Value::Object(missing)).is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn response_rejects_aliases_duplicate_fields_and_string_lengths() {
        for alias in [
            "storageTicket",
            "manifest_b64",
            "manifestNorito",
            "chunkPlan",
        ] {
            let mut value = json::to_value(&response()).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .insert(alias.into(), Value::Null);
            assert!(
                json::from_value::<DaManifestResponse>(value).is_err(),
                "{alias}"
            );
        }
        let encoded = json::to_json(&response()).unwrap();
        let duplicate = format!("{{\"epoch\":9,{}", &encoded[1..]);
        assert!(json::from_json::<DaManifestResponse>(&duplicate).is_err());
        let mut value = json::to_value(&response()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("manifest_len".into(), Value::from("3"));
        assert!(json::from_value::<DaManifestResponse>(value).is_err());
    }
}
