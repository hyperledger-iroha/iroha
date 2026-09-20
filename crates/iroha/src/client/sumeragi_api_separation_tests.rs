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

    let (diagnostics, _) = sample_sumeragi_status_with_relay();
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
fn get_sumeragi_diagnostics_rejects_json_payload_missing_required_fields() {
    let client = client_with_base_url(base_url());
    let response = HttpResponse::builder()
        .status(StatusCode::OK)
        .header("content-type", APPLICATION_JSON)
        .body(br"{}".to_vec())
        .unwrap();
    let result = with_mock_http(
        respond_with(&Arc::new(Mutex::new(Vec::new())), response),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            crate::blocking::Client::from_client(client)?.get_sumeragi_diagnostics()
        },
    );
    assert!(
        result.is_err(),
        "structurally invalid json payload should be rejected"
    );

    let (diagnostics, _) = sample_sumeragi_status_with_relay();
    let mut value = norito::json::to_value(&diagnostics).expect("serialize diagnostics fixture");
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
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            crate::blocking::Client::from_client(client)?.get_sumeragi_diagnostics()
        },
    );
    assert!(
        result.is_err(),
        "the first-release autonomous diagnostics vector is required"
    );

    let response = HttpResponse::builder()
        .status(StatusCode::OK)
        .header("content-type", APPLICATION_JSON)
        .body(norito::json::to_vec(&sample_sumeragi_status()).expect("encode status-shaped JSON"))
        .unwrap();
    let result = with_mock_http(
        respond_with(&Arc::new(Mutex::new(Vec::new())), response),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            crate::blocking::Client::from_client(client)?.get_sumeragi_diagnostics()
        },
    );
    assert!(
        result.is_err(),
        "diagnostics endpoint must reject a status-shaped payload"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn async_diagnostics_uses_async_transport_and_preserves_strict_evidence_validation() {
    #[derive(Debug)]
    struct DiagnosticsTransport {
        response: Mutex<Option<HttpResponse<Vec<u8>>>>,
        requests: Arc<Mutex<Vec<crate::http::TransportRequest>>>,
    }
    impl crate::http::HttpTransport for DiagnosticsTransport {
        fn send_blocking(&self, _: crate::http::TransportRequest) -> Result<HttpResponse<Vec<u8>>> {
            panic!("async diagnostics must never enter synchronous transport");
        }
        fn send(&self, request: crate::http::TransportRequest) -> crate::http::TransportFuture<'_> {
            self.requests.lock().expect("requests").push(request);
            let response = self
                .response
                .lock()
                .expect("response")
                .take()
                .expect("one request");
            Box::pin(async move { Ok(response) })
        }
    }
    let (status, _) = sample_sumeragi_status_with_relay();
    for content_type in [APPLICATION_NORITO, APPLICATION_JSON] {
        for tampered in [false, true] {
            let mut status = status.clone();
            if tampered {
                status.lane_relay_envelopes[0].settlement_hash =
                    HashOf::from_untyped_unchecked(Hash::prehashed([0xFF; Hash::LENGTH]));
            }
            let requests = Arc::new(Mutex::new(Vec::new()));
            let transport = Arc::new(DiagnosticsTransport {
                response: Mutex::new(Some(encoded_sumeragi_diagnostics_response(
                    &status,
                    content_type,
                    "async diagnostics fixture",
                ))),
                requests: Arc::clone(&requests),
            });
            let builder = client_with_base_url(base_url()).to_builder();
            // Builder transport ownership is injected before constructing the immutable context.
            let client = builder
                .http_transport(transport)
                .build()
                .expect("async diagnostics client");
            let result = client.get_sumeragi_diagnostics().await;
            if tampered {
                assert!(
                    result
                        .expect_err("same strict relay validation applies asynchronously")
                        .to_string()
                        .contains("Invalid lane relay envelope")
                );
            } else {
                assert_eq!(result.expect("typed async diagnostics"), status);
            }
            let requests = requests.lock().expect("requests");
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].method, HttpMethod::GET);
            assert_eq!(requests[0].url.path(), "/v1/sumeragi/diagnostics");
        }
    }
}
