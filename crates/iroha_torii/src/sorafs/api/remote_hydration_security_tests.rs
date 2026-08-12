// Regressions for the bounded local SoraFS gateway response path.

use super::*;

#[test]
fn site_ranges_are_bounded_and_single_only() {
    let empty = HeaderMap::new();
    assert_eq!(
        parse_site_response_range(&empty, 7).expect("full range"),
        SiteResponseRange {
            offset: 0,
            length: 7,
            partial: false,
        }
    );
    assert_eq!(
        parse_site_response_range(&empty, MAX_SITE_RESPONSE_BYTES + 1)
            .expect_err("oversized full response")
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );

    let mut headers = HeaderMap::new();
    headers.insert(header::RANGE, HeaderValue::from_static("bytes=2-5"));
    assert_eq!(
        parse_site_response_range(&headers, 10).expect("bounded range"),
        SiteResponseRange {
            offset: 2,
            length: 4,
            partial: true,
        }
    );
    headers.insert(header::RANGE, HeaderValue::from_static("bytes=-3"));
    assert_eq!(
        parse_site_response_range(&headers, 10).expect("suffix range"),
        SiteResponseRange {
            offset: 7,
            length: 3,
            partial: true,
        }
    );
    headers.insert(header::RANGE, HeaderValue::from_static("bytes=0-1,4-5"));
    assert_eq!(
        parse_site_response_range(&headers, 10)
            .expect_err("multiple ranges")
            .status(),
        StatusCode::BAD_REQUEST
    );
}

#[test]
fn active_content_types_require_isolated_origin() {
    for media_type in [
        "text/html; charset=utf-8",
        "text/css; charset=utf-8",
        "text/javascript; charset=utf-8",
        "image/svg+xml",
        "application/xml; charset=utf-8",
        "application/pdf",
        "application/wasm",
    ] {
        assert!(content_type_is_active(media_type), "missed {media_type}");
    }
    assert!(!content_type_is_active("image/png"));
    assert!(!content_type_is_active("application/octet-stream"));
}
