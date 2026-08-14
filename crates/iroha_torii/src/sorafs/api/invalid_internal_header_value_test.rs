#[test]
fn invalid_internal_header_value_fails_closed_without_panicking() {
    assert_eq!(
        header_value("invalid\0value", "X-Test"),
        HeaderValue::from_static("unavailable")
    );
}
