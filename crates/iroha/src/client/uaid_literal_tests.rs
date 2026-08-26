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
fn canonicalize_uaid_literal_accepts_only_exact_current_form() {
    let suffix = "abcdef01".repeat(8);
    let literal = format!("uaid:{suffix}");
    assert_eq!(
        canonicalize_uaid_literal(&literal, "tests.uaid").expect("canonical UAID literal"),
        literal
    );
    for noncanonical in [
        suffix.clone(),
        format!("UAID:{suffix}"),
        format!("uaid:{}", suffix.to_uppercase()),
        format!(" {literal}"),
        format!("{literal} "),
    ] {
        assert!(
            canonicalize_uaid_literal(&noncanonical, "tests.uaid").is_err(),
            "noncanonical UAID must reject: {noncanonical:?}"
        );
    }
}
#[test]
fn canonicalize_uaid_literal_rejects_invalid_lsb() {
    let literal = format!("uaid:{}", "10".repeat(32));
    let err = canonicalize_uaid_literal(&literal, "tests.uaid")
        .expect_err("invalid UAID should be rejected");
    assert!(
        err.to_string().contains("exact canonical"),
        "unexpected error: {err}"
    );
}
