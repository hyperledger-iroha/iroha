fn operator_signature_header_parameters() -> Vec<Value> {
    vec![
        string_header_param(
            "X-Iroha-Operator-Public-Key",
            "Iroha multihash public key of the canonical request signer.",
            true,
        ),
        string_header_param(
            "X-Iroha-Operator-Timestamp-Ms",
            "Unix timestamp in milliseconds bound into the operator request signature.",
            true,
        ),
        string_header_param(
            "X-Iroha-Operator-Nonce",
            "Fresh caller-chosen nonce bound into the operator request signature.",
            true,
        ),
        string_header_param(
            "X-Iroha-Operator-Signature",
            "Base64 signature over the canonical method, path, sorted query, body hash, timestamp, and nonce.",
            true,
        ),
    ]
}

pub(super) fn insert_operator_signature_auth_contract(operation: &mut Map) {
    operation.insert(
        "security".into(),
        norito::json!([{
            "IrohaOperatorPublicKey": [],
            "IrohaOperatorTimestampMs": [],
            "IrohaOperatorNonce": [],
            "IrohaOperatorSignature": []
        }]),
    );
    operation.insert(
        "x-iroha-operator-signature-v1".into(),
        norito::json!({
            "exact_network_id": true,
            "exact_method": true,
            "exact_path_and_sorted_query": true,
            "empty_body_hash": true,
            "fresh_timestamp_and_nonce": true,
            "replay_rejected": true,
            "redirects": false,
            "retries": false,
            "token_fallback": false
        }),
    );
}

pub(super) fn insert_operator_signature_error_responses(responses: &mut Map) {
    responses.insert(
        "401".to_owned(),
        dual_format_response(
            "Operator request signature is missing, stale, replayed, or invalid.",
            "#/components/schemas/ErrorEnvelope",
        ),
    );
    responses.insert(
        "403".to_owned(),
        dual_format_response(
            "Operator signing key is not allow-listed.",
            "#/components/schemas/ErrorEnvelope",
        ),
    );
}

fn kaigi_relays_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        dual_format_response(
            "Relay summaries retrieved.",
            "#/components/schemas/KaigiRelaySummaryList",
        ),
    );
    insert_operator_signature_error_responses(&mut responses);
    responses.insert(
        "422".to_owned(),
        dual_format_response(
            "Relay registry exceeds the hard diagnostic snapshot cap.",
            "#/components/schemas/ErrorEnvelope",
        ),
    );
    responses.insert(
        "503".to_owned(),
        dual_format_response(
            "Telemetry profile does not permit relay telemetry.",
            "#/components/schemas/ErrorEnvelope",
        ),
    );
    responses
}

fn kaigi_relays_health_responses() -> Map {
    let mut responses = Map::new();
    responses.insert(
        "200".to_owned(),
        dual_format_response(
            "Relay health snapshot retrieved.",
            "#/components/schemas/KaigiRelayHealthSnapshot",
        ),
    );
    insert_operator_signature_error_responses(&mut responses);
    responses.insert(
        "422".to_owned(),
        dual_format_response(
            "Relay registry exceeds the hard diagnostic aggregation cap.",
            "#/components/schemas/ErrorEnvelope",
        ),
    );
    responses.insert(
        "503".to_owned(),
        dual_format_response(
            "Telemetry profile does not permit relay telemetry.",
            "#/components/schemas/ErrorEnvelope",
        ),
    );
    responses
}

fn kaigi_relays_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Kaigi".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("List registered Kaigi relays with their latest health sample.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns relay identifiers, domains, bandwidth classes, HPKE fingerprints, \
             and the freshest health report (if available). This expensive operator-only \
             snapshot requires a fresh one-shot signature bound to the exact NetworkId, GET \
             method, path, sorted query, and empty body; redirects and retries are forbidden. \
             The snapshot fails closed with 422 before materializing more than the hard relay \
             diagnostic cap."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("kaigiRelaysList".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(operator_signature_header_parameters()),
    );
    insert_operator_signature_auth_contract(&mut operation);
    operation.insert("responses".into(), Value::Object(kaigi_relays_responses()));
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}

fn kaigi_relays_health_operation() -> Map {
    let mut operation = Map::new();
    operation.insert(
        "tags".into(),
        Value::Array(vec![Value::String("Kaigi".to_owned())]),
    );
    operation.insert(
        "summary".into(),
        Value::String("Aggregate Kaigi relay health counters across the network.".to_owned()),
    );
    operation.insert(
        "description".into(),
        Value::String(
            "Returns totals for healthy/degraded/unavailable relays along with \
             registrations, failovers, and per-domain counters. This expensive operator-only \
             aggregate requires a fresh one-shot signature bound to the exact NetworkId, GET \
             method, path, sorted query, and empty body; redirects and retries are forbidden. \
             Aggregation fails closed with 422 before scanning more than the hard relay \
             diagnostic cap."
                .to_owned(),
        ),
    );
    operation.insert(
        "operationId".into(),
        Value::String("kaigiRelayHealth".to_owned()),
    );
    operation.insert(
        "parameters".into(),
        Value::Array(operator_signature_header_parameters()),
    );
    insert_operator_signature_auth_contract(&mut operation);
    operation.insert(
        "responses".into(),
        Value::Object(kaigi_relays_health_responses()),
    );
    let mut methods = Map::new();
    methods.insert("get".to_owned(), Value::Object(operation));
    methods
}
