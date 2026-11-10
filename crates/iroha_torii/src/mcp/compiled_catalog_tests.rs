//! Actual compiled MCP catalog projection and enabled-schema refusal controls.

use super::*;

#[test]
fn compiled_musubi_registry_preserves_common_tools_and_exact_feature_projection() {
    let tools = build_tool_specs(&iroha_config::parameters::actual::ToriiMcp::default());
    assert!(tools.iter().any(|tool| tool.name == "iroha.health"));
    for definition in MUSUBI_V1_TOOL_DEFINITIONS {
        let contract_enabled = cfg!(feature = "app_api");
        assert_eq!(
            tools
                .iter()
                .filter(|tool| tool.name == definition.name)
                .count(),
            usize::from(contract_enabled),
            "the independent application feature contract owns {}",
            definition.name
        );
        let enabled = catalog_mcp_projection_decision(
            CATALOG_PROJECTION_GROUPS,
            &Method::POST,
            definition.path,
        ) == Some(true);
        assert_eq!(
            tools.iter().any(|tool| tool.name == definition.name),
            enabled,
            "the compiled catalog owns {}",
            definition.path,
        );
    }
}

#[cfg(not(feature = "app_api"))]
#[test]
fn disabled_musubi_schema_is_never_dereferenced() {
    let absent_application_spec = norito::json!({"paths": {}});
    assert!(
        iroha_musubi_v1_tools(&absent_application_spec)
            .next()
            .is_none()
    );
}

#[cfg(feature = "app_api")]
#[test]
fn enabled_musubi_schema_corruption_still_refuses_registry_construction() {
    let definition = MUSUBI_V1_TOOL_DEFINITIONS
        .iter()
        .find(|definition| {
            catalog_mcp_projection_decision(
                CATALOG_PROJECTION_GROUPS,
                &Method::POST,
                definition.path,
            ) == Some(true)
        })
        .expect("application build has an enabled Musubi operation");
    let mut corrupt = openapi::compiled_spec().clone();
    let operation = corrupt
        .get_mut("paths")
        .and_then(Value::as_object_mut)
        .and_then(|paths| paths.get_mut(definition.path))
        .and_then(|path| path.get_mut("post"))
        .and_then(Value::as_object_mut)
        .expect("actual compiled enabled operation");
    assert!(operation.remove("requestBody").is_some());
    assert!(
        std::panic::catch_unwind(|| iroha_musubi_v1_tools(&corrupt).collect::<Vec<_>>()).is_err(),
        "an enabled operation cannot silently lose its typed request schema"
    );
}

#[test]
fn sorafs_authority_reads_project_exact_typed_schemas_and_required_canonical_auth() {
    use iroha_torii_shared::route_catalog::runtime_governance::{
        GOV_SORAFS_AUTHORITY_HEAD, GOV_SORAFS_AUTHORITY_LEASE, GOV_SORAFS_AUTHORITY_PROPOSAL,
    };

    let spec = openapi::compiled_spec();
    let tools = build_tool_specs(&iroha_config::parameters::actual::ToriiMcp::default());
    for (route, response_schema, id, retained_record) in [
        (
            GOV_SORAFS_AUTHORITY_PROPOSAL,
            "SorafsAuthorityProposalReadV1",
            "proposal_content_id",
            "record",
        ),
        (
            GOV_SORAFS_AUTHORITY_HEAD,
            "SorafsAuthorityHeadReadV1",
            "subject_id",
            "head",
        ),
        (
            GOV_SORAFS_AUTHORITY_LEASE,
            "SorafsAuthorityLeaseReadV1",
            "plan_id",
            "lease",
        ),
    ] {
        let path = route.path();
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature
        );
        let enabled = cfg!(feature = "app_api");
        assert!(route.projections().openapi() && route.projections().sdk());
        assert!(!route.projections().mcp());
        assert_eq!(
            catalog_route_mounted_decision(CATALOG_PROJECTION_GROUPS, &Method::GET, path),
            Some(enabled)
        );
        assert_eq!(
            catalog_mcp_projection_decision(CATALOG_PROJECTION_GROUPS, &Method::GET, path),
            Some(false),
            "OpenAPI presence cannot widen the explicit SDK-only catalog projection"
        );
        let projected = tools
            .iter()
            .filter(|tool| {
                tool.route_backing()
                    .is_some_and(|(_, method, target)| method == &Method::GET && target == path)
            })
            .count();
        assert_eq!(
            projected, 0,
            "the catalog excludes generated MCP tools for {path}"
        );
        let operation = spec["paths"].get(path).and_then(|item| item.get("get"));
        if !enabled {
            assert!(
                operation.is_none(),
                "disabled application route must be absent"
            );
            continue;
        }
        let operation = operation.expect("compiled authority read operation");
        assert_eq!(operation["x-iroha-tool-effect"].as_str(), Some("read"));
        assert_eq!(
            operation["x-iroha-route-auth"]["authentication"].as_str(),
            Some("canonical_account_signature")
        );
        assert_eq!(
            operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"].as_str(),
            Some(format!("#/components/schemas/{response_schema}").as_str())
        );
        let schema = &spec["components"]["schemas"][response_schema];
        assert_eq!(schema["additionalProperties"].as_bool(), Some(false));
        let required: std::collections::BTreeSet<_> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            required,
            std::collections::BTreeSet::from(["network", id, "current_height", retained_record])
        );
        assert_eq!(
            operation["responses"]["200"]["headers"]["Cache-Control"]["schema"]["const"].as_str(),
            Some("private, no-store")
        );
        assert_eq!(
            operation["x-iroha-route-auth"]["stableRouteId"].as_str(),
            Some(route.stable_route_id())
        );
        assert_eq!(
            operation["x-iroha-route-auth"]["admission"].as_str(),
            Some("authenticated_account")
        );
        assert_eq!(
            operation["security"],
            norito::json!([
                {"IrohaCanonicalAccount": [], "IrohaCanonicalNonce": [],
                 "IrohaCanonicalSignature": [], "IrohaCanonicalTimestampMs": []},
                {"IrohaCanonicalWitness": []}
            ])
        );
        // This checks the original mounted HTTP target's canonical-auth policy.
        // It does not create or imply an MCP tool for the target.
        let policy = target_extra_header_policy(&Method::GET, path).unwrap();
        assert_eq!(policy, ExtraHeaderPolicy::CanonicalAccountAuthentication);
        for headers in [None, Some(norito::json!({"X-Iroha-Account": "untrusted"}))] {
            assert!(
                apply_extra_headers_with_policy(&mut HeaderMap::new(), headers.as_ref(), policy)
                    .is_err(),
                "absent or partial inner proof cannot be dispatched to {path}"
            );
        }
    }
}

#[test]
fn runtime_release_projects_typed_operator_contract_without_mcp_exposure() {
    use iroha_torii_shared::route_catalog::runtime_governance::RUNTIME_RELEASE;

    let route = RUNTIME_RELEASE;
    let path = route.path();
    assert_eq!(
        route.authentication(),
        AuthenticationPolicy::OperatorSignature
    );
    assert!(route.projections().openapi());
    assert!(!route.projections().mcp() && !route.projections().sdk());
    assert_eq!(
        catalog_route_mounted_decision(CATALOG_PROJECTION_GROUPS, &Method::GET, path),
        Some(true)
    );
    assert_eq!(
        catalog_mcp_projection_decision(CATALOG_PROJECTION_GROUPS, &Method::GET, path),
        Some(false)
    );
    let tools = build_tool_specs(&iroha_config::parameters::actual::ToriiMcp::default());
    assert!(tools.iter().all(|tool| {
        !tool
            .route_backing()
            .is_some_and(|(_, method, target)| method == &Method::GET && target == path)
    }));

    let spec = openapi::compiled_spec();
    let operation = &spec["paths"][path]["get"];
    assert_eq!(operation["x-iroha-tool-effect"].as_str(), Some("operator"));
    assert_eq!(
        operation["x-iroha-route-auth"],
        norito::json!({
            "schemaVersion": 1,
            "stableRouteId": "operator.runtime.release",
            "authentication": "operator_signature",
            "admission": "operator"
        })
    );
    assert_eq!(
        operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"].as_str(),
        Some("#/components/schemas/RuntimeReleaseResponse")
    );
    assert_eq!(
        operation["responses"]["200"]["headers"]["Cache-Control"]["schema"]["const"].as_str(),
        Some("private, no-store")
    );
    assert_eq!(
        operation["security"],
        norito::json!([{
            "IrohaOperatorNonce": [], "IrohaOperatorPublicKey": [],
            "IrohaOperatorSignature": [], "IrohaOperatorTimestampMs": []
        }])
    );
    // Check the mounted HTTP target's policy without creating an MCP route.
    let policy = target_extra_header_policy(&Method::GET, path).unwrap();
    assert_eq!(policy, ExtraHeaderPolicy::OperatorAuthentication);
    for headers in [
        None,
        Some(norito::json!({"X-Iroha-Operator-Public-Key": "untrusted"})),
    ] {
        assert!(
            apply_extra_headers_with_policy(&mut HeaderMap::new(), headers.as_ref(), policy)
                .is_err()
        );
    }
}
