#[test]
fn signed_query_operation_documents_memory_and_proxy_failures() {
    let document = canonical_document();
    let operation = openapi_operation(&document, uri::QUERY, "post");
    let responses = operation
        .get("responses")
        .and_then(Value::as_object)
        .expect("signed-query response map");
    let request_content = operation
        .get("requestBody")
        .and_then(Value::as_object)
        .and_then(|body| body.get("content"))
        .and_then(Value::as_object)
        .expect("signed-query request media map");
    assert_eq!(
        request_content
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["application/x-norito"],
        "the bounded signed-query ingress protocol must not advertise JSON"
    );
    for status in [
        "200", "400", "406", "409", "413", "415", "429", "502", "503", "504",
    ] {
        assert!(
            responses.contains_key(status),
            "signed-query OpenAPI is missing {status}"
        );
    }
    let description = operation
        .get("description")
        .and_then(Value::as_str)
        .expect("signed-query description");
    assert!(description.contains("predicate/selector classification"));
    assert!(!description.contains("before nested component decoding"));
}
