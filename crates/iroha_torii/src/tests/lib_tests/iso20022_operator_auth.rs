#[test]
fn iso_profile_selection_is_query_bound_and_rejects_the_retired_header() {
    let config = sample_iso_bridge_config("DE89370400440532013000", &ALICE_ID);
    let runtime = crate::iso20022_bridge::Iso20022BridgeRuntime::from_config(&config)
        .expect("ISO bridge test runtime")
        .expect("sample config enables the ISO bridge");
    let query =
        std::collections::HashMap::from([("profile".to_owned(), "generic-iso20022".to_owned())]);
    let selected = iso_profile_from_request(&runtime, &HeaderMap::new(), &query)
        .expect("signed query profile");
    assert_eq!(selected.id, "generic-iso20022");
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-iroha-iso-profile",
        HeaderValue::from_static("generic-iso20022"),
    );
    let error = iso_profile_from_request(&runtime, &headers, &query)
        .expect_err("unsigned profile header must fail closed");
    assert!(
        matches!(error, Error::Query(ValidationFail::NotPermitted(message)) if message.contains("retired"))
    );
}
