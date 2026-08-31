//! Fail-closed validation for OpenAPI release artifacts.

use norito::json::{Map, Value};
use std::error::Error;

const JSON_VALUE_SCHEMA_NAME: &str = "JsonValue";
const JSON_VALUE_SCHEMA_REF: &str = "#/components/schemas/JsonValue";
const COMPONENT_REF_PREFIX: &str = "#/components/";

pub(crate) fn validate_release_openapi_spec(spec: &Value) -> Result<(), Box<dyn Error>> {
    let document = spec
        .as_object()
        .ok_or("release OpenAPI document must be a JSON object")?;
    let version = document
        .get("openapi")
        .and_then(Value::as_str)
        .ok_or("release OpenAPI document is missing the openapi version")?;
    if !is_supported_openapi_version(version) {
        return Err(format!(
            "release OpenAPI document uses unsupported version `{version}`; expected OpenAPI 3.1.x"
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
    validate_component_refs(
        spec,
        components,
        "$",
        ReferenceContext::Document,
        &mut references_json_value,
    )?;
    if references_json_value || schemas.contains_key(JSON_VALUE_SCHEMA_NAME) {
        validate_json_value_schema(schemas)?;
    }
    Ok(())
}

fn is_supported_openapi_version(version: &str) -> bool {
    let mut segments = version.split('.');
    matches!(segments.next(), Some("3"))
        && matches!(segments.next(), Some("1"))
        && segments.next().is_some_and(|patch| {
            !patch.is_empty() && patch.bytes().all(|byte| byte.is_ascii_digit())
        })
        && segments.next().is_none()
}

#[derive(Clone, Copy)]
enum ReferenceContext {
    Document,
    Components,
    SchemaMap,
    Schema,
    HeaderMap,
    Header,
}

impl ReferenceContext {
    fn child(self, key: &str) -> Self {
        match self {
            Self::Schema | Self::SchemaMap => Self::Schema,
            Self::HeaderMap => Self::Header,
            Self::Components => match key {
                "schemas" => Self::SchemaMap,
                "headers" => Self::HeaderMap,
                _ => Self::Document,
            },
            Self::Document if key == "components" => Self::Components,
            Self::Document if key == "headers" => Self::HeaderMap,
            Self::Document | Self::Header if is_schema_position(key) => Self::Schema,
            Self::Document => Self::Document,
            Self::Header => Self::Header,
        }
    }

    fn expected_component(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Schema => Some(("schemas", "schema")),
            Self::Header => Some(("headers", "header")),
            _ => None,
        }
    }
}

fn is_schema_position(key: &str) -> bool {
    key == "schema" || key.ends_with("-schema")
}

fn validate_component_refs(
    value: &Value,
    components: &Map,
    location: &str,
    context: ReferenceContext,
    references_json_value: &mut bool,
) -> Result<(), Box<dyn Error>> {
    match value {
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_component_refs(
                    value,
                    components,
                    &format!("{location}[{index}]"),
                    context,
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
                        .strip_prefix(COMPONENT_REF_PREFIX)
                        .ok_or_else(|| {
                            format!(
                                "release OpenAPI $ref at {child_location} must target a local component root: {reference}"
                            )
                        })?;
                    let mut segments = component_path.split('/');
                    let kind = segments.next().unwrap_or_default();
                    let name = segments.next().unwrap_or_default();
                    if kind.is_empty() || name.is_empty() || segments.next().is_some() {
                        return Err(format!(
                            "release OpenAPI $ref at {child_location} must target a component root: {reference}"
                        )
                        .into());
                    }
                    let (expected_kind, singular_kind) = context.expected_component().ok_or_else(
                        || {
                            format!(
                                "release OpenAPI $ref at {child_location} is not permitted at this location: {reference}"
                            )
                        },
                    )?;
                    if kind != expected_kind {
                        return Err(format!(
                            "release OpenAPI $ref at {child_location} must target a local component {singular_kind} root at this location: {reference}"
                        )
                        .into());
                    }
                    let collection = components
                        .get(kind)
                        .and_then(Value::as_object)
                        .ok_or_else(|| {
                            format!(
                                "release OpenAPI $ref at {child_location} targets missing component {singular_kind} {name}"
                            )
                        })?;
                    if !collection.contains_key(name) {
                        return Err(format!(
                            "release OpenAPI $ref at {child_location} targets missing component {singular_kind} {name}"
                        )
                        .into());
                    }
                    *references_json_value |= reference == JSON_VALUE_SCHEMA_REF;
                }
                validate_component_refs(
                    value,
                    components,
                    &child_location,
                    context.child(key),
                    references_json_value,
                )?;
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
                                "headers": {
                                    "X-Trace": {"$ref": (reference)}
                                },
                                "content": {
                                    "application/json": {
                                        "schema": {"$ref": "#/components/schemas/Health"}
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

    fn insert_component(spec: &mut Value, kind: &str, name: &str, component: Value) {
        let components = spec
            .as_object_mut()
            .and_then(|document| document.get_mut("components"))
            .and_then(Value::as_object_mut)
            .expect("fixture components");
        components
            .entry(kind.to_owned())
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("fixture component collection")
            .insert(name.to_owned(), component);
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
    fn local_component_header_reference_is_accepted() {
        let mut spec = release_spec_with_header_reference("#/components/headers/TraceHeader");
        insert_component(
            &mut spec,
            "headers",
            "TraceHeader",
            norito::json!({"schema": {"type": "string"}}),
        );
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
    fn component_reference_kind_must_match_its_openapi_location() {
        let mut schema_position = release_spec_with_reference("#/components/headers/TraceHeader");
        insert_component(
            &mut schema_position,
            "headers",
            "TraceHeader",
            norito::json!({"schema": {"type": "string"}}),
        );
        let error = validate_release_openapi_spec(&schema_position)
            .expect_err("a header reference in a schema position must fail")
            .to_string();
        assert!(error.contains("component schema root at this location"));

        let header_position = release_spec_with_header_reference("#/components/schemas/Health");
        let error = validate_release_openapi_spec(&header_position)
            .expect_err("a schema reference in a header position must fail")
            .to_string();
        assert!(error.contains("component header root at this location"));
    }

    #[test]
    fn schema_named_headers_keeps_schema_reference_context() {
        let mut spec = release_spec_with_reference("#/components/schemas/Health");
        insert_component(
            &mut spec,
            "schemas",
            "Health",
            norito::json!({
                "type": "object",
                "properties": {
                    "headers": {
                        "type": "object",
                        "additionalProperties": {"$ref": "#/components/schemas/Health"}
                    }
                }
            }),
        );
        validate_release_openapi_spec(&spec)
            .expect("a schema property named headers must remain in schema context");
    }

    #[test]
    fn only_openapi_3_1_documents_are_supported() {
        for version in ["3.0.3", "3.2.0", "3.1", "3.1.x"] {
            let mut spec = release_spec_with_reference("#/components/schemas/Health");
            spec.as_object_mut()
                .expect("fixture document")
                .insert("openapi".to_owned(), Value::String(version.to_owned()));
            let error = validate_release_openapi_spec(&spec)
                .expect_err("unsupported OpenAPI version must fail")
                .to_string();
            assert!(
                error.contains("expected OpenAPI 3.1.x"),
                "{version}: {error}"
            );
        }
        for version in ["3.1.0", "3.1.1"] {
            let mut spec = release_spec_with_reference("#/components/schemas/Health");
            spec.as_object_mut()
                .expect("fixture document")
                .insert("openapi".to_owned(), Value::String(version.to_owned()));
            validate_release_openapi_spec(&spec)
                .unwrap_or_else(|error| panic!("{version} must be accepted: {error}"));
        }
    }

    #[test]
    fn nested_component_reference_is_rejected() {
        let spec = release_spec_with_reference("#/components/schemas/Health/properties/status");
        let error = validate_release_openapi_spec(&spec)
            .expect_err("a nested component reference must fail closed")
            .to_string();
        assert!(error.contains("must target a component root"));
    }

    #[test]
    fn non_string_reference_is_rejected() {
        let spec = release_spec_with_reference_value(Value::Bool(true));
        let error = validate_release_openapi_spec(&spec)
            .expect_err("a non-string component reference must fail closed")
            .to_string();
        assert!(error.contains("must be a string"));
    }

    #[test]
    fn external_reference_is_rejected() {
        let spec = release_spec_with_reference("https://example.invalid/schema.json");
        let error = validate_release_openapi_spec(&spec)
            .expect_err("an external reference must fail closed")
            .to_string();
        assert!(error.contains("must target a local component root"));
    }
}
