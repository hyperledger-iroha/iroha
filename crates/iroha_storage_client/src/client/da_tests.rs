//! Storage DA workflows use async SDK transport and preserve typed request failures.

use super::{
    DaProofConfig, SorafsGatewayFetchConfig, SorafsGatewayFetchOptions, SorafsGatewayProviderInput,
    StorageClient, tests::test_client,
};
use crate::da::{DaManifestBundle, tests::manifest_response};
use iroha::{
    Error,
    client::Client,
    http::{HttpTransport, Method, Response, TransportFuture, TransportRequest},
};
use iroha_data_model::da::types::StorageTicketId;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

const OPERATION: &str = "data_availability.manifest.read";
type Responder = dyn Fn(&TransportRequest) -> eyre::Result<Response<Vec<u8>>> + Send + Sync;

struct AsyncTransport {
    responder: Box<Responder>,
    requests: Arc<Mutex<Vec<TransportRequest>>>,
    completed: Arc<AtomicUsize>,
    delay: Duration,
}

impl std::fmt::Debug for AsyncTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("StorageDaAsyncTransport")
    }
}

impl HttpTransport for AsyncTransport {
    fn send_blocking(&self, _: TransportRequest) -> eyre::Result<Response<Vec<u8>>> {
        panic!("storage async workflows must never dispatch synchronous HTTP")
    }

    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move {
            let response = (self.responder)(&request);
            self.requests.lock().unwrap().push(request);
            tokio::time::sleep(self.delay).await;
            self.completed.fetch_add(1, Ordering::SeqCst);
            response
        })
    }
}

fn attach(
    responder: impl Fn(&TransportRequest) -> eyre::Result<Response<Vec<u8>>> + Send + Sync + 'static,
    delay: Duration,
    timeout: Duration,
) -> (Client, Arc<Mutex<Vec<TransportRequest>>>, Arc<AtomicUsize>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let completed = Arc::new(AtomicUsize::new(0));
    let transport = Arc::new(AsyncTransport {
        responder: Box::new(responder),
        requests: Arc::clone(&requests),
        completed: Arc::clone(&completed),
        delay,
    });
    let mut builder = test_client().to_builder().http_transport(transport);
    builder.torii_request_timeout = timeout;
    (builder.build().unwrap(), requests, completed)
}

fn ticket() -> StorageTicketId {
    StorageTicketId::new(
        hex::decode(manifest_response().storage_ticket)
            .unwrap()
            .try_into()
            .unwrap(),
    )
}

fn response() -> Response<Vec<u8>> {
    Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(norito::json::to_vec(&manifest_response()).unwrap())
        .unwrap()
}

#[tokio::test(flavor = "current_thread")]
async fn storage_manifest_is_async_and_validates_the_canonical_artifact() {
    fn require_send(_: impl Send) {}

    let (client, requests, completed) = attach(
        |_| Ok(response()),
        Duration::from_millis(15),
        Duration::from_secs(1),
    );
    let storage = StorageClient::new(&client);
    let ticket = ticket();
    require_send(storage.da_manifest(&ticket));
    let (result, responsive) = tokio::join!(storage.da_manifest(&ticket), async {
        tokio::task::yield_now().await;
        completed.load(Ordering::SeqCst) == 0
    });
    assert!(
        responsive,
        "manifest I/O must yield on the current-thread executor"
    );
    let actual = result.unwrap();
    let expected = DaManifestBundle::try_from(manifest_response()).unwrap();
    assert_eq!(actual.manifest_bytes, expected.manifest_bytes);
    assert_eq!(actual.manifest_json, expected.manifest_json);
    assert_eq!(actual.chunk_plan, expected.chunk_plan);
    assert_eq!(actual.decode_manifest().unwrap().storage_ticket, ticket);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, Method::GET);
    assert_eq!(
        requests[0].url.path(),
        format!("/v1/da/manifests/{}", hex::encode(ticket.as_bytes()))
    );
    assert!(requests[0].url.query().is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn storage_manifest_preserves_sdk_http_and_response_bound_errors() {
    let (client, requests, _) = attach(
        |_| {
            Ok(Response::builder()
                .status(503)
                .header("Retry-After", "4")
                .body(b"not ready".to_vec())
                .unwrap())
        },
        Duration::ZERO,
        Duration::ZERO,
    );
    let error = StorageClient::new(&client)
        .da_manifest(&ticket())
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<Error>(),
        Some(&Error::Http {
            operation: OPERATION,
            status: 503,
            retry_after: Some(Duration::from_secs(4)),
            body: b"not ready".to_vec(),
        })
    );
    assert_eq!(requests.lock().unwrap().len(), 1);

    let (client, requests, _) = attach(
        |request| {
            Ok(Response::builder()
                .status(200)
                .header("Content-Length", request.max_response_bytes + 1)
                .body(Vec::new())
                .unwrap())
        },
        Duration::ZERO,
        Duration::ZERO,
    );
    let error = StorageClient::new(&client)
        .da_manifest(&ticket())
        .await
        .unwrap_err();
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        error.downcast_ref::<Error>(),
        Some(&Error::ResponseTooLarge {
            maximum: requests[0].max_response_bytes,
            actual: None,
        })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn fetch_manifest_to_dir_persists_the_validated_artifact_and_projections() {
    let (client, requests, _) = attach(|_| Ok(response()), Duration::ZERO, Duration::ZERO);
    let storage = StorageClient::new(&client);
    let ticket = ticket();
    let root = tempfile::tempdir().unwrap();
    let paths = storage
        .fetch_da_manifest_to_dir(&ticket, root.path())
        .await
        .unwrap();
    let expected = DaManifestBundle::try_from(manifest_response()).unwrap();
    assert_eq!(
        std::fs::read(&paths.manifest_raw).unwrap(),
        expected.manifest_bytes
    );
    let manifest: norito::json::Value =
        norito::json::from_slice(&std::fs::read(&paths.manifest_json).unwrap()).unwrap();
    let chunk_plan: norito::json::Value =
        norito::json::from_slice(&std::fs::read(&paths.chunk_plan).unwrap()).unwrap();
    assert_eq!(manifest, expected.manifest_json);
    assert_eq!(chunk_plan, expected.chunk_plan);
    assert_eq!(
        paths.manifest_raw,
        root.path().join(format!(
            "manifest_{}.norito",
            hex::encode(ticket.as_bytes())
        ))
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 3);
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn proof_manifest_timeout_cancels_before_gateway_work() {
    struct UnusedProviders;
    impl IntoIterator for UnusedProviders {
        type Item = SorafsGatewayProviderInput;
        type IntoIter = std::iter::Empty<Self::Item>;
        fn into_iter(self) -> Self::IntoIter {
            panic!("providers must not be consumed before a valid manifest arrives")
        }
    }
    let (client, requests, completed) = attach(
        |_| Ok(response()),
        Duration::from_secs(60),
        Duration::from_millis(10),
    );
    let gateway = SorafsGatewayFetchConfig {
        manifest_id_hex: "ab".repeat(32),
        chunker_handle: "sorafs.sf1@1.0.0".to_owned(),
        manifest_envelope_b64: None,
        client_id: None,
        expected_manifest_cid_hex: None,
        blinded_cid_b64: None,
        salt_epoch: None,
        expected_cache_version: None,
    };
    let error = StorageClient::new(&client)
        .prove_da_availability(
            &ticket(),
            gateway,
            UnusedProviders,
            SorafsGatewayFetchOptions::default(),
            DaProofConfig::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<Error>(),
        Some(&Error::Timeout {
            operation: OPERATION
        })
    );
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert_eq!(completed.load(Ordering::SeqCst), 0);
}
