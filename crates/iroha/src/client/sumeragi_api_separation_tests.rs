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
        || client.get_sumeragi_status(),
    );
    assert!(result.is_err(), "unknown status fields must be rejected");

    let (diagnostics, _) = sample_sumeragi_status_with_relay();
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", APPLICATION_JSON)
        .body(norito::json::to_vec(&diagnostics).expect("encode diagnostics-shaped JSON"))
        .unwrap();
    let result = with_mock_http(
        respond_with(&Arc::new(Mutex::new(Vec::new())), response),
        || client.get_sumeragi_status(),
    );
    assert!(
        result.is_err(),
        "status endpoint must reject a diagnostics-shaped payload"
    );
}

#[test]
fn get_sumeragi_diagnostics_rejects_json_payload_missing_required_fields() {
    let client = client_with_base_url(base_url());
    let response = HttpResponse::builder()
        .status(StatusCode::OK)
        .header("content-type", APPLICATION_JSON)
        .body(br"{}".to_vec())
        .unwrap();
    let result = with_mock_http(
        respond_with(&Arc::new(Mutex::new(Vec::new())), response),
        || client.get_sumeragi_diagnostics(),
    );
    assert!(
        result.is_err(),
        "structurally invalid json payload should be rejected"
    );

    let (diagnostics, _) = sample_sumeragi_status_with_relay();
    let mut value =
        norito::json::to_value(&diagnostics).expect("serialize diagnostics fixture");
    value
        .as_object_mut()
        .expect("diagnostics object")
        .remove("autonomous_lane_executions");
    let response = HttpResponse::builder()
        .status(StatusCode::OK)
        .header("content-type", APPLICATION_JSON)
        .body(norito::json::to_vec(&value).expect("encode incomplete diagnostics JSON"))
        .unwrap();
    let result = with_mock_http(
        respond_with(&Arc::new(Mutex::new(Vec::new())), response),
        || client.get_sumeragi_diagnostics(),
    );
    assert!(
        result.is_err(),
        "the first-release autonomous diagnostics vector is required"
    );

    let response = HttpResponse::builder()
        .status(StatusCode::OK)
        .header("content-type", APPLICATION_JSON)
        .body(
            norito::json::to_vec(&sample_sumeragi_status())
                .expect("encode status-shaped JSON"),
        )
        .unwrap();
    let result = with_mock_http(
        respond_with(&Arc::new(Mutex::new(Vec::new())), response),
        || client.get_sumeragi_diagnostics(),
    );
    assert!(
        result.is_err(),
        "diagnostics endpoint must reject a status-shaped payload"
    );
}
