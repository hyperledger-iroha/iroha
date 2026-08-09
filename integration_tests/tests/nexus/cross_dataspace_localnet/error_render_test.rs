// Display/debug composition coverage for cross-dataspace probe errors.

#[derive(Debug)]
struct DisplayOnlyTxError;

impl Display for DisplayOnlyTxError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str("route probe failed")
    }
}

#[test]
fn routed_header_string_reads_present_headers_and_ignores_absent_ones() {
    let mut headers = HeaderMap::new();
    headers.insert("x-iroha-routed-by", HeaderValue::from_static("proxy"));
    headers.insert(
        "x-iroha-invalid",
        HeaderValue::from_bytes(&[0xFF]).expect("binary header value"),
    );

    assert_eq!(
        routed_header_string(&headers, "x-iroha-routed-by"),
        Some("proxy".to_owned())
    );
    assert_eq!(
        routed_header_string(&headers, "x-iroha-route-lane-id"),
        None
    );
    assert_eq!(routed_header_string(&headers, "x-iroha-invalid"), None);
}

#[test]
fn render_error_with_debug_keeps_display_and_debug_context() {
    assert_eq!(
        render_error_with_debug(&DisplayOnlyTxError),
        "route probe failed (DisplayOnlyTxError)"
    );
}
