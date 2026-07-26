//! Capability refusal fixture loader for the SoraFS gateway conformance suite (SF-5c).
//!
//! This module provides helpers for integration tests and tooling to consume the
//! refusal scenario fixtures defined in `fixtures/sorafs_gateway/capability_refusal`.
//! Scenarios drive the capability refusal conformance suite, operator
//! self-certification kit, and SDK regression matrix.

use std::{
    fs,
    path::{Path, PathBuf},
};

use eyre::{Result as EyreResult, WrapErr};
use norito::json::{self, Value};

const SCENARIOS_FILE: &str = "scenarios.json";

/// Describes a single capability refusal scenario.
#[derive(Debug, Clone)]
pub struct CapabilityRefusalScenario {
    /// Stable identifier (e.g., `C1`) matching roadmap documentation.
    pub id: String,
    /// Human-readable description of the refusal.
    pub description: String,
    /// HTTP status code returned by the gateway.
    pub status: u16,
    /// Machine-readable error code mirrored in telemetry/logs.
    pub error: String,
    /// Operator-facing reason string.
    pub reason: String,
    /// Optional detail payload providing structured context.
    pub details: Value,
    /// Optional relative path pointing at the request fixture.
    pub request_fixture: Option<String>,
    /// Optional relative path pointing at the response fixture.
    pub response_fixture: Option<String>,
    /// Optional relative path pointing at the gateway mutation fixture.
    pub gateway_fixture: Option<String>,
    /// Canonical request payload stored for the scenario.
    pub request: Option<Value>,
    /// Canonical response payload stored for the scenario.
    pub response: Option<Value>,
    /// Canonical gateway mutation payload stored for the scenario.
    pub gateway: Option<Value>,
}

/// Canonical response emitted by the gateway for a capability refusal.
#[derive(Debug, Clone)]
pub struct CapabilityRefusalResponse {
    /// HTTP status code expected from the gateway.
    pub status: u16,
    /// Canonical JSON value representing the refusal body.
    pub body: Value,
    /// Canonical request payload associated with the refusal.
    pub request: Option<Value>,
    /// Canonical gateway mutation payload associated with the refusal.
    pub gateway: Option<Value>,
}

/// Load all capability refusal scenarios from disk.
///
/// # Errors
///
/// Returns an error if the fixture directory cannot be read, parsed, or contains
/// malformed entries.
pub fn load_scenarios() -> EyreResult<Vec<CapabilityRefusalScenario>> {
    let root = fixtures_root();
    let path = root.join(SCENARIOS_FILE);
    let bytes = fs::read(&path).wrap_err_with(|| format!("failed to read {}", display(&path)))?;
    let value: Value =
        json::from_slice(&bytes).wrap_err("failed to parse capability refusal scenarios JSON")?;
    let array = value
        .as_array()
        .ok_or_else(|| eyre::eyre!("capability refusal scenarios file must contain an array"))?;

    array
        .iter()
        .map(|entry| {
            parse_scenario(&root, entry).wrap_err("invalid capability refusal scenario entry")
        })
        .collect()
}

/// Load capability refusal scenarios and convert them into canonical responses.
///
/// # Errors
///
/// Propagates errors from [`load_scenarios`] or the conversion routines.
pub fn load_responses() -> EyreResult<Vec<CapabilityRefusalResponse>> {
    load_scenarios()?
        .into_iter()
        .map(|scenario| Ok(CapabilityRefusalResponse::from(&scenario)))
        .collect()
}

fn parse_scenario(root: &Path, value: &Value) -> EyreResult<CapabilityRefusalScenario> {
    let map = value
        .as_object()
        .cloned()
        .ok_or_else(|| eyre::eyre!("scenario entry must be a JSON object"))?;

    let id = required_string(&map, "id")?;
    let description = required_string(&map, "description")?;
    let status = required_u64(&map, "status")?;
    let status =
        u16::try_from(status).wrap_err_with(|| format!("status out of range for scenario {id}"))?;
    let error = required_string(&map, "error")?;
    let reason = required_string(&map, "reason")?;
    let details = map.get("details").cloned().unwrap_or(Value::Null);
    let request_fixture = map
        .get("request_fixture")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let response_fixture = map
        .get("response_fixture")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let gateway_fixture = map
        .get("gateway_fixture")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let request = load_optional_fixture(root, request_fixture.as_deref(), &id, "request")?;
    let response = load_optional_fixture(root, response_fixture.as_deref(), &id, "response")?;
    let gateway = load_optional_fixture(root, gateway_fixture.as_deref(), &id, "gateway")?;

    Ok(CapabilityRefusalScenario {
        id,
        description,
        status,
        error,
        reason,
        details,
        request_fixture,
        response_fixture,
        gateway_fixture,
        request,
        response,
        gateway,
    })
}

fn required_string(map: &json::Map, key: &str) -> EyreResult<String> {
    map.get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| eyre::eyre!("missing or invalid `{key}` field in capability refusal entry"))
}

fn required_u64(map: &json::Map, key: &str) -> EyreResult<u64> {
    map.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre::eyre!("missing or invalid `{key}` field in capability refusal entry"))
}

/// Root directory that contains the capability refusal fixtures.
pub fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/sorafs_gateway/capability_refusal")
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

fn load_optional_fixture(
    root: &Path,
    relative: Option<&str>,
    scenario_id: &str,
    kind: &str,
) -> EyreResult<Option<Value>> {
    let Some(rel_path) = relative else {
        return Ok(None);
    };
    let path = root.join(rel_path);
    let bytes = fs::read(&path).wrap_err_with(|| {
        format!(
            "failed to read {kind} fixture for scenario {} at {}",
            scenario_id,
            display(&path)
        )
    })?;
    let value = json::from_slice(&bytes).wrap_err_with(|| {
        format!(
            "failed to parse {kind} fixture JSON for scenario {} ({})",
            scenario_id,
            display(&path)
        )
    })?;
    Ok(Some(value))
}

impl From<&CapabilityRefusalScenario> for CapabilityRefusalResponse {
    fn from(scenario: &CapabilityRefusalScenario) -> Self {
        let body = build_response_body(scenario);
        CapabilityRefusalResponse {
            status: scenario.status,
            body,
            request: scenario.request.clone(),
            gateway: scenario.gateway.clone(),
        }
    }
}

fn build_response_body(scenario: &CapabilityRefusalScenario) -> Value {
    match scenario.response.clone() {
        Some(Value::Object(mut map)) => {
            map.entry("error".to_string())
                .or_insert_with(|| Value::from(scenario.error.clone()));
            map.entry("reason".to_string())
                .or_insert_with(|| Value::from(scenario.reason.clone()));
            let details_source = map
                .get("details")
                .cloned()
                .unwrap_or_else(|| scenario.details.clone());
            map.insert("details".into(), normalize_details(&details_source));
            Value::Object(map)
        }
        Some(other) => {
            let mut map = json::Map::new();
            map.insert("error".into(), Value::from(scenario.error.clone()));
            map.insert("reason".into(), Value::from(scenario.reason.clone()));
            let mut details_map = json::Map::new();
            details_map.insert("raw".into(), other);
            map.insert(
                "details".into(),
                normalize_details(&Value::Object(details_map)),
            );
            Value::Object(map)
        }
        None => {
            let mut map = json::Map::new();
            map.insert("error".into(), Value::from(scenario.error.clone()));
            map.insert("reason".into(), Value::from(scenario.reason.clone()));
            map.insert(
                "details".into(),
                normalize_details(&scenario.details.clone()),
            );
            Value::Object(map)
        }
    }
}

fn normalize_details(details: &Value) -> Value {
    match details {
        Value::Object(map) if !map.is_empty() => Value::Object(map.clone()),
        Value::Array(arr) if !arr.is_empty() => Value::Array(arr.clone()),
        _ => {
            let mut map = json::Map::new();
            map.insert("scenario".into(), Value::from("capability_refusal"));
            Value::Object(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenarios_fixture_is_well_formed() {
        let scenarios = load_scenarios().expect("load capability refusal scenarios");
        assert!(
            !scenarios.is_empty(),
            "capability refusal fixtures must contain at least one scenario"
        );
        for scenario in scenarios {
            assert!(
                !scenario.id.trim().is_empty(),
                "scenario id must not be empty"
            );
            assert!(
                !scenario.description.trim().is_empty(),
                "scenario description must not be empty"
            );
            assert!(
                !scenario.error.trim().is_empty(),
                "scenario error code must not be empty"
            );
            assert!(
                !scenario.reason.trim().is_empty(),
                "scenario reason must not be empty"
            );
        }
    }
}
