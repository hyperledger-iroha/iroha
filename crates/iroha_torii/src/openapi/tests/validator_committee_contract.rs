//! Closed OpenAPI contracts for generation authority and frozen committee preparation.

use super::*;

#[test]
fn validator_committee_openapi_is_closed_and_retires_epoch_roster_fields() {
    let schemas = openapi_schemas();
    for (name, fields) in [
        (
            "ValidatorCommitteeStatusV1",
            "network_id target_epoch latest_finality selected candidate_keys pending_beacon_session",
        ),
        (
            "ValidatorCommitteeSelectionStatusV1",
            "transition selecting_finality",
        ),
        (
            "ValidatorCommitteeTransitionV1",
            "preparation credentials readiness outcome",
        ),
        (
            "ValidatorCandidateKeysV1",
            "network_id generation keys possession peer_signature",
        ),
        (
            "KagemushaMintFinalityAuthorityGenerationV1",
            "version network_id generation validators",
        ),
        (
            "KagemushaMintFinalityEpochAuthorizationV1",
            "version network_id epoch first_height last_height authority_generation authority_id beacon previous_authorization_id transition_id decision",
        ),
    ] {
        let schema = schemas.get(name).unwrap();
        assert_eq!(
            schema.get("additionalProperties").and_then(Value::as_bool),
            Some(false)
        );
        let expected = fields.split_whitespace().collect::<BTreeSet<_>>();
        let properties = schema.get("properties").and_then(Value::as_object).unwrap();
        assert_eq!(
            properties
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            expected
        );
        assert_eq!(
            schema
                .get("required")
                .and_then(Value::as_array)
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect::<BTreeSet<_>>(),
            expected
        );
    }
    for (owner, field) in [
        ("ValidatorCommitteeStatusV1", "latest_finality"),
        ("ValidatorCommitteeSelectionStatusV1", "selecting_finality"),
    ] {
        assert_eq!(
            schemas[owner]["properties"][field]["$ref"].as_str(),
            Some("#/components/schemas/NativeFinalityArtifact")
        );
    }
    let native = &schemas["NativeFinalityArtifact"];
    assert_eq!(native["additionalProperties"].as_bool(), Some(false));
    assert_eq!(
        native["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["block_wire"]
    );
    assert_eq!(native["properties"].as_object().unwrap().len(), 1);
    assert_eq!(
        native["properties"]["block_wire"]["format"].as_str(),
        Some("byte")
    );
    assert_eq!(
        native["properties"]["block_wire"]["maxLength"].as_u64(),
        Some(44_739_244)
    );
    assert_eq!(
        schemas["ValidatorCommitteeStatusV1"]["properties"]["candidate_keys"]["maxItems"].as_u64(),
        Some(31)
    );
    let document = generate_spec();
    let paths = document.get("paths").and_then(Value::as_object).unwrap();
    let operation = paths
        .get("/v1/nexus/validator-committee")
        .unwrap()
        .get("get")
        .unwrap();
    assert_eq!(
        operation.get("x-iroha-tool-effect").and_then(Value::as_str),
        Some("read")
    );
    let response = operation
        .get("responses")
        .unwrap()
        .get("200")
        .unwrap()
        .get("content")
        .unwrap();
    for media in ["application/json", "application/x-norito"] {
        assert_eq!(
            response
                .get(media)
                .unwrap()
                .get("schema")
                .unwrap()
                .get("$ref")
                .and_then(Value::as_str),
            Some("#/components/schemas/ValidatorCommitteeStatusV1")
        );
    }
}
