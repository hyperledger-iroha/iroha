//! Exact authenticated HTTP and bounded advisory-response controls; no native serving evidence.
use super::*;
use crate::client::evidence_http_tests::{
    base_url, capture_requests, client_with_base_url, with_mock_http,
};
use crate::http_default::RequestSnapshot;
use iroha_torii_shared::sorafs_gateway_compliance_api::{
    GATEWAY_COMPLIANCE_ACTION_SCHEMA_V1, GATEWAY_COMPLIANCE_STATUS_SCHEMA_V1,
};
use sorafs_manifest::gateway_compliance::{
    GATEWAY_COMPLIANCE_ACK_VERSION_V1, GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1,
    GATEWAY_COMPLIANCE_CATALOG_VERSION_V1, GatewayComplianceAcknowledgementPayloadV1,
    GatewayComplianceCatalogApprovalV1, GatewayComplianceCatalogPayloadV1,
    GatewayComplianceTrustedSignerV1,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
struct Fixture {
    trust: GatewayComplianceTrustPolicyV1,
    catalog: GatewayComplianceCatalogV1,
    ack: GatewayComplianceAcknowledgementV1,
}
fn fixture() -> Fixture {
    fixture_at(now().unwrap())
}

#[test]
fn sdk_preflight_enforces_the_shared_finite_clock_skew_boundary() {
    let observed = now().unwrap();
    for (offset, accepted) in [(clock_skew(), true), (clock_skew() + 1, false)] {
        let selected = fixture_at(observed + offset);
        let digest = selected.catalog.payload.catalog_digest().unwrap();
        assert_eq!(
            selected
                .catalog
                .verify(&selected.trust, observed, clock_skew())
                .is_ok(),
            accepted,
        );
        assert_eq!(
            selected
                .ack
                .verify(&selected.trust, digest, observed, clock_skew())
                .is_ok(),
            accepted,
        );
    }
}

fn fixture_at(observed: u64) -> Fixture {
    let keys = (1..=3)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519).unwrap()
        })
        .collect::<Vec<_>>();
    let signer = |i: usize, id: &str| GatewayComplianceTrustedSignerV1 {
        signer_id: id.into(),
        public_key: keys[i].public_key().to_bytes().1.try_into().unwrap(),
    };
    let trust = GatewayComplianceTrustPolicyV1 {
        policy_id: [7; 32],
        catalog_threshold: 2,
        catalog_signers: vec![signer(0, "catalog-a"), signer(1, "catalog-b")],
        revoked_catalog_signer_ids: vec![],
        gateway_ack_threshold: 1,
        gateway_signers: vec![signer(2, "gateway")],
        revoked_gateway_signer_ids: vec![],
    };
    let payload = GatewayComplianceCatalogPayloadV1 {
        version: GATEWAY_COMPLIANCE_CATALOG_VERSION_V1,
        sequence: 1,
        predecessor_digest: None,
        policy_digest: trust.canonical_digest().unwrap(),
        generated_at_unix: observed,
        valid_until_unix: observed + 600,
        source_anchors: vec![],
        baseline_rules: vec![],
        appeal_overrides: vec![],
        legal_safety_holds: vec![],
        toggles: vec![],
    }
    .normalize()
    .unwrap();
    let digest = payload.signing_digest().unwrap();
    let catalog = GatewayComplianceCatalogV1 {
        payload,
        approvals: (0..2)
            .map(|i| GatewayComplianceCatalogApprovalV1 {
                version: GATEWAY_COMPLIANCE_APPROVAL_VERSION_V1,
                signer_id: trust.catalog_signers[i].signer_id.clone(),
                signature: Signature::try_new(keys[i].private_key(), &digest)
                    .unwrap()
                    .payload()
                    .try_into()
                    .unwrap(),
            })
            .collect(),
    };
    let payload = GatewayComplianceAcknowledgementPayloadV1 {
        version: GATEWAY_COMPLIANCE_ACK_VERSION_V1,
        gateway_id: "gateway".into(),
        catalog_digest: catalog.payload.catalog_digest().unwrap(),
        observed_at_unix: observed,
        accepted: true,
        rejection_code: None,
    };
    let ack = GatewayComplianceAcknowledgementV1 {
        signature: Signature::try_new(keys[2].private_key(), &payload.signing_digest().unwrap())
            .unwrap()
            .payload()
            .try_into()
            .unwrap(),
        payload,
    };
    Fixture {
        trust,
        catalog,
        ack,
    }
}
fn status() -> GatewayComplianceStatusResponseV1 {
    GatewayComplianceStatusResponseV1 {
        schema: GATEWAY_COMPLIANCE_STATUS_SCHEMA_V1.into(),
        checkpoint_version: 1,
        policy_digest_hex: hex::encode([7; 32]),
        observed_at_unix: now().unwrap(),
        serving_ready: false,
        chain_head: None,
        serving: None,
        previous_serving: None,
        candidate: None,
        acknowledgement_count: 0,
        accepted_acknowledgement_count: 0,
        rejected_acknowledgement_count: 0,
        history_count: 0,
        idempotency_record_count: 0,
        latest_action: None,
    }
}
fn json<T: norito::json::JsonSerialize>(status: StatusCode, value: &T) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(http::header::CONTENT_TYPE, APPLICATION_JSON)
        .body(norito::json::to_vec(value).unwrap())
        .unwrap()
}
fn target(request: &RequestSnapshot) -> &str {
    &request.url[url::Position::BeforePath..url::Position::AfterQuery]
}
fn action(request: &RequestSnapshot, digest: [u8; 32]) -> GatewayComplianceActionResponseV1 {
    let action = request.url.path().rsplit('/').next().unwrap();
    GatewayComplianceActionResponseV1 {
        schema: GATEWAY_COMPLIANCE_ACTION_SCHEMA_V1.into(),
        action: action.into(),
        catalog_digest_hex: hex::encode(digest),
        idempotency_key: hex::encode(request_idempotency_binding(
            action,
            target(request),
            &request.body,
        )),
        operation_timestamp_unix: now().unwrap(),
    }
}
fn header<'a>(request: &'a RequestSnapshot, name: &str) -> &'a str {
    let values = request
        .headers
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(name))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 1, "one {name}");
    &values[0].1
}
fn assert_auth(client: &Client, request: &RequestSnapshot) {
    assert_eq!(
        header(request, HEADER_ACCOUNT),
        canonical_request_account_header_value(&client.account).unwrap()
    );
    let timestamp = header(request, HEADER_TIMESTAMP_MS).parse().unwrap();
    let nonce = header(request, HEADER_NONCE);
    let signature = Signature::from_bytes(
        &base64::engine::general_purpose::STANDARD
            .decode(header(request, HEADER_SIGNATURE))
            .unwrap(),
    );
    let message = Client::exact_network_request_message(
        &client.network_id,
        &request.method,
        &request.url,
        &request.body,
        timestamp,
        nonce,
    )
    .unwrap();
    signature
        .verify(client.key_pair.public_key(), &message)
        .unwrap();
    let mut other = request.url.clone();
    other.set_path("/v1/other");
    for message in [
        Client::exact_network_request_message(
            &client.network_id,
            &HttpMethod::DELETE,
            &request.url,
            &request.body,
            timestamp,
            nonce,
        )
        .unwrap(),
        Client::exact_network_request_message(
            &client.network_id,
            &request.method,
            &other,
            &request.body,
            timestamp,
            nonce,
        )
        .unwrap(),
        Client::exact_network_request_message(
            &client.network_id,
            &request.method,
            &request.url,
            b"other",
            timestamp,
            nonce,
        )
        .unwrap(),
    ] {
        assert!(
            signature
                .verify(client.key_pair.public_key(), &message)
                .is_err()
        );
    }
    assert_eq!(
        request.max_response_bytes,
        GATEWAY_COMPLIANCE_RESPONSE_MAX_BYTES_V1
    );
    assert!(
        request
            .timeout
            .is_some_and(|timeout| timeout <= Duration::from_secs(5))
    );
    assert_eq!(header(request, "accept"), APPLICATION_JSON);
}
fn mutate(client: &Client, f: &Fixture, index: usize) -> Result<GatewayComplianceActionResponseV1> {
    match index {
        0 => client.stage_sorafs_gateway_compliance_catalog(&f.catalog, &f.trust),
        1 => client.acknowledge_sorafs_gateway_compliance_catalog(
            &f.ack,
            &f.trust,
            f.ack.payload.catalog_digest,
        ),
        _ => client.promote_sorafs_gateway_compliance_catalog(
            GatewayCompliancePromoteExpectationV1 {
                catalog_digest: f.ack.payload.catalog_digest,
                sequence: 1,
            },
        ),
    }
}
#[test]
fn all_four_controls_use_exact_account_signed_request_and_owned_idempotency_header() {
    let f = fixture();
    let mut builder = client_with_base_url(base_url()).to_builder();
    for name in [
        HEADER_ACCOUNT,
        HEADER_SIGNATURE,
        HEADER_NONCE,
        HEADER_TIMESTAMP_MS,
        "Idempotency-Key",
        "Content-Type",
        "Accept",
    ] {
        builder
            .headers
            .insert(name.into(), "untrusted-default".into());
    }
    let client = builder.build().unwrap();
    let original_deadline = std::time::Instant::now() + Duration::from_secs(5);
    let store = Arc::new(Mutex::new(Vec::new()));
    let observed = store.clone();
    let digest = f.ack.payload.catalog_digest;
    with_mock_http(
        move |request| {
            let result = if request.method == HttpMethod::GET {
                json(StatusCode::OK, &status())
            } else {
                json(
                    if request.url.path().ends_with("promote") {
                        StatusCode::OK
                    } else {
                        StatusCode::ACCEPTED
                    },
                    &action(&request, digest),
                )
            };
            observed.lock().unwrap().push(request);
            Ok(result)
        },
        |transport| {
            let client = client
                .clone()
                .with_test_http_transport(transport)
                .with_request_deadline(original_deadline);
            assert!(
                !client
                    .get_sorafs_gateway_compliance_status()
                    .unwrap()
                    .serving_ready
            );
            for i in 0..3 {
                assert_eq!(
                    mutate(&client, &f, i).unwrap().catalog_digest_hex,
                    hex::encode(digest)
                );
            }
        },
    );
    let requests = store.lock().unwrap();
    assert_eq!(requests.len(), 4);
    for request in requests.iter() {
        assert_auth(&client, request);
    }
    assert_eq!(requests[0].method, HttpMethod::GET);
    assert_eq!(
        requests[0].url.path(),
        routes::SORAFS_GATEWAY_COMPLIANCE_STATUS_GET.path()
    );
    assert!(requests[0].body.is_empty());
    for (index, path) in [
        routes::SORAFS_GATEWAY_COMPLIANCE_STAGE_POST.path(),
        routes::SORAFS_GATEWAY_COMPLIANCE_ACKNOWLEDGE_POST.path(),
        routes::SORAFS_GATEWAY_COMPLIANCE_PROMOTE_POST.path(),
    ]
    .into_iter()
    .enumerate()
    {
        let request = &requests[index + 1];
        assert_eq!(request.method, HttpMethod::POST);
        assert_eq!(request.url.path(), path);
        assert_eq!(header(request, "content-type"), APPLICATION_JSON);
        assert_eq!(
            header(request, GATEWAY_COMPLIANCE_IDEMPOTENCY_KEY_HEADER),
            hex::encode(request_idempotency_binding(
                path.rsplit('/').next().unwrap(),
                target(request),
                &request.body
            ))
        );
    }
    assert_eq!(requests[1].body, canonical_body(&f.catalog).unwrap());
    assert_eq!(requests[2].body, canonical_body(&f.ack).unwrap());
    assert!(requests[1].url.query().is_none() && requests[2].url.query().is_none());
    assert!(requests[3].body.is_empty());
    assert_eq!(
        requests[3].url.query(),
        Some(
            GatewayCompliancePromoteExpectationV1 {
                catalog_digest: digest,
                sequence: 1
            }
            .canonical_query()
            .unwrap()
            .as_str()
        )
    );
    let nonces = requests
        .iter()
        .map(|r| header(r, HEADER_NONCE))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(nonces.len(), 4);
}
#[test]
fn mutation_http_and_transport_failures_are_one_attempt_and_keep_bounded_status() {
    let f = fixture();
    let client = client_with_base_url(base_url());
    for i in 0..3 {
        let response = Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .header("retry-after", "3")
            .body(b"controller unavailable".to_vec())
            .unwrap();
        let (result, requests) = capture_requests(response, |transport| {
            mutate(&client.clone().with_test_http_transport(transport), &f, i)
        });
        assert_eq!(requests.len(), 1);
        match result.unwrap_err().downcast_ref::<crate::Error>().unwrap() {
            crate::Error::Http {
                status,
                retry_after,
                body,
                ..
            } => {
                assert_eq!(*status, 503);
                assert_eq!(*retry_after, Some(Duration::from_secs(3)));
                assert_eq!(body, b"controller unavailable");
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    with_mock_http(
        move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
            Err(eyre!("dispatch transport failure"))
        },
        |transport| {
            assert!(
                client
                    .with_test_http_transport(transport)
                    .stage_sorafs_gateway_compliance_catalog(&f.catalog, &f.trust)
                    .is_err()
            )
        },
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
#[test]
fn invalid_original_material_and_elapsed_deadline_do_no_http() {
    let f = fixture();
    let client = client_with_base_url(base_url());
    with_mock_http(
        |_| panic!("invalid input must be refused before HTTP"),
        |transport| {
            let client = client.with_test_http_transport(transport);
            let expired_material = fixture_at(now().unwrap() - 1_200);
            assert!(
                client
                    .stage_sorafs_gateway_compliance_catalog(
                        &expired_material.catalog,
                        &expired_material.trust
                    )
                    .is_err()
            );
            assert!(
                client
                    .acknowledge_sorafs_gateway_compliance_catalog(
                        &expired_material.ack,
                        &expired_material.trust,
                        expired_material.ack.payload.catalog_digest
                    )
                    .is_err()
            );
            let mut catalog = f.catalog.clone();
            catalog.approvals[0].signature[0] ^= 1;
            assert!(
                client
                    .stage_sorafs_gateway_compliance_catalog(&catalog, &f.trust)
                    .is_err()
            );
            let mut trust = f.trust.clone();
            trust.policy_id[0] ^= 1;
            assert!(
                client
                    .stage_sorafs_gateway_compliance_catalog(&f.catalog, &trust)
                    .is_err()
            );
            let mut catalog = f.catalog.clone();
            catalog.approvals[0].signer_id = "x".repeat(MAX_GATEWAY_COMPLIANCE_CATALOG_BYTES_V1);
            assert!(
                client
                    .stage_sorafs_gateway_compliance_catalog(&catalog, &f.trust)
                    .is_err()
            );
            let mut ack = f.ack.clone();
            ack.signature[0] ^= 1;
            assert!(
                client
                    .acknowledge_sorafs_gateway_compliance_catalog(
                        &ack,
                        &f.trust,
                        f.ack.payload.catalog_digest
                    )
                    .is_err()
            );
            assert!(
                client
                    .acknowledge_sorafs_gateway_compliance_catalog(&f.ack, &f.trust, [0xff; 32])
                    .is_err()
            );
            let mut ack = f.ack.clone();
            ack.payload.observed_at_unix = 1;
            assert!(
                client
                    .acknowledge_sorafs_gateway_compliance_catalog(
                        &ack,
                        &f.trust,
                        f.ack.payload.catalog_digest
                    )
                    .is_err()
            );
            assert!(
                client
                    .promote_sorafs_gateway_compliance_catalog(
                        GatewayCompliancePromoteExpectationV1 {
                            catalog_digest: [1; 32],
                            sequence: 0
                        }
                    )
                    .is_err()
            );
            let expired = client.with_request_deadline(std::time::Instant::now());
            assert!(expired.get_sorafs_gateway_compliance_status().is_err());
            for i in 0..3 {
                assert!(mutate(&expired, &f, i).is_err());
            }
        },
    );
}
#[test]
fn successful_action_must_match_exact_schema_action_catalog_and_request_binding() {
    let f = fixture();
    let client = client_with_base_url(base_url());
    let digest = f.ack.payload.catalog_digest;
    for which in 0..6 {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        with_mock_http(
            move |request| {
                observed.fetch_add(1, Ordering::SeqCst);
                let mut report = action(&request, digest);
                match which {
                    0 => report.schema = "other".into(),
                    1 => report.action = "promote".into(),
                    2 => report.catalog_digest_hex = hex::encode([0x99; 32]),
                    3 => report.idempotency_key = hex::encode([0x99; 32]),
                    4 => report.operation_timestamp_unix = 0,
                    _ => return Ok(json(StatusCode::OK, &report)),
                }
                Ok(json(StatusCode::ACCEPTED, &report))
            },
            |transport| {
                assert!(
                    client
                        .clone()
                        .with_test_http_transport(transport)
                        .stage_sorafs_gateway_compliance_catalog(&f.catalog, &f.trust)
                        .is_err()
                )
            },
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn status_requires_one_bounded_json_response_and_valid_native_reason_spelling() {
    let client = client_with_base_url(base_url());
    let original = status();
    let mut bad_schema = original.clone();
    bad_schema.schema = "other".into();
    let mut bad_digest = original.clone();
    bad_digest.policy_digest_hex = "AF".repeat(32);
    let mut bad_reason = original.clone();
    bad_reason.latest_action = Some(
        iroha_torii_shared::sorafs_gateway_compliance_api::GatewayComplianceLatestActionStatusV1 {
            operation_id_hex: hex::encode([1; 32]),
            action: "promotion".into(),
            previous_serving_digest_hex: None,
            serving_digest_hex: hex::encode([2; 32]),
            recorded_at_unix: 1,
            reason_code: "Not Canonical".into(),
        },
    );
    let mut duplicate_type = json(StatusCode::OK, &original);
    duplicate_type.headers_mut().append(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static(APPLICATION_JSON),
    );
    for response in [
        json(StatusCode::OK, &bad_schema),
        json(StatusCode::OK, &bad_digest),
        json(StatusCode::OK, &bad_reason),
        duplicate_type,
        Response::builder()
            .status(StatusCode::OK)
            .header(http::header::CONTENT_TYPE, "text/plain")
            .body(norito::json::to_vec(&original).unwrap())
            .unwrap(),
        Response::builder()
            .status(StatusCode::OK)
            .header(http::header::CONTENT_TYPE, APPLICATION_JSON)
            .body(vec![0; GATEWAY_COMPLIANCE_RESPONSE_MAX_BYTES_V1 + 1])
            .unwrap(),
        Response::builder()
            .status(StatusCode::OK)
            .header(http::header::CONTENT_TYPE, APPLICATION_JSON)
            .body(b"{} trailing".to_vec())
            .unwrap(),
    ] {
        let (result, requests) = capture_requests(response, |transport| {
            client
                .clone()
                .with_test_http_transport(transport)
                .get_sorafs_gateway_compliance_status()
        });
        assert!(result.is_err());
        assert_eq!(requests.len(), 1);
    }
}
