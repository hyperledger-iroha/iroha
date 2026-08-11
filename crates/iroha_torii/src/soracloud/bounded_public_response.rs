//! Response-byte boundary for the intentionally public Soracloud discovery reads.

use axum::{
    body::Body,
    http::{HeaderValue, StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse as _, Response},
};
use iroha_core::soracloud_runtime::SoracloudUploadedModelEncryptionRecipient;
use iroha_data_model::soracloud::SoraUploadedModelEncryptionRecipientV1;
use norito::json::JsonSerialize;

/// Maximum encoded JSON bytes returned by one public discovery read.
pub(super) const MAX_JSON_BYTES: usize = 64 * 1024;

/// Encode one public discovery object under the fixed first-release byte cap.
pub(super) fn json<T: JsonSerialize>(value: &T) -> Response {
    let bytes = match norito::json::to_vec(value) {
        Ok(bytes) if bytes.len() <= MAX_JSON_BYTES => bytes,
        Ok(_) | Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let mut response = Response::new(Body::from(bytes));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

/// Convert and encode the current public uploaded-model encryption recipient.
pub(super) fn encryption_recipient(
    recipient: SoracloudUploadedModelEncryptionRecipient,
) -> Response {
    json(&super::UploadedModelEncryptionRecipientResponse {
        recipient: SoraUploadedModelEncryptionRecipientV1 {
            schema_version: recipient.schema_version,
            key_id: recipient.key_id,
            key_version: recipient.key_version,
            kem: recipient.kem,
            aead: recipient.aead,
            public_key_bytes: recipient.public_key_bytes,
            public_key_fingerprint: recipient.public_key_fingerprint,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_json_is_capped_before_response_dispatch() {
        let value = norito::json!({ "value": ("x".repeat(MAX_JSON_BYTES)) });
        assert_eq!(json(&value).status(), StatusCode::SERVICE_UNAVAILABLE);

        let value = norito::json!({ "value": "bounded" });
        let response = json(&value);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "application/json"
        );
    }
}
