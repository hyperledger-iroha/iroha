// Bounded node-status and parameter response contracts.

#[test]
fn decode_status_response_returns_err_on_internal_server_error() {
    let response = mk_response(StatusCode::INTERNAL_SERVER_ERROR, Vec::new(), None);
    assert!(Client::decode_status_for_test(&response).is_err());
}

#[test]
fn get_status_does_not_retry_as_json_after_decode_failure() {
    let expected = Status::default();
    let json_body = norito::json::to_vec(&expected).expect("serialize status JSON");
    let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let responder = {
        let snapshots = Arc::clone(&snapshots);
        move |snapshot: RequestSnapshot| {
            let attempt = {
                let mut snapshots = snapshots.lock().expect("snapshot lock");
                snapshots.push(snapshot);
                snapshots.len()
            };
            if attempt == 1 {
                Ok(mk_response(
                    StatusCode::OK,
                    b"invalid Norito status".to_vec(),
                    Some(APPLICATION_NORITO),
                ))
            } else {
                Ok(mk_response(
                    StatusCode::OK,
                    json_body.clone(),
                    Some(APPLICATION_JSON),
                ))
            }
        }
    };
    let error = with_mock_http(responder, || client_with_base_url(base_url()).get_status())
        .expect_err("malformed negotiated status response must fail without retry");
    assert!(error.to_string().contains("failed to decode status Norito"));
    let snapshots = snapshots.lock().expect("snapshot lock");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].url.path(), torii_uri::STATUS);
    assert_eq!(
        snapshots[0].max_response_bytes,
        NODE_STATUS_RESPONSE_MAX_BYTES
    );
    assert_single_accept_header(&snapshots[0], ACCEPT_NORITO_PREFERRED);
}

#[test]
fn get_status_norito_only_rejects_json_without_retry() {
    let body = norito::json::to_vec(&Status::default()).expect("serialize status JSON");
    let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let response = mk_response(StatusCode::OK, body, Some(APPLICATION_JSON));
    let mut client = client_with_base_url(base_url());
    client.set_wire_format_preference(WireFormatPreference::NoritoOnly);
    let error = with_mock_http(respond_with(&snapshots, response), || client.get_status())
        .expect_err("NoritoOnly must reject a JSON response");
    assert!(error.to_string().contains("violates NoritoOnly"));
    let snapshots = snapshots.lock().expect("snapshot lock");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].url.path(), torii_uri::STATUS);
    assert_eq!(
        snapshots[0].max_response_bytes,
        NODE_STATUS_RESPONSE_MAX_BYTES
    );
    assert_single_accept_header(&snapshots[0], APPLICATION_NORITO);
}

#[test]
fn decode_parameters_response_returns_err_on_bad_status() {
    let response = mk_response(StatusCode::BAD_REQUEST, Vec::new(), None);
    assert!(decode_parameters_for_test(&response).is_err());
}

#[test]
fn transaction_response_handler_rejects_oversized_success() {
    let body = vec![0; TRANSACTION_SUBMISSION_RESPONSE_MAX_BYTES + 1];
    let response = mk_response(StatusCode::ACCEPTED, body, None);
    assert!(TransactionResponseHandler::handle(&response).is_err());
}
