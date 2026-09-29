#[test]
fn get_sumeragi_status_rejects_unknown_json_fields() {
    let client = client_with_base_url(base_url());
    let mut value =
        norito::json::to_value(&sample_sumeragi_status()).expect("serialize status fixture");
    value.as_object_mut().expect("status object").insert(
        "legacy_height".to_owned(),
        norito::json::Value::from(12_u64),
    );
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", APPLICATION_JSON)
        .body(norito::json::to_vec(&value).expect("encode adversarial status JSON"))
        .unwrap();
    let result = with_mock_http(
        respond_with(&Arc::new(Mutex::new(Vec::new())), response),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            client.get_sumeragi_status()
        },
    );
    assert!(result.is_err(), "unknown status fields must be rejected");

    let diagnostics = sample_sumeragi_diagnostics();
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", APPLICATION_JSON)
        .body(norito::json::to_vec(&diagnostics).expect("encode diagnostics-shaped JSON"))
        .unwrap();
    let result = with_mock_http(
        respond_with(&Arc::new(Mutex::new(Vec::new())), response),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            client.get_sumeragi_status()
        },
    );
    assert!(
        result.is_err(),
        "status endpoint must reject a diagnostics-shaped payload"
    );
}

#[test]
fn native_status_versions_are_checked_in_every_client_response() {
    use iroha_data_model::sumeragi_lanes::{
        SumeragiLaneFrontier, SumeragiLaneRecord, SumeragiLaneStatus,
    };

    for version in [iroha_data_model::sumeragi::PROTOCOL_VERSION, 0, 2, 4, 8] {
        let mut status = sample_sumeragi_status();
        status.protocol_version = version;
        let accepted = version == iroha_data_model::sumeragi::PROTOCOL_VERSION;
        let lanes = vec![SumeragiLaneStatus {
            record: SumeragiLaneRecord {
                da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
                lane: LaneId::new(1),
                dataspace: DataSpaceId::UNIVERSAL,
                incarnation: [1; 32],
                params: iroha_data_model::parameter::system::SumeragiParameters::default(),
                committee: Vec::new(),
                created_at: 1,
                active_from: 3,
                closing: None,
                anchor_freshness: 10,
                merged: SumeragiLaneFrontier::default(),
                merged_at: 3,
                rescued: 0,
            },
            instance: Some(status.clone()),
        }];
        for content_type in [APPLICATION_NORITO, APPLICATION_JSON] {
            for lane_response in [false, true] {
                let body = match (content_type, lane_response) {
                    (APPLICATION_NORITO, false) => norito::to_bytes(&status).unwrap(),
                    (APPLICATION_NORITO, true) => norito::to_bytes(&lanes).unwrap(),
                    (_, false) => norito::json::to_vec(&status).unwrap(),
                    (_, true) => norito::json::to_vec(&lanes).unwrap(),
                };
                let result = with_mock_http(
                    respond_with(
                        &Arc::new(Mutex::new(Vec::new())),
                        mk_response(StatusCode::OK, body, Some(content_type)),
                    ),
                    |transport| {
                        let client = client_with_base_url(base_url())
                            .with_test_http_transport(transport.clone());
                        if lane_response {
                            client.get_sumeragi_lanes().map(|_| ())
                        } else {
                            client.get_sumeragi_status().map(|_| ())
                        }
                    },
                );
                assert_eq!(result.is_ok(), accepted, "version {version}: {result:?}");
                if !accepted {
                    assert!(result.unwrap_err().to_string().contains("protocol version"));
                }
            }
        }
        let result = with_mock_http(
            respond_with(
                &Arc::new(Mutex::new(Vec::new())),
                mk_response(
                    StatusCode::OK,
                    norito::json::to_vec(&status).unwrap(),
                    Some(APPLICATION_JSON),
                ),
            ),
            |transport| {
                client_with_base_url(base_url())
                    .with_test_http_transport(transport.clone())
                    .get_sumeragi_status_json()
            },
        );
        assert_eq!(
            result.is_ok(),
            accepted,
            "JSON version {version}: {result:?}"
        );
    }
}

#[test]
fn get_sumeragi_lanes_decodes_the_shared_rust_lane_corpus() {
    // Rows are complete `GET /v1/sumeragi/lanes` bodies emitted by
    // `kotlin-fixture-gen native-sumeragi-lanes-v1` and parsed by every SDK.
    const CORPUS: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sumeragi/native_lanes_v1.tsv"
    ));
    let mut names = Vec::new();
    for line in CORPUS.lines().filter(|line| !line.starts_with('#')) {
        let mut columns = line.split('\t');
        let (Some(name), Some(json), Some(norito_hex), None) = (
            columns.next(),
            columns.next(),
            columns.next(),
            columns.next(),
        ) else {
            panic!("malformed lane corpus row: {line}");
        };
        let norito = hex::decode(norito_hex).expect("lane corpus Norito column is hex");
        let mut decoded = Vec::new();
        for (content_type, body) in [
            (APPLICATION_JSON, json.as_bytes().to_vec()),
            (APPLICATION_NORITO, norito),
        ] {
            let (lanes, snapshot) = capture_request(
                mk_response(StatusCode::OK, body, Some(content_type)),
                |transport| {
                    client_with_base_url(base_url())
                        .with_test_http_transport(transport.clone())
                        .get_sumeragi_lanes()
                },
            );
            let lanes = lanes.unwrap_or_else(|error| panic!("{name} as {content_type}: {error}"));
            assert_eq!(snapshot.method, HttpMethod::GET);
            assert_eq!(snapshot.url.path(), "/v1/sumeragi/lanes");
            assert!(snapshot.body.is_empty(), "the lane list is a bodiless GET");
            assert_operator_signature_headers(&snapshot);
            decoded.push(lanes);
        }
        assert_eq!(
            decoded[0], decoded[1],
            "{name}: the JSON and Norito bodies of one response decode to the same lanes"
        );
        assert_eq!(
            norito::json::to_json(&decoded[0]).expect("re-encode decoded lanes"),
            json,
            "{name}: decoded lanes re-encode to the served JSON body"
        );
        names.push(name);
    }
    assert_eq!(names, ["empty", "running_lane", "mixed_lanes"]);
}
