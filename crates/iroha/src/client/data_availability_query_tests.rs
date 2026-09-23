//! DA query signing, immutable authority, bounded transport and response validation.

mod binding;
mod fixtures;

use super::{Client, capability_test_support::AsyncOnlyTransport, evidence_http_tests::*};
use crate::{
    Error, blocking,
    http::{Method, Response, TransportRequest},
};
use base64::Engine as _;
use fixtures::*;
use iroha_crypto::{Hash, HashOf, Signature};
use iroha_data_model::{
    NetworkId,
    account::address::ChainDiscriminantGuard,
    da::{commitment::DaCommitmentProof, pin_intent::DaPinIntentProof},
};
use iroha_torii_shared::da::*;
use norito::json::{self, JsonSerialize};
use std::{
    num::NonZeroU64,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn json_response<T: JsonSerialize>(value: &T) -> Response<Vec<u8>> {
    Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(json::to_vec(value).unwrap())
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
    (builder.build().unwrap(), requests, completed)
}

fn header<'a>(request: &'a TransportRequest, name: &str) -> &'a str {
    let values: Vec<_> = request
        .headers
        .iter()
        .filter(|(key, _)| key.as_str() == name)
        .collect();
    assert_eq!(values.len(), 1, "one {name}");
    values[0].1.to_str().unwrap()
}

fn assert_signed(client: &Client, request: &TransportRequest) {
    let signature = Signature::try_from_bytes(
        &base64::engine::general_purpose::STANDARD
            .decode(header(request, "x-iroha-signature"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        header(request, "x-iroha-account"),
        client.account.to_canonical_hex().unwrap()
    );
    let timestamp = header(request, "x-iroha-timestamp-ms").parse().unwrap();
    let nonce = header(request, "x-iroha-nonce");
    assert!(!nonce.is_empty());
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
    let wrong_network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"wrong-da-network",
    )));
    for (network, body) in [
        (&wrong_network, request.body.as_slice()),
        (&client.network_id, b"altered".as_slice()),
    ] {
        let message = Client::exact_network_request_message(
            network,
            &request.method,
            &request.url,
            body,
            timestamp,
            nonce,
        )
        .unwrap();
        assert!(
            signature
                .verify(client.key_pair.public_key(), &message)
                .is_err()
        );
    }
    assert_eq!(request.method, Method::POST);
    assert_eq!(header(request, "content-type"), "application/json");
    assert_eq!(header(request, "accept"), "application/json");
    assert!(request.body.len() <= DA_QUERY_REQUEST_MAX_BYTES);
}

fn commitment_query(proof: &DaCommitmentProof) -> DaCommitmentProofRequest {
    DaCommitmentProofRequest {
        manifest_hash: Some(proof.commitment.manifest_hash),
        lane_id: Some(proof.commitment.lane_id.as_u32()),
        epoch: Some(proof.commitment.epoch),
        sequence: Some(proof.commitment.sequence),
    }
}

fn pin_query(proof: &DaPinIntentProof) -> DaPinIntentQueryRequest {
    DaPinIntentQueryRequest {
        manifest_hash: Some(proof.intent.manifest_hash),
        storage_ticket: Some(proof.intent.storage_ticket),
        alias: proof.intent.alias.clone(),
        lane_id: Some(proof.intent.lane_id.as_u32()),
        epoch: Some(proof.intent.epoch),
        sequence: Some(proof.intent.sequence),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn public_queries_are_async_and_strip_stale_account_authority() {
    let (client, requests, completed) = attach(
        |request| {
            Ok(match request.url.path() {
                "/v1/da/proof-policies" => json_response(&sample_da_proof_policy_bundle()),
                "/v1/da/commitments" => json_response(&DaCommitmentListResponse {
                    policies: sample_da_proof_policy_bundle(),
                    commitments: vec![sample_da_commitment_with_location()],
                    next_cursor: None,
                }),
                "/v1/da/pin-intents" => json_response(&DaPinIntentListResponse {
                    intents: vec![],
                    next_cursor: None,
                }),
                path => panic!("unexpected route {path}"),
            })
        },
        Duration::from_millis(10),
        Duration::from_secs(1),
    );
    let mut builder = client.to_builder();
    for name in [
        "X-Iroha-Account",
        "x-Iroha-Signature",
        "X-Iroha-Timestamp-Ms",
        "X-Iroha-Nonce",
        "X-Iroha-Witness",
    ] {
        builder.headers.insert(name.into(), "stale".into());
    }
    builder
        .headers
        .insert("Authorization".into(), "Bearer public-token".into());
    let client = builder.build().unwrap();
    let da = client.da();
    let (policies, responsive) = tokio::join!(da.proof_policies(), async {
        tokio::task::yield_now().await;
        completed.load(Ordering::SeqCst) == 0
    });
    assert!(responsive);
    assert_eq!(policies.unwrap(), sample_da_proof_policy_bundle());
    let commitments = DaCommitmentListRequest {
        limit: NonZeroU64::new(3),
        cursor: None,
    };
    assert_eq!(
        da.commitments(&commitments)
            .await
            .unwrap()
            .commitments
            .len(),
        1
    );
    let pins = DaPinIntentListRequest::default();
    assert!(da.pin_intents(&pins).await.unwrap().intents.is_empty());
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].method, Method::GET);
    assert!(requests[0].body.is_empty());
    assert_eq!(requests[1].body, json::to_vec(&commitments).unwrap());
    assert_eq!(requests[2].body, json::to_vec(&pins).unwrap());
    for request in requests.iter() {
        assert!(
            !request
                .headers
                .iter()
                .any(|(name, _)| name.as_str().starts_with("x-iroha-"))
        );
        assert_eq!(header(request, "authorization"), "Bearer public-token");
        assert_eq!(header(request, "accept"), "application/json");
        assert_eq!(request.timeout, Some(Duration::from_secs(1)));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn all_four_proof_operations_sign_the_exact_bounded_body() {
    fn require_send(_: impl Send) {}
    let network = client_with_base_url(base_url()).network_id;
    let commitment = sample_da_commitment_proof();
    let pin = sample_da_pin_intent_proof(network);
    let c = commitment.clone();
    let p = pin.clone();
    let (client, requests, _) = attach(
        move |request| {
            Ok(match request.url.path() {
                "/v1/da/commitments/prove" => json_response(&Some(DaCommitmentProofResponse {
                    policies: sample_da_proof_policy_bundle(),
                    proof: c.clone(),
                })),
                "/v1/da/commitments/verify" => json_response(&DaCommitmentVerifyResponse {
                    valid: false,
                    error: Some("untrusted block".into()),
                }),
                "/v1/da/pin-intents/prove" => json_response(&Some(p.clone())),
                "/v1/da/pin-intents/verify" => json_response(&DaPinIntentVerifyResponse {
                    valid: true,
                    error: None,
                }),
                path => panic!("unexpected route {path}"),
            })
        },
        Duration::ZERO,
        Duration::from_secs(1),
    );
    let account = client.account_client().unwrap();
    let da = account.da();
    require_send(da.verify_commitment(&commitment));
    assert_eq!(
        da.prove_commitment(&commitment_query(&commitment))
            .await
            .unwrap()
            .unwrap()
            .proof,
        commitment
    );
    let verification = da.verify_commitment(&commitment).await.unwrap();
    assert!(!verification.valid);
    assert_eq!(verification.error.as_deref(), Some("untrusted block"));
    assert_eq!(
        da.prove_pin_intent(&pin_query(&pin))
            .await
            .unwrap()
            .unwrap(),
        pin
    );
    assert!(da.verify_pin_intent(&pin).await.unwrap().valid);
    let expected = [
        json::to_vec(&commitment_query(&commitment)).unwrap(),
        json::to_vec(&commitment).unwrap(),
        json::to_vec(&pin_query(&pin)).unwrap(),
        json::to_vec(&pin).unwrap(),
    ];
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    for (request, expected) in requests.iter().zip(expected) {
        assert_eq!(request.body, expected);
        assert_signed(&client, request);
    }
}

#[tokio::test]
async fn invalid_queries_witnesses_and_oversized_proofs_fail_before_dispatch() {
    let (client, requests, _) = attach(
        |_| panic!("invalid request dispatched"),
        Duration::ZERO,
        Duration::ZERO,
    );
    let account = client.account_client().unwrap();
    assert!(matches!(
        account
            .da()
            .prove_commitment(&DaCommitmentProofRequest::default())
            .await,
        Err(Error::InvalidRequest { .. })
    ));
    assert!(matches!(
        account
            .da()
            .prove_pin_intent(&DaPinIntentQueryRequest::default())
            .await,
        Err(Error::InvalidRequest { .. })
    ));
    assert!(matches!(
        client
            .da()
            .commitments(&DaCommitmentListRequest {
                limit: NonZeroU64::new(1001),
                cursor: None
            })
            .await,
        Err(Error::InvalidRequest { .. })
    ));
    assert!(matches!(
        client
            .da()
            .pin_intents(&DaPinIntentListRequest {
                limit: NonZeroU64::new(1001),
                cursor: None
            })
            .await,
        Err(Error::InvalidRequest { .. })
    ));
    let mut oversized = sample_da_commitment_proof();
    let item = iroha_data_model::da::commitment::MerklePathItem {
        direction: iroha_data_model::da::commitment::MerkleDirection::Left,
        sibling: Hash::new(b"sibling"),
    };
    oversized.path = vec![item; 2048];
    assert!(matches!(
        account.da().verify_commitment(&oversized).await,
        Err(Error::InvalidRequest { .. })
    ));
    let mut builder = client.to_builder();
    builder
        .headers
        .insert("x-IrOhA-WiTnEsS".into(), "injected".into());
    let account = builder.build().unwrap().account_client().unwrap();
    assert!(matches!(
        account
            .da()
            .verify_commitment(&sample_da_commitment_proof())
            .await,
        Err(Error::InvalidRequest { .. })
    ));
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn proof_absence_is_json_null_and_contradictory_verification_is_rejected() {
    let (client, requests, _) = attach(
        |request| {
            Ok(if request.url.path().ends_with("/prove") {
                json_response(&Option::<DaCommitmentProofResponse>::None)
            } else {
                json_response(&DaCommitmentVerifyResponse {
                    valid: true,
                    error: Some("failure".into()),
                })
            })
        },
        Duration::ZERO,
        Duration::ZERO,
    );
    let account = client.account_client().unwrap();
    let proof = sample_da_commitment_proof();
    assert!(
        account
            .da()
            .prove_commitment(&commitment_query(&proof))
            .await
            .unwrap()
            .is_none()
    );
    let pin = sample_da_pin_intent_proof(client.network_id);
    assert!(
        account
            .da()
            .prove_pin_intent(&pin_query(&pin))
            .await
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        account.da().verify_commitment(&proof).await,
        Err(Error::ResponseBinding {
            field: "valid/error",
            ..
        })
    ));
    assert!(matches!(
        account.da().verify_pin_intent(&pin).await,
        Err(Error::ResponseBinding {
            field: "valid/error",
            ..
        })
    ));
    assert_eq!(requests.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn signed_query_cancellation_never_replays() {
    let (client, requests, completed) = attach(
        |_| {
            Ok(json_response(&DaCommitmentVerifyResponse {
                valid: true,
                error: None,
            }))
        },
        Duration::from_secs(60),
        Duration::from_millis(10),
    );
    let account = client.account_client().unwrap();
    assert_eq!(
        account
            .da()
            .verify_commitment(&sample_da_commitment_proof())
            .await
            .unwrap_err(),
        Error::Timeout {
            operation: "data_availability.commitment.verify"
        }
    );
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert_eq!(completed.load(Ordering::SeqCst), 0);
}

#[test]
fn blocking_da_reuses_owned_runtime_for_public_and_account_calls() {
    let (client, requests, _) = attach(
        |request| {
            Ok(if request.url.path().ends_with("proof-policies") {
                json_response(&sample_da_proof_policy_bundle())
            } else {
                json_response(&DaCommitmentVerifyResponse {
                    valid: true,
                    error: None,
                })
            })
        },
        Duration::ZERO,
        Duration::ZERO,
    );
    let public = blocking::Client::from_client(client.clone()).unwrap();
    let account = blocking::AccountClient::from_client(client.account_client().unwrap()).unwrap();
    for _ in 0..2 {
        public.da().proof_policies().unwrap();
        account
            .da()
            .verify_commitment(&sample_da_commitment_proof())
            .unwrap();
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            assert!(matches!(
                public.da().proof_policies(),
                Err(Error::Blocking(_))
            ));
            assert!(matches!(
                account
                    .da()
                    .verify_commitment(&sample_da_commitment_proof()),
                Err(Error::Blocking(_))
            ));
            drop(account);
            drop(public);
        });
    assert_eq!(requests.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn proof_contexts_isolate_endpoint_network_authority_and_address_formatting() {
    fn attach_pin(client: &Client) -> (Client, Arc<Mutex<Vec<TransportRequest>>>) {
        let pin = sample_da_pin_intent_proof(client.network_id);
        let discriminant = client.account_chain_discriminant;
        let requests = Arc::new(Mutex::new(Vec::new()));
        let transport = Arc::new(AsyncOnlyTransport {
            responder: Box::new(move |_| {
                let _format = ChainDiscriminantGuard::enter(discriminant);
                Ok(json_response(&Some(pin.clone())))
            }),
            requests: requests.clone(),
            completed: Arc::new(AtomicUsize::new(0)),
            delay: Duration::from_millis(2),
        });
        (
            client
                .to_builder()
                .http_transport(transport)
                .build()
                .unwrap(),
            requests,
        )
    }
    let first = client_with_base_url(base_url());
    let key = iroha_crypto::KeyPair::try_from_seed(vec![42; 32], iroha_crypto::Algorithm::Ed25519)
        .unwrap();
    let mut builder = first.to_builder();
    builder.torii_url = "http://different.example:8081/prefix/".parse().unwrap();
    builder.network_id =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"second network")));
    builder.key_pair = key.clone();
    builder.account = iroha_data_model::account::AccountId::new(key.public_key().clone());
    builder.account_chain_discriminant = 753;
    let (first, first_requests) = attach_pin(&first);
    let (second, second_requests) = attach_pin(&builder.build().unwrap());
    let a = first.account_client().unwrap();
    let b = second.account_client().unwrap();
    let a_da = a.da();
    let b_da = b.da();
    let query = pin_query(&sample_da_pin_intent_proof(first.network_id));
    let original = iroha_data_model::account::address::chain_discriminant();
    let (first_result, second_result) =
        tokio::join!(a_da.prove_pin_intent(&query), b_da.prove_pin_intent(&query));
    assert_eq!(
        first_result
            .unwrap()
            .unwrap()
            .intent
            .authorization
            .network_id,
        first.network_id
    );
    assert_eq!(
        second_result
            .unwrap()
            .unwrap()
            .intent
            .authorization
            .network_id,
        second.network_id
    );
    assert_eq!(
        iroha_data_model::account::address::chain_discriminant(),
        original
    );
    let first_requests = first_requests.lock().unwrap();
    let second_requests = second_requests.lock().unwrap();
    let a = &first_requests[0];
    let b = &second_requests[0];
    assert_ne!(a.url.origin(), b.url.origin());
    assert_eq!(b.url.path(), "/prefix/v1/da/pin-intents/prove");
    assert_ne!(header(a, "x-iroha-account"), header(b, "x-iroha-account"));
    assert_signed(&first, a);
    assert_signed(&second, b);
}

#[tokio::test]
async fn proof_http_errors_and_decode_failures_remain_structured() {
    for (status, media, body) in [
        (404, "application/json", "null"),
        (429, "application/json", "{\"error\":\"busy\"}"),
        (200, "text/plain", "null"),
        (200, "application/json", "{}"),
    ] {
        let (client, requests, _) = attach(
            move |_| {
                Ok(Response::builder()
                    .status(status)
                    .header("Content-Type", media)
                    .header("Retry-After", "12")
                    .body(body.as_bytes().to_vec())
                    .unwrap())
            },
            Duration::ZERO,
            Duration::ZERO,
        );
        let account = client.account_client().unwrap();
        let result = account
            .da()
            .prove_commitment(&commitment_query(&sample_da_commitment_proof()))
            .await;
        if status == 200 {
            assert!(matches!(result, Err(Error::Decode { .. })));
        } else {
            assert!(
                matches!(result, Err(Error::Http { status: actual, body: ref received, retry_after: Some(_), .. }) if actual == status && received == body.as_bytes())
            );
        }
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}
