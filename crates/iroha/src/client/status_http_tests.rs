//! Async status negotiation, transport bounds, deadlines and facade contracts.

use super::{
    Client, WireFormatPreference, capability_test_support::AsyncOnlyTransport, dispatch,
    evidence_http_tests::*, status,
};
use crate::{
    Error, StatusFailureReason, blocking,
    http::{Method, Response, TransportRequest},
};
use iroha_torii_shared::{route_catalog, status::Status as NodeStatus};
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
    assert_eq!(request.url.path(), route_catalog::diagnostic::STATUS.path());
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
        {
            let actual_error = error;
            let Error::Timeout {
                operation: actual_operation,
            } = &actual_error
            else {
                panic!("unexpected SDK error: {actual_error:?}");
            };
            assert_eq!(
                (actual_operation,),
                (&(if version {
                    "core.api_version"
                } else {
                    "diagnostic.status"
                }),)
            );
        };
        assert_eq!(requests.lock().unwrap().len(), 1);
        assert_eq!(completed.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn status_preserves_typed_transport_errors_and_maps_untyped_failures() {
    let failure = Mutex::new(Some(Error::ResponseTooLarge {
        maximum: 7,
        actual: Some(8),
    }));
    let (client, _, _) = attach(
        move |_| {
            Err(failure
                .lock()
                .unwrap()
                .take()
                .expect("one original failure")
                .into())
        },
        Duration::ZERO,
        Duration::ZERO,
        WireFormatPreference::JsonOnly,
    );
    assert!(matches!(
        client.status().get().await.unwrap_err(),
        Error::ResponseTooLarge {
            maximum: 7,
            actual: Some(8)
        }
    ));
    let (client, _, _) = attach(
        |_| Err(eyre::eyre!("fixture connection reset")),
        Duration::ZERO,
        Duration::ZERO,
        WireFormatPreference::JsonOnly,
    );
    {
        let actual_error = client.status().get().await.unwrap_err();
        let Error::Transport {
            operation: actual_operation,
            kind: actual_kind,
            details: actual_details,
        } = &actual_error
        else {
            panic!("unexpected SDK error: {actual_error:?}");
        };
        assert_eq!(
            (actual_operation, actual_kind, actual_details,),
            (
                &("diagnostic.status"),
                &(crate::TransportErrorKind::Other),
                &("fixture connection reset".to_owned()),
            )
        );
    };
}

#[test]
fn transport_conversion_moves_original_typed_response_custody_through_contexts() {
    let mut body = Vec::with_capacity(128);
    body.extend_from_slice(b"original bounded transport response");
    let original_address = body.as_ptr();
    let original_capacity = body.capacity();
    let error = eyre::Report::new(Error::Http {
        operation: "nexus.validator_committee.read",
        status: 429,
        retry_after: Some(Duration::from_secs(3)),
        body,
    })
    .wrap_err("first dispatch context")
    .wrap_err("outer operation context");
    let converted = dispatch::transport_error("diagnostic.status", error);
    let Error::Http {
        operation,
        status,
        retry_after,
        body,
    } = converted
    else {
        panic!("original typed error must survive contexts: {converted:?}");
    };
    assert_eq!(operation, "nexus.validator_committee.read");
    assert_eq!(status, 429);
    assert_eq!(retry_after, Some(Duration::from_secs(3)));
    assert_eq!(body, b"original bounded transport response");
    assert_eq!(body.as_ptr(), original_address, "move the same backing");
    assert_eq!(body.capacity(), original_capacity);
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
            matches!(dispatch::transport_error("diagnostic.status", error),
            Error::Transport { kind: crate::TransportErrorKind::Io(observed), .. } if observed == kind)
        );
    }
    let error = eyre::Report::new(std::io::Error::from(std::io::ErrorKind::TimedOut));
    {
        let actual_error = dispatch::transport_error("diagnostic.status", error);
        let Error::Timeout {
            operation: actual_operation,
        } = &actual_error
        else {
            panic!("unexpected SDK error: {actual_error:?}");
        };
        assert_eq!((actual_operation,), (&("diagnostic.status"),));
    };
}

#[tokio::test]
async fn status_unavailable_is_typed_without_changing_other_http_errors_or_replaying() {
    for version in [false, true] {
        for status in [400, 429, 500, 503] {
            let body = br#"{"code":"unavailable","retryable":false}"#.to_vec();
            let reply = Response::builder()
                .status(status)
                .body(body.clone())
                .unwrap();
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
            if !version && status == 503 {
                assert!(matches!(
                    error,
                    Error::StatusUnavailable {
                        reason: None,
                        retry_after: None
                    }
                ));
            } else {
                let Error::Http {
                    operation,
                    status: actual_status,
                    retry_after,
                    body: actual_body,
                } = error
                else {
                    panic!("unexpected SDK error: {error:?}");
                };
                assert_eq!(
                    operation,
                    if version {
                        "core.api_version"
                    } else {
                        "diagnostic.status"
                    }
                );
                assert_eq!(actual_status, status);
                assert_eq!(retry_after, None);
                assert_eq!(actual_body, body);
            }
            assert_eq!(requests.lock().unwrap().len(), 1);
        }
    }
}

#[tokio::test]
async fn status_unavailable_reasons_are_safe_in_errors_and_do_not_trigger_retries() {
    let cases = [
        StatusFailureReason::Disabled,
        StatusFailureReason::MailboxUnavailable,
        StatusFailureReason::ActorClosed,
        StatusFailureReason::DeadlineElapsed,
        StatusFailureReason::StateBusy,
        StatusFailureReason::StateUnavailable,
        StatusFailureReason::CheckpointChanged,
        StatusFailureReason::MissingBlock,
        StatusFailureReason::JournalMismatch,
        StatusFailureReason::CounterOverflow,
        StatusFailureReason::CounterMismatch,
        StatusFailureReason::MetricsStale,
        StatusFailureReason::ProfileRestricted,
    ];
    for reason in cases {
        let reply = Response::builder()
            .status(503)
            .header("x-iroha-reject-code", reason.code())
            .header("Retry-After", "3")
            .body(b"DO_NOT_LOG_STATUS_BODY".to_vec())
            .unwrap();
        let (client, requests, completed) = attach(
            move |_| Ok(reply.clone()),
            Duration::ZERO,
            Duration::ZERO,
            WireFormatPreference::NoritoPreferred,
        );
        let error = client.status().get().await.unwrap_err();
        {
            let actual_error = &error;
            let Error::StatusUnavailable {
                reason: actual_reason,
                retry_after: actual_retry_after,
            } = &actual_error
            else {
                panic!("unexpected SDK error: {actual_error:?}");
            };
            assert_eq!(
                (actual_reason, actual_retry_after,),
                (&(Some(reason)), &(Some(Duration::from_secs(3))),)
            );
        };
        assert_eq!(
            error.to_string(),
            format!("diagnostic.status returned HTTP 503 ({})", reason.code(),)
        );
        assert!(!format!("{error:?}").contains("DO_NOT_LOG_STATUS_BODY"));
        let report = eyre::Report::from(error);
        assert!(format!("{report:?}").contains(reason.code()));
        assert!(!format!("{report:?}").contains("DO_NOT_LOG_STATUS_BODY"));
        assert_eq!(requests.lock().unwrap().len(), 1);
        assert_eq!(completed.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn status_unavailable_rejects_missing_unknown_invalid_and_duplicate_reason_headers() {
    use http::HeaderValue;
    let known = HeaderValue::from_static("status_state_unavailable");
    let busy = HeaderValue::from_static("status_state_busy");
    let cases = [
        Vec::new(),
        vec![HeaderValue::from_static("DO_NOT_LOG_UNKNOWN_HEADER")],
        vec![HeaderValue::from_static(" status_state_busy")],
        vec![HeaderValue::from_static("STATUS_STATE_BUSY")],
        vec![HeaderValue::from_static(
            "status_state_busy,status_state_busy",
        )],
        vec![busy.clone(), busy.clone()],
        vec![busy, known.clone()],
        vec![HeaderValue::from_static(" status_state_unavailable")],
        vec![HeaderValue::from_static("STATUS_STATE_UNAVAILABLE")],
        vec![HeaderValue::from_static(
            "status_state_unavailable,status_state_unavailable",
        )],
        vec![HeaderValue::from_bytes(&[0xff]).unwrap()],
        vec![known.clone(), known.clone()],
        vec![known, HeaderValue::from_static("DO_NOT_LOG_UNKNOWN_HEADER")],
    ];
    for codes in cases {
        let mut reply = Response::builder()
            .status(503)
            .header("Retry-After", "18446744073709551616")
            .body(b"DO_NOT_LOG_STATUS_BODY".to_vec())
            .unwrap();
        for code in codes {
            reply.headers_mut().append("x-iroha-reject-code", code);
        }
        let error =
            status::decode_response(reply, WireFormatPreference::NoritoPreferred).unwrap_err();
        {
            let actual_error = &error;
            let Error::StatusUnavailable {
                reason: actual_reason,
                retry_after: actual_retry_after,
            } = &actual_error
            else {
                panic!("unexpected SDK error: {actual_error:?}");
            };
            assert_eq!((actual_reason, actual_retry_after,), (&(None), &(None),));
        };
        assert_eq!(
            error.to_string(),
            "diagnostic.status returned HTTP 503 (unclassified)"
        );
        for display in [error.to_string(), format!("{error:?}")] {
            assert!(!display.contains("DO_NOT_LOG"));
        }
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
            {
                let actual_error = error;
                let Error::ResponseTooLarge {
                    maximum: actual_maximum,
                    actual: actual_actual,
                } = &actual_error
                else {
                    panic!("unexpected SDK error: {actual_error:?}");
                };
                assert_eq!(
                    (actual_maximum, actual_actual,),
                    (&(maximum), &((!declared_length).then_some(maximum + 1)),)
                );
            };
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
    assert_eq!(request.url.path(), route_catalog::core::API_VERSION.path());
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
            Ok(
                if request.url.path() == route_catalog::diagnostic::STATUS.path() {
                    status_response()
                } else {
                    response(b" 1\n".to_vec(), "text/plain")
                },
            )
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
