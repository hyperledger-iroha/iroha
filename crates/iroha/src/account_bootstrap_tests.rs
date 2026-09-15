//! Bootstrap discovery rejects ambiguous trust inputs and never requires account credentials.

use super::*;
use crate::http::{TransportFuture, TransportRequest};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use std::sync::Mutex;

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"wallet bootstrap tests",
    )))
}

fn capabilities() -> AccountCapabilitiesV1 {
    AccountCapabilitiesV1::from_admission(network(), 369, &[Algorithm::Ed25519]).unwrap()
}

#[derive(Debug)]
struct Transport {
    requests: Mutex<Vec<TransportRequest>>,
    body: Vec<u8>,
}

impl HttpTransport for Transport {
    fn send_blocking(&self, _: TransportRequest) -> Result<Response<Vec<u8>>> {
        panic!("bootstrap uses the async SDK transport")
    }

    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        self.requests.lock().unwrap().push(request);
        Box::pin(async {
            Ok(Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(self.body.clone())
                .unwrap())
        })
    }
}

#[test]
fn validates_trusted_origin_and_bounded_deadline() {
    for bad in [
        "http://example.org",
        "https://user:secret@example.org",
        "https://example.org?token=secret",
        "https://example.org#fragment",
        "file:///tmp/node",
    ] {
        assert!(validate_endpoint(&bad.parse().unwrap()).is_err());
        assert!(
            Client::new(bad.parse().unwrap(), Duration::from_secs(5)).is_err(),
            "{bad}"
        );
    }
    for allowed in [
        "https://taira.sora.org",
        "http://localhost:8080",
        "http://127.0.0.1:8080",
        "http://[::1]:8080",
    ] {
        assert!(validate_endpoint(&allowed.parse().unwrap()).is_ok());
        assert!(
            Client::new(allowed.parse().unwrap(), Duration::from_secs(5)).is_ok(),
            "{allowed}"
        );
    }
    assert!(
        validate_endpoint(
            &format!("https://taira.sora.org/{}", "a".repeat(2048))
                .parse()
                .unwrap()
        )
        .is_err()
    );
    assert!(Client::new("https://taira.sora.org".parse().unwrap(), Duration::ZERO).is_err());
    assert!(
        crate::blocking::account_bootstrap::Client::new(
            "https://taira.sora.org".parse().unwrap(),
            Duration::from_secs(5)
        )
        .is_ok()
    );
}

#[test]
fn rejects_ambiguous_or_oversized_discovery_responses() {
    let missing = validate_response(Response::builder().status(404).body(Vec::new()).unwrap(), 4)
        .wrap_err("GET /v1/accounts/faucet/policy")
        .unwrap_err();
    assert_eq!(
        missing.downcast_ref::<DiscoveryHttpError>().unwrap().status,
        404
    );
    assert!(
        validate_response(Response::builder().status(502).body(Vec::new()).unwrap(), 4).is_err()
    );
    assert!(
        validate_response(
            Response::builder()
                .header("content-type", "text/html")
                .body(Vec::new())
                .unwrap(),
            4
        )
        .is_err()
    );
    assert!(
        validate_response(
            Response::builder()
                .header("content-type", "application/json")
                .header("content-type", "application/json")
                .body(Vec::new())
                .unwrap(),
            4
        )
        .is_err()
    );
    assert!(
        validate_response(
            Response::builder()
                .header("content-type", "application/json")
                .body(vec![0; 5])
                .unwrap(),
            4
        )
        .is_err()
    );
    assert_eq!(
        validate_response(
            Response::builder()
                .header("content-type", "application/json; charset=utf-8")
                .body(b"{}".to_vec())
                .unwrap(),
            4
        )
        .unwrap(),
        b"{}"
    );
}

#[test]
fn rejects_unusable_or_noncanonical_signing_policy() {
    let valid = capabilities();
    validate_capabilities(&valid).unwrap();
    let mut bad = valid.clone();
    bad.schema_version = 2;
    assert!(validate_capabilities(&bad).is_err());
    let mut bad = valid.clone();
    bad.network_prefix = 0;
    assert!(validate_capabilities(&bad).is_err());
    let mut bad = valid.clone();
    bad.default_signing = "secp256k1".into();
    assert!(validate_capabilities(&bad).is_err());
    let mut bad = valid.clone();
    bad.allowed_signing.clear();
    assert!(validate_capabilities(&bad).is_err());
    let mut bad = valid.clone();
    bad.allowed_signing.push("ed25519".into());
    assert!(validate_capabilities(&bad).is_err());
    let mut bad = valid;
    bad.allowed_signing.push("unsupported".into());
    assert!(validate_capabilities(&bad).is_err());
}

#[tokio::test]
async fn public_capabilities_use_one_bounded_unsigned_request() {
    let transport = Arc::new(Transport {
        requests: Mutex::new(Vec::new()),
        body: norito::json::to_vec(&capabilities()).unwrap(),
    });
    let client = Client::with_transport(
        "https://taira.sora.org/".parse().unwrap(),
        Duration::from_secs(5),
        transport.clone(),
    )
    .unwrap();
    assert_eq!(client.capabilities().await.unwrap(), capabilities());
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(
        request.url.as_str(),
        "https://taira.sora.org/v1/accounts/capabilities"
    );
    assert_eq!(request.method, Method::GET);
    assert!(request.body.is_empty());
    assert_eq!(
        request.max_response_bytes,
        ACCOUNT_CAPABILITIES_MAX_BYTES_V1
    );
    assert_eq!(request.timeout, Some(Duration::from_secs(5)));
    assert_eq!(
        request.headers.len(),
        1,
        "only content negotiation, no account/signature/credentials"
    );
    assert_eq!(request.headers[0].0, "accept");
    assert!(
        crate::blocking::account_bootstrap::Client::new(
            "https://taira.sora.org".parse().unwrap(),
            Duration::from_secs(5)
        )
        .is_err(),
        "blocking facade rejects nested runtime"
    );
}

#[tokio::test]
async fn faucet_discovery_requires_pinned_network_and_valid_issuance() {
    let _profile = ChainDiscriminantGuard::enter(369);
    let key = KeyPair::random();
    let policy = AccountFaucetAdvertisement {
        schema_version: 1,
        network_id: network(),
        network_prefix: 369,
        authority: iroha_data_model::account::AccountId::new(key.public_key().clone()),
        asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".parse().unwrap(),
        amount: "10".parse().unwrap(),
    };
    let transport = Arc::new(Transport {
        requests: Mutex::new(Vec::new()),
        body: norito::json::to_vec(&policy).unwrap(),
    });
    let client = Client::with_transport(
        "http://127.0.0.1:8080".parse().unwrap(),
        Duration::from_secs(5),
        transport.clone(),
    )
    .unwrap();
    assert_eq!(client.faucet_policy(network(), 369).await.unwrap(), policy);
    assert!(transport.requests.lock().unwrap()[0].direct_loopback);
    assert_eq!(
        transport.requests.lock().unwrap()[0].url.path(),
        uri::ACCOUNTS_FAUCET_POLICY
    );
    assert!(client.faucet_policy(network(), 753).await.is_err());
    let other = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"different genesis",
    )));
    assert!(client.faucet_policy(other, 369).await.is_err());
    let mut bad = policy;
    bad.amount = "0".parse().unwrap();
    let invalid = Client::with_transport(
        "https://taira.sora.org".parse().unwrap(),
        Duration::from_secs(5),
        Arc::new(Transport {
            requests: Mutex::new(Vec::new()),
            body: norito::json::to_vec(&bad).unwrap(),
        }),
    )
    .unwrap();
    assert!(invalid.faucet_policy(network(), 369).await.is_err());
}
