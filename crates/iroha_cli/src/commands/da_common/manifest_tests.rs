//! Canonical DA manifest CLI configuration, transport and argument contracts.

use super::DaManifestFetcher;
use clap::Args as _;
use iroha::{
    Error,
    client::AuthorityContextError,
    da::DaManifestResponse,
    http::{HttpTransport, Response, TransportFuture, TransportRequest},
};
use norito::json::Value;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

const TICKET: &str = "abababababababababababababababababababababababababababababababab";
const OPERATION: &str = "data_availability.manifest.read";
type Responder = dyn Fn(&TransportRequest) -> eyre::Result<Response<Vec<u8>>> + Send + Sync;

struct AsyncTransport {
    requests: Arc<Mutex<Vec<TransportRequest>>>,
    responder: Box<Responder>,
}

impl std::fmt::Debug for AsyncTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CliDaManifestAsyncTransport")
    }
}

impl HttpTransport for AsyncTransport {
    fn send_blocking(&self, _: TransportRequest) -> eyre::Result<Response<Vec<u8>>> {
        panic!("CLI facade must dispatch through the canonical asynchronous implementation")
    }

    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move {
            let response = (self.responder)(&request);
            self.requests.lock().unwrap().push(request);
            response
        })
    }
}

fn fetcher(
    responder: impl Fn(&TransportRequest) -> eyre::Result<Response<Vec<u8>>> + Send + Sync + 'static,
) -> (DaManifestFetcher, Arc<Mutex<Vec<TransportRequest>>>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let transport = Arc::new(AsyncTransport {
        requests: Arc::clone(&requests),
        responder: Box::new(responder),
    });
    let client = iroha::blocking::Client::with_http_transport(crate::fallback_config(), transport)
        .expect("valid CLI fixture context");
    (DaManifestFetcher { client }, requests)
}

#[test]
fn manifest_fetcher_override_creates_a_new_context_without_mutating_configuration() {
    let config = crate::fallback_config();
    let original_endpoint = config.torii_api_url.clone();
    let original_account = config.account.clone();
    let original_network = config.network_id;
    let configured = DaManifestFetcher::new(&config, None).unwrap();
    let overridden = DaManifestFetcher::new(&config, Some("https://node.example/tenant/")).unwrap();
    assert_eq!(config.torii_api_url, original_endpoint);
    assert_eq!(config.account, original_account);
    assert_eq!(config.network_id, original_network);
    assert_eq!(configured.client.client().endpoint(), &original_endpoint);
    assert_eq!(
        overridden.client.client().endpoint().as_str(),
        "https://node.example/tenant/"
    );
    assert_eq!(overridden.client.client().network_id(), &original_network);
    assert_eq!(
        overridden.client.account_client().authority(),
        &original_account
    );
    assert_eq!(
        overridden.client.client().torii_request_timeout(),
        config.torii_request_timeout
    );
}

#[test]
fn manifest_fetcher_rejects_invalid_base_urls_before_construction() {
    let config = crate::fallback_config();
    for (url, expected) in [
        (
            "https://node.example/tenant",
            AuthorityContextError::EndpointPathMissingTrailingSlash,
        ),
        (
            "https://node.example/?ticket=wrong",
            AuthorityContextError::EndpointHasQueryOrFragment,
        ),
        (
            "https://node.example/#fragment",
            AuthorityContextError::EndpointHasQueryOrFragment,
        ),
        (
            "https://user:password@node.example/",
            AuthorityContextError::EmbeddedEndpointCredentials,
        ),
    ] {
        let error = DaManifestFetcher::new(&config, Some(url))
            .err()
            .expect("invalid base URL must fail");
        assert_eq!(
            error.downcast_ref::<Error>(),
            Some(&Error::Context(expected))
        );
        assert_eq!(config.torii_api_url.as_str(), "http://127.0.0.1:8080/");
    }
}

#[test]
fn manifest_fetcher_rejects_invalid_tickets_before_dispatch() {
    let (fetcher, requests) = fetcher(|_| panic!("invalid ticket must not reach HTTP"));
    for ticket in [
        String::new(),
        "a".repeat(62),
        "a".repeat(66),
        "g".repeat(64),
        format!("0x{TICKET}"),
    ] {
        assert!(fetcher.fetch(&ticket).is_err(), "{ticket}");
    }
    assert!(requests.lock().unwrap().is_empty());
}

#[test]
fn manifest_fetcher_preserves_http_errors_and_response_ticket_binding() {
    let (fetcher, requests) = fetcher(|_| {
        Ok(Response::builder()
            .status(503)
            .header("Retry-After", "3")
            .body(b"unavailable".to_vec())
            .unwrap())
    });
    let error = fetcher.fetch(TICKET).unwrap_err();
    assert_eq!(
        error.downcast_ref::<Error>(),
        Some(&Error::Http {
            operation: OPERATION,
            status: 503,
            retry_after: Some(Duration::from_secs(3)),
            body: b"unavailable".to_vec(),
        })
    );
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.path(), format!("/v1/da/manifests/{TICKET}"));
    assert!(requests[0].url.query().is_none());
    drop(requests);

    let response = DaManifestResponse {
        storage_ticket: "cd".repeat(32),
        client_blob_id: "01".repeat(32),
        blob_hash: "02".repeat(32),
        chunk_root: "03".repeat(32),
        manifest_hash: "04".repeat(32),
        lane_id: 0,
        epoch: 1,
        manifest_len: 0,
        manifest_norito: String::new(),
        manifest: Value::Null,
        chunk_plan: Value::Null,
    };
    let (fetcher, requests) = self::fetcher(move |_| {
        Ok(Response::builder()
            .status(200)
            .header("Content-Type", "application/json")
            .body(norito::json::to_vec(&response).unwrap())
            .unwrap())
    });
    let error = fetcher.fetch(TICKET).unwrap_err();
    assert_eq!(
        error.downcast_ref::<Error>(),
        Some(&Error::ResponseBinding {
            operation: OPERATION,
            field: "storage_ticket",
        })
    );
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[test]
fn manifest_arguments_accept_the_canonical_base_url_and_reject_retired_flags() {
    use crate::commands::{
        da::{GetBlobArgs, ProveAvailabilityArgs},
        sorafs::FetchArgs,
    };

    let commands = [
        (
            GetBlobArgs::augment_args(clap::Command::new("get-blob")),
            false,
        ),
        (
            ProveAvailabilityArgs::augment_args(clap::Command::new("prove-availability")),
            true,
        ),
        (FetchArgs::augment_args(clap::Command::new("fetch")), true),
    ];
    for (command, provider_required) in commands {
        let mut arguments = vec!["command", "--storage-ticket", TICKET];
        if provider_required {
            arguments.extend(["--gateway-provider", "name=fixture"]);
        }
        let mut canonical = arguments.clone();
        canonical.extend(["--torii-url", "https://node.example/tenant/"]);
        let matches = command
            .clone()
            .try_get_matches_from(canonical)
            .expect("canonical Torii base URL flag");
        assert_eq!(
            matches.get_one::<String>("torii_url").map(String::as_str),
            Some("https://node.example/tenant/")
        );
        for retired in ["--manifest-endpoint", "--endpoint"] {
            let mut rejected = arguments.clone();
            rejected.extend([retired, "https://node.example/v1/da/manifests/"]);
            let error = command
                .clone()
                .try_get_matches_from(rejected)
                .expect_err("retired endpoint flag must be absent");
            assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
        }
    }
}
