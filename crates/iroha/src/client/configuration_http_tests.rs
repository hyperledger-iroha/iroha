//! Operator configuration authority, asynchronous dispatch and DTO contracts.

use super::{
    Client, HEADER_OPERATOR_NONCE, HEADER_OPERATOR_PUBLIC_KEY, HEADER_OPERATOR_SIGNATURE,
    HEADER_OPERATOR_TIMESTAMP_MS, WireFormatPreference,
    capability_test_support::AsyncOnlyTransport,
    configuration,
    evidence_http_tests::{base_url, client_with_base_url},
};
use crate::{
    Error, TransportErrorKind, blocking,
    http::{Method, Response, TransportRequest},
};
use base64::Engine as _;
use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::NetworkId;
use iroha_torii_shared::{configuration::Configuration as NodeConfiguration, route_catalog, uri};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

const GET: &str = "operator.configuration.read";
const FIXTURE: &[u8] = include_bytes!("../../../../fixtures/torii/configuration.json");

fn response() -> Response<Vec<u8>> {
    Response::builder()
        .status(200)
        .header("Content-Type", "application/json; charset=utf-8")
        .body(FIXTURE.to_vec())
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
        requests: requests.clone(),
        completed: completed.clone(),
        delay,
    });
    let mut builder = client_with_base_url(base_url())
        .to_builder()
        .http_transport(transport);
    builder.torii_request_timeout = timeout;
    // Explicit operator binding must override this unrelated configured identity.
    builder.operator_key_pair = Some(super::checked_random_keypair());
    for name in [
        "authorization",
        "x-api-token",
        "x-iroha-witness",
        "x-iroha-account",
        "x-iroha-signature",
        HEADER_OPERATOR_PUBLIC_KEY,
        HEADER_OPERATOR_SIGNATURE,
        "accept",
        "x-application",
    ] {
        builder
            .headers
            .insert(name.to_owned(), "untrusted-default".to_owned());
    }
    (builder.build().unwrap(), requests, completed)
}

fn assert_fixture(actual: &NodeConfiguration) {
    let expected: NodeConfiguration = norito::json::from_slice(FIXTURE).unwrap();
    assert_eq!(
        norito::json::to_json(actual).unwrap(),
        norito::json::to_json(&expected).unwrap(),
    );
    assert_eq!(actual.confidential_gas.proof_base, 777_777);
    assert_eq!(actual.queue.capacity.get(), 656_565);
    assert_eq!(actual.consensus.protocol_version, 4);
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

fn assert_signature(request: &TransportRequest, network: &NetworkId, key: &KeyPair) {
    let public_key: iroha_crypto::PublicKey =
        header(request, HEADER_OPERATOR_PUBLIC_KEY).parse().unwrap();
    assert_eq!(&public_key, key.public_key());
    let timestamp = header(request, HEADER_OPERATOR_TIMESTAMP_MS)
        .parse()
        .unwrap();
    let nonce = header(request, HEADER_OPERATOR_NONCE);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(header(request, HEADER_OPERATOR_SIGNATURE))
        .unwrap();
    let signature = Signature::from_bytes(&bytes);
    let message = Client::operator_network_request_message(
        network,
        &request.method,
        &request.url,
        &request.body,
        timestamp,
        nonce,
    )
    .unwrap();
    signature.verify(key.public_key(), &message).unwrap();
    let mut other_target = request.url.clone();
    other_target.set_path("/v1/other-operator-operation");
    let altered = Client::operator_network_request_message(
        network,
        &request.method,
        &other_target,
        &request.body,
        timestamp,
        nonce,
    )
    .unwrap();
    assert!(signature.verify(key.public_key(), &altered).is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn configuration_is_async_and_signed_by_the_bound_operator_for_exact_request() {
    let (client, requests, completed) = attach(
        |_| Ok(response()),
        Duration::from_millis(15),
        Duration::from_secs(1),
    );
    let key = super::checked_random_keypair();
    let operator = client.operator_client(key.clone()).unwrap();
    let capability = operator.configuration();
    fn require_send(_: impl Send) {}
    require_send(capability.get());
    let (result, progressed) = tokio::join!(capability.get(), async {
        tokio::task::yield_now().await;
        completed.load(Ordering::SeqCst) == 0
    });
    assert!(
        progressed,
        "executor must progress while dispatch is pending"
    );
    assert_fixture(&result.unwrap());
    let requests = requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        1,
        "one read without compatibility probes or retries"
    );
    let request = &requests[0];
    assert_eq!(request.method, Method::GET);
    assert_eq!(
        request.url.path(),
        route_catalog::core::CONFIGURATION_GET.path()
    );
    assert_eq!(request.url.path(), uri::CONFIGURATION);
    assert!(request.url.query().is_none());
    assert!(request.body.is_empty());
    assert_eq!(
        request.max_response_bytes,
        configuration::MAX_RESPONSE_BYTES
    );
    assert_eq!(request.timeout, Some(Duration::from_secs(1)));
    assert_eq!(header(request, "accept"), "application/json");
    assert_eq!(header(request, "x-application"), "untrusted-default");
    for forbidden in [
        "authorization",
        "x-api-token",
        "x-iroha-witness",
        "x-iroha-account",
        "x-iroha-signature",
        "x-iroha-timestamp-ms",
        "x-iroha-nonce",
    ] {
        assert!(!request.headers.iter().any(|(name, _)| name == forbidden));
    }
    assert_signature(request, operator.network_id(), &key);
}

#[tokio::test]
async fn configuration_contexts_isolate_operator_network_and_endpoint() {
    let (client, requests, _) = attach(|_| Ok(response()), Duration::ZERO, Duration::ZERO);
    let first_key = super::checked_random_keypair();
    let second_key = super::checked_random_keypair();
    let first = client.operator_client(first_key.clone()).unwrap();
    let second = client.operator_client(second_key.clone()).unwrap();
    let mut builder = client.to_builder();
    builder.network_id =
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"configuration-other-network"),
        ));
    builder.torii_url = "https://other.mock/root/".parse().unwrap();
    let third_client = builder.build().unwrap();
    let third = third_client.operator_client(first_key.clone()).unwrap();
    for operator in [&first, &second, &third, &first.clone()] {
        assert_fixture(&operator.configuration().get().await.unwrap());
    }
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert_signature(&requests[0], first.network_id(), &first_key);
    assert_signature(&requests[1], second.network_id(), &second_key);
    assert_signature(&requests[2], third.network_id(), &first_key);
    assert_signature(&requests[3], first.network_id(), &first_key);
    assert_eq!(
        requests[2].url.as_str(),
        "https://other.mock/root/v1/configuration"
    );
    assert_eq!(requests[0].url, requests[3].url);
    assert_ne!(
        header(&requests[0], HEADER_OPERATOR_NONCE),
        header(&requests[3], HEADER_OPERATOR_NONCE)
    );
    let request = &requests[2];
    let wrong_network_message = Client::operator_network_request_message(
        first.network_id(),
        &request.method,
        &request.url,
        &request.body,
        header(request, HEADER_OPERATOR_TIMESTAMP_MS)
            .parse()
            .unwrap(),
        header(request, HEADER_OPERATOR_NONCE),
    )
    .unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(header(request, HEADER_OPERATOR_SIGNATURE))
        .unwrap();
    assert!(
        Signature::from_bytes(&bytes)
            .verify(first_key.public_key(), &wrong_network_message)
            .is_err()
    );
}

#[tokio::test]
async fn configuration_requires_one_json_response_and_exact_dto() {
    let mut responses = Vec::new();
    for types in [
        vec![],
        vec!["text/plain"],
        vec!["application/x-norito"],
        vec!["application/json", "application/json"],
        vec!["application/json, text/plain"],
    ] {
        let mut reply = Response::builder()
            .status(200)
            .body(FIXTURE.to_vec())
            .unwrap();
        for media in types {
            reply
                .headers_mut()
                .append(http::header::CONTENT_TYPE, media.parse().unwrap());
        }
        responses.push(reply);
    }
    for body in [b"{}".to_vec(), vec![0xff], [FIXTURE, b"{}"].concat()] {
        let mut reply = response();
        *reply.body_mut() = body;
        responses.push(reply);
    }
    for reply in responses {
        let (client, requests, _) =
            attach(move |_| Ok(reply.clone()), Duration::ZERO, Duration::ZERO);
        let operator = client
            .operator_client(super::checked_random_keypair())
            .unwrap();
        assert!(matches!(
            operator.configuration().get().await,
            Err(Error::Decode { operation: GET, .. })
        ));
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
    for preference in [
        WireFormatPreference::NoritoOnly,
        WireFormatPreference::NoritoPreferred,
        WireFormatPreference::JsonOnly,
        WireFormatPreference::JsonPreferred,
    ] {
        let (client, requests, _) = attach(|_| Ok(response()), Duration::ZERO, Duration::ZERO);
        let mut builder = client.to_builder();
        builder.wire_format_preference = preference;
        let client = builder.build().unwrap();
        assert_fixture(
            &client
                .operator_client(super::checked_random_keypair())
                .unwrap()
                .configuration()
                .get()
                .await
                .unwrap(),
        );
        assert_eq!(
            header(&requests.lock().unwrap()[0], "accept"),
            "application/json"
        );
    }
}

#[tokio::test]
async fn configuration_bounds_success_and_error_bodies_before_decoding() {
    for status in [200, 503] {
        let (client, requests, _) = attach(
            move |_| {
                Ok(Response::builder()
                    .status(status)
                    .body(vec![b' '; configuration::MAX_RESPONSE_BYTES + 1])
                    .unwrap())
            },
            Duration::ZERO,
            Duration::ZERO,
        );
        let operator = client
            .operator_client(super::checked_random_keypair())
            .unwrap();
        assert_eq!(
            operator.configuration().get().await.unwrap_err(),
            Error::ResponseTooLarge {
                maximum: configuration::MAX_RESPONSE_BYTES,
                actual: Some(configuration::MAX_RESPONSE_BYTES + 1),
            }
        );
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
    let (client, _, _) = attach(
        |_| {
            let mut reply = response();
            reply
                .body_mut()
                .resize(configuration::MAX_RESPONSE_BYTES, b' ');
            Ok(reply)
        },
        Duration::ZERO,
        Duration::ZERO,
    );
    assert_fixture(
        &client
            .operator_client(super::checked_random_keypair())
            .unwrap()
            .configuration()
            .get()
            .await
            .unwrap(),
    );
}

#[tokio::test]
async fn configuration_errors_retain_http_and_transport_identity_without_replay() {
    for status in [401, 403, 503] {
        let (client, requests, _) = attach(
            move |_| {
                Ok(Response::builder()
                    .status(status)
                    .body(b"operator-denied".to_vec())
                    .unwrap())
            },
            Duration::ZERO,
            Duration::ZERO,
        );
        let operator = client
            .operator_client(super::checked_random_keypair())
            .unwrap();
        assert_eq!(
            operator.configuration().get().await.unwrap_err(),
            Error::Http {
                operation: GET,
                status,
                retry_after: None,
                body: b"operator-denied".to_vec(),
            }
        );
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
    let (client, requests, _) = attach(
        |_| Err(std::io::Error::from(std::io::ErrorKind::ConnectionRefused).into()),
        Duration::ZERO,
        Duration::ZERO,
    );
    assert!(matches!(
        client
            .operator_client(super::checked_random_keypair())
            .unwrap()
            .configuration()
            .get()
            .await,
        Err(Error::Transport {
            operation: GET,
            kind: TransportErrorKind::Io(std::io::ErrorKind::ConnectionRefused),
            ..
        })
    ));
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn configuration_deadline_cancels_pending_dispatch() {
    let (client, requests, completed) = attach(
        |_| Ok(response()),
        Duration::from_secs(60),
        Duration::from_millis(10),
    );
    let operator = client
        .operator_client(super::checked_random_keypair())
        .unwrap();
    assert_eq!(
        operator.configuration().get().await.unwrap_err(),
        Error::Timeout { operation: GET }
    );
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert_eq!(completed.load(Ordering::SeqCst), 0);
}

#[test]
fn blocking_configuration_reuses_runtime_and_rejects_async_entry_before_dispatch() {
    let (client, requests, _) = attach(|_| Ok(response()), Duration::ZERO, Duration::ZERO);
    let facade = blocking::Client::from_client(client).unwrap();
    let operator = facade
        .operator_client(super::checked_random_keypair())
        .unwrap();
    assert_fixture(&operator.configuration().get().unwrap());
    assert_fixture(&operator.clone().configuration().get().unwrap());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        assert!(matches!(
            operator.configuration().get(),
            Err(Error::Blocking(_))
        ));
        drop(operator);
        drop(facade);
    });
    assert_eq!(requests.lock().unwrap().len(), 2);
}

#[test]
fn blocking_operator_construction_does_not_bind_an_unrelated_account() {
    let (mut client, requests, _) = attach(|_| Ok(response()), Duration::ZERO, Duration::ZERO);
    // Deliberately invalidate only the account relationship to prove that an
    // operator facade never asks this independent authority to sign or bind.
    client.account = iroha_data_model::account::AccountId::new(
        super::checked_random_keypair().public_key().clone(),
    );
    assert!(client.account_client().is_err());
    let key = super::checked_random_keypair();
    let operator = client.operator_client(key.clone()).unwrap();
    let facade = blocking::OperatorClient::from_client(operator).unwrap();
    assert_fixture(&facade.configuration().get().unwrap());
    assert_fixture(&facade.clone().configuration().get().unwrap());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        assert!(matches!(
            facade.configuration().get(),
            Err(Error::Blocking(_))
        ));
        drop(facade);
    });
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests.iter() {
        assert_signature(request, client.network_id(), &key);
    }
}
