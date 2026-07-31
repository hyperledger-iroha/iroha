#[test]
fn uaid_bindings_query_leaves_query_string_empty() {
    let client = Client::new(config_factory());
    let url = join_torii_url(&client.torii_url, "v1/space-directory/uaids/demo");
    let query = UaidBindingsQuery;
    let request = query
        .apply(client.default_request(HttpMethod::GET, url))
        .build()
        .expect("build request");
    assert_eq!(request.uri().query(), None);
}

#[test]
fn canonicalize_uaid_literal_is_case_insensitive() {
    let suffix = "ABCDEF01".repeat(8);
    let literal = format!("UAID:{suffix}");
    let canonical =
        canonicalize_uaid_literal(&literal, "tests.uaid").expect("canonicalize literal");
    assert_eq!(canonical, format!("uaid:{}", suffix.to_ascii_lowercase()));
}

#[test]
fn canonicalize_uaid_literal_rejects_invalid_lsb() {
    let literal = format!("uaid:{}", "10".repeat(32));
    let err = canonicalize_uaid_literal(&literal, "tests.uaid")
        .expect_err("invalid UAID should be rejected");
    assert!(
        err.to_string().contains("LSB set to 1"),
        "unexpected error: {err}"
    );
}
