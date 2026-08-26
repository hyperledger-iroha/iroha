//! Response-byte boundary for the intentionally public Soracloud discovery reads.
use axum::{
    body::Body,
    http::{HeaderValue, StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse as _, Response},
};
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
