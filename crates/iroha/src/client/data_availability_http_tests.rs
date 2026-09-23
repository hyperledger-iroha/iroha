//! DA manifest transport, response binding and explicit blocking facade regressions.

use super::{
    Client, capability_test_support::AsyncOnlyTransport,
    data_availability::MAX_MANIFEST_RESPONSE_BYTES, evidence_http_tests::*,
};
use crate::{
    Error, blocking,
    http::{Method, Response, TransportRequest},
};
use iroha_data_model::da::types::StorageTicketId;
use iroha_torii_shared::da::DaManifestResponse;
use norito::json::Value;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

const OPERATION: &str = "data_availability.manifest.read";
const TICKET: StorageTicketId = StorageTicketId::new([0xab; 32]);

fn manifest() -> DaManifestResponse {
    DaManifestResponse {
        storage_ticket: hex::encode(TICKET.as_bytes()),
        client_blob_id: "01".repeat(32),
        blob_hash: "02".repeat(32),
        chunk_root: "03".repeat(32),
        manifest_hash: "04".repeat(32),
        lane_id: 1,
        epoch: 2,
        manifest_len: 3,
        manifest_norito: "YWJj".to_owned(),
        manifest: Value::Null,
        chunk_plan: Value::Null,
    }
}

fn response(value: &DaManifestResponse) -> Response<Vec<u8>> {
    Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(norito::json::to_vec(value).unwrap())
        .unwrap()
}

fn attach(
    responder: impl Fn(&TransportRequest) -> eyre::Result<Response<Vec<u8>>> + Send + Sync + 'static,
    delay: Duration,
    timeout: Duration,
) -> (Client, Arc<Mutex<Vec<TransportRequest>>>, Arc<AtomicUsize>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let completed = Arc::new(AtomicUsize::new(0));
    let transport = Arc::new(AsyncOnlyTransport {
        responder: Box::new(responder),
        requests: Arc::clone(&requests),
        completed: Arc::clone(&completed),
        delay,
    });
    let mut builder = client_with_base_url(base_url())
        .to_builder()
        .http_transport(transport);
    builder.torii_request_timeout = timeout;
    builder
        .headers
        .insert("accept".to_owned(), "application/obsolete".to_owned());
    for header in [
        "X-Iroha-Account",
        "x-Iroha-Signature",
        "X-Iroha-Timestamp-Ms",
        "X-Iroha-Nonce",
        "X-Iroha-Witness",
    ] {
        builder
            .headers
            .insert(header.to_owned(), "stale".to_owned());
    }
    (builder.build().unwrap(), requests, completed)
}

#[tokio::test(flavor = "current_thread")]
async fn manifest_uses_async_transport_without_blocking_the_executor() {
    fn require_send(_: impl Send) {}

    let (client, requests, completed) = attach(
        |_| Ok(response(&manifest())),
        Duration::from_millis(15),
        Duration::from_secs(1),
    );
    let capability = client.da();
    require_send(capability.manifest(&TICKET));
    let (result, ticked) = tokio::join!(capability.manifest(&TICKET), async {
        tokio::task::yield_now().await;
        completed.load(Ordering::SeqCst) == 0
    });
    assert!(ticked, "manifest I/O must leave the executor responsive");
    assert_eq!(
        norito::json::to_vec(&result.unwrap()).unwrap(),
        norito::json::to_vec(&manifest()).unwrap()
    );
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1, "manifest reads do not probe or retry");
    let request = &requests[0];
    assert_eq!(request.method, Method::GET);
    assert!(
        !request
            .headers
            .iter()
            .any(|(name, _)| name.as_str().starts_with("x-iroha-"))
    );
    assert_eq!(
        request.url.path(),
        format!("/v1/da/manifests/{}", hex::encode(TICKET.as_bytes()))
    );
    assert!(request.url.query().is_none());
    assert!(request.body.is_empty());
    assert_eq!(request.timeout, Some(Duration::from_secs(1)));
    assert_eq!(request.max_response_bytes, MAX_MANIFEST_RESPONSE_BYTES);
    let accepts: Vec<_> = request
        .headers
        .iter()
        .filter(|(name, _)| name == http::header::ACCEPT)
        .collect();
    assert_eq!(accepts.len(), 1);
    assert_eq!(accepts[0].1, "application/json");
}

#[tokio::test]
async fn manifest_rejects_a_response_for_another_or_noncanonical_ticket() {
    for ticket in [
        "cd".repeat(32),
        "AB".repeat(32),
        format!("0x{}", "ab".repeat(32)),
    ] {
        let mut wrong = manifest();
        wrong.storage_ticket = ticket;
        let (client, requests, _) = attach(
            move |_| Ok(response(&wrong)),
            Duration::ZERO,
            Duration::ZERO,
        );
        assert_eq!(
            client.da().manifest(&TICKET).await.unwrap_err(),
            Error::ResponseBinding {
                operation: OPERATION,
                field: "storage_ticket",
            }
        );
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn manifest_deadline_cancels_pending_async_transport() {
    let (client, requests, completed) = attach(
        |_| Ok(response(&manifest())),
        Duration::from_secs(60),
        Duration::from_millis(10),
    );
    assert_eq!(
        client.da().manifest(&TICKET).await.unwrap_err(),
        Error::Timeout {
            operation: OPERATION
        }
    );
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert_eq!(completed.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn manifest_bounds_injected_responses_before_decoding() {
    for declared in [false, true] {
        let (client, requests, _) = attach(
            move |_| {
                Ok(if declared {
                    Response::builder()
                        .status(200)
                        .header("Content-Length", MAX_MANIFEST_RESPONSE_BYTES + 1)
                        .body(Vec::new())
                        .unwrap()
                } else {
                    Response::builder()
                        .status(200)
                        .body(vec![b'x'; MAX_MANIFEST_RESPONSE_BYTES + 1])
                        .unwrap()
                })
            },
            Duration::ZERO,
            Duration::ZERO,
        );
        assert_eq!(
            client.da().manifest(&TICKET).await.unwrap_err(),
            Error::ResponseTooLarge {
                maximum: MAX_MANIFEST_RESPONSE_BYTES,
                actual: (!declared).then_some(MAX_MANIFEST_RESPONSE_BYTES + 1),
            }
        );
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn manifest_requires_one_json_content_type_and_a_typed_body() {
    for content_types in [
        vec![],
        vec!["text/plain"],
        vec!["application/json", "application/json"],
        vec!["application/json, application/x-norito"],
    ] {
        let (client, _, _) = attach(
            move |_| {
                let mut reply = response(&manifest());
                reply.headers_mut().remove(http::header::CONTENT_TYPE);
                for value in &content_types {
                    reply
                        .headers_mut()
                        .append(http::header::CONTENT_TYPE, value.parse().unwrap());
                }
                Ok(reply)
            },
            Duration::ZERO,
            Duration::ZERO,
        );
        assert!(matches!(
            client.da().manifest(&TICKET).await,
            Err(Error::Decode {
                operation: OPERATION,
                ..
            })
        ));
    }
    for body in [b"{}".to_vec(), b"not-json".to_vec()] {
        let (client, _, _) = attach(
            move |_| {
                Ok(Response::builder()
                    .status(200)
                    .header("Content-Type", "application/json")
                    .body(body.clone())
                    .unwrap())
            },
            Duration::ZERO,
            Duration::ZERO,
        );
        assert!(matches!(
            client.da().manifest(&TICKET).await,
            Err(Error::Decode {
                operation: OPERATION,
                ..
            })
        ));
    }
}

#[tokio::test]
async fn manifest_preserves_http_and_transport_failures_without_replay() {
    let (client, requests, _) = attach(
        |_| {
            Ok(Response::builder()
                .status(503)
                .header("Retry-After", "7")
                .body(b"unavailable".to_vec())
                .unwrap())
        },
        Duration::ZERO,
        Duration::ZERO,
    );
    assert_eq!(
        client.da().manifest(&TICKET).await.unwrap_err(),
        Error::Http {
            operation: OPERATION,
            status: 503,
            retry_after: Some(Duration::from_secs(7)),
            body: b"unavailable".to_vec(),
        }
    );
    assert_eq!(requests.lock().unwrap().len(), 1);
    let (client, requests, _) = attach(
        |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "manifest connection reset",
            )
            .into())
        },
        Duration::ZERO,
        Duration::ZERO,
    );
    assert!(matches!(
        client.da().manifest(&TICKET).await,
        Err(Error::Transport {
            operation: OPERATION,
            kind: crate::TransportErrorKind::Io(std::io::ErrorKind::ConnectionReset),
            ..
        })
    ));
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[test]
fn blocking_manifest_uses_owned_runtime_and_rejects_async_entry() {
    let (client, requests, _) = attach(
        |_| Ok(response(&manifest())),
        Duration::ZERO,
        Duration::ZERO,
    );
    let facade = blocking::Client::from_client(client).unwrap();
    assert_eq!(
        facade.da().manifest(&TICKET).unwrap().storage_ticket,
        hex::encode(TICKET.as_bytes())
    );
    assert_eq!(
        facade.clone().da().manifest(&TICKET).unwrap().manifest_len,
        3
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        assert!(matches!(
            facade.da().manifest(&TICKET),
            Err(Error::Blocking(blocking::BlockingCallError::AsyncRuntime {
                flavor: blocking::AsyncRuntimeFlavor::CurrentThread
            }))
        ));
        drop(facade);
    });
    assert_eq!(requests.lock().unwrap().len(), 2);
}
