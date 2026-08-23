// Bounded node-status and parameter response contracts.

#[test]
fn decode_status_response_returns_err_on_internal_server_error() {
    let response = mk_response(StatusCode::INTERNAL_SERVER_ERROR, Vec::new(), None);
    assert!(Client::decode_status_for_test(&response).is_err());
}

#[test]
fn get_status_bounds_preferred_and_json_fallback_responses() {
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
    let decoded = with_mock_http(responder, || client_with_base_url(base_url()).get_status())
        .expect("JSON fallback status response");
    assert_eq!(
        norito::json::to_value(&decoded).expect("serialize decoded status"),
        norito::json::to_value(&expected).expect("serialize expected status")
    );
    let snapshots = snapshots.lock().expect("snapshot lock");
    assert_eq!(snapshots.len(), 2);
    for snapshot in snapshots.iter() {
        assert_eq!(snapshot.url.path(), torii_uri::STATUS);
        assert_eq!(snapshot.max_response_bytes, NODE_STATUS_RESPONSE_MAX_BYTES);
    }
    assert_single_accept_header(&snapshots[1], APPLICATION_JSON);
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
