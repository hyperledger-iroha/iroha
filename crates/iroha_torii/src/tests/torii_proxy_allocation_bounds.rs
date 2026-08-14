/// Regression coverage for allocation bounds on every Torii proxy response.
mod torii_proxy_allocation_bounds_tests {
    use iroha_core::torii_proxy::{ToriiProxyHeaderV1, ToriiProxyHttpResponseV1};
    use super::{
        TORII_PROXY_MAX_HEADER_VALUE_BYTES_V1, TORII_PROXY_MAX_HEADERS_V1,
        bounded_torii_proxy_headers, validate_torii_proxy_snapshot_bounds,
    };
    #[test]
    fn rejects_oversized_body_before_delivery() {
        let snapshot = ToriiProxyHttpResponseV1 {
            status_code: 200,
            headers: Vec::new(),
            body: vec![0_u8; 17],
        };
        assert!(validate_torii_proxy_snapshot_bounds(&snapshot, 16).is_err());
    }
    #[test]
    fn rejects_too_many_decoded_headers() {
        let header = ToriiProxyHeaderV1 {
            name: "x-test".to_owned(),
            value: vec![0_u8],
        };
        let snapshot = ToriiProxyHttpResponseV1 {
            status_code: 200,
            headers: vec![header; TORII_PROXY_MAX_HEADERS_V1 + 1],
            body: Vec::new(),
        };
        assert!(validate_torii_proxy_snapshot_bounds(&snapshot, 1).is_err());
    }
    #[test]
    fn rejects_oversized_headers_before_cloning_them() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::HeaderName::from_static("x-test"),
            axum::http::HeaderValue::from_bytes(&vec![
                b'x';
                TORII_PROXY_MAX_HEADER_VALUE_BYTES_V1 + 1
            ])
            .expect("visible test bytes form a header value"),
        );
        assert!(bounded_torii_proxy_headers(&headers).is_err());
    }
}
