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
    for name in [
        "SumeragiV2HeightContext",
        "SumeragiV2FinalizedNextEpochSnapshot",
    ] {
        let properties = schemas
            .get(name)
            .unwrap()
            .get("properties")
            .and_then(Value::as_object)
            .unwrap();
        assert!(properties.contains_key("kagemusha_mint_finality_authority"));
        assert!(properties.contains_key("kagemusha_mint_finality_authorization"));
        assert!(!properties.contains_key("kagemusha_mint_finality_epoch_id"));
        assert!(!properties.contains_key("kagemusha_mint_finality_epoch_roster"));
    }
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
