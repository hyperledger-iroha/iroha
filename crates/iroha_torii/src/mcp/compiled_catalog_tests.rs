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
