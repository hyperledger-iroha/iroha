//! Async status negotiation, transport bounds, deadlines and facade contracts.

use super::{
    Client, WireFormatPreference, capability_test_support::AsyncOnlyTransport, dispatch,
    evidence_http_tests::*, status,
};
use crate::{
    Error, blocking,
    http::{Method, Response, TransportRequest},
};
use iroha_torii_shared::{status::Status as NodeStatus, uri};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn attach(
    responder: impl Fn(&TransportRequest) -> eyre::Result<Response<Vec<u8>>> + Send + Sync + 'static,
    delay: Duration,
    timeout: Duration,
    preference: WireFormatPreference,
) -> (Client, Arc<Mutex<Vec<TransportRequest>>>, Arc<AtomicUsize>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let completed = Arc::new(AtomicUsize::new(0));
    let transport = Arc::new(AsyncOnlyTransport {
        responder: Box::new(responder),
        requests: requests.clone(),
        completed: completed.clone(),
        delay,
    });
    let mut builder = client_with_base_url(base_url())
        .to_builder()
        .http_transport(transport);
    builder.torii_request_timeout = timeout;
    builder.wire_format_preference = preference;
    builder
        .headers
        .insert("accept".to_owned(), "application/obsolete".to_owned());
    builder
        .headers
        .insert("x-application".to_owned(), "status-test".to_owned());
    (builder.build().unwrap(), requests, completed)
}

fn response(body: Vec<u8>, media_type: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(200)
        .header("Content-Type", media_type)
        .body(body)
        .unwrap()
}

fn status_response() -> Response<Vec<u8>> {
    response(
        norito::json::to_vec(&NodeStatus::default()).unwrap(),
        "application/json",
    )
}

fn assert_default_status(actual: &NodeStatus) {
    assert_eq!(
        norito::to_bytes(actual).expect("actual status frame"),
        norito::to_bytes(&NodeStatus::default()).expect("expected status frame"),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn status_uses_async_transport_and_one_catalog_route_with_exact_negotiation() {
    fn require_send(_: impl Send) {}

    let (client, requests, completed) = attach(
        |_| Ok(status_response()),
        Duration::from_millis(15),
        Duration::from_secs(1),
        WireFormatPreference::JsonPreferred,
    );
    let capability = client.status();
    require_send(capability.get());
    let (result, ticked) = tokio::join!(capability.get(), async {
        tokio::task::yield_now().await;
        completed.load(Ordering::SeqCst) == 0
    });
    assert!(
        ticked,
        "the executor must progress while the status read is pending"
    );
    assert_default_status(&result.unwrap());
    let requests = requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        1,
        "a diagnostic read must not issue compatibility probes or retries"
    );
    let request = &requests[0];
    assert_eq!(request.method, Method::GET);
    assert_eq!(
        request.url.path(),
        iroha_torii_shared::route_catalog::diagnostic::STATUS.path()
    );
    assert!(request.url.query().is_none());
    assert!(request.body.is_empty());
    assert_eq!(request.max_response_bytes, status::MAX_RESPONSE_BYTES);
    assert_eq!(request.timeout, Some(Duration::from_secs(1)));
    let accepts: Vec<_> = request
        .headers
        .iter()
        .filter(|(name, _)| name == http::header::ACCEPT)
        .collect();
    assert_eq!(accepts.len(), 1);
    assert_eq!(
        accepts[0].1,
        WireFormatPreference::JsonPreferred.accept_header()
    );
    assert!(
        request
            .headers
            .iter()
            .any(|(name, value)| name == "x-application" && value == "status-test")
    );
}

#[tokio::test]
async fn status_and_version_deadlines_cancel_pending_custom_dispatch() {
    for version in [false, true] {
        let (client, requests, completed) = attach(
            |_| Ok(status_response()),
            Duration::from_secs(60),
            Duration::from_millis(10),
            WireFormatPreference::NoritoPreferred,
        );
        let error = if version {
            client.status().version().await.unwrap_err()
        } else {
            client.status().get().await.unwrap_err()
        };
        assert_eq!(
            error,
            Error::Timeout {
                operation: if version {
                    "core.api_version"
                } else {
                    "diagnostic.status"
                }
            }
        );
        assert_eq!(requests.lock().unwrap().len(), 1);
        assert_eq!(completed.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn status_preserves_typed_transport_errors_and_maps_untyped_failures() {
    let expected = Error::ResponseTooLarge {
        maximum: 7,
        actual: Some(8),
    };
    let failure = expected.clone();
    let (client, _, _) = attach(
        move |_| Err(failure.clone().into()),
        Duration::ZERO,
        Duration::ZERO,
        WireFormatPreference::JsonOnly,
    );
    assert_eq!(client.status().get().await.unwrap_err(), expected);
    let (client, _, _) = attach(
        |_| Err(eyre::eyre!("fixture connection reset")),
        Duration::ZERO,
        Duration::ZERO,
        WireFormatPreference::JsonOnly,
    );
    assert_eq!(
        client.status().get().await.unwrap_err(),
        Error::Transport {
            operation: "diagnostic.status",
            kind: crate::TransportErrorKind::Other,
            details: "fixture connection reset".to_owned(),
        }
    );
}

#[test]
fn transport_conversion_preserves_io_categories_through_contexts() {
    for kind in [
        std::io::ErrorKind::ConnectionRefused,
        std::io::ErrorKind::ConnectionReset,
    ] {
        let error = eyre::Report::new(std::io::Error::new(kind, "fixture failure"))
            .wrap_err("dispatch context");
        assert!(
            matches!(dispatch::transport_error("diagnostic.status", &error),
            Error::Transport { kind: crate::TransportErrorKind::Io(observed), .. } if observed == kind)
        );
    }
    let error = eyre::Report::new(std::io::Error::from(std::io::ErrorKind::TimedOut));
    assert_eq!(
        dispatch::transport_error("diagnostic.status", &error),
        Error::Timeout {
            operation: "diagnostic.status"
        }
    );
}

#[tokio::test]
async fn status_and_version_preserve_bounded_http_error_bodies_without_replay() {
    for version in [false, true] {
        let body = br#"{"code":"unavailable","retryable":false}"#.to_vec();
        let reply = Response::builder().status(503).body(body.clone()).unwrap();
        let (client, requests, _) = attach(
            move |_| Ok(reply.clone()),
            Duration::ZERO,
            Duration::ZERO,
            WireFormatPreference::NoritoPreferred,
        );
        let error = if version {
            client.status().version().await.unwrap_err()
        } else {
            client.status().get().await.unwrap_err()
        };
        assert_eq!(
            error,
            Error::Http {
                operation: if version {
                    "core.api_version"
                } else {
                    "diagnostic.status"
                },
                status: 503,
                retry_after: None,
                body
            }
        );
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn custom_transports_cannot_exceed_status_or_version_body_bounds() {
    for (version, maximum) in [(false, status::MAX_RESPONSE_BYTES), (true, 16 * 1024)] {
        for declared_length in [false, true] {
            let reply = if declared_length {
                Response::builder()
                    .status(200)
                    .header("Content-Length", maximum + 1)
                    .body(Vec::new())
                    .unwrap()
            } else {
                response(vec![b'x'; maximum + 1], "text/plain")
            };
            let (client, _, _) = attach(
                move |_| Ok(reply.clone()),
                Duration::ZERO,
                Duration::ZERO,
                WireFormatPreference::NoritoPreferred,
            );
            let error = if version {
                client.status().version().await.unwrap_err()
            } else {
                client.status().get().await.unwrap_err()
            };
            assert_eq!(
                error,
                Error::ResponseTooLarge {
                    maximum,
                    actual: (!declared_length).then_some(maximum + 1)
                }
            );
        }
    }
}

#[test]
fn status_rejects_missing_duplicate_folded_and_unrecognized_content_types() {
    let body = norito::to_bytes(&NodeStatus::default()).unwrap();
    for types in [
        vec![],
        vec!["application/x-norito", "application/x-norito"],
        vec!["application/json", "application/x-norito"],
        vec!["application/x-norito, application/json"],
        vec!["text/plain"],
    ] {
        let mut reply = Response::builder().status(200).body(body.clone()).unwrap();
        for value in types {
            reply
                .headers_mut()
                .append(http::header::CONTENT_TYPE, value.parse().unwrap());
        }
        assert!(matches!(
            status::decode_response(reply, WireFormatPreference::NoritoPreferred),
            Err(Error::Decode {
                operation: "diagnostic.status",
                ..
            })
        ));
    }
}

#[test]
fn status_negotiation_accepts_only_advertised_representations() {
    for preference in [
        WireFormatPreference::NoritoOnly,
        WireFormatPreference::JsonOnly,
        WireFormatPreference::NoritoPreferred,
        WireFormatPreference::JsonPreferred,
    ] {
        for json in [false, true] {
            let reply = if json {
                status_response()
            } else {
                response(
                    norito::to_bytes(&NodeStatus::default()).unwrap(),
                    "Application/X-Norito; charset=binary",
                )
            };
            let result = status::decode_response(reply, preference);
            let rejected = matches!(
                (preference, json),
                (WireFormatPreference::NoritoOnly, true) | (WireFormatPreference::JsonOnly, false)
            );
            assert_eq!(result.is_err(), rejected);
            if !rejected {
                assert_default_status(&result.unwrap());
            }
        }
    }
}

#[tokio::test]
async fn version_uses_the_canonical_text_contract_and_exact_size_limit() {
    let (client, requests, _) = attach(
        |_| Ok(response(vec![b'v'; 16 * 1024], "text/plain; charset=utf-8")),
        Duration::ZERO,
        Duration::ZERO,
        WireFormatPreference::NoritoOnly,
    );
    assert_eq!(client.status().version().await.unwrap().len(), 16 * 1024);
    let request = requests.lock().unwrap().pop().unwrap();
    assert_eq!(request.url.path(), uri::API_VERSION);
    assert_eq!(request.max_response_bytes, 16 * 1024);
    assert_eq!(request.timeout, None);
    assert!(request.body.is_empty());
    let accepts: Vec<_> = request
        .headers
        .iter()
        .filter(|(name, _)| name == http::header::ACCEPT)
        .collect();
    assert_eq!(accepts.len(), 1);
    assert_eq!(accepts[0].1, "text/plain, application/json");
    for reply in [
        response(Vec::new(), "text/plain"),
        response(b" \n ".to_vec(), "text/plain"),
        response(vec![0xff], "text/plain"),
        response(b"1".to_vec(), "application/json"),
    ] {
        let (client, _, _) = attach(
            move |_| Ok(reply.clone()),
            Duration::ZERO,
            Duration::ZERO,
            WireFormatPreference::JsonOnly,
        );
        assert!(matches!(
            client.status().version().await,
            Err(Error::Decode {
                operation: "core.api_version",
                ..
            })
        ));
    }
}

#[test]
fn blocking_status_uses_the_shared_async_implementation_and_rejects_async_runtime() {
    let (client, requests, _) = attach(
        |request| {
            Ok(if request.url.path() == uri::STATUS {
                status_response()
            } else {
                response(b" 1\n".to_vec(), "text/plain")
            })
        },
        Duration::ZERO,
        Duration::ZERO,
        WireFormatPreference::JsonOnly,
    );
    let facade = blocking::Client::from_client(client).unwrap();
    assert_default_status(&facade.status().get().unwrap());
    assert_eq!(facade.clone().status().version().unwrap(), "1");
    assert_eq!(requests.lock().unwrap().len(), 2);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        assert!(matches!(facade.status().get(), Err(Error::Blocking(_))));
        assert!(matches!(facade.status().version(), Err(Error::Blocking(_))));
        drop(facade);
    });
    assert_eq!(requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn capability_dispatch_returns_structured_request_construction_errors() {
    use crate::http::RequestBuilder as _;
    let client = client_with_base_url(base_url());
    let builder = client
        .default_request(Method::GET, base_url())
        .header("invalid\nname", "value");
    assert!(matches!(
        dispatch::send(&client, "diagnostic.status", builder, "application/json").await,
        Err(Error::InvalidRequest {
            operation: "diagnostic.status",
            ..
        })
    ));
}
