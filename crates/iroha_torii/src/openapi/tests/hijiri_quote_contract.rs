#[test]
fn hijiri_validation_fee_quote_contract_is_native_bounded_and_authenticated() {
    use iroha_torii_shared::{
        route_catalog::runtime_governance::VALIDATION_FEE_HIJIRI_QUOTE_PATH,
        validation_fee_api::{
            VALIDATION_FEE_HIJIRI_QUOTE_EVALUATED_ASSURANCE_V1,
            VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1,
            VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES_V1,
            VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES_V1,
            VALIDATION_FEE_HIJIRI_QUOTE_PROJECTION_SCHEMA_NAME,
            VALIDATION_FEE_HIJIRI_QUOTE_REQUEST_SCHEMA_NAME,
        },
    };

    fn native_schema<'a>(operation: &'a Map, response: Option<&str>) -> &'a Map {
        let content = if let Some(status) = response {
            operation
                .get("responses")
                .and_then(Value::as_object)
                .and_then(|responses| responses.get(status))
                .and_then(Value::as_object)
                .and_then(|response| response.get("content"))
        } else {
            operation
                .get("requestBody")
                .and_then(Value::as_object)
                .and_then(|body| body.get("content"))
        }
        .and_then(Value::as_object)
        .expect("Hijiri quote media map");
        assert_eq!(
            content.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["application/x-norito"]
        );
        content["application/x-norito"]["schema"]
            .as_object()
            .expect("Hijiri quote native schema")
    }

    for (label, document) in [
        ("package authority", canonical_document()),
        ("generated spec", generate_spec()),
    ] {
        let operation = openapi_operation(&document, VALIDATION_FEE_HIJIRI_QUOTE_PATH, "post");
        assert_eq!(
            operation.get("operationId").and_then(Value::as_str),
            Some("validationFeeHijiriQuote"),
            "{label} operation id"
        );
        assert_eq!(
            operation.get(TOOL_EFFECT_EXTENSION).and_then(Value::as_str),
            Some("read"),
            "{label} effect"
        );
        assert!(operation.contains_key("security"), "{label} security");
        assert!(
            operation.contains_key("x-iroha-canonical-auth-v1"),
            "{label} canonical auth contract"
        );
        assert_canonical_auth_required_response(
            operation,
            VALIDATION_FEE_HIJIRI_QUOTE_PATH,
            "canonical_authentication_required",
        );
        let responses = operation["responses"]
            .as_object()
            .expect("Hijiri quote responses");
        assert_eq!(
            documented_reject_codes(responses, "403"),
            vec!["validation_fee_hijiri_quote_account_mismatch"],
            "{label} principal-mismatch code"
        );
        assert!(
            responses["403"]["description"]
                .as_str()
                .is_some_and(|description| description.contains("direct signatory")),
            "{label} member-aware forbidden response"
        );
        assert!(
            operation["description"]
                .as_str()
                .is_some_and(|description| {
                    description.contains("direct signatory")
                        && description.contains("same snapshot")
                }),
            "{label} same-snapshot member authorization"
        );

        let request = native_schema(operation, None);
        assert_eq!(
            request.get("x-iroha-norito-schema").and_then(Value::as_str),
            Some(VALIDATION_FEE_HIJIRI_QUOTE_REQUEST_SCHEMA_NAME),
            "{label} request schema"
        );
        assert_eq!(
            request.get("x-iroha-max-bytes").and_then(Value::as_u64),
            Some(VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES_V1 as u64),
            "{label} request bound"
        );
        let response = native_schema(operation, Some("200"));
        assert_eq!(
            response
                .get("x-iroha-norito-schema")
                .and_then(Value::as_str),
            Some(VALIDATION_FEE_HIJIRI_QUOTE_PROJECTION_SCHEMA_NAME),
            "{label} response schema"
        );
        assert_eq!(
            response.get("x-iroha-max-bytes").and_then(Value::as_u64),
            Some(VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES_V1 as u64),
            "{label} response bound"
        );

        let schemas = component_schemas(&document);
        let request_properties = schemas["ValidationFeeHijiriQuoteRequestV1"]["properties"]
            .as_object()
            .expect("Hijiri request properties");
        assert_eq!(
            request_properties["qualifyingTransferCount"]
                .get("maximum")
                .and_then(Value::as_u64),
            Some(u64::from(
                VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1
            ))
        );
        assert!(
            request_properties["accountId"]["description"]
                .as_str()
                .is_some_and(|description| {
                    description.contains("authenticated")
                        && description.contains("direct signatory")
                }),
            "{label} account-principal binding"
        );
        let response_properties = schemas["ValidationFeeHijiriQuoteResponseV1"]["properties"]
            .as_object()
            .expect("Hijiri response properties");
        assert_eq!(
            response_properties["assurance"]
                .get("const")
                .and_then(Value::as_str),
            Some(VALIDATION_FEE_HIJIRI_QUOTE_EVALUATED_ASSURANCE_V1),
            "{label} honest assurance"
        );
        assert_eq!(
            response_properties["schema"]
                .get("const")
                .and_then(Value::as_str),
            Some(VALIDATION_FEE_HIJIRI_QUOTE_PROJECTION_SCHEMA_NAME),
            "{label} response marker"
        );
    }
}
