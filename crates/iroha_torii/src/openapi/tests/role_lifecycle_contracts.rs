//! `OpenAPI` contracts of the stored-role reads and the trigger lifecycle reads.

use super::*;

#[cfg(feature = "app_api")]
#[test]
fn role_reads_publish_their_refusals_and_exact_rows() {
    use crate::collections::specs::{ACCOUNT_ROLES, CollectionSpec, ROLES};
    // A GET authenticates in its handler, which refuses a lone account header separately.
    const GET_CODES: &[&str] = &[
        "alias_auth_invalid",
        "canonical_authentication_invalid",
        "canonical_authentication_required",
    ];
    const POST_CODES: &[&str] = &[
        "canonical_authentication_invalid",
        "canonical_authentication_required",
    ];
    const GET_STATUSES: &[&str] = &["400", "403", "429", "503"];
    const POST_STATUSES: &[&str] = &["400", "403", "413", "415", "429", "503"];
    fn spec_fields(spec: &CollectionSpec) -> BTreeSet<&'static str> {
        spec.fields.iter().map(|field| field.name).collect()
    }
    fn schema_members<'a>(schemas: &'a Map, name: &str) -> BTreeSet<&'a str> {
        schemas[name]["properties"]
            .as_object()
            .unwrap_or_else(|| panic!("{name} properties"))
            .keys()
            .map(String::as_str)
            .collect()
    }
    let document = generate_spec();
    for (path, method, reject_codes, statuses) in [
        ("/v1/roles", "get", GET_CODES, GET_STATUSES),
        ("/v1/roles/query", "post", POST_CODES, POST_STATUSES),
        (
            "/v1/accounts/{account_id}/roles",
            "get",
            GET_CODES,
            GET_STATUSES,
        ),
        (
            "/v1/accounts/{account_id}/roles/query",
            "post",
            POST_CODES,
            POST_STATUSES,
        ),
    ] {
        let operation = openapi_operation(&document, path, method);
        let responses = operation["responses"].as_object().expect("role responses");
        // The signature is optional at the transport, so `security` keeps its anonymous
        // alternative; these handlers still refuse it, which the responses must publish.
        assert_eq!(
            documented_reject_codes(responses, "401"),
            reject_codes,
            "{method} {path} 401 codes"
        );
        assert_eq!(
            responses["401"]["headers"]["WWW-Authenticate"]["schema"]["const"].as_str(),
            Some("Signature"),
            "{method} {path} challenge"
        );
        for status in statuses {
            assert_eq!(
                operation_response_schema_ref(operation, status, path),
                "#/components/schemas/ErrorEnvelope",
                "{method} {path} {status}"
            );
        }
    }
    // Ledger rows keep their exact fields; query output separately models projections
    // and aggregate aliases rather than weakening the native ledger representation.
    let schemas = component_schemas(&document);
    assert_eq!(
        schemas["RolePage"]["properties"]["items"]["items"]["anyOf"],
        norito::json!([
            {"$ref": "#/components/schemas/NativeRoleV1"},
            {"$ref": "#/components/schemas/RoleProjectionRow"},
            {"$ref": "#/components/schemas/RoleAggregateRow"}
        ])
    );
    assert_eq!(
        schemas["AccountRolePage"]["properties"]["items"]["items"]["anyOf"],
        norito::json!([
            {"$ref": "#/components/schemas/AccountRoleRow"},
            {"$ref": "#/components/schemas/AccountRoleAggregateRow"}
        ])
    );
    assert_eq!(schema_members(schemas, "NativeRoleV1"), spec_fields(&ROLES));
    assert_strict_object_schema(schemas, "NativeRoleV1", &["id", "permissions"], &[]);
    assert_strict_object_schema(schemas, "AccountRoleRow", &["id"], &[]);
    assert_eq!(
        schema_members(schemas, "AccountRoleRow"),
        spec_fields(&ACCOUNT_ROLES)
    );
    assert_eq!(
        schema_members(schemas, "RoleProjectionRow"),
        spec_fields(&ROLES)
    );
    let projection = &schemas["RoleProjectionRow"];
    assert_eq!(projection["additionalProperties"], norito::json!(false));
    assert_eq!(projection["minProperties"], norito::json!(1));
    assert_eq!(projection["maxProperties"], norito::json!(1));
    assert!(projection.get("required").is_none());
    assert_eq!(
        projection["properties"],
        schemas["NativeRoleV1"]["properties"]
    );
}

#[test]
fn role_aggregate_schemas_bound_aliases_and_preserve_metric_value_types() {
    use iroha_torii_shared::list_query::AGGREGATE_MAX_METRICS;

    let document = canonical_document();
    let schemas = component_schemas(&document);
    // `norito::json!` takes one token per object value, so the path is bound first.
    let maximum = u64::MAX;
    let count = norito::json!({"type": "integer", "minimum": 0, "maximum": maximum});
    for (name, metric) in [
        ("AccountRoleAggregateRow", count.clone()),
        (
            "RoleAggregateRow",
            norito::json!({"oneOf": [count, {"type": "null"}]}),
        ),
    ] {
        let branches = schemas[name]["oneOf"].as_array().unwrap();
        assert_eq!(branches.len(), 2);
        for (index, branch) in branches.iter().enumerate() {
            assert_eq!(branch["type"].as_str(), Some("object"));
            assert_eq!(branch["additionalProperties"], norito::json!(false));
            assert_eq!(branch["minProperties"].as_u64(), Some(1 + index as u64));
            assert_eq!(
                branch["maxProperties"].as_u64(),
                Some((AGGREGATE_MAX_METRICS + index) as u64)
            );
            let patterns = branch["patternProperties"].as_object().unwrap();
            assert_eq!(patterns.len(), 1);
            assert_eq!(patterns.values().next().unwrap(), &metric);
        }
        assert!(branches[0].get("properties").is_none());
        assert_eq!(branches[1]["required"], norito::json!(["id"]));
        assert_eq!(
            branches[1]["properties"],
            norito::json!({"id": {"$ref": "#/components/schemas/NativeNameV1"}})
        );
    }
}

#[test]
fn lifecycle_witness_schemas_preserve_native_selection_and_branch_json_shapes() {
    use iroha_data_model::{
        runtime::definition::{
            DefinitionId, RestrictionEntryV1, RestrictionV1, RootSelectionV1, SelectionId,
        },
        sumeragi_finality::lifecycle_runtime::{
            LifecycleRuntimeProofStepV1, LifecycleRuntimeProofV1,
        },
    };
    let document = canonical_document();
    let schemas = component_schemas(&document);
    let id = DefinitionId([0x71; 32]);
    let restriction = RestrictionEntryV1 {
        definition: id,
        restriction: RestrictionV1::AdmissionClosed(7),
    };
    let selection = RootSelectionV1 {
        kernel: id,
        commitment_profile: id,
        services_profile: id,
        installed_storage: vec![],
        active: vec![],
        bindings: vec![],
        restrictions: vec![restriction],
        completion_closures: vec![],
    };
    let step = LifecycleRuntimeProofStepV1 {
        level: u8::MAX,
        hash: iroha_crypto::Hash::new(b"native lifecycle schema"),
    };
    let proof = LifecycleRuntimeProofV1 {
        height: 2,
        execution_selection: SelectionId([0x71; 32]),
        post_selection: SelectionId([0x72; 32]),
        siblings: vec![step],
    };
    // Typed JSON shape coverage confers no execution or certificate authority.
    for (schema, value) in [
        (
            "RuntimeRootSelectionV1",
            norito::json::to_value(&selection).unwrap(),
        ),
        (
            "RuntimeRestrictionEntryV1",
            norito::json::to_value(&restriction).unwrap(),
        ),
        (
            "LifecycleRuntimeProofStepV1",
            norito::json::to_value(&step).unwrap(),
        ),
        (
            "LifecycleRuntimeProofV1",
            norito::json::to_value(&proof).unwrap(),
        ),
    ] {
        let fields: Vec<_> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_strict_object_schema(schemas, schema, &fields, &[]);
    }
    let proof = norito::json::to_value(&proof).unwrap();
    assert_eq!(proof["height"].as_u64(), Some(2));
    assert_eq!(
        proof["execution_selection"].as_str(),
        Some(hex::encode([0x71; 32]).as_str())
    );
    assert_eq!(
        schemas["LifecycleRuntimeProofV1"]["properties"]["siblings"]["maxItems"].as_u64(),
        Some(256)
    );
    assert_eq!(
        schemas["LifecycleRuntimeProofStepV1"]["properties"]["level"]["maximum"].as_u64(),
        Some(u64::from(u8::MAX))
    );
}

#[test]
fn lifecycle_reads_publish_every_refusal_their_handlers_return() {
    let document = canonical_document();
    for (route, statuses) in [
        (
            route_catalog::core::TRIGGER_LIFECYCLE_ACTIVATION,
            ["200", "400", "429", "503"].as_slice(),
        ),
        (
            route_catalog::core::TRIGGER_LIFECYCLE_WITNESS,
            [
                "200", "400", "403", "404", "408", "413", "429", "500", "503",
            ]
            .as_slice(),
        ),
    ] {
        let operation = &document["paths"][route.path()]["get"];
        let responses = operation["responses"].as_object().unwrap();
        assert_eq!(
            responses.keys().map(String::as_str).collect::<Vec<_>>(),
            statuses,
            "{} response statuses",
            route.path()
        );
        for status in statuses.iter().filter(|status| **status != "200") {
            assert_eq!(
                responses[*status]["content"]["application/json"]["schema"]["$ref"].as_str(),
                Some("#/components/schemas/ErrorEnvelope"),
                "{} {status}",
                route.path()
            );
        }
    }
}
