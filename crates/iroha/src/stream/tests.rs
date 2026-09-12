//! Loopback wire and resource-limit tests for the default stream transport.

use super::*;
use futures_util::{SinkExt, StreamExt};
use http::{HeaderMap, HeaderValue, StatusCode, header::SEC_WEBSOCKET_PROTOCOL};
use iroha_data_model::events::{
    EventFilterBox, data::DataEventFilter, stream::EventSubscriptionRequest,
};
use iroha_torii_shared::NORITO_V1_WEBSOCKET_SUBPROTOCOL;
use std::{io::ErrorKind, net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tungstenite::{
    Message,
    client::IntoClientRequest,
    handshake::server::{Request, Response},
    protocol::{
        CloseFrame,
        frame::{
            Frame,
            coding::{CloseCode, Data, OpCode},
        },
    },
};

const OPERATION: &str = "events.stream_websocket";
const TEST_DEADLINE: Duration = Duration::from_secs(5);
const NO_CONNECT_WINDOW: Duration = Duration::from_millis(50);

async fn bounded<F: Future>(future: F) -> F::Output {
    timeout(TEST_DEADLINE, future)
        .await
        .expect("loopback stream operation must finish before the test deadline")
}

fn spawn_server(future: impl Future<Output = ()> + Send + 'static) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move { bounded(future).await })
}

async fn listener() -> TcpListener {
    TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback WebSocket listener")
}

fn request(address: SocketAddr, maximum: usize) -> StreamRequest {
    let mut request = format!("ws://{address}/v1/events/ws")
        .into_client_request()
        .expect("build exact loopback upgrade");
    request.headers_mut().insert(
        SEC_WEBSOCKET_PROTOCOL,
        HeaderValue::from_static(NORITO_V1_WEBSOCKET_SUBPROTOCOL),
    );
    request.headers_mut().insert(
        http::header::AUTHORIZATION,
        HeaderValue::from_static("Bearer test-runtime-secret"),
    );
    request.headers_mut().insert(
        "x-iroha-stream-test",
        HeaderValue::from_static("exact-transport-header"),
    );
    StreamRequest {
        request,
        operation: OPERATION,
        max_message_bytes: maximum,
        timeout: TEST_DEADLINE,
    }
}

async fn accept(
    listener: TcpListener,
    expected: HeaderMap,
) -> tokio_tungstenite::WebSocketStream<TcpStream> {
    let (stream, _) = bounded(listener.accept())
        .await
        .expect("accept loopback client");
    bounded(tokio_tungstenite::accept_hdr_async(
        stream,
        move |request: &Request, mut response: Response| {
            assert_eq!(request.method(), http::Method::GET);
            assert_eq!(request.uri().path(), "/v1/events/ws");
            assert_eq!(request.uri().query(), None);
            assert_eq!(request.headers(), &expected);
            response.headers_mut().insert(
                SEC_WEBSOCKET_PROTOCOL,
                HeaderValue::from_static(NORITO_V1_WEBSOCKET_SUBPROTOCOL),
            );
            Ok(response)
        },
    ))
    .await
    .expect("complete exact loopback upgrade")
}

#[tokio::test]
async fn default_transport_preserves_upgrade_subscription_ping_pong_binary_and_close() {
    let listener = listener().await;
    let request = request(listener.local_addr().unwrap(), 4096);
    let expected_headers = request.request.headers().clone();
    let subscription =
        EventSubscriptionRequest::new(vec![EventFilterBox::Data(DataEventFilter::Any)]);
    let encoded = norito::to_bytes(&subscription).expect("canonical event subscription");
    let expected_subscription = encoded.clone();
    let server = spawn_server(async move {
        let mut socket = accept(listener, expected_headers).await;
        assert_eq!(
            bounded(socket.next()).await.unwrap().unwrap(),
            Message::Binary(expected_subscription.into()),
        );
        bounded(socket.send(Message::Pong(vec![1, 2].into())))
            .await
            .unwrap();
        bounded(socket.send(Message::Ping(vec![3, 5, 8].into())))
            .await
            .unwrap();
        assert_eq!(
            bounded(socket.next()).await.unwrap().unwrap(),
            Message::Pong(vec![3, 5, 8].into()),
            "the adapter must service control frames while awaiting binary data",
        );
        bounded(socket.send(Message::Binary(vec![13, 21, 34].into())))
            .await
            .unwrap();
        assert!(matches!(
            bounded(socket.next()).await.unwrap().unwrap(),
            Message::Close(None),
        ));
        bounded(socket.flush()).await.unwrap();
    });
    let client = async {
        let mut connection = bounded(DefaultStreamTransport.connect(request))
            .await
            .unwrap();
        assert_eq!(
            connection.response.status(),
            StatusCode::SWITCHING_PROTOCOLS
        );
        assert_eq!(
            connection
                .response
                .headers()
                .get(SEC_WEBSOCKET_PROTOCOL)
                .unwrap(),
            NORITO_V1_WEBSOCKET_SUBPROTOCOL,
        );
        bounded(connection.socket.send(encoded)).await.unwrap();
        assert!(matches!(
            bounded(connection.socket.next()).await.unwrap().unwrap(),
            StreamFrame::Binary(bytes) if bytes == [13, 21, 34],
        ));
        bounded(connection.socket.close()).await.unwrap();
    };
    let ((), server) = tokio::join!(client, server);
    server.expect("loopback server joined successfully");
}

#[tokio::test]
async fn default_transport_preserves_close_code_and_reason() {
    for (code, reason) in [
        (1000, "stream_closed"),
        (1008, "stream_authorization_revoked"),
        (1013, "event_stream_lagged:7"),
    ] {
        let listener = listener().await;
        let request = request(listener.local_addr().unwrap(), 1024);
        let headers = request.request.headers().clone();
        let server = spawn_server(async move {
            let mut socket = accept(listener, headers).await;
            bounded(socket.send(Message::Close(Some(CloseFrame {
                code: CloseCode::from(code),
                reason: reason.into(),
            }))))
            .await
            .unwrap();
        });
        let client = async {
            let mut connection = bounded(DefaultStreamTransport.connect(request))
                .await
                .unwrap();
            match bounded(connection.socket.next()).await.unwrap().unwrap() {
                StreamFrame::Close {
                    code: actual,
                    reason: actual_reason,
                } => {
                    assert_eq!(actual, Some(code));
                    assert_eq!(actual_reason, reason);
                }
                StreamFrame::Binary(_) => panic!("close disposition must not become binary data"),
            }
        };
        let ((), server) = tokio::join!(client, server);
        server.expect("close disposition server joined");
    }
}

#[tokio::test]
async fn default_transport_rejects_text_frames() {
    let listener = listener().await;
    let request = request(listener.local_addr().unwrap(), 1024);
    let headers = request.request.headers().clone();
    let server = spawn_server(async move {
        let mut socket = accept(listener, headers).await;
        bounded(socket.send(Message::Text("not binary Norito".into())))
            .await
            .unwrap();
    });
    let client = async {
        let mut connection = bounded(DefaultStreamTransport.connect(request))
            .await
            .unwrap();
        assert!(matches!(
            bounded(connection.socket.next()).await.unwrap(),
            Err(Error::StreamProtocol {
                operation: OPERATION,
                ..
            }),
        ));
    };
    let ((), server) = tokio::join!(client, server);
    server.expect("text rejection server joined");
}

#[tokio::test]
async fn default_transport_bounds_single_frames_and_fragmented_messages() {
    const MAXIMUM: usize = 16;
    for fragmented in [false, true] {
        for length in [MAXIMUM, MAXIMUM + 1] {
            let listener = listener().await;
            let request = request(listener.local_addr().unwrap(), MAXIMUM);
            let headers = request.request.headers().clone();
            let payload = vec![0x5a; length];
            let expected = payload.clone();
            let server = spawn_server(async move {
                let mut socket = accept(listener, headers).await;
                if fragmented {
                    bounded(socket.send(Message::Frame(Frame::message(
                        payload[..8].to_vec(),
                        OpCode::Data(Data::Binary),
                        false,
                    ))))
                    .await
                    .unwrap();
                    bounded(socket.send(Message::Frame(Frame::message(
                        payload[8..].to_vec(),
                        OpCode::Data(Data::Continue),
                        true,
                    ))))
                    .await
                    .unwrap();
                } else {
                    bounded(socket.send(Message::Binary(payload.into())))
                        .await
                        .unwrap();
                }
            });
            let client = async {
                let mut connection = bounded(DefaultStreamTransport.connect(request))
                    .await
                    .unwrap();
                let result = bounded(connection.socket.next()).await.unwrap();
                if length == MAXIMUM {
                    assert!(matches!(result, Ok(StreamFrame::Binary(bytes)) if bytes == expected));
                } else {
                    assert_eq!(
                        result.expect_err("oversized message must be rejected"),
                        Error::ResponseTooLarge {
                            maximum: MAXIMUM,
                            actual: Some(length)
                        },
                        "fragmented={fragmented}",
                    );
                }
            };
            let ((), server) = tokio::join!(client, server);
            server.expect("capacity server joined");
        }
    }
}

#[tokio::test]
async fn default_transport_rejects_oversized_send_without_writing_a_frame() {
    const MAXIMUM: usize = 16;
    let listener = listener().await;
    let request = request(listener.local_addr().unwrap(), MAXIMUM);
    let headers = request.request.headers().clone();
    let server = spawn_server(async move {
        let mut socket = accept(listener, headers).await;
        assert_eq!(
            bounded(socket.next()).await.unwrap().unwrap(),
            Message::Binary(vec![2; MAXIMUM].into()),
            "the rejected send must not have written any frame",
        );
        assert!(matches!(
            bounded(socket.next()).await.unwrap().unwrap(),
            Message::Close(None)
        ));
        bounded(socket.flush()).await.unwrap();
    });
    let client = async {
        let mut connection = bounded(DefaultStreamTransport.connect(request))
            .await
            .unwrap();
        assert_eq!(
            bounded(connection.socket.send(vec![1; MAXIMUM + 1]))
                .await
                .unwrap_err(),
            Error::ResponseTooLarge {
                maximum: MAXIMUM,
                actual: Some(MAXIMUM + 1)
            },
        );
        bounded(connection.socket.send(vec![2; MAXIMUM]))
            .await
            .unwrap();
        bounded(connection.socket.close()).await.unwrap();
    };
    let ((), server) = tokio::join!(client, server);
    server.expect("bounded send server joined");
}

#[tokio::test]
async fn default_transport_rejects_zero_bound_before_connecting() {
    let listener = listener().await;
    let request = request(listener.local_addr().unwrap(), 0);
    assert!(matches!(
        bounded(DefaultStreamTransport.connect(request)).await,
        Err(Error::InvalidRequest {
            operation: OPERATION,
            ..
        }),
    ));
    assert!(timeout(NO_CONNECT_WINDOW, listener.accept()).await.is_err());
}

#[tokio::test]
async fn default_transport_does_not_follow_redirects_or_retry_upgrade() {
    let source = Arc::new(listener().await);
    let destination = listener().await;
    let request = request(source.local_addr().unwrap(), 1024);
    let destination_address = destination.local_addr().unwrap();
    let server_listener = Arc::clone(&source);
    let server = spawn_server(async move {
        let (mut stream, _) = bounded(server_listener.accept()).await.unwrap();
        let mut header = Vec::new();
        loop {
            header.push(bounded(stream.read_u8()).await.unwrap());
            assert!(
                header.len() <= 16 * 1024,
                "test upgrade headers are bounded"
            );
            if header.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        let response = format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: ws://{destination_address}/v1/events/ws\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        bounded(stream.write_all(response.as_bytes()))
            .await
            .unwrap();
        bounded(stream.shutdown()).await.unwrap();
    });
    let client = bounded(DefaultStreamTransport.connect(request));
    let (result, server) = tokio::join!(client, server);
    server.expect("redirect response server joined");
    assert!(matches!(
        result,
        Err(Error::Http {
            operation: OPERATION,
            status: 307,
            ..
        })
    ));
    let (source_accept, destination_accept) = tokio::join!(
        timeout(NO_CONNECT_WINDOW, source.accept()),
        timeout(NO_CONNECT_WINDOW, destination.accept()),
    );
    assert!(source_accept.is_err(), "upgrade must not retry the source");
    assert!(
        destination_accept.is_err(),
        "upgrade must not follow Location"
    );
}

#[test]
fn stream_request_debug_redacts_upgrade_credentials_and_target() {
    let mut request = request("127.0.0.1:1234".parse().unwrap(), 4096);
    *request.request.uri_mut() = "ws://127.0.0.1:1234/private-resource?token=hidden-query-secret"
        .parse()
        .unwrap();
    let debug = format!("{request:?}");
    assert!(debug.contains(OPERATION));
    assert!(debug.contains("4096"));
    for private in [
        "test-runtime-secret",
        "exact-transport-header",
        "private-resource",
        "hidden-query-secret",
        "127.0.0.1",
    ] {
        assert!(!debug.contains(private), "Debug leaked {private}");
    }
}

#[test]
fn socket_error_preserves_io_http_capacity_and_protocol_categories() {
    for kind in [
        ErrorKind::ConnectionRefused,
        ErrorKind::PermissionDenied,
        ErrorKind::ConnectionReset,
    ] {
        let error = socket_error(
            OPERATION,
            tungstenite::Error::Io(std::io::Error::new(kind, "retained cause")),
        );
        assert_eq!(
            error,
            Error::Transport {
                operation: OPERATION,
                kind: TransportErrorKind::Io(kind),
                details: "retained cause".to_owned(),
            }
        );
    }
    assert_eq!(
        socket_error(
            OPERATION,
            tungstenite::Error::Io(std::io::Error::from(ErrorKind::TimedOut))
        ),
        Error::Timeout {
            operation: OPERATION
        },
    );
    for body in [None, Some(b"retained API rejection".to_vec())] {
        let expected = body.clone().unwrap_or_default();
        let response = http::Response::builder().status(403).body(body).unwrap();
        assert_eq!(
            socket_error(OPERATION, tungstenite::Error::Http(Box::new(response))),
            Error::Http {
                operation: OPERATION,
                status: 403,
                retry_after: None,
                body: expected,
            }
        );
    }
    assert_eq!(
        socket_error(
            OPERATION,
            tungstenite::Error::Capacity(tungstenite::error::CapacityError::MessageTooLong {
                size: 17,
                max_size: 16
            })
        ),
        Error::ResponseTooLarge {
            maximum: 16,
            actual: Some(17)
        },
    );
    assert!(matches!(
        socket_error(OPERATION, tungstenite::Error::Protocol(tungstenite::error::ProtocolError::ResetWithoutClosingHandshake)),
        Error::StreamProtocol { operation: OPERATION, details } if !details.is_empty(),
    ));
}

#[test]
fn rejected_upgrade_retains_exact_retry_after_delta() {
    let response = http::Response::builder()
        .status(429)
        .header(http::header::RETRY_AFTER, "3")
        .body(Some(b"slow down".to_vec()))
        .unwrap();
    assert_eq!(
        socket_error(OPERATION, tungstenite::Error::Http(Box::new(response))),
        Error::Http {
            operation: OPERATION,
            status: 429,
            retry_after: Some(Duration::from_secs(3)),
            body: b"slow down".to_vec(),
        }
    );
}
