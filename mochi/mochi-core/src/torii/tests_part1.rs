use super::*;
use futures::SinkExt;
use httpmock::{
    Method::{DELETE, GET, POST},
    MockServer,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    block::consensus::{ExecWitness, ExecWitnessMsg},
    events::{
        EventBox, SharedDataEvent,
        data::{
            DataEvent,
            prelude::{
                AccountControllerReplaced, AccountEvent, AccountRecoveryEvent,
                AccountRecoveryPolicySet, PeerEvent,
            },
        },
        pipeline::PipelineEventBox,
        stream::EventMessage,
        time::{TimeEvent, TimeInterval},
    },
    isi::InstructionBox,
    nexus::{LaneCatalog, LaneLifecyclePlan, LaneLifecycleStatusV1},
    peer::PeerId,
    prelude::{DomainId, Quantity},
    query::{
        QueryOutput, QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryRequest,
        executor::FindExecutorDataModel, prelude::SingularQueryBox,
    },
    transaction::signed::TransactionBuilder,
};
use iroha_telemetry::metrics::{Status as TelemetryStatus, Uptime};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR, PEER_KEYPAIR};
use reqwest::{
    StatusCode,
    header::{HeaderMap, HeaderValue},
};
use std::{
    collections::VecDeque,
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpListener as StdTcpListener},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::http;
fn operator_test_client(base_url: impl AsRef<str>) -> ToriiClient {
    let network_id = test_network_id();
    let context = OperatorSigningContext::new(
        network_id,
        KeyPair::random_with_algorithm(Algorithm::Ed25519),
    );
    ToriiClient::builder(base_url)
        .expect("client builder")
        .with_network_id(network_id)
        .with_operator_signing_context(context)
        .build()
        .expect("operator client")
}
fn handle_bind_result<T>(result: std::io::Result<T>, context: &str) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            eprintln!("skipping {context}: {err}");
            None
        }
        Err(err) => panic!("{context}: {err}"),
    }
}
async fn raw_chunked_response(chunks: Vec<Vec<u8>>) -> Option<(Response, JoinHandle<()>)> {
    let listener = handle_bind_result(
        TcpListener::bind("127.0.0.1:0").await,
        "bind bounded response listener",
    )?;
    let address = listener.local_addr().expect("bounded response address");
    let server = tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut request = Vec::new();
        loop {
            let Ok(byte) = socket.read_u8().await else {
                return;
            };
            request.push(byte);
            if request.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        if socket
            .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
            .await
            .is_err()
        {
            return;
        }
        for chunk in chunks {
            let header = format!("{:X}\r\n", chunk.len());
            if socket.write_all(header.as_bytes()).await.is_err()
                || socket.write_all(&chunk).await.is_err()
                || socket.write_all(b"\r\n").await.is_err()
            {
                return;
            }
        }
        let _ = socket.write_all(b"0\r\n\r\n").await;
    });
    let response = Client::new()
        .get(format!("http://{address}/bounded"))
        .send()
        .await
        .expect("read raw chunked response headers");
    Some((response, server))
}
async fn accept_canonical_norito_websocket(stream: TcpStream) -> WebSocketStream<TcpStream> {
    tokio_tungstenite::accept_hdr_async(
        stream,
        |request: &WsHandshakeRequest, mut response: WsHandshakeResponse| {
            assert_eq!(
                request
                    .headers()
                    .get(SEC_WEBSOCKET_PROTOCOL)
                    .and_then(|value| value.to_str().ok()),
                Some(NORITO_V1_WEBSOCKET_SUBPROTOCOL)
            );
            response.headers_mut().insert(
                SEC_WEBSOCKET_PROTOCOL,
                HeaderValue::from_static(NORITO_V1_WEBSOCKET_SUBPROTOCOL),
            );
            Ok(response)
        },
    )
    .await
    .expect("canonical Norito WebSocket handshake")
}
#[test]
fn stream_endpoints_and_default_event_filters_match_torii_contract() {
    let client = ToriiClient::new("http://127.0.0.1:8080").expect("client");
    assert_eq!(
        client
            .block_stream_endpoint()
            .expect("block endpoint")
            .path(),
        torii_uri::BLOCKS_STREAM
    );
    assert_eq!(
        client
            .events_stream_endpoint()
            .expect("events endpoint")
            .path(),
        torii_uri::SUBSCRIPTION
    );
    let filters = canonical_event_filters();
    assert_eq!(filters.len(), 8);
    let request = EventSubscriptionRequest::new(filters.clone());
    let encoded = norito::to_bytes(&request).expect("encode event subscription");
    let decoded: EventSubscriptionRequest =
        norito::decode_from_bytes(&encoded).expect("decode event subscription");
    assert_eq!(decoded.filters, filters);
}
use std::iter;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{Mutex as AsyncMutex, Notify, broadcast, oneshot},
    time::{sleep, timeout},
};
use tokio_tungstenite::tungstenite::{
    handshake::server::{Request as WsHandshakeRequest, Response as WsHandshakeResponse},
    protocol::Message as WsMessage,
};
fn try_start_mock_server() -> Option<MockServer> {
    std::panic::catch_unwind(MockServer::start)
        .ok()
        .or_else(|| {
            eprintln!("Skipping test: unable to bind mock HTTP server in sandboxed environment.");
            None
        })
}
fn lifecycle_status() -> LaneLifecycleStatusV1 {
    let catalog = LaneCatalog::default();
    let incarnations = std::collections::BTreeMap::from([(
        iroha_data_model::nexus::LaneId::SINGLE,
        Hash::new(b"mochi-lifecycle-status-incarnation"),
    )]);
    LaneLifecycleStatusV1::new(&catalog, &incarnations).expect("valid lifecycle status")
}
fn spawn_status_stub(
    responses: Vec<(u16, Vec<u8>)>,
) -> Option<(SocketAddr, mpsc::Sender<()>, thread::JoinHandle<()>)> {
    let listener = match StdTcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(err)
            if matches!(
                err.kind(),
                ErrorKind::PermissionDenied | ErrorKind::AddrNotAvailable
            ) =>
        {
            eprintln!("skipping spawn_status_stub: {err}");
            return None;
        }
        Err(err) => panic!("bind status stub: {err}"),
    };
    listener
        .set_nonblocking(true)
        .expect("configure nonblocking");
    let addr = listener.local_addr().expect("local addr");
    let (shutdown_tx, shutdown_rx) = mpsc::channel();
    let shared = Arc::new(Mutex::new(VecDeque::from(responses)));
    let fallback = shared
        .lock()
        .expect("responses mutex")
        .back()
        .cloned()
        .unwrap_or((503, Vec::new()));
    let handle = thread::spawn(move || {
        loop {
            if shutdown_rx.try_recv().is_ok() {
                break;
            }
            match listener.accept() {
                Ok((mut stream, _peer)) => {
                    let mut buffer = [0u8; 1024];
                    let _ = stream.read(&mut buffer);
                    let (status, body) = {
                        let mut guard = shared.lock().expect("responses mutex");
                        guard.pop_front().unwrap_or_else(|| fallback.clone())
                    };
                    let reason = if status == 200 {
                        "OK"
                    } else {
                        "Service Unavailable"
                    };
                    let header = format!(
                        "HTTP/1.1 {status} {reason}\r\ncontent-length: {}\r\ncontent-type: {NORITO_MIME_TYPE}\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(&body);
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    Some((addr, shutdown_tx, handle))
}
#[test]
fn normalises_base_url_without_trailing_slash() {
    let client = ToriiClient::new("http://localhost:8080/").expect("valid url");
    assert_eq!(client.base_url(), "http://localhost:8080/");
}
#[test]
fn summarizes_errors_with_kind_and_detail() {
    let summary = ToriiError::UnsupportedScheme {
        scheme: "ftp".to_string(),
    }
    .summarize();
    assert_eq!(summary.kind, ToriiErrorKind::UnsupportedScheme);
    assert_eq!(summary.message, "Unsupported Torii URL scheme");
    assert_eq!(summary.detail.as_deref(), Some("ftp"));
    let summary = ToriiError::UnexpectedStatus {
        status: StatusCode::NOT_FOUND,
        reject_code: None,
        message: None,
    }
    .summarize();
    assert_eq!(summary.kind, ToriiErrorKind::UnexpectedStatus);
    assert_eq!(summary.detail.as_deref(), Some("404 Not Found"));
    assert!(
        summary
            .message
            .starts_with("Unexpected Torii status code 404")
    );
    let summary = ToriiError::Decode("bad payload".into()).summarize();
    assert_eq!(summary.kind, ToriiErrorKind::Decode);
    assert_eq!(summary.detail.as_deref(), Some("bad payload"));
    assert_eq!(
        summary.message,
        "Failed to decode Norito payload from Torii"
    );
    let summary = ToriiError::ResponseResourceLimit {
        context: "Explorer JSON",
        maximum: 1024,
    }
    .summarize();
    assert_eq!(summary.kind, ToriiErrorKind::ResponseResourceLimit);
    assert_eq!(
        summary.detail.as_deref(),
        Some("Explorer JSON response limit: 1024 bytes")
    );
}
#[tokio::test(flavor = "current_thread")]
async fn bounded_response_accepts_exact_chunked_body_and_rejects_next_byte() {
    let Some((response, server)) = raw_chunked_response(vec![b"a".to_vec(), b"bc".to_vec()]).await
    else {
        return;
    };
    assert_eq!(
        read_bounded_response(response, 3, "test")
            .await
            .expect("exact response"),
        b"abc"
    );
    server.await.expect("exact response server");
    let Some((response, server)) = raw_chunked_response(vec![b"abc".to_vec(), b"d".to_vec()]).await
    else {
        return;
    };
    assert!(matches!(
        read_bounded_response(response, 3, "test")
            .await
            .expect_err("max + 1 response"),
        ToriiError::ResponseResourceLimit {
            context: "test",
            maximum: 3,
        }
    ));
    server.await.expect("overflow response server");
}
#[test]
fn bounded_json_response_rejects_excessive_depth_before_tree_decode() {
    let mut nested = "[".repeat(65);
    nested.push('0');
    nested.push_str(&"]".repeat(65));
    assert!(matches!(
        decode_bounded_json_response(nested.as_bytes(), "test"),
        Err(ToriiError::Decode(_))
    ));
    let value = decode_bounded_json_response(br#"{"ok":true}"#, "test")
        .expect("small bounded JSON response");
    assert_eq!(value.get("ok").and_then(json::Value::as_bool), Some(true));
}
#[test]
fn queue_plan_outcome_unknown_requires_exact_submission_reconciliation() {
    let ambiguous = ToriiError::UnexpectedStatus {
        status: StatusCode::SERVICE_UNAVAILABLE,
        reject_code: Some(QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN_REJECT_CODE.to_owned()),
        message: Some("admission outcome unknown".to_owned()),
    };
    assert!(ambiguous.is_queue_plan_journal_outcome_unknown());
    assert!(!ambiguous.confirms_existing_submission());
    for reject_code in ["PRTRY:ALREADY_ENQUEUED", "PRTRY:ALREADY_COMMITTED"] {
        let reconciled = ToriiError::UnexpectedStatus {
            status: StatusCode::TOO_MANY_REQUESTS,
            reject_code: Some(reject_code.to_owned()),
            message: None,
        };
        assert!(reconciled.confirms_existing_submission());
    }
    let unresolved = ToriiError::SmokeAdmissionOutcomeUnknown {
        hash: "abcd".to_owned(),
    };
    let mut cursor = ReadinessSmokeAttemptCursor::default();
    cursor.record_failure(0, &unresolved);
    assert_eq!(cursor.current_index(), 0);
    cursor.record_failure(
        0,
        &ToriiError::Timeout {
            context: "same exact transaction".to_owned(),
        },
    );
    assert_eq!(
        cursor.current_index(),
        0,
        "later observational errors must not advance to a replacement transaction"
    );
    let mut ordinary_retry = ReadinessSmokeAttemptCursor::default();
    ordinary_retry.record_failure(
        0,
        &ToriiError::Timeout {
            context: "ordinary unambiguous timeout".to_owned(),
        },
    );
    assert_eq!(ordinary_retry.current_index(), 1);
    let unresolved = unresolved.summarize();
    assert_eq!(
        unresolved.kind,
        ToriiErrorKind::SmokeAdmissionOutcomeUnknown
    );
    assert!(unresolved.message.contains("abcd"));
    assert!(
        unresolved
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("byte-identical"))
    );
}
#[test]
fn reject_code_and_message_helpers_extract_values() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-iroha-reject-code",
        HeaderValue::from_static("PRTRY:TX_SIGNATURE_INVALID"),
    );
    assert_eq!(
        reject_code_from_headers(&headers).as_deref(),
        Some("PRTRY:TX_SIGNATURE_INVALID")
    );
    let mut axt_headers = HeaderMap::new();
    axt_headers.insert(
        "x-iroha-axt-code",
        HeaderValue::from_static("AXT_HANDLE_ERA"),
    );
    assert_eq!(
        reject_code_from_headers(&axt_headers).as_deref(),
        Some("AXT_HANDLE_ERA")
    );
    let envelope = ToriiErrorEnvelope {
        code: "PRTRY:AXT_HANDLE_ERA".to_owned(),
        message: "handle era too low".to_owned(),
    };
    let body = norito::to_bytes(&envelope).expect("encode envelope");
    assert_eq!(
        error_message_from_body(&body).as_deref(),
        Some("PRTRY:AXT_HANDLE_ERA: handle era too low")
    );
}
#[test]
fn builder_applies_basic_auth_header() {
    let client = ToriiClient::builder("http://localhost:8080")
        .expect("builder")
        .with_basic_auth("demo", "secret")
        .expect("basic auth header")
        .build()
        .expect("client");
    let header = client
        .default_headers
        .get("authorization")
        .expect("authorization header");
    assert_eq!(header.to_str().unwrap(), "Basic ZGVtbzpzZWNyZXQ=");
}
#[test]
fn rejects_unsupported_base_scheme() {
    let err =
        ToriiClient::new("ftp://localhost:8080").expect_err("unsupported scheme should error");
    match err {
        ToriiError::UnsupportedScheme { scheme } => assert_eq!(scheme, "ftp"),
        other => panic!("expected UnsupportedScheme, got {other:?}"),
    }
}
#[test]
fn builds_ws_base_from_https_scheme() {
    let builder = ToriiClientBuilder::new("https://example.com/api/").expect("valid base url");
    assert_eq!(builder.http_base.scheme(), "https");
    assert_eq!(builder.ws_base.scheme(), "wss");
    assert_eq!(builder.ws_base.host_str(), Some("example.com"));
    assert_eq!(builder.ws_base.path(), "/api/");
}
#[test]
fn composes_transaction_endpoint() {
    let client = ToriiClient::new("http://127.0.0.1:8080").expect("valid url");
    assert_eq!(
        client.transaction_endpoint().unwrap().as_str(),
        "http://127.0.0.1:8080/v1/pipeline/transactions"
    );
}
#[test]
fn composes_nexus_lifecycle_endpoint() {
    let client = ToriiClient::new("http://127.0.0.1:8080").expect("valid url");
    assert_eq!(
        client.nexus_lifecycle_endpoint().unwrap().as_str(),
        "http://127.0.0.1:8080/v1/nexus/lifecycle"
    );
}
#[test]
fn composes_mcp_endpoint() {
    let client = ToriiClient::new("http://127.0.0.1:8080").expect("valid url");
    assert_eq!(
        client.mcp_endpoint().unwrap().as_str(),
        "http://127.0.0.1:8080/v1/mcp"
    );
}
fn mock_json_body(value: norito::json::Value) -> String {
    norito::json::to_string(&value).expect("serialize mock json body")
}
#[tokio::test(flavor = "current_thread")]
async fn local_mcp_rate_limit_preserves_retry_after() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let throttled = server.mock(|when, then| {
        when.method(GET).path("/v1/mcp");
        then.status(429).header("retry-after", "7");
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let error = client
        .validate_local_mcp()
        .await
        .expect_err("throttled MCP capabilities probe must remain retryable");
    assert!(matches!(
        error,
        ToriiError::RateLimited {
            retry_after: Some(delay),
        } if delay == Duration::from_secs(7)
    ));
    throttled.assert();
}
#[test]
fn websocket_rate_limit_preserves_retry_after() {
    let response = http::Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header(reqwest::header::RETRY_AFTER, "3")
        .body(None)
        .expect("valid WebSocket HTTP response");
    let error = websocket_connect_error(WebSocketError::Http(Box::new(response)));
    assert!(matches!(
        error,
        ToriiError::RateLimited {
            retry_after: Some(delay),
        } if delay == Duration::from_secs(3)
    ));
}
#[tokio::test(flavor = "current_thread")]
async fn validate_local_mcp_accepts_curated_iroha_tools() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let capabilities = server.mock(|when, then| {
        when.method(GET).path("/v1/mcp");
        then.status(200)
            .header("content-type", "application/json")
            .body(mock_json_body(norito::json!({
                "capabilities": {
                    "tools": {
                        "count": 4
                    }
                }
            })));
    });
    let initialize = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/mcp")
            .body(mock_json_body(norito::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "mochi-local-sandbox",
                        "version": "1"
                    }
                }
            })));
        then.status(200)
            .header("content-type", "application/json")
            .body(mock_json_body(norito::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {
                        "tools": {
                            "count": 4
                        }
                    }
                }
            })));
    });
    let initialized = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/mcp")
            .body(mock_json_body(norito::json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            })));
        then.status(202);
    });
    let tools_list = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/mcp")
            .body(mock_json_body(norito::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/list",
                "params": {}
            })));
        then.status(200)
            .header("content-type", "application/json")
            .body(mock_json_body(norito::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "toolsetVersion": "demo-v1",
                    "tools": [
                        { "name": "iroha.health" },
                        { "name": "iroha.parameters.get" },
                        { "name": "iroha.transactions.submit" },
                        { "name": "iroha.transactions.submit_and_wait" }
                    ]
                }
            })));
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let result = client.validate_local_mcp().await.expect("mcp probe");
    assert_eq!(result.protocol_version, "2025-06-18");
    assert_eq!(result.toolset_version.as_deref(), Some("demo-v1"));
    assert_eq!(result.tool_count, 4);
    assert!(
        result
            .tool_names
            .iter()
            .all(|name| name.starts_with("iroha."))
    );
    capabilities.assert();
    initialize.assert();
    initialized.assert();
    tools_list.assert();
}
#[tokio::test(flavor = "current_thread")]
async fn validate_local_mcp_rejects_raw_torii_tools() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    server.mock(|when, then| {
        when.method(GET).path("/v1/mcp");
        then.status(200)
            .header("content-type", "application/json")
            .body(mock_json_body(norito::json!({ "capabilities": {} })));
    });
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/mcp")
            .body(mock_json_body(norito::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "mochi-local-sandbox",
                        "version": "1"
                    }
                }
            })));
        then.status(200)
            .header("content-type", "application/json")
            .body(mock_json_body(norito::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": { "protocolVersion": "2025-06-18" }
            })));
    });
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/mcp")
            .body(mock_json_body(norito::json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            })));
        then.status(202);
    });
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/mcp")
            .body(mock_json_body(norito::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/list",
                "params": {}
            })));
        then.status(200)
            .header("content-type", "application/json")
            .body(mock_json_body(norito::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "tools": [
                        { "name": "iroha.health" },
                        { "name": "torii.get_v1_accounts" },
                        { "name": "iroha.parameters.get" },
                        { "name": "iroha.transactions.submit" },
                        { "name": "iroha.transactions.submit_and_wait" }
                    ]
                }
            })));
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let err = client
        .validate_local_mcp()
        .await
        .expect_err("raw torii tools should fail");
    assert!(
        err.to_string().contains("torii.* tools"),
        "unexpected error: {err}"
    );
}
#[tokio::test(flavor = "current_thread")]
async fn submit_transaction_reports_http_error() {
    let client = ToriiClient::new("http://127.0.0.1:65535").expect("valid url");
    let err = client
        .submit_transaction(&[])
        .await
        .expect_err("connection should fail");
    matches!(err, ToriiError::Http(_));
}
#[tokio::test(flavor = "current_thread")]
async fn submit_transaction_returns_unexpected_status_on_non_success() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/pipeline/transactions");
        then.status(503);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let err = client
        .submit_transaction(&[1, 2, 3])
        .await
        .expect_err("non-success status should error");
    mock.assert();
    match err {
        ToriiError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        }
        other => panic!("expected UnexpectedStatus, got {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_status_returns_unexpected_status_with_accept_header() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET).path("/status");
        then.status(503);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let err = client
        .fetch_status()
        .await
        .expect_err("non-success status should error");
    mock.assert();
    match err {
        ToriiError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        }
        other => panic!("expected UnexpectedStatus, got {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_status_returns_decode_error_on_invalid_payload() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET).path("/status");
        then.status(200).body(vec![0xAA, 0xBB]);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let err = client
        .fetch_status()
        .await
        .expect_err("invalid payload should fail");
    mock.assert();
    match err {
        ToriiError::Decode(message) => {
            assert!(
                !message.is_empty(),
                "decode error should propagate message: {message:?}"
            );
        }
        other => panic!("expected Decode error, got {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_sumeragi_status_returns_unexpected_status_on_non_success() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET).path("/v1/sumeragi/status");
        then.status(500);
    });
    let client = operator_test_client(server.url("/"));
    let err = client
        .fetch_sumeragi_status()
        .await
        .expect_err("non-success status should error");
    mock.assert();
    match err {
        ToriiError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        }
        other => panic!("expected UnexpectedStatus, got {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_sumeragi_status_returns_decode_error_on_invalid_payload() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET).path("/v1/sumeragi/status");
        then.status(200).body(vec![0x10, 0x42]);
    });
    let client = operator_test_client(server.url("/"));
    let err = client
        .fetch_sumeragi_status()
        .await
        .expect_err("invalid payload should fail");
    mock.assert();
    match err {
        ToriiError::Decode(message) => {
            assert!(
                !message.is_empty(),
                "decode error should propagate message: {message:?}"
            );
        }
        other => panic!("expected Decode error, got {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn sumeragi_operator_reads_require_context_before_dispatch() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let status_mock = server.mock(|when, then| {
        when.method(GET).path("/v1/sumeragi/status");
        then.status(200);
    });
    let diagnostics_mock = server.mock(|when, then| {
        when.method(GET).path("/v1/sumeragi/diagnostics");
        then.status(200);
    });
    let client = ToriiClient::new(server.url("/")).expect("unsigned client");
    let status_error = client
        .fetch_sumeragi_status()
        .await
        .expect_err("status must require operator context");
    let diagnostics_error = client
        .fetch_sumeragi_diagnostics()
        .await
        .expect_err("diagnostics must require operator context");
    assert!(matches!(status_error, ToriiError::SignedQueryContext(_)));
    assert!(matches!(
        diagnostics_error,
        ToriiError::SignedQueryContext(_)
    ));
    assert_eq!(status_mock.calls(), 0);
    assert_eq!(diagnostics_mock.calls(), 0);
}
#[tokio::test(flavor = "current_thread")]
async fn sumeragi_operator_read_rejects_announced_oversize_before_buffering() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let oversized = (MAX_SUMERAGI_OPERATOR_RESPONSE_BYTES + 1).to_string();
    let status_mock = server.mock(|when, then| {
        when.method(GET).path("/v1/sumeragi/status");
        then.status(200).header("content-length", oversized);
    });
    let client = operator_test_client(server.url("/"));
    let error = client
        .fetch_sumeragi_status()
        .await
        .expect_err("oversize response must be rejected before buffering");
    assert!(matches!(
        &error,
        ToriiError::ResponseResourceLimit {
            context: "Sumeragi operator",
            maximum: MAX_SUMERAGI_OPERATOR_RESPONSE_BYTES,
        }
    ));
    assert_eq!(status_mock.calls(), 1);
}
#[tokio::test(flavor = "current_thread")]
async fn sumeragi_operator_read_does_not_retry_transient_response() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let status_mock = server.mock(|when, then| {
        when.method(GET).path("/v1/sumeragi/status");
        then.status(503);
    });
    let client = operator_test_client(server.url("/"));
    let error = client
        .fetch_sumeragi_status()
        .await
        .expect_err("transient status must be returned without retry");
    assert!(matches!(error, ToriiError::UnexpectedStatus { .. }));
    assert_eq!(status_mock.calls(), 1);
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_configuration_returns_unexpected_status_on_non_success() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET).path("/v1/configuration");
        then.status(404);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let err = client
        .fetch_configuration()
        .await
        .expect_err("non-success status should error");
    mock.assert();
    match err {
        ToriiError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, StatusCode::NOT_FOUND);
        }
        other => panic!("expected UnexpectedStatus, got {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_configuration_returns_decode_error_on_invalid_json() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET).path("/v1/configuration");
        then.status(200).body("not-json");
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let err = client
        .fetch_configuration()
        .await
        .expect_err("invalid json should fail");
    mock.assert();
    match err {
        ToriiError::Decode(message) => {
            assert!(
                !message.is_empty(),
                "decode error should propagate message: {message:?}"
            );
        }
        other => panic!("expected Decode error, got {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_metrics_returns_unexpected_status_on_non_success() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET).path("/metrics");
        then.status(503);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let err = client
        .fetch_metrics()
        .await
        .expect_err("non-success status should error");
    mock.assert();
    match err {
        ToriiError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        }
        other => panic!("expected UnexpectedStatus, got {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn submit_query_returns_bytes_on_success() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let response_body = vec![0xAA, 0xBB, 0xCC];
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/query");
        then.status(200).body(response_body.clone());
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let bytes = client
        .submit_query(&[0x10, 0x11, 0x12])
        .await
        .expect("query bytes");
    mock.assert();
    assert_eq!(bytes, response_body);
}
#[tokio::test(flavor = "current_thread")]
async fn submit_query_does_not_follow_redirects() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let target = server.mock(|when, then| {
        when.method(POST).path("/redirect-target");
        then.status(200).body(vec![0xAA]);
    });
    let target_url = server.url("/redirect-target");
    let original = server.mock(|when, then| {
        when.method(POST).path("/v1/query");
        then.status(307).header("Location", target_url.clone());
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let error = client
        .submit_query(&[0x10, 0x11, 0x12])
        .await
        .expect_err("redirect must be returned instead of followed");
    original.assert();
    target.assert_calls(0);
    match error {
        ToriiError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
        }
        other => panic!("expected UnexpectedStatus, got {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn builder_applies_api_token_to_http_requests() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let status = TelemetryStatus::default();
    let body = encode_status_payload(&status);
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/status")
            .header("x-api-token", "secret-token");
        then.status(200).body(body.clone());
    });
    let client = ToriiClient::builder(server.url("/"))
        .expect("builder")
        .with_api_token("secret-token")
        .expect("token builder")
        .build()
        .expect("client");
    let fetched = client.fetch_status().await.expect("status");
    mock.assert();
    assert_eq!(fetched.queue_size, status.queue_size);
    assert_eq!(fetched.txs_approved, status.txs_approved);
}
#[tokio::test(flavor = "current_thread")]
async fn submit_query_returns_unexpected_status_on_non_success() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/query");
        then.status(500);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let err = client
        .submit_query(&[])
        .await
        .expect_err("non-success status should error");
    mock.assert();
    match err {
        ToriiError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        }
        other => panic!("expected UnexpectedStatus, got {other:?}"),
    }
}
#[test]
fn builder_rejects_invalid_api_token_value() {
    let err = ToriiClient::builder("http://127.0.0.1:8080")
        .and_then(|builder| builder.with_api_token("invalid\r\ntoken"))
        .expect_err("invalid header should error");
    matches!(err, ToriiError::InvalidHeader { .. });
}
#[tokio::test(flavor = "current_thread")]
async fn builder_applies_api_token_to_websocket_requests() {
    let listener =
        match handle_bind_result(TcpListener::bind("127.0.0.1:0").await, "bind ws listener") {
            Some(listener) => listener,
            None => return,
        };
    let addr = listener.local_addr().expect("listener addr");
    let (header_tx, header_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept ws stream");
        let mut tx = Some(header_tx);
        let callback = move |req: &WsHandshakeRequest, mut response: WsHandshakeResponse| {
            if let Some(sender) = tx.take() {
                let value = req
                    .headers()
                    .get("x-api-token")
                    .and_then(|header| header.to_str().ok())
                    .map(str::to_owned);
                let _ = sender.send(value);
            }
            assert_eq!(
                req.headers()
                    .get(SEC_WEBSOCKET_PROTOCOL)
                    .and_then(|value| value.to_str().ok()),
                Some(NORITO_V1_WEBSOCKET_SUBPROTOCOL)
            );
            response.headers_mut().insert(
                SEC_WEBSOCKET_PROTOCOL,
                HeaderValue::from_static(NORITO_V1_WEBSOCKET_SUBPROTOCOL),
            );
            Ok(response)
        };
        let mut ws = tokio_tungstenite::accept_hdr_async(stream, callback)
            .await
            .expect("handshake");
        ws.close(None).await.expect("server close");
    });
    let mut ws = ToriiClient::builder(format!("http://{addr}"))
        .expect("builder")
        .with_api_token("secret-token")
        .expect("token builder")
        .build()
        .expect("client")
        .connect_block_stream()
        .await
        .expect("connect block stream with header");
    ws.close(None).await.expect("client close");
    let header = header_rx.await.expect("header observed");
    assert_eq!(header.as_deref(), Some("secret-token"));
    server.await.expect("server join");
}
#[tokio::test(flavor = "current_thread")]
async fn websocket_rejects_missing_selected_norito_subprotocol() {
    let listener =
        match handle_bind_result(TcpListener::bind("127.0.0.1:0").await, "bind ws listener") {
            Some(listener) => listener,
            None => return,
        };
    let addr = listener.local_addr().expect("listener addr");
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept ws stream");
        let mut ws = tokio_tungstenite::accept_async(stream)
            .await
            .expect("handshake without selected subprotocol");
        ws.close(None).await.expect("server close");
    });
    let client = ToriiClient::new(format!("http://{addr}")).expect("client");
    let error = client
        .connect_block_stream()
        .await
        .expect_err("missing selected subprotocol must fail closed");
    assert!(matches!(
        error,
        ToriiError::WebSocket(WebSocketError::Protocol(
            tokio_tungstenite::tungstenite::error::ProtocolError::SecWebSocketSubProtocolError(
                tokio_tungstenite::tungstenite::error::SubProtocolError::NoSubProtocol
            )
        ))
    ));
    server.await.expect("server join");
}
#[tokio::test(flavor = "current_thread")]
async fn block_stream_reports_ws_error() {
    let client = ToriiClient::new("http://127.0.0.1:65535").expect("valid url");
    let err = client
        .connect_block_stream()
        .await
        .expect_err("connection should fail");
    matches!(err, ToriiError::WebSocket(_));
}
#[tokio::test(flavor = "current_thread")]
async fn subscribe_block_stream_reports_ws_error() {
    let client = ToriiClient::new("http://127.0.0.1:65535").expect("valid url");
    let err = client
        .subscribe_block_stream()
        .await
        .expect_err("connection should fail");
    matches!(err, ToriiError::WebSocket(_));
}
#[test]
fn readiness_smoke_plan_uses_checked_transaction_signing() {
    let signer = crate::compose::development_signing_authorities()
        .iter()
        .next()
        .expect("development signer available");
    let network_id = test_network_id();
    let plan = ReadinessSmokePlan::for_signer_with_attempts(network_id, signer, 2)
        .expect("build readiness smoke plan");
    assert_eq!(plan.transactions.len(), 2);
    for tx in &plan.transactions {
        tx.verify_signature()
            .expect("checked smoke transaction signature verifies");
        assert_eq!(tx.network_id(), Some(&network_id));
    }
    assert_ne!(
        plan.transactions[0].hash(),
        plan.transactions[1].hash(),
        "smoke attempts should carry distinct nonces"
    );
}
#[test]
fn readiness_smoke_mutates_only_its_signing_account_metadata() {
    let signer = crate::compose::development_signing_authorities()
        .first()
        .expect("development signer available");
    let plan = ReadinessSmokePlan::for_signer(test_network_id(), signer)
        .expect("build readiness smoke plan");
    let iroha_data_model::transaction::Executable::Instructions(instructions) =
        plan.transactions[0].instructions()
    else {
        panic!("readiness smoke must contain instructions");
    };
    assert_eq!(instructions.len(), 1);
    let set_key_value = instructions[0]
        .as_any()
        .downcast_ref::<iroha_data_model::isi::SetKeyValueBox>()
        .expect("readiness smoke instruction is SetKeyValue");
    match set_key_value {
        iroha_data_model::isi::SetKeyValueBox::Account(set) => {
            assert_eq!(set.object(), signer.account_id());
        }
        other => panic!("readiness smoke must target its own account, got {other:?}"),
    }
}
#[test]
fn generated_readiness_transactions_renew_for_the_full_retry_budget() {
    let signer = crate::compose::development_signing_authorities()
        .first()
        .expect("development signer available");
    let mut plan = ReadinessSmokePlan::for_signer_with_attempts(test_network_id(), signer, 3)
        .expect("build readiness smoke plan");
    let old_hashes = plan.tx_hashes().collect::<Vec<_>>();
    let creation_time = plan.transactions[0].creation_time();
    let required_lifetime = plan.required_submission_lifetime();
    assert!(
        required_lifetime > SMOKE_TTL,
        "the default three-attempt commit budget intentionally exceeds the base TTL"
    );
    plan.renew_generated_transactions_if_needed(creation_time)
        .expect("renew generated smoke transactions");
    assert_ne!(plan.tx_hashes().collect::<Vec<_>>(), old_hashes);
    for transaction in &plan.transactions {
        assert_eq!(transaction.creation_time(), creation_time);
        assert!(
            transaction
                .time_to_live()
                .is_some_and(|ttl| { ttl >= required_lifetime && ttl >= SMOKE_TTL })
        );
        transaction
            .verify_signature()
            .expect("renewed smoke transaction signature verifies");
    }
}
#[test]
fn caller_supplied_readiness_transactions_keep_the_exact_signed_envelope() {
    let signer = crate::compose::development_signing_authorities()
        .first()
        .expect("development signer available");
    let generated = ReadinessSmokePlan::for_signer(test_network_id(), signer)
        .expect("build readiness smoke plan");
    let transaction = generated.transactions[0].clone();
    let hash = transaction.hash();
    let mut exact = ReadinessSmokePlan::new(vec![transaction]);
    let after_expiry = exact.transactions[0]
        .creation_time()
        .saturating_add(SMOKE_TTL)
        .saturating_add(Duration::from_secs(1));
    exact
        .renew_generated_transactions_if_needed(after_expiry)
        .expect("exact plan does not require renewal");
    assert_eq!(exact.transactions[0].hash(), hash);
}
const BLOCK_WIRE_FIXTURE: &[u8] = include_bytes!("../../tests/fixtures/canonical_block_wire.bin");
const EVENT_MESSAGE_FIXTURE: &[u8] =
    include_bytes!("../../tests/fixtures/canonical_event_message.bin");
const PIPELINE_EVENT_MESSAGE_FIXTURE: &[u8] =
    include_bytes!("../../tests/fixtures/canonical_pipeline_event_message.bin");
const DATA_EVENT_MESSAGE_FIXTURE: &[u8] =
    include_bytes!("../../tests/fixtures/canonical_data_event_message.bin");
fn block_stream_frame(block: &SignedBlock) -> Vec<u8> {
    norito::to_bytes(&BlockMessage(block.clone())).expect("encode block stream message")
}
fn sample_block_proposal() -> SignedBlock {
    let mut builder = TransactionBuilder::new_genesis(
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_secs(42));
    let builder = builder.with_instructions(core::iter::empty::<InstructionBox>());
    let tx = builder.sign(ALICE_KEYPAIR.private_key());
    SignedBlock::genesis(vec![tx], PEER_KEYPAIR.private_key(), None, None)
}
fn sample_block() -> SignedBlock {
    sample_block_with_result(Ok(
        iroha_data_model::transaction::DataTriggerSequence::default(),
    ))
}
fn sample_block_with_result(
    result: iroha_data_model::transaction::TransactionResultInner,
) -> SignedBlock {
    let mut block = sample_block_proposal();
    let entrypoint_hashes = block
        .external_entrypoints_cloned()
        .map(|entrypoint| entrypoint.hash())
        .collect::<Vec<_>>();
    block
        .set_transaction_results(Vec::new(), &entrypoint_hashes, vec![result])
        .expect("attach aligned sample transaction result");
    let final_signature = iroha_data_model::block::BlockSignature::new(
        0,
        iroha_crypto::SignatureOf::try_from_hash(PEER_KEYPAIR.private_key(), block.hash())
            .expect("sign result-bearing sample block"),
    );
    block
        .replace_signatures(std::collections::BTreeSet::from([final_signature]))
        .expect("replace result-bearing sample-block signature");
    {
        let mut final_signatures = block.signatures();
        let final_signature = final_signatures
            .next()
            .expect("result-bearing sample-block signature");
        assert_eq!(final_signature.index(), 0);
        assert!(final_signatures.next().is_none());
        final_signature
            .signature()
            .verify_hash(PEER_KEYPAIR.public_key(), block.hash())
            .expect("verify result-bearing sample-block signature");
    }
    block
}
#[test]
fn committed_block_rejection_is_not_reported_as_smoke_success() {
    use iroha_data_model::transaction::error::{TransactionLimitError, TransactionRejectionReason};
    let rejection = TransactionRejectionReason::LimitCheck(TransactionLimitError {
        reason: "limit".to_owned(),
    });
    let expected_reason = format!("{rejection:?}");
    let block = sample_block_with_result(Err(rejection));
    let tx_hash = block
        .external_transactions()
        .next()
        .expect("sample block tx")
        .hash();
    match smoke_transaction_result_in_block(&block, &tx_hash) {
        Some(Err(ToriiError::SmokeRejected { hash, reason })) => {
            assert_eq!(hash, tx_hash.to_string());
            assert_eq!(reason, expected_reason);
        }
        other => panic!("expected aligned block rejection, got {other:?}"),
    }
}
#[test]
fn block_hash_presence_without_aligned_result_is_not_smoke_success() {
    let block = sample_block_proposal();
    assert!(block.is_resultless_proposal());
    let tx_hash = block
        .external_transactions()
        .next()
        .expect("sample block tx")
        .hash();
    assert!(smoke_transaction_result_in_block(&block, &tx_hash).is_none());
}
#[tokio::test(flavor = "current_thread")]
async fn submit_and_wait_for_commit_reports_block_height() {
    let block = sample_block_with_result(Ok(
        iroha_data_model::transaction::DataTriggerSequence::default(),
    ));
    let tx_hash = block
        .external_transactions()
        .next()
        .expect("sample block tx")
        .hash();
    let expected_height = block.header().height().get();
    let block: SignedBlock = norito::decode_from_bytes::<BlockMessage>(&block_stream_frame(&block))
        .expect("round-trip sample block through the Torii stream envelope")
        .into();
    assert_eq!(block.external_transactions().len(), 1);
    let (block_tx, block_rx) = broadcast::channel(8);
    let (_event_tx, event_rx) = broadcast::channel(8);
    let summary = BlockSummary::from_block(&block);
    let event = BlockStreamEvent::Block {
        summary,
        block: Arc::new(block),
        raw_len: 0,
    };
    let handle = tokio::spawn(async move {
        submit_and_wait_for_commit_with_receivers(
            tx_hash,
            SmokeCommitOptions::new(Duration::from_secs(1)),
            async { Ok(()) },
            block_rx,
            event_rx,
        )
        .await
    });
    let _ = block_tx.send(event);
    let observed_height = handle
        .await
        .expect("join smoke wait task")
        .expect("smoke commit should succeed");
    assert_eq!(observed_height, expected_height);
}
#[tokio::test(flavor = "current_thread")]
async fn submit_and_wait_for_commit_times_out_without_events() {
    let block = sample_block();
    let tx_hash = block
        .external_transactions()
        .next()
        .expect("sample block tx")
        .hash();
    let (_block_tx, block_rx) = broadcast::channel(8);
    let (_event_tx, event_rx) = broadcast::channel(8);
    let err = submit_and_wait_for_commit_with_receivers(
        tx_hash,
        SmokeCommitOptions::new(Duration::from_millis(25)),
        async { Ok(()) },
        block_rx,
        event_rx,
    )
    .await
    .expect_err("should time out");
    assert!(matches!(err, ToriiError::Timeout { .. }));
}
#[tokio::test(flavor = "current_thread")]
async fn submit_and_wait_for_commit_reports_rejected_when_expired_event_arrives() {
    use iroha_data_model::{
        events::pipeline::{TransactionEvent, TransactionStatus},
        nexus::{DataSpaceId, LaneId},
    };
    let block = sample_block();
    let tx_hash = block
        .external_transactions()
        .next()
        .expect("sample block tx")
        .hash();
    let task_tx_hash = tx_hash;
    let (_block_tx, block_rx) = broadcast::channel(8);
    let (event_tx, event_rx) = broadcast::channel(8);
    let handle = tokio::spawn(async move {
        submit_and_wait_for_commit_with_receivers(
            task_tx_hash,
            SmokeCommitOptions::new(Duration::from_secs(1)),
            async { Ok(()) },
            block_rx,
            event_rx,
        )
        .await
    });
    let event_box = EventBox::Pipeline(PipelineEventBox::Transaction(TransactionEvent {
        hash: tx_hash,
        block_height: None,
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::UNIVERSAL,
        status: TransactionStatus::Expired,
    }));
    let summary = EventSummary::from_event(&event_box);
    let _ = event_tx.send(EventStreamEvent::Event {
        summary,
        event: Arc::new(event_box),
        raw_len: 0,
    });
    let err = handle
        .await
        .expect("join smoke wait task")
        .expect_err("expired event should reject the smoke transaction");
    match err {
        ToriiError::SmokeRejected { hash, reason } => {
            assert!(hash.contains(&tx_hash.to_string()));
            assert_eq!(reason, "expired");
        }
        other => panic!("expected SmokeRejected error, got {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn submit_and_wait_for_commit_reports_rejected_when_pipeline_event_rejects() {
    use iroha_data_model::{
        events::pipeline::{TransactionEvent, TransactionStatus},
        nexus::{DataSpaceId, LaneId},
        transaction::error::{TransactionLimitError, TransactionRejectionReason},
    };
    let block = sample_block();
    let tx_hash = block
        .external_transactions()
        .next()
        .expect("sample block tx")
        .hash();
    let task_tx_hash = tx_hash;
    let (_block_tx, block_rx) = broadcast::channel(8);
    let (event_tx, event_rx) = broadcast::channel(8);
    let handle = tokio::spawn(async move {
        submit_and_wait_for_commit_with_receivers(
            task_tx_hash,
            SmokeCommitOptions::new(Duration::from_secs(1)),
            async { Ok(()) },
            block_rx,
            event_rx,
        )
        .await
    });
    let rejection = Box::new(TransactionRejectionReason::LimitCheck(
        TransactionLimitError {
            reason: "limit".to_owned(),
        },
    ));
    let expected_reason = format!("{rejection:?}");
    let event_box = EventBox::Pipeline(PipelineEventBox::Transaction(TransactionEvent {
        hash: tx_hash,
        block_height: None,
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::UNIVERSAL,
        status: TransactionStatus::Rejected(rejection),
    }));
    let summary = EventSummary::from_event(&event_box);
    let _ = event_tx.send(EventStreamEvent::Event {
        summary,
        event: Arc::new(event_box),
        raw_len: 0,
    });
    let err = handle
        .await
        .expect("join smoke wait task")
        .expect_err("rejected event should reject the smoke transaction");
    match err {
        ToriiError::SmokeRejected { hash, reason } => {
            assert_eq!(hash, tx_hash.to_string());
            assert_eq!(reason, expected_reason);
        }
        other => panic!("expected SmokeRejected error, got {other:?}"),
    }
}
fn sample_time_event_box() -> EventBox {
    let interval = TimeInterval::new(Duration::from_secs(42), Duration::from_secs(3));
    EventBox::Time(TimeEvent::new(interval))
}
fn sample_pipeline_event_box() -> EventBox {
    let block = sample_block();
    let header = block.header();
    let witness = ExecWitnessMsg {
        block_hash: block.hash(),
        height: header.height().get(),
        view: header.view_change_index(),
        epoch: 0,
        witness: ExecWitness {
            reads: Vec::new(),
            writes: Vec::new(),
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        },
    };
    EventBox::Pipeline(PipelineEventBox::Witness(witness))
}
fn sample_data_event_box() -> EventBox {
    let peer_id = PeerId::from(PEER_KEYPAIR.public_key().clone());
    let event = DataEvent::Peer(PeerEvent::Added(peer_id));
    EventBox::Data(SharedDataEvent::from(event))
}
fn sample_time_event_message() -> EventMessage {
    event_message_from_box(sample_time_event_box())
}
fn sample_pipeline_event_message() -> EventMessage {
    event_message_from_box(sample_pipeline_event_box())
}
fn sample_data_event_message() -> EventMessage {
    event_message_from_box(sample_data_event_box())
}
fn event_message_from_box(event: EventBox) -> EventMessage {
    EventMessage::new(event)
}
fn time_event_fixture_message() -> EventMessage {
    decode_norito_with_alignment(EVENT_MESSAGE_FIXTURE).expect("decode event fixture")
}
fn pipeline_event_fixture_message() -> EventMessage {
    decode_norito_with_alignment(PIPELINE_EVENT_MESSAGE_FIXTURE)
        .expect("decode pipeline event fixture")
}
fn data_event_fixture_message() -> EventMessage {
    decode_norito_with_alignment(DATA_EVENT_MESSAGE_FIXTURE).expect("decode data event fixture")
}
fn time_event_fixture_event() -> EventBox {
    time_event_fixture_message().into()
}
fn pipeline_event_fixture_event() -> EventBox {
    pipeline_event_fixture_message().into()
}
fn data_event_fixture_event() -> EventBox {
    data_event_fixture_message().into()
}
fn encode_status_payload(status: &TelemetryStatus) -> Vec<u8> {
    norito::codec::encode_adaptive(status)
}
fn encode_sumeragi_status_payload(status: &SumeragiV2Status) -> Vec<u8> {
    let mut encoded = Vec::new();
    norito::core::to_bytes_in(status, &mut encoded).expect("encode framed status");
    encoded
}
fn sample_sumeragi_status_wire() -> SumeragiV2Status {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::consensus_v2::{
        ConsensusMode, DualQuorum, HeightContextId, PROTOCOL_VERSION, SumeragiV2BodyState,
        SumeragiV2HeightContextStatus, SumeragiV2StatusPhase,
    };
    SumeragiV2Status {
        protocol_version: PROTOCOL_VERSION,
        node_fingerprint: Hash::new(b"mochi-status-node"),
        build_fingerprint: Hash::new(b"mochi-status-build"),
        config_fingerprint: Hash::new(b"mochi-status-config"),
        restart_required: false,
        height_context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"mochi-status-context",
        ))),
        height: 15,
        view: 6,
        phase: SumeragiV2StatusPhase::Prepare,
        leader: 3,
        locked_prepare_qc: None,
        highest_prepare_qc: None,
        last_timeout_certificate: None,
        body_state: SumeragiV2BodyState::Validated,
        pending_persistence_id: None,
        last_committed_height: 14,
        last_committed_subject: None,
        height_context: SumeragiV2HeightContextStatus {
            epoch: 1,
            epoch_end_height: 100,
            mode: ConsensusMode::Permissioned,
            epoch_seed: [0xA5; 32],
            validator_count: 4,
            quorum: DualQuorum {
                min_signers: 3,
                total_power: 4,
            },
        },
        last_commit_qc: None,
        liveness: Default::default(),
    }
}
#[path = "tests/canonical_fixture_owner.rs"]
mod canonical_fixture_owner;
#[tokio::test(flavor = "current_thread")]
async fn block_stream_decodes_block_events() {
    let expected_block = sample_block();
    let expected_summary = BlockSummary::from_block(&expected_block);
    let frame = block_stream_frame(&expected_block);
    let (sender, _) = broadcast::channel(8);
    let handle = tokio::spawn(async {});
    let subscription = WsSubscription {
        sender: sender.clone(),
        handle,
    };
    let stream = BlockStream::new(subscription);
    let mut receiver = stream.subscribe();
    sender
        .send(WsFrame::Binary(frame.clone()))
        .expect("send frame");
    sender.send(WsFrame::Closed).expect("send close");
    let event = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("timely block event")
        .expect("block event value");
    match event {
        BlockStreamEvent::Block {
            summary,
            block,
            raw_len,
        } => {
            assert_eq!(raw_len, frame.len());
            assert_eq!(summary.height, expected_summary.height);
            assert_eq!(
                summary.transaction_count,
                expected_summary.transaction_count
            );
            assert_eq!(
                summary.rejected_transaction_count,
                expected_summary.rejected_transaction_count
            );
            assert_eq!(summary.signature_count, expected_summary.signature_count);
            assert_eq!(summary.hash_hex, expected_summary.hash_hex);
            assert_eq!(
                summary.view_change_index,
                expected_summary.view_change_index
            );
            assert_eq!(summary.creation_time_ms, expected_summary.creation_time_ms);
            assert_eq!(summary.is_genesis, expected_summary.is_genesis);
            assert_eq!(block.as_ref(), &expected_block);
        }
        other => panic!("expected block event, got {other:?}"),
    }
    stream.abort();
}
#[test]
fn block_canonical_wire_matches_fixture() {
    let block = sample_block();
    block
        .validate_entrypoint_merkle_cache()
        .expect("canonical fixture entrypoint Merkle cache");
    block
        .validate_result_merkle_cache()
        .expect("canonical fixture result Merkle cache");
    assert_eq!(block.committed_fragment_count(), Some(1));
    assert_eq!(
        block.header().result_merkle_root(),
        block
            .result_merkle_commitment()
            .map(|commitment| *commitment.root())
    );
    let wire = block.canonical_wire().expect("canonical wire").into_vec();
    assert_eq!(wire.as_slice(), BLOCK_WIRE_FIXTURE);
}
#[tokio::test(flavor = "current_thread")]
async fn block_stream_end_to_end_decodes_canonical_block() {
    let listener = match handle_bind_result(
        TcpListener::bind("127.0.0.1:0").await,
        "bind block stream listener",
    ) {
        Some(listener) => listener,
        None => return,
    };
    let addr = listener.local_addr().expect("listener addr");
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept block stream");
        let mut ws = accept_canonical_norito_websocket(stream).await;
        let subscription = ws
            .next()
            .await
            .expect("block subscription frame")
            .expect("valid block subscription frame");
        let WsMessage::Binary(subscription) = subscription else {
            panic!("expected binary block subscription request");
        };
        let request: BlockSubscriptionRequest =
            norito::decode_from_bytes(&subscription).expect("decode block subscription");
        assert_eq!(request.0, NonZeroU64::MIN);
        let frame = block_stream_frame(&sample_block());
        ws.send(WsMessage::Binary(frame.into()))
            .await
            .expect("send block fixture");
        ws.send(WsMessage::Close(None))
            .await
            .expect("send block close");
    });
    let client = ToriiClient::new(format!("http://{addr}")).expect("block client");
    let stream = client
        .block_stream()
        .await
        .expect("connect block stream end-to-end");
    let mut receiver = stream.subscribe();
    let event = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("timely block event")
        .expect("block event value");
    let expected_block = sample_block();
    let expected_summary = BlockSummary::from_block(&expected_block);
    match event {
        BlockStreamEvent::Block {
            summary,
            block,
            raw_len,
        } => {
            assert_eq!(raw_len, block_stream_frame(&expected_block).len());
            assert_eq!(summary.hash_hex, expected_summary.hash_hex);
            assert_eq!(summary.height, expected_summary.height);
            assert_eq!(summary.signature_count, expected_summary.signature_count);
            assert_eq!(
                summary.transaction_count,
                expected_summary.transaction_count
            );
            assert_eq!(
                summary.rejected_transaction_count,
                expected_summary.rejected_transaction_count
            );
            assert_eq!(
                summary.view_change_index,
                expected_summary.view_change_index
            );
            assert_eq!(summary.creation_time_ms, expected_summary.creation_time_ms);
            assert_eq!(summary.is_genesis, expected_summary.is_genesis);
            assert_eq!(block.as_ref(), &expected_block);
        }
        other => panic!("expected block event, got {other:?}"),
    }
    stream.abort();
    server.await.expect("server task finished");
}
#[tokio::test(flavor = "current_thread")]
async fn block_stream_rejects_unwrapped_signed_block_wire() {
    let (sender, _) = broadcast::channel(8);
    let handle = tokio::spawn(async {});
    let subscription = WsSubscription {
        sender: sender.clone(),
        handle,
    };
    let stream = BlockStream::new(subscription);
    let mut receiver = stream.subscribe();
    sender
        .send(WsFrame::Binary(BLOCK_WIRE_FIXTURE.to_vec()))
        .expect("send unwrapped block wire");
    let event = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("timely decode result")
        .expect("decode result");
    assert!(
        matches!(&event, BlockStreamEvent::DecodeError { error } if error.stage == BlockDecodeStage::Frame),
        "raw SignedBlock wire must not be accepted as a Torii BlockMessage: {event:?}"
    );
    stream.abort();
}
#[tokio::test(flavor = "current_thread")]
async fn block_stream_reports_decode_errors() {
    let (sender, _) = broadcast::channel(8);
    let handle = tokio::spawn(async {});
    let subscription = WsSubscription {
        sender: sender.clone(),
        handle,
    };
    let stream = BlockStream::new(subscription);
    let mut receiver = stream.subscribe();
    sender
        .send(WsFrame::Binary(vec![0, 1, 2]))
        .expect("send invalid frame");
    sender.send(WsFrame::Closed).expect("send close");
    let event = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("timely error event")
        .expect("error event value");
    match event {
        BlockStreamEvent::DecodeError { error } => {
            assert!(matches!(
                error.stage,
                BlockDecodeStage::Frame | BlockDecodeStage::Block
            ));
            assert_eq!(error.raw_len, 3);
        }
        other => panic!("expected decode error event, got {other:?}"),
    }
    stream.abort();
}
#[tokio::test(flavor = "current_thread")]
async fn event_stream_reports_decode_errors() {
    let (sender, _) = broadcast::channel(8);
    let handle = tokio::spawn(async {});
    let subscription = WsSubscription {
        sender: sender.clone(),
        handle,
    };
    let stream = EventStream::new(subscription);
    let mut receiver = stream.subscribe();
    sender
        .send(WsFrame::Binary(vec![1, 2, 3]))
        .expect("send invalid frame");
    sender.send(WsFrame::Closed).expect("send close");
    let event = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("error available")
        .expect("error value");
    match event {
        EventStreamEvent::DecodeError { error } => {
            assert_eq!(error.stage, EventDecodeStage::Frame);
        }
        other => panic!("expected decode error event, got {other:?}"),
    }
    stream.abort();
}
#[tokio::test(flavor = "current_thread")]
async fn event_stream_decodes_time_events() {
    let expected_event = sample_time_event_box();
    assert_eq!(time_event_fixture_event(), expected_event);
    let expected_summary = EventSummary::from_event(&expected_event);
    let (sender, _) = broadcast::channel(8);
    let handle = tokio::spawn(async {});
    let subscription = WsSubscription {
        sender: sender.clone(),
        handle,
    };
    let stream = EventStream::new(subscription);
    let mut receiver = stream.subscribe();
    sender
        .send(WsFrame::Binary(EVENT_MESSAGE_FIXTURE.to_vec()))
        .expect("send event frame");
    sender.send(WsFrame::Closed).expect("send close");
    let event = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("timely event")
        .expect("event value");
    match event {
        EventStreamEvent::Event {
            summary: emitted_summary,
            event,
            raw_len,
        } => {
            assert_eq!(raw_len, EVENT_MESSAGE_FIXTURE.len());
            assert_eq!(emitted_summary.category, EventCategory::Time);
            assert_eq!(emitted_summary.label, expected_summary.label);
            assert_eq!(emitted_summary.detail, expected_summary.detail);
            match (event.as_ref(), &expected_event) {
                (EventBox::Time(actual), EventBox::Time(expected)) => {
                    assert_eq!(actual.interval().since(), expected.interval().since());
                    assert_eq!(actual.interval().length(), expected.interval().length());
                }
                (other, _) => panic!("expected time event, got {other:?}"),
            }
            assert_eq!(event.as_ref(), &expected_event);
        }
        other => panic!("expected decoded event, got {other:?}"),
    }
    stream.abort();
}
#[tokio::test(flavor = "current_thread")]
async fn event_stream_decodes_pipeline_events() {
    let expected_event_box = sample_pipeline_event_box();
    assert_eq!(pipeline_event_fixture_event(), expected_event_box);
    let expected_summary = EventSummary::from_event(&expected_event_box);
    let expected_event = Arc::new(expected_event_box);
    let (sender, _) = broadcast::channel(8);
    let handle = tokio::spawn(async {});
    let subscription = WsSubscription {
        sender: sender.clone(),
        handle,
    };
    let stream = EventStream::new(subscription);
    let mut receiver = stream.subscribe();
    sender
        .send(WsFrame::Binary(PIPELINE_EVENT_MESSAGE_FIXTURE.to_vec()))
        .expect("send pipeline event frame");
    sender.send(WsFrame::Closed).expect("send close");
    let event = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("timely pipeline event")
        .expect("pipeline event value");
    match event {
        EventStreamEvent::Event {
            summary: emitted_summary,
            event,
            raw_len,
        } => {
            assert_eq!(raw_len, PIPELINE_EVENT_MESSAGE_FIXTURE.len());
            assert_eq!(emitted_summary.category, EventCategory::Pipeline);
            assert_eq!(emitted_summary.label, expected_summary.label);
            assert_eq!(emitted_summary.detail, expected_summary.detail);
            match (event.as_ref(), expected_event.as_ref()) {
                (EventBox::Pipeline(actual), EventBox::Pipeline(expected)) => {
                    assert_eq!(actual, expected);
                }
                (other, _) => panic!("expected pipeline event, got {other:?}"),
            }
        }
        other => panic!("expected decoded pipeline event, got {other:?}"),
    }
    stream.abort();
}
#[tokio::test(flavor = "current_thread")]
async fn events_stream_end_to_end_decodes_pipeline_event() {
    let expected_event_box = sample_pipeline_event_box();
    let frame = norito::to_bytes(&sample_pipeline_event_message())
        .expect("encode canonical pipeline event message");
    let expected_frame_len = frame.len();
    let server_frame = frame.clone();
    let listener = match handle_bind_result(
        TcpListener::bind("127.0.0.1:0").await,
        "bind events listener",
    ) {
        Some(listener) => listener,
        None => return,
    };
    let addr = listener.local_addr().expect("events listener addr");
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept events stream");
        let mut ws = accept_canonical_norito_websocket(stream).await;
        let subscription = ws
            .next()
            .await
            .expect("event subscription frame")
            .expect("valid event subscription frame");
        let WsMessage::Binary(subscription) = subscription else {
            panic!("expected binary event subscription request");
        };
        let request: EventSubscriptionRequest =
            norito::decode_from_bytes(&subscription).expect("decode event subscription");
        assert_eq!(request.filters, canonical_event_filters());
        ws.send(WsMessage::Binary(server_frame.into()))
            .await
            .expect("send canonical pipeline event");
        ws.send(WsMessage::Close(None))
            .await
            .expect("send events close");
    });
    let client = ToriiClient::new(format!("http://{addr}")).expect("events client");
    let stream = client
        .events_stream()
        .await
        .expect("connect events stream end-to-end");
    let mut receiver = stream.subscribe();
    let event = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("timely pipeline event")
        .expect("pipeline event value");
    let expected_summary = EventSummary::from_event(&expected_event_box);
    match event {
        EventStreamEvent::Event {
            summary: emitted_summary,
            event,
            raw_len,
        } => {
            assert_eq!(raw_len, expected_frame_len);
            assert_eq!(emitted_summary.category, EventCategory::Pipeline);
            assert_eq!(emitted_summary.label, expected_summary.label);
            assert_eq!(emitted_summary.detail, expected_summary.detail);
            assert_eq!(event.as_ref(), &expected_event_box);
        }
        other => panic!("expected decoded pipeline event, got {other:?}"),
    }
    stream.abort();
    server.await.expect("events server task finished");
}
#[tokio::test(flavor = "current_thread")]
async fn event_stream_decodes_data_events() {
    let expected_event_box = sample_data_event_box();
    assert_eq!(data_event_fixture_event(), expected_event_box);
    let expected_summary = EventSummary::from_event(&expected_event_box);
    let expected_event = Arc::new(expected_event_box);
    let (sender, _) = broadcast::channel(8);
    let handle = tokio::spawn(async {});
    let subscription = WsSubscription {
        sender: sender.clone(),
        handle,
    };
    let stream = EventStream::new(subscription);
    let mut receiver = stream.subscribe();
    sender
        .send(WsFrame::Binary(DATA_EVENT_MESSAGE_FIXTURE.to_vec()))
        .expect("send data event frame");
    sender.send(WsFrame::Closed).expect("send close");
    let event = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("timely data event")
        .expect("data event value");
    match event {
        EventStreamEvent::Event {
            summary: emitted_summary,
            event,
            raw_len,
        } => {
            assert_eq!(raw_len, DATA_EVENT_MESSAGE_FIXTURE.len());
            assert_eq!(emitted_summary.category, EventCategory::Data);
            assert_eq!(emitted_summary.label, expected_summary.label);
            assert_eq!(emitted_summary.detail, expected_summary.detail);
            match (event.as_ref(), expected_event.as_ref()) {
                (EventBox::Data(actual), EventBox::Data(expected)) => {
                    assert_eq!(actual.as_ref(), expected.as_ref());
                }
                (other, _) => panic!("expected data event, got {other:?}"),
            }
        }
        other => panic!("expected decoded data event, got {other:?}"),
    }
    stream.abort();
}
#[test]
fn time_event_message_matches_fixture() {
    let message = time_event_fixture_message();
    let decoded_again: EventBox =
        decode_norito_with_alignment::<EventMessage>(EVENT_MESSAGE_FIXTURE)
            .expect("decode fixture message")
            .into();
    assert_eq!(EventBox::from(message), decoded_again);
}
#[test]
fn pipeline_event_message_matches_fixture() {
    let message = pipeline_event_fixture_message();
    let decoded_again: EventBox =
        decode_norito_with_alignment::<EventMessage>(PIPELINE_EVENT_MESSAGE_FIXTURE)
            .expect("decode pipeline fixture")
            .into();
    assert_eq!(EventBox::from(message), decoded_again);
}
#[test]
fn data_event_message_matches_fixture() {
    let message = data_event_fixture_message();
    let decoded_again: EventBox =
        decode_norito_with_alignment::<EventMessage>(DATA_EVENT_MESSAGE_FIXTURE)
            .expect("decode data fixture")
            .into();
    assert_eq!(EventBox::from(message), decoded_again);
}
#[tokio::test(flavor = "current_thread")]
async fn managed_block_stream_reconnects_and_forwards_events() {
    let handle = tokio::runtime::Handle::current();
    let senders = Arc::new(Mutex::new(Vec::<broadcast::Sender<WsFrame>>::new()));
    let notify = Arc::new(Notify::new());
    let factory = {
        let senders = senders.clone();
        let notify = notify.clone();
        move || {
            let senders = senders.clone();
            let notify = notify.clone();
            async move {
                let (sender, _) = broadcast::channel(16);
                let task = tokio::spawn(async {});
                let subscription = WsSubscription {
                    sender: sender.clone(),
                    handle: task,
                };
                senders.lock().expect("factory mutex poisoned").push(sender);
                notify.notify_waiters();
                Ok(subscription)
            }
        }
    };
    let stream = ManagedBlockStream::spawn_with_factory(&handle, "reconnect-peer", factory);
    let mut receiver = stream.subscribe();
    notify.notified().await;
    assert_eq!(stream.alias(), "reconnect-peer");
    tokio::task::yield_now().await;
    let first_sender = {
        let guard = senders.lock().expect("sender mutex poisoned");
        guard.last().expect("first sender present").clone()
    };
    first_sender
        .send(WsFrame::Text("hello".to_owned()))
        .expect("send hello frame");
    match timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("receive first frame")
    {
        Ok(BlockStreamEvent::Text { text }) => assert_eq!(text, "hello"),
        other => panic!("unexpected event: {other:?}"),
    }
    first_sender
        .send(WsFrame::Closed)
        .expect("send closed frame");
    match timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("receive closed event")
    {
        Ok(BlockStreamEvent::Closed) => {}
        other => panic!("expected closed event, got {other:?}"),
    }
    sleep(INITIAL_BACKOFF).await;
    notify.notified().await;
    tokio::task::yield_now().await;
    let second_sender = {
        let guard = senders.lock().expect("sender mutex poisoned");
        guard.last().expect("second sender present").clone()
    };
    second_sender
        .send(WsFrame::Text("reconnected".to_owned()))
        .expect("send reconnection frame");
    let mut saw_notice = false;
    let mut saw_custom = false;
    for _ in 0..3 {
        match timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("receive reconnection events")
        {
            Ok(BlockStreamEvent::Text { text })
                if text == "Block stream `reconnect-peer` reconnected." =>
            {
                saw_notice = true;
            }
            Ok(BlockStreamEvent::Text { text }) if text == "reconnected" => {
                saw_custom = true;
            }
            Ok(other) => panic!("unexpected event after reconnect: {other:?}"),
            Err(err) => panic!("receiver closed unexpectedly: {err:?}"),
        }
        if saw_notice && saw_custom {
            break;
        }
    }
    assert!(saw_notice, "reconnection notice event expected");
    assert!(saw_custom, "custom reconnection frame expected");
    stream.abort();
    sleep(Duration::from_millis(20)).await;
    assert!(stream.is_finished());
}
#[tokio::test(flavor = "current_thread")]
async fn managed_event_stream_reconnects_and_forwards_events() {
    let handle = tokio::runtime::Handle::current();
    let senders = Arc::new(Mutex::new(Vec::<broadcast::Sender<WsFrame>>::new()));
    let notify = Arc::new(Notify::new());
    let factory = {
        let senders = senders.clone();
        let notify = notify.clone();
        move || {
            let senders = senders.clone();
            let notify = notify.clone();
            async move {
                let (sender, _) = broadcast::channel(16);
                let task = tokio::spawn(async {});
                let subscription = WsSubscription {
                    sender: sender.clone(),
                    handle: task,
                };
                senders.lock().expect("factory mutex poisoned").push(sender);
                notify.notify_waiters();
                Ok(subscription)
            }
        }
    };
    let stream = ManagedEventStream::spawn_with_factory(&handle, "events-peer", factory);
    let mut receiver = stream.subscribe();
    notify.notified().await;
    assert_eq!(stream.alias(), "events-peer");
    tokio::task::yield_now().await;
    let first_sender = {
        let guard = senders.lock().expect("sender mutex poisoned");
        guard.last().expect("first sender present").clone()
    };
    first_sender
        .send(WsFrame::Text("hello-events".to_owned()))
        .expect("send text frame");
    match timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("receive first frame")
    {
        Ok(EventStreamEvent::Text { text }) => assert_eq!(text, "hello-events"),
        other => panic!("unexpected event: {other:?}"),
    }
    first_sender
        .send(WsFrame::Closed)
        .expect("send closed frame");
    match timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("receive closed event")
    {
        Ok(EventStreamEvent::Closed) => {}
        other => panic!("expected closed event, got {other:?}"),
    }
    sleep(INITIAL_BACKOFF).await;
    notify.notified().await;
    tokio::task::yield_now().await;
    let second_sender = {
        let guard = senders.lock().expect("sender mutex poisoned");
        guard.last().expect("second sender present").clone()
    };
    second_sender
        .send(WsFrame::Text("reconnected-events".to_owned()))
        .expect("send reconnection frame");
    let mut saw_notice = false;
    let mut saw_custom = false;
    for _ in 0..3 {
        match timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("receive reconnection events")
        {
            Ok(EventStreamEvent::Text { text })
                if text == "Event stream `events-peer` reconnected." =>
            {
                saw_notice = true;
            }
            Ok(EventStreamEvent::Text { text }) if text == "reconnected-events" => {
                saw_custom = true;
            }
            Ok(other) => panic!("unexpected event after reconnect: {other:?}"),
            Err(err) => panic!("receiver closed unexpectedly: {err:?}"),
        }
        if saw_notice && saw_custom {
            break;
        }
    }
    assert!(saw_notice, "reconnection notice event expected");
    assert!(saw_custom, "custom reconnection frame expected");
    stream.abort();
    sleep(Duration::from_millis(20)).await;
    assert!(stream.is_finished());
}
#[tokio::test(flavor = "current_thread")]
async fn managed_status_stream_emits_snapshots() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let status = TelemetryStatus {
        queue_size: 7,
        ..TelemetryStatus::default()
    };
    let body = encode_status_payload(&status);
    let sumeragi = sample_sumeragi_status_wire();
    let sumeragi_body = encode_sumeragi_status_payload(&sumeragi);
    server.mock(|when, then| {
        when.method(GET).path("/status");
        then.status(200)
            .header("content-type", NORITO_MIME_TYPE)
            .body(body.clone());
    });
    server.mock(|when, then| {
        when.method(GET).path("/v1/sumeragi/status");
        then.status(200)
            .header("content-type", NORITO_MIME_TYPE)
            .body(sumeragi_body.clone());
    });
    server.mock(|when, then| {
        when.method(GET).path("/metrics");
        then.status(200)
            .body("queue_size 7\nsumeragi_tx_queue_depth 4");
    });
    let client = operator_test_client(server.url("/"));
    let handle = tokio::runtime::Handle::current();
    let stream =
        ManagedStatusStream::spawn(&handle, "status-peer", client, Duration::from_millis(10));
    let mut receiver = stream.subscribe();
    match timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("receive snapshot")
    {
        Ok(StatusStreamEvent::Snapshot {
            snapshot,
            sumeragi,
            metrics,
            metrics_error,
            ..
        }) => {
            assert_eq!(snapshot.status.queue_size, 7);
            assert!(sumeragi.is_some());
            assert!(metrics.is_some());
            assert!(metrics_error.is_none());
        }
        other => panic!("expected status snapshot event, got {other:?}"),
    }
    stream.abort();
    sleep(Duration::from_millis(10)).await;
    assert!(stream.is_finished());
}
#[tokio::test(flavor = "current_thread")]
async fn managed_status_stream_reports_errors() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    server.mock(|when, then| {
        when.method(GET).path("/status");
        then.status(503);
    });
    let client = operator_test_client(server.url("/"));
    let handle = tokio::runtime::Handle::current();
    let stream =
        ManagedStatusStream::spawn(&handle, "status-peer", client, Duration::from_millis(10));
    let mut receiver = stream.subscribe();
    match timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("receive error event")
    {
        Ok(StatusStreamEvent::Error {
            error,
            consecutive_failures,
        }) => {
            assert_eq!(error.kind, ToriiErrorKind::UnexpectedStatus);
            assert_eq!(consecutive_failures, 1);
        }
        other => panic!("expected status error event, got {other:?}"),
    }
    stream.abort();
    sleep(Duration::from_millis(10)).await;
    assert!(stream.is_finished());
}
#[tokio::test(flavor = "current_thread")]
async fn managed_status_stream_reports_sumeragi_errors() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let status = TelemetryStatus {
        queue_size: 3,
        ..TelemetryStatus::default()
    };
    let body = encode_status_payload(&status);
    server.mock(|when, then| {
        when.method(GET).path("/status");
        then.status(200)
            .header("content-type", NORITO_MIME_TYPE)
            .body(body.clone());
    });
    server.mock(|when, then| {
        when.method(GET).path("/v1/sumeragi/status");
        then.status(503);
    });
    server.mock(|when, then| {
        when.method(GET).path("/metrics");
        then.status(200).body("queue_size 3");
    });
    let client = operator_test_client(server.url("/"));
    let handle = tokio::runtime::Handle::current();
    let stream =
        ManagedStatusStream::spawn(&handle, "status-peer", client, Duration::from_millis(10));
    let mut receiver = stream.subscribe();
    let error_event = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("receive sumeragi error")
        .expect("error event");
    match error_event {
        StatusStreamEvent::Error {
            error,
            consecutive_failures,
        } => {
            assert_eq!(error.kind, ToriiErrorKind::UnexpectedStatus);
            assert_eq!(consecutive_failures, 0);
        }
        other => panic!("expected sumeragi error event, got {other:?}"),
    }
    let snapshot_event = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("receive snapshot after sumeragi error")
        .expect("snapshot event");
    match snapshot_event {
        StatusStreamEvent::Snapshot {
            snapshot,
            sumeragi,
            metrics,
            metrics_error,
            ..
        } => {
            assert_eq!(snapshot.status.queue_size, 3);
            assert!(sumeragi.is_none());
            assert!(metrics.is_some());
            assert!(metrics_error.is_none());
        }
        other => panic!("expected snapshot event, got {other:?}"),
    }
    stream.abort();
    sleep(Duration::from_millis(10)).await;
    assert!(stream.is_finished());
}
