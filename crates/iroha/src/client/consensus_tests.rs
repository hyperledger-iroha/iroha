//! Consensus diagnostic authority, strict decoding and transport contracts.

use super::*;
use crate::client::{
    Client,
    evidence_http_tests::{
        base_url, capture_request, client_with_base_url, respond_with, with_mock_http,
    },
    mk_response,
    tests::sample_sumeragi_status,
};
use crate::http::{Method as HttpMethod, Response as HttpResponse, StatusCode};
use eyre::Result;
use std::{
    num::NonZeroU64,
    sync::{Arc, Mutex},
    time::Duration,
};

fn blocking_diagnostics(client: &Client) -> Result<SumeragiDiagnosticsStatus> {
    let operator = client.operator_client(
        client
            .operator_key_pair()
            .expect("test operator key")
            .clone(),
    )?;
    Ok(crate::blocking::OperatorClient::from_client(operator)?
        .consensus()
        .diagnostics()?)
}

pub(in crate::client) fn sample_sumeragi_diagnostics() -> SumeragiDiagnosticsStatus {
    SumeragiDiagnosticsStatus {
        tx_queue_depth: 7,
        tx_queue_capacity: 20,
        tx_queue_retained_bytes: 3_072,
        tx_queue_max_retained_bytes: 4_096,
        tx_queue_saturated: true,
        tx_queue_saturated_by_count: false,
        tx_queue_saturated_by_bytes: true,
        tx_queue_saturated_by_age: true,
        tx_queue_oldest_queued_age_ms: 1_250,
        npos: None,
        lane_governance_sealed_total: 0,
        lane_governance_sealed_aliases: Vec::new(),
        lane_governance: Vec::new(),
    }
}
fn encoded_sumeragi_diagnostics_response(
    status: &SumeragiDiagnosticsStatus,
    content_type: &'static str,
    context: &'static str,
) -> HttpResponse<Vec<u8>> {
    let body = if content_type == APPLICATION_JSON {
        norito::json::to_vec(status).expect(context)
    } else {
        norito::to_bytes(status).expect(context)
    };
    mk_response(StatusCode::OK, body, Some(content_type))
}
fn request_sumeragi_diagnostics(
    status: &SumeragiDiagnosticsStatus,
    content_type: &'static str,
    context: &'static str,
) -> Result<SumeragiDiagnosticsStatus> {
    let client = client_with_base_url(base_url());
    capture_request(
        encoded_sumeragi_diagnostics_response(status, content_type, context),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            blocking_diagnostics(&client)
        },
    )
    .0
}
#[test]
fn get_sumeragi_diagnostics_rejects_malformed_json_payload() {
    let client = client_with_base_url(base_url());
    let (result, _) = capture_request(
        mk_response(
            StatusCode::OK,
            br#"{"lane_relay_envelopes":["#.to_vec(),
            Some(APPLICATION_JSON),
        ),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            blocking_diagnostics(&client)
        },
    );
    assert!(result.is_err(), "malformed json should be rejected");
}
#[test]
fn get_sumeragi_diagnostics_rejects_unknown_json_fields() {
    let client = client_with_base_url(base_url());
    let mut status = sample_sumeragi_diagnostics();
    status.npos = Some(
        iroha_data_model::block::consensus::SumeragiNposDiagnostics {
            epoch_length_blocks: NonZeroU64::new(100).unwrap(),
            epoch_seed: [0xA5; 32],
        },
    );
    let current = norito::json::to_value(&status).expect("serialize diagnostics fixture");
    let mut nested = current.clone();
    nested
        .pointer_mut("/npos")
        .and_then(norito::json::Value::as_object_mut)
        .expect("diagnostics fixture contains the current NPoS schedule")
        .insert("canonical".to_owned(), norito::json::Value::Null);
    let mut rejected = vec![("unknown nested schedule field", nested)];
    for field in [
        "lane_commitments",
        "dataspace_commitments",
        "pipeline_execution",
    ] {
        let mut retired = current.clone();
        retired
            .as_object_mut()
            .expect("diagnostics object")
            .insert(field.to_owned(), norito::json::Value::Array(Vec::new()));
        rejected.push((field, retired));
    }
    for (case, value) in rejected {
        let (result, _) = capture_request(
            mk_response(
                StatusCode::OK,
                norito::json::to_vec(&value).expect("encode adversarial diagnostics JSON"),
                Some(APPLICATION_JSON),
            ),
            |mock_transport| {
                let client = client
                    .clone()
                    .with_test_http_transport(mock_transport.clone());
                blocking_diagnostics(&client)
            },
        );
        assert!(result.is_err(), "{case} must be rejected");
    }
}
#[test]
fn get_sumeragi_diagnostics_rejects_zero_npos_seed() {
    let mut status = sample_sumeragi_diagnostics();
    status.npos = Some(
        iroha_data_model::block::consensus::SumeragiNposDiagnostics {
            epoch_length_blocks: NonZeroU64::new(100).unwrap(),
            epoch_seed: [0; 32],
        },
    );
    let error =
        request_sumeragi_diagnostics(&status, APPLICATION_JSON, "encode invalid diagnostics JSON")
            .expect_err("zero NPoS seed must be rejected");
    assert!(error.to_string().contains("epoch seed must be non-zero"));
}
#[test]
fn get_sumeragi_diagnostics_requires_declared_current_media_type() {
    let client = client_with_base_url(base_url());
    let status = sample_sumeragi_diagnostics();
    let body = norito::json::to_vec(&status).expect("encode status payload as json");
    let (decoded, _) = capture_request(
        mk_response(StatusCode::OK, body.clone(), Some(APPLICATION_JSON)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            blocking_diagnostics(&client)
        },
    );
    let decoded = decoded.expect("decode declared current diagnostics JSON");
    assert_eq!(decoded, status);
    for content_type in [
        None,
        Some("application/octet-stream"),
        Some("application/x-norito-legacy"),
    ] {
        let error = with_mock_http(
            respond_with(
                &Arc::new(Mutex::new(Vec::new())),
                mk_response(StatusCode::OK, body.clone(), content_type),
            ),
            |mock_transport| {
                let client = client
                    .clone()
                    .with_test_http_transport(mock_transport.clone());
                blocking_diagnostics(&client)
            },
        )
        .expect_err("undeclared or noncanonical diagnostics media must fail closed");
        assert!(
            error.to_string().contains("invalid content-type"),
            "{error}"
        );
    }
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
            blocking_diagnostics(&client)
        },
    );
    assert!(
        result.is_err(),
        "structurally invalid json payload should be rejected"
    );

    let diagnostics = sample_sumeragi_diagnostics();
    let mut value = norito::json::to_value(&diagnostics).expect("serialize diagnostics fixture");
    value
        .as_object_mut()
        .expect("diagnostics object")
        .remove("lane_governance");
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
            blocking_diagnostics(&client)
        },
    );
    assert!(
        result.is_err(),
        "the first-release lane governance diagnostics vector is required"
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
            blocking_diagnostics(&client)
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
    let status = sample_sumeragi_diagnostics();
    for content_type in [APPLICATION_NORITO, APPLICATION_JSON] {
        for tampered in [false, true] {
            let mut status = status.clone();
            if tampered {
                status.npos = Some(
                    iroha_data_model::block::consensus::SumeragiNposDiagnostics {
                        epoch_length_blocks: std::num::NonZeroU64::new(100).unwrap(),
                        epoch_seed: [0; 32],
                    },
                );
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
            let operator = client
                .operator_client(client.operator_key_pair().unwrap().clone())
                .unwrap();
            let result = operator.consensus().diagnostics().await;
            if tampered {
                assert!(
                    result
                        .expect_err("same strict NPoS validation applies asynchronously")
                        .to_string()
                        .contains("epoch seed must be non-zero")
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

use crate::client::{
    capability_test_support::{AsyncOnlyTransport, GatedTransport, TransportGate},
    checked_random_keypair,
};
use crate::http::TransportRequest;
use base64::Engine as _;
use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::NetworkId;
use std::sync::atomic::{AtomicUsize, Ordering};

fn attach(
    responder: impl Fn(&TransportRequest) -> eyre::Result<Response<Vec<u8>>> + Send + Sync + 'static,
    delay: Duration,
    timeout: Duration,
) -> (Client, Arc<Mutex<Vec<TransportRequest>>>, Arc<AtomicUsize>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let completed = Arc::new(AtomicUsize::new(0));
    let mut builder = client_with_base_url(base_url())
        .to_builder()
        .http_transport(Arc::new(AsyncOnlyTransport {
            responder: Box::new(responder),
            requests: requests.clone(),
            completed: completed.clone(),
            delay,
        }));
    builder.torii_request_timeout = timeout;
    for name in [
        "accept",
        "authorization",
        "x-api-token",
        "x-iroha-account",
        "x-iroha-signature",
        "x-iroha-operator-public-key",
        "x-iroha-operator-signature",
    ] {
        builder
            .headers
            .insert(name.to_owned(), "untrusted-default".to_owned());
    }
    (builder.build().unwrap(), requests, completed)
}

fn response() -> Response<Vec<u8>> {
    encoded_sumeragi_diagnostics_response(
        &sample_sumeragi_diagnostics(),
        APPLICATION_NORITO,
        "canonical diagnostics fixture",
    )
}

fn header<'a>(request: &'a TransportRequest, name: &str) -> &'a str {
    let values: Vec<_> = request
        .headers
        .iter()
        .filter(|(key, _)| key == name)
        .collect();
    assert_eq!(values.len(), 1, "exactly one {name}");
    values[0].1.to_str().unwrap()
}

fn assert_operator_signature(request: &TransportRequest, network: &NetworkId, key: &KeyPair) {
    use crate::client::{
        HEADER_OPERATOR_NONCE, HEADER_OPERATOR_PUBLIC_KEY, HEADER_OPERATOR_SIGNATURE,
        HEADER_OPERATOR_TIMESTAMP_MS,
    };
    let public_key: iroha_crypto::PublicKey =
        header(request, HEADER_OPERATOR_PUBLIC_KEY).parse().unwrap();
    assert_eq!(&public_key, key.public_key());
    let signature = Signature::from_bytes(
        &base64::engine::general_purpose::STANDARD
            .decode(header(request, HEADER_OPERATOR_SIGNATURE))
            .unwrap(),
    );
    let message = Client::operator_network_request_message(
        network,
        &request.method,
        &request.url,
        &request.body,
        header(request, HEADER_OPERATOR_TIMESTAMP_MS)
            .parse()
            .unwrap(),
        header(request, HEADER_OPERATOR_NONCE),
    )
    .unwrap();
    signature.verify(key.public_key(), &message).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn diagnostics_is_async_and_bound_to_the_explicit_operator() {
    fn require_send(_: impl Send) {}
    let fixture = response();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let completed = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new(TransportGate::default());
    let transport = Arc::new(GatedTransport {
        inner: AsyncOnlyTransport {
            responder: Box::new(move |_| Ok(fixture.clone())),
            requests: requests.clone(),
            completed: completed.clone(),
            delay: Duration::ZERO,
        },
        gate: gate.clone(),
    });
    let mut builder = client_with_base_url(base_url())
        .to_builder()
        .http_transport(transport);
    builder.torii_request_timeout = Duration::ZERO;
    for name in [
        "accept",
        "authorization",
        "x-api-token",
        "x-iroha-account",
        "x-iroha-signature",
        "x-iroha-operator-public-key",
        "x-iroha-operator-signature",
    ] {
        builder
            .headers
            .insert(name.to_owned(), "untrusted-default".to_owned());
    }
    let client = builder.build().unwrap();
    let key = checked_random_keypair();
    let operator = client.operator_client(key.clone()).unwrap();
    let capability = operator.consensus();
    require_send(capability.diagnostics());
    let operation = capability.diagnostics();
    tokio::pin!(operation);
    tokio::select! {
        early = &mut operation => panic!("operation completed before transport release: {early:?}"),
        () = gate.wait_until_entered() => {}
    }
    assert_eq!(
        completed.load(Ordering::SeqCst),
        0,
        "transport cannot finish before release"
    );
    gate.release();
    let response = operation.await;
    assert_eq!(response.unwrap(), sample_sumeragi_diagnostics());
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.method, Method::GET);
    assert_eq!(
        request.url.path(),
        route_catalog::sumeragi::DIAGNOSTICS.path()
    );
    assert!(request.url.query().is_none());
    assert!(request.body.is_empty());
    assert_eq!(request.timeout, None);
    assert_eq!(request.max_response_bytes, MAX_DIAGNOSTICS_RESPONSE_BYTES);
    assert_eq!(header(request, "accept"), ACCEPT_NORITO_PREFERRED);
    for name in [
        "authorization",
        "x-api-token",
        "x-iroha-account",
        "x-iroha-signature",
        "x-iroha-witness",
    ] {
        assert!(!request.headers.iter().any(|(key, _)| key == name));
    }
    assert_operator_signature(request, client.network_id(), &key);
}

#[tokio::test]
async fn diagnostics_keeps_operator_network_and_endpoint_contexts_isolated() {
    let (client, requests, _) = attach(|_| Ok(response()), Duration::ZERO, Duration::ZERO);
    let first_key = checked_random_keypair();
    let second_key = checked_random_keypair();
    let first = client.operator_client(first_key.clone()).unwrap();
    let second = client.operator_client(second_key.clone()).unwrap();
    let mut builder = client.to_builder();
    builder.network_id =
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"different-diagnostics-network"),
        ));
    builder.torii_url = "https://other.mock/root/".parse().unwrap();
    let third_client = builder.build().unwrap();
    let third = third_client.operator_client(first_key.clone()).unwrap();
    for operator in [&first, &second, &third] {
        operator.consensus().diagnostics().await.unwrap();
    }
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_operator_signature(&requests[0], first.network_id(), &first_key);
    assert_operator_signature(&requests[1], second.network_id(), &second_key);
    assert_operator_signature(&requests[2], third.network_id(), &first_key);
    assert_eq!(
        requests[2].url.as_str(),
        "https://other.mock/root/v1/sumeragi/diagnostics"
    );
}

#[tokio::test]
async fn diagnostics_retains_structured_http_errors_without_retries() {
    for status in [401, 403, 429, 503] {
        let (client, requests, _) = attach(
            move |_| {
                Ok(Response::builder()
                    .status(status)
                    .header("Retry-After", "2")
                    .body(b"diagnostics-unavailable".to_vec())
                    .unwrap())
            },
            Duration::ZERO,
            Duration::ZERO,
        );
        let operator = client.operator_client(checked_random_keypair()).unwrap();
        assert_eq!(
            operator.consensus().diagnostics().await.unwrap_err(),
            Error::Http {
                operation: DIAGNOSTICS,
                status,
                retry_after: Some(Duration::from_secs(2)),
                body: b"diagnostics-unavailable".to_vec(),
            }
        );
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn diagnostics_rejects_oversized_and_ambiguous_responses() {
    let (client, _, _) = attach(
        |_| {
            Ok(Response::builder()
                .status(200)
                .header(
                    "Content-Length",
                    (MAX_DIAGNOSTICS_RESPONSE_BYTES + 1).to_string(),
                )
                .body(vec![])
                .unwrap())
        },
        Duration::ZERO,
        Duration::ZERO,
    );
    let operator = client.operator_client(checked_random_keypair()).unwrap();
    assert_eq!(
        operator.consensus().diagnostics().await.unwrap_err(),
        Error::ResponseTooLarge {
            maximum: MAX_DIAGNOSTICS_RESPONSE_BYTES,
            actual: None,
        }
    );
    let (client, _, _) = attach(
        |_| {
            let mut response = response();
            response.headers_mut().append(
                http::header::CONTENT_TYPE,
                APPLICATION_NORITO.parse().unwrap(),
            );
            Ok(response)
        },
        Duration::ZERO,
        Duration::ZERO,
    );
    let operator = client.operator_client(checked_random_keypair()).unwrap();
    assert!(matches!(
        operator.consensus().diagnostics().await,
        Err(Error::Decode {
            operation: DIAGNOSTICS,
            ..
        })
    ));
}

#[tokio::test]
async fn diagnostics_deadline_cancels_pending_transport_and_blocks_expired_context() {
    let (client, requests, completed) = attach(
        |_| Ok(response()),
        Duration::from_secs(60),
        Duration::from_millis(10),
    );
    let operator = client.operator_client(checked_random_keypair()).unwrap();
    assert_eq!(
        operator.consensus().diagnostics().await.unwrap_err(),
        Error::Timeout {
            operation: DIAGNOSTICS
        }
    );
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert_eq!(completed.load(Ordering::SeqCst), 0);
    let client = client.with_request_deadline(std::time::Instant::now());
    let operator = client.operator_client(checked_random_keypair()).unwrap();
    assert_eq!(
        operator.consensus().diagnostics().await.unwrap_err(),
        Error::Timeout {
            operation: DIAGNOSTICS
        }
    );
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocking_diagnostics_reject_async_runtime_before_io() {
    let (client, sends, _) = attach(
        |_| panic!("must not dispatch"),
        Duration::ZERO,
        Duration::ZERO,
    );
    let operator = client.operator_client(checked_random_keypair()).unwrap();
    let facade = crate::blocking::OperatorClient::from_client(operator).unwrap();
    assert_eq!(
        facade.consensus().diagnostics().unwrap_err(),
        Error::Blocking(crate::blocking::BlockingCallError::AsyncRuntime {
            flavor: crate::blocking::AsyncRuntimeFlavor::MultiThread,
        })
    );
    assert!(sends.lock().unwrap().is_empty());
}

#[test]
fn blocking_diagnostics_reuses_runtime_and_drops_safely_inside_async_runtime() {
    let (client, requests, _) = attach(|_| Ok(response()), Duration::ZERO, Duration::ZERO);
    let operator = client.operator_client(checked_random_keypair()).unwrap();
    let facade = crate::blocking::OperatorClient::from_client(operator).unwrap();
    assert_eq!(
        facade.consensus().diagnostics().unwrap(),
        sample_sumeragi_diagnostics()
    );
    assert_eq!(
        facade.clone().consensus().diagnostics().unwrap(),
        sample_sumeragi_diagnostics()
    );
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { drop(facade) });
    assert_eq!(requests.lock().unwrap().len(), 2);
}
