//! Resumable transport for the signed DA ingest request.

use iroha_data_model::{
    NetworkId,
    da::{ingest::DaIngestRequest, types::BlobDigest},
};
use iroha_model_base::topology::LaneId;
use norito::derive::{JsonDeserialize, JsonSerialize};

/// Fixed size of every transport part except the last.
pub const DA_UPLOAD_PART_BYTES: usize = 1024 * 1024;
/// Largest transmitted payload accepted by this transport.
pub const DA_UPLOAD_MAX_BYTES: usize = iroha_data_model::da::MAX_PAYLOAD_BYTES as usize;
/// Maximum HTTP JSON body for a part or final signed request metadata.
pub const DA_UPLOAD_REQUEST_MAX_BYTES: usize = 4 * DA_UPLOAD_PART_BYTES;

/// Identity of one producer's transmitted payload, before decompression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct DaUploadDescriptor {
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Lane whose committed admission policy applies.
    pub lane_id: LaneId,
    /// Admitted epoch on that lane.
    pub epoch: u64,
    /// Total transmitted length in bytes.
    pub wire_bytes: u64,
    /// BLAKE3 digest of the exact transmitted bytes.
    pub wire_hash: BlobDigest,
}

/// An authenticated part-transfer request.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct DaUploadRequest {
    /// Descriptor common to every request for this payload.
    pub upload: DaUploadDescriptor,
    /// Operation applied to this producer's session.
    pub action: DaUploadAction,
}

/// Transfer operations on a single upload.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(tag = "kind", content = "value", deny_unknown_fields)]
pub enum DaUploadAction {
    /// Allocate or resume the matching upload, returning its first missing part.
    Begin,
    /// Write an immutable contiguous part.
    Put(DaUploadPart),
    /// Restore payload bytes and run ordinary signed ingest and pin authorization.
    Finish(Box<DaIngestRequest>),
    /// Release this exact upload.
    Release,
}

/// Encoded bytes of one authenticated part.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct DaUploadPart {
    /// Zero-based part index.
    pub index: u32,
    /// Standard padded base64 of the transmitted bytes.
    pub bytes_b64: String,
}

/// Durable upload progress returned by begin, put and release.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_torii_shared::da::upload::DaUploadProgress")]
pub struct DaUploadProgress {
    /// First contiguous part still required, or zero after release.
    pub next_part: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_digest_has_one_fixed_length_byte_array() {
        let hash = BlobDigest::new([0x9a; 32]);
        let value = norito::json::to_value(&hash).unwrap();
        let wrapped = value.as_array().unwrap();
        assert_eq!(wrapped.len(), 1);
        let bytes = wrapped[0].as_array().unwrap();
        assert_eq!(bytes.len(), 32);
        assert!(bytes.iter().all(|byte| byte.as_u64() == Some(0x9a)));
    }

    #[test]
    fn upload_control_messages_use_the_authored_tagged_json_shape() {
        for (action, expected) in [
            (DaUploadAction::Begin, r#"{"kind":"Begin","value":null}"#),
            (
                DaUploadAction::Release,
                r#"{"kind":"Release","value":null}"#,
            ),
        ] {
            let encoded = norito::json::to_vec(&action).unwrap();
            assert_eq!(encoded.as_slice(), expected.as_bytes());
            let restored: DaUploadAction = norito::json::from_slice(&encoded).unwrap();
            assert_eq!(norito::json::to_vec(&restored).unwrap(), encoded);
        }
        assert!(
            norito::json::from_str::<DaUploadProgress>(r#"{"next_part":1,"accepted":true}"#)
                .is_err()
        );
    }
}
