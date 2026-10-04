//! Canonical SDK transport controls; signed adverts are protocol messages, not admission proofs.

use super::*;
use crate::client::evidence_http_tests::{
    base_url, capture_requests, client_with_base_url, empty_response, with_mock_http,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn signed_advert(client: &Client) -> ProviderAdvertV1 {
    let bytes =
        include_bytes!("../../../../fixtures/sorafs_manifest/provider_admission/advert_v1.to");
    let mut advert = sorafs_manifest::provider_advert::decode_provider_advert_v1(bytes).unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    advert.network_id = *client.network_id.as_bytes();
    advert.issued_at = now.saturating_sub(1);
    advert.expires_at = now + 600;
    advert.signature_strict = true;
    advert.allow_unknown_capabilities = false;
    advert.signature.public_key = client.key_pair.public_key().to_bytes().1.to_vec();
    advert.signature.signature = iroha_crypto::Signature::try_new(
        client.key_pair.private_key(),
        &advert.signature_payload_bytes().unwrap(),
    )
    .unwrap()
    .payload()
    .to_vec();
    advert.validate_with_body(now).unwrap();
    advert.verify_signature().unwrap();
    advert
}

#[test]
fn provider_advert_sends_exact_bounded_single_post_without_account_or_transaction_signing() {
    let client = client_with_base_url(base_url());
    let advert = signed_advert(&client);
    let original = norito::encode_canonical(&advert).unwrap();
    for status in [http::StatusCode::OK, http::StatusCode::SERVICE_UNAVAILABLE] {
        let (result, requests) = capture_requests(empty_response(status), |transport| {
            client
                .clone()
                .with_test_http_transport(transport)
                .with_request_deadline(std::time::Instant::now() + Duration::from_secs(5))
                .post_sorafs_provider_advert(&advert)
        });
        assert_eq!(result.unwrap().status(), status);
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.method, HttpMethod::POST);
        assert_eq!(
            request.url.path(),
            iroha_torii_shared::route_catalog::sorafs::PROVIDER_ADVERT.path()
        );
        assert_eq!(request.body, original);
        assert_eq!(request.max_response_bytes, RESPONSE_MAX);
        assert!(
            request
                .timeout
                .is_some_and(|timeout| timeout <= Duration::from_secs(5))
        );
        for name in [
            "x-iroha-account",
            "x-iroha-signature",
            "x-iroha-nonce",
            "x-iroha-timestamp-ms",
            "x-iroha-witness",
        ] {
            assert!(
                !request
                    .headers
                    .iter()
                    .any(|(header, _)| header.eq_ignore_ascii_case(name))
            );
        }
        assert!(
            request
                .headers
                .iter()
                .any(|(name, value)| name.eq_ignore_ascii_case("content-type")
                    && value == APPLICATION_NORITO)
        );
    }
}

#[test]
fn provider_advert_transport_failure_is_one_attempt_and_invalid_inputs_are_no_http() {
    let client = client_with_base_url(base_url());
    let advert = signed_advert(&client);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    with_mock_http(
        move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
            Err(eyre!("transport failure"))
        },
        |transport| {
            assert!(
                client
                    .clone()
                    .with_test_http_transport(transport)
                    .post_sorafs_provider_advert(&advert)
                    .is_err()
            );
        },
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let oversized = Response::builder()
        .status(http::StatusCode::OK)
        .body(vec![0; RESPONSE_MAX + 1])
        .unwrap();
    let (result, requests) = capture_requests(oversized, |transport| {
        client
            .clone()
            .with_test_http_transport(transport)
            .post_sorafs_provider_advert(&advert)
    });
    assert!(result.is_err());
    assert_eq!(requests.len(), 1);
    with_mock_http(
        |_| panic!("invalid advert must not perform HTTP"),
        |transport| {
            let client = client.with_test_http_transport(transport);
            for mutate in [
                |a: &mut ProviderAdvertV1| a.network_id[0] ^= 1,
                |a: &mut ProviderAdvertV1| a.signature_strict = false,
                |a: &mut ProviderAdvertV1| a.signature.signature[0] ^= 1,
                |a: &mut ProviderAdvertV1| a.expires_at = a.issued_at.saturating_sub(1),
                |a: &mut ProviderAdvertV1| {
                    a.issued_at = 1;
                    a.expires_at = 2;
                },
                |a: &mut ProviderAdvertV1| {
                    a.body.notes = Some("x".repeat(PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1))
                },
            ] {
                let mut changed = advert.clone();
                mutate(&mut changed);
                assert!(client.post_sorafs_provider_advert(&changed).is_err());
            }
            assert!(
                client
                    .with_request_deadline(std::time::Instant::now())
                    .post_sorafs_provider_advert(&advert)
                    .is_err()
            );
        },
    );
}
