//! Fail-closed validation for OpenAPI release artifacts.

use norito::json::{Map, Value};
use std::error::Error;

const JSON_VALUE_SCHEMA_NAME: &str = "JsonValue";
const JSON_VALUE_SCHEMA_REF: &str = "#/components/schemas/JsonValue";

pub(crate) fn validate_release_openapi_spec(spec: &Value) -> Result<(), Box<dyn Error>> {
    let document = spec
        .as_object()
        .ok_or("release OpenAPI document must be a JSON object")?;
    let version = document
        .get("openapi")
        .and_then(Value::as_str)
        .ok_or("release OpenAPI document is missing the openapi version")?;
    if !version.starts_with("3.") {
        return Err(format!(
            "release OpenAPI document uses unsupported version `{version}`; expected OpenAPI 3.x"
        )
        .into());
    }
    let info = document
        .get("info")
        .and_then(Value::as_object)
        .ok_or("release OpenAPI document is missing the info object")?;
    for field in ["title", "version"] {
        if info
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(format!(
                "release OpenAPI document info.{field} must be a non-empty string"
            )
            .into());
        }
    }
    let paths = document
        .get("paths")
        .and_then(Value::as_object)
        .ok_or("release OpenAPI document is missing the paths object")?;
    if paths.is_empty() {
        return Err(
            "release OpenAPI document must define at least one path; empty/stub specifications are forbidden"
                .into(),
        );
    }
    let components = document
        .get("components")
        .and_then(Value::as_object)
        .ok_or("release OpenAPI document is missing components")?;
    let schemas = components
        .get("schemas")
        .and_then(Value::as_object)
        .ok_or("release OpenAPI document is missing components.schemas")?;
    if schemas.is_empty() {
        return Err(
            "release OpenAPI document must define at least one component schema; empty/stub specifications are forbidden"
                .into(),
        );
    }

    let mut references_json_value = false;
    validate_component_refs(spec, components, "$", &mut references_json_value)?;
    if references_json_value || schemas.contains_key(JSON_VALUE_SCHEMA_NAME) {
        validate_json_value_schema(schemas)?;
    }
    Ok(())
}

fn validate_component_refs(
    value: &Value,
    components: &Map,
    location: &str,
    references_json_value: &mut bool,
) -> Result<(), Box<dyn Error>> {
    match value {
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_component_refs(
                    value,
                    components,
                    &format!("{location}[{index}]"),
                    references_json_value,
                )?;
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                let child_location = format!("{location}.{key}");
                if key == "$ref" {
                    let reference = value.as_str().ok_or_else(|| {
                        format!("release OpenAPI $ref at {child_location} must be a string")
                    })?;
                    let component_path = reference
                        .strip_prefix("#/components/")
                        .ok_or_else(|| {
                            format!(
                                "release OpenAPI $ref at {child_location} must target a supported local component: {reference}"
                            )
                        })?;
                    let (component_kind, component_name) =
                        component_path.split_once('/').ok_or_else(|| {
                            format!(
                                "release OpenAPI $ref at {child_location} must target a component root: {reference}"
                            )
                        })?;
                    let component_label = match component_kind {
                        "schemas" => "schema",
                        "headers" => "header",
                        _ => {
                            return Err(format!(
                                "release OpenAPI $ref at {child_location} must target a supported local component schema or header: {reference}"
                            )
                            .into());
                        }
                    };
                    if component_name.is_empty() || component_name.contains('/') {
                        return Err(format!(
                            "release OpenAPI $ref at {child_location} must target a component {component_label} root: {reference}"
                        )
                        .into());
                    }
                    let component_map = components
                        .get(component_kind)
                        .and_then(Value::as_object)
                        .ok_or_else(|| {
                            format!(
                                "release OpenAPI $ref at {child_location} targets missing component {component_label} map {component_kind}"
                            )
                        })?;
                    if !component_map.contains_key(component_name) {
                        return Err(format!(
                            "release OpenAPI $ref at {child_location} targets missing component {component_label} {component_name}"
                        )
                        .into());
                    }
                    *references_json_value |= reference == JSON_VALUE_SCHEMA_REF;
                }
                validate_component_refs(value, components, &child_location, references_json_value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_json_value_schema(schemas: &Map) -> Result<(), Box<dyn Error>> {
    let schema = schemas
        .get(JSON_VALUE_SCHEMA_NAME)
        .ok_or("release OpenAPI document references missing component schema JsonValue")?;
    let canonical = norito::json!({
        "additionalProperties": true,
        "description": "Arbitrary JSON payload.",
        "type": ["object", "array", "string", "number", "boolean", "null"]
    });
    if schema != &canonical {
        return Err(
            "release OpenAPI component schema JsonValue must be the canonical arbitrary-JSON union"
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release_spec_with_reference(reference: &str) -> Value {
        release_spec_with_reference_value(Value::String(reference.to_owned()))
    }

    fn release_spec_with_reference_value(reference: Value) -> Value {
        norito::json!({
            "openapi": "3.1.0",
            "info": {"title": "Torii fixture", "version": "1.0.0"},
            "paths": {
                "/config": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "ok",
                                "content": {
                                    "application/json": {
                                        "schema": {"$ref": (reference)}
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "components": {"schemas": {"Health": {"type": "object"}}}
        })
    }

    fn insert_json_value_schema(spec: &mut Value, schema: Value) {
        spec.as_object_mut()
            .and_then(|document| document.get_mut("components"))
            .and_then(Value::as_object_mut)
            .and_then(|components| components.get_mut("schemas"))
            .and_then(Value::as_object_mut)
            .expect("fixture components.schemas")
            .insert(JSON_VALUE_SCHEMA_NAME.to_owned(), schema);
    }

    fn release_spec_with_header_reference(reference: &str) -> Value {
        norito::json!({
            "openapi": "3.1.0",
            "info": {"title": "Torii fixture", "version": "1.0.0"},
            "paths": {
                "/config": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "ok",
                                "headers": {"X-Iroha-Signature": {"$ref": (reference)}}
                            }
                        }
                    }
                }
            },
            "components": {
                "schemas": {"Health": {"type": "object"}},
                "headers": {
                    "IrohaSignature": {
                        "required": true,
                        "schema": {"type": "string"}
                    }
                }
            }
        })
    }

    #[test]
    fn referenced_json_value_requires_the_canonical_schema() {
        let spec = release_spec_with_reference("#/components/schemas/JsonValue");
        let error = validate_release_openapi_spec(&spec)
            .expect_err("a referenced but missing JsonValue schema must fail")
            .to_string();
        assert!(error.contains("missing component schema JsonValue"));
    }

    #[test]
    fn malformed_json_value_schema_is_rejected() {
        let mut spec = release_spec_with_reference("#/components/schemas/JsonValue");
        insert_json_value_schema(
            &mut spec,
            norito::json!({
                "additionalProperties": false,
                "description": "Arbitrary JSON payload.",
                "type": ["object", "array", "string", "number", "boolean", "null"]
            }),
        );
        let error = validate_release_openapi_spec(&spec)
            .expect_err("a non-canonical JsonValue schema must fail")
            .to_string();
        assert!(error.contains("canonical arbitrary-JSON union"));
    }

    #[test]
    fn canonical_json_value_schema_is_accepted() {
        let mut spec = release_spec_with_reference("#/components/schemas/JsonValue");
        insert_json_value_schema(
            &mut spec,
            norito::json!({
                "additionalProperties": true,
                "description": "Arbitrary JSON payload.",
                "type": ["object", "array", "string", "number", "boolean", "null"]
            }),
        );
        validate_release_openapi_spec(&spec).expect("canonical JsonValue schema must pass");
    }

    #[test]
    fn every_local_component_schema_reference_must_resolve_recursively() {
        let spec = release_spec_with_reference("#/components/schemas/Missing");
        let error = validate_release_openapi_spec(&spec)
            .expect_err("a nested missing component schema must fail")
            .to_string();
        assert!(error.contains("missing component schema Missing"));
        assert!(error.contains("$.paths./config.get.responses.200"));
    }

    #[test]
    fn local_component_header_reference_must_resolve() {
        let spec = release_spec_with_header_reference("#/components/headers/IrohaSignature");
        validate_release_openapi_spec(&spec).expect("local component header must resolve");
    }

    #[test]
    fn missing_local_component_header_is_rejected() {
        let spec = release_spec_with_header_reference("#/components/headers/Missing");
        let error = validate_release_openapi_spec(&spec)
            .expect_err("a missing component header must fail")
            .to_string();
        assert!(error.contains("missing component header Missing"));
        assert!(error.contains("$.paths./config.get.responses.200.headers"));
    }

    #[test]
    fn malformed_or_unsupported_component_references_are_rejected() {
        for reference in [
            "https://example.invalid/schema.json",
            "#/components/schemas/Health/properties/status",
            "#/components/schemas/",
            "#/components/schemas",
            "#/components/responses/Success",
        ] {
            let spec = release_spec_with_reference(reference);
            validate_release_openapi_spec(&spec)
                .expect_err("external, nested, empty, and unsupported references must fail");
        }
    }

    #[test]
    fn non_string_component_reference_is_rejected() {
        let spec = release_spec_with_reference_value(norito::json!(7));
        let error = validate_release_openapi_spec(&spec)
            .expect_err("a non-string reference must fail")
            .to_string();
        assert!(error.contains("must be a string"));
    }
}
