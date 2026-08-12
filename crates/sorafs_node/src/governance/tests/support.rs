// Shared governance publication fixtures and request-ingress regressions.

use std::{
    collections::BTreeMap,
    fs, io,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{self, StatusCode},
    response::Response,
    routing::get,
};
use iroha_crypto::{Algorithm, KeyPair, Signature as IrohaSignature};
use iroha_data_model::sorafs::transparency::{
    MODERATION_PRIVACY_AGGREGATE_VERSION_V1, MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
    ModerationLedgerMetadataV1, ModerationPrivacyAggregateMetricV1, ModerationPrivacyAggregateV1,
    ModerationPrivacyModeV1, ModerationPrivacyNoiseSourceV1, ModerationPrivacyParametersV1,
    ModerationPrivacyThresholdPrfCommitmentV1,
};
use norito::codec::Encode;
use sorafs_manifest::PorReportIsoWeek;
use sorafs_manifest::deal::{
    DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1, XorQuantity,
};
use sorafs_manifest::por::{
    POR_CHALLENGE_VERSION_V1, POR_WEEKLY_REPORT_VERSION_V1, PorChallengeV1, derive_challenge_id,
    derive_challenge_seed,
};
use sorafs_manifest::repair::{
    GC_AUDIT_EVENT_VERSION_V1, GC_AUDIT_PAYLOAD_VERSION_V1, GC_AUDIT_SIGNER_V1, GcAuditEventV1,
    GcAuditPayloadV1, SorafsAuditHeaderV1, gc_audit_payload_digest_v1,
};
use sorafs_manifest::{
    GovernanceDagBlockV1, GovernanceDagHeadV1, GovernanceLogPayloadV1,
    MODERATION_LEDGER_PUBLICATION_VERSION_V1, REPUTATION_PROVIDER_INPUT_VERSION_V1,
    REPUTATION_PROVIDER_METRICS_VERSION_V1, REPUTATION_SCORING_EVIDENCE_VERSION_V1,
    ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
    ReputationScoringEvidenceV1, ReputationSnapshotSignatureV1, ReputationWeightsV1,
    SIGNED_REPUTATION_SNAPSHOT_VERSION_V1, SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
    SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
    SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1, SORAFS_RECONCILIATION_REPORT_VERSION_V1,
    SignedReputationSnapshotV1, SoraFsAppealFinanceAccountFlowV1, SoraFsAppealFinanceJurorPayoutV1,
    SoraFsAppealFinanceOutcomeV1, SoraFsAppealFinanceReportV1,
    SoraFsAppealFinanceSettlementReceiptV1, SoraFsAppealFinanceWeeklyRollupV1,
    SoraFsModerationBallotGovernanceEventKindV1, SoraFsModerationBallotGovernanceEventV1,
    SoraFsModerationBallotGovernanceTallyV1, SoraFsModerationVoteChoiceV1,
    SoraFsModerationVoteCountsV1, SorafsReconciliationReportV1, build_reputation_snapshot,
    validate_governance_dag_head_against_chain_v1,
};
use tempfile::TempDir;
use tokio::net::TcpListener;

use super::*;

fn request_ingress_test_public_key() -> [u8; 32] {
    KeyPair::try_from_seed(vec![0xA7; 32], Algorithm::Ed25519)
        .expect("derive request-ingress test key")
        .public_key()
        .to_bytes()
        .1
        .try_into()
        .expect("Ed25519 public key width")
}

fn request_ingress_test_binding() -> GovernanceDagRequestIngressBindingV1 {
    let scope = GovernanceDagAuthenticationScope::Ipfs;
    GovernanceDagRequestIngressBindingV1::try_new(
        scope,
        governance_dag_request_ingress_endpoint_binding_v1(
            scope,
            "https://governance.example/ipfs/",
        )
        .expect("canonical ingress endpoint"),
        request_ingress_test_public_key(),
        1_048_576,
        30,
        5,
    )
    .expect("valid request-ingress binding")
}

fn request_receiver_test_key_pair() -> KeyPair {
    KeyPair::try_from_seed(vec![0xA7; 32], Algorithm::Ed25519)
        .expect("derive request-receiver test key")
}

fn request_receiver_test_envelope(
    key_pair: &KeyPair,
    descriptor: &GovernanceDagCanonicalRequestV1,
    issued_at_unix_secs: u64,
    nonce: [u8; 32],
) -> GovernanceDagRequestAuthenticationEnvelopeV1 {
    let public_key = key_pair
        .public_key()
        .to_bytes()
        .1
        .try_into()
        .expect("Ed25519 public key width");
    let expires_at_unix_secs = issued_at_unix_secs + 15;
    let payload = GovernanceDagRequestAuthenticationEnvelopeV1::signing_payload(
        descriptor,
        issued_at_unix_secs,
        expires_at_unix_secs,
        nonce,
        public_key,
    );
    let signature = IrohaSignature::try_new(key_pair.private_key(), &payload)
        .expect("sign request-receiver fixture")
        .payload()
        .try_into()
        .expect("Ed25519 signature width");
    GovernanceDagRequestAuthenticationEnvelopeV1::try_new(
        descriptor,
        issued_at_unix_secs,
        expires_at_unix_secs,
        nonce,
        public_key,
        signature,
    )
    .expect("construct request-receiver envelope")
}

fn request_receiver_test_descriptor(endpoint: &str) -> GovernanceDagCanonicalRequestV1 {
    GovernanceDagCanonicalRequestV1::try_from_http_parts(
        GovernanceDagAuthenticationScope::SignedHead,
        "GET",
        endpoint,
        [
            ("accept", b"*/*".as_slice()),
            ("accept-encoding", b"identity".as_slice()),
            ("user-agent", b"iroha-gdag-receiver-test/1".as_slice()),
        ],
        b"",
        1_048_576,
    )
    .expect("construct canonical request-receiver descriptor")
}

fn request_receiver_test_binding_for_endpoint(
    endpoint: &str,
    public_key: [u8; 32],
) -> GovernanceDagRequestIngressBindingV1 {
    GovernanceDagRequestIngressBindingV1::try_new(
        GovernanceDagAuthenticationScope::SignedHead,
        governance_dag_request_ingress_endpoint_binding_v1(
            GovernanceDagAuthenticationScope::SignedHead,
            endpoint,
        )
        .expect("bind request-receiver endpoint"),
        public_key,
        1_048_576,
        30,
        5,
    )
    .expect("construct request-receiver binding")
}

fn request_receiver_http1_request(
    client: &reqwest::Client,
    endpoint: &str,
    envelope: &GovernanceDagRequestAuthenticationEnvelopeV1,
) -> reqwest::RequestBuilder {
    let mut request = client
        .get(endpoint)
        .header(header::ACCEPT, "*/*")
        .header(header::ACCEPT_ENCODING, "identity")
        .header(header::USER_AGENT, "iroha-gdag-receiver-test/1");
    for (name, value) in governance_dag_request_authentication_headers_v1(envelope) {
        request = request.header(name, value);
    }
    request
}

fn request_receiver_test_response(status: StatusCode) -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = status;
    response
}

#[derive(Clone)]
struct RequestReceiverHttp1State {
    endpoint: Arc<str>,
    binding: GovernanceDagRequestIngressBindingV1,
    replay_store: Arc<Mutex<GovernanceDagRequestAuthenticationReplayCacheV1>>,
    backend_calls: Arc<AtomicU64>,
    now_unix_secs: u64,
}

async fn request_receiver_http1_handler(
    State(state): State<RequestReceiverHttp1State>,
    request: Request<Body>,
) -> Response {
    let (parts, body) = request.into_parts();
    let max_body_bytes = match usize::try_from(state.binding.max_body_bytes()) {
        Ok(max_body_bytes) => max_body_bytes,
        Err(_) => return request_receiver_test_response(StatusCode::INTERNAL_SERVER_ERROR),
    };
    let body = match to_bytes(body, max_body_bytes).await {
        Ok(body) => body,
        Err(_) => return request_receiver_test_response(StatusCode::PAYLOAD_TOO_LARGE),
    };
    let request = Request::from_parts(parts, body);
    let mut replay_store = match state.replay_store.lock() {
        Ok(replay_store) => replay_store,
        Err(_) => return request_receiver_test_response(StatusCode::INTERNAL_SERVER_ERROR),
    };
    let mut receiver = match GovernanceDagHttpRequestReceiverV1::try_new(
        state.endpoint.as_ref(),
        state.binding,
        &mut *replay_store,
    ) {
        Ok(receiver) => receiver,
        Err(_) => return request_receiver_test_response(StatusCode::INTERNAL_SERVER_ERROR),
    };
    match receiver.verify_http_request(request, state.now_unix_secs) {
        Ok(verified) => {
            if !verified.request().headers().contains_key(header::HOST)
                || verified.request().uri().scheme().is_some()
                || verified.request().uri().authority().is_some()
                || verified
                    .request()
                    .headers()
                    .keys()
                    .any(|name| governance_request_auth_header_has_prefix_v1(name.as_str()))
            {
                return request_receiver_test_response(StatusCode::INTERNAL_SERVER_ERROR);
            }
            state.backend_calls.fetch_add(1, Ordering::SeqCst);
            request_receiver_test_response(StatusCode::NO_CONTENT)
        }
        Err(GovernanceDagRequestAuthenticationErrorV1::Replay) => {
            request_receiver_test_response(StatusCode::CONFLICT)
        }
        Err(GovernanceDagRequestAuthenticationErrorV1::UnexpectedHeader) => {
            request_receiver_test_response(StatusCode::BAD_REQUEST)
        }
        Err(
            GovernanceDagRequestAuthenticationErrorV1::InvalidAuthority
            | GovernanceDagRequestAuthenticationErrorV1::AuthorityMismatch,
        ) => request_receiver_test_response(StatusCode::MISDIRECTED_REQUEST),
        Err(_) => request_receiver_test_response(StatusCode::UNAUTHORIZED),
    }
}

#[test]
fn request_ingress_endpoint_binding_is_normalized_scoped_and_fail_closed() {
    let ipfs_without_slash = governance_dag_request_ingress_endpoint_binding_v1(
        GovernanceDagAuthenticationScope::Ipfs,
        "https://governance.example/ipfs",
    )
    .expect("canonical IPFS base");
    let ipfs_with_slash = governance_dag_request_ingress_endpoint_binding_v1(
        GovernanceDagAuthenticationScope::Ipfs,
        "https://governance.example/ipfs/",
    )
    .expect("canonical IPFS base with slash");
    assert_eq!(ipfs_without_slash, ipfs_with_slash);
    assert_ne!(
        ipfs_with_slash,
        governance_dag_request_ingress_endpoint_binding_v1(
            GovernanceDagAuthenticationScope::SignedHead,
            "https://governance.example/ipfs/",
        )
        .expect("canonical signed-head endpoint")
    );
    for invalid in [
        "ftp://governance.example/ipfs/",
        "https://user@governance.example/ipfs/",
        "https://governance.example/ipfs/?token=public",
        "https://governance.example/ipfs/#fragment",
        "https://governance.example/ipfs/%2Fadmin",
    ] {
        assert_eq!(
            governance_dag_request_ingress_endpoint_binding_v1(
                GovernanceDagAuthenticationScope::Ipfs,
                invalid,
            ),
            Err(GovernanceDagRequestIngressQualificationErrorV1::InvalidEndpointBinding)
        );
    }
}

#[test]
fn request_ingress_binding_digest_commits_every_policy_field() {
    let binding = request_ingress_test_binding();
    let digest = binding.binding_digest();
    assert_ne!(digest, [0; 32]);

    let variants = [
        GovernanceDagRequestIngressBindingV1::try_new(
            GovernanceDagAuthenticationScope::SignedHead,
            binding.endpoint_binding(),
            binding.public_key(),
            binding.max_body_bytes(),
            binding.max_envelope_lifetime_secs(),
            binding.max_future_skew_secs(),
        )
        .expect("changed scope"),
        GovernanceDagRequestIngressBindingV1::try_new(
            binding.scope(),
            [0x11; 32],
            binding.public_key(),
            binding.max_body_bytes(),
            binding.max_envelope_lifetime_secs(),
            binding.max_future_skew_secs(),
        )
        .expect("changed endpoint"),
        GovernanceDagRequestIngressBindingV1::try_new(
            binding.scope(),
            binding.endpoint_binding(),
            KeyPair::try_from_seed(vec![0xA8; 32], Algorithm::Ed25519)
                .expect("derive alternate request-ingress key")
                .public_key()
                .to_bytes()
                .1
                .try_into()
                .expect("Ed25519 public key width"),
            binding.max_body_bytes(),
            binding.max_envelope_lifetime_secs(),
            binding.max_future_skew_secs(),
        )
        .expect("changed public key"),
        GovernanceDagRequestIngressBindingV1::try_new(
            binding.scope(),
            binding.endpoint_binding(),
            binding.public_key(),
            binding.max_body_bytes() + 1,
            binding.max_envelope_lifetime_secs(),
            binding.max_future_skew_secs(),
        )
        .expect("changed body limit"),
        GovernanceDagRequestIngressBindingV1::try_new(
            binding.scope(),
            binding.endpoint_binding(),
            binding.public_key(),
            binding.max_body_bytes(),
            binding.max_envelope_lifetime_secs() + 1,
            binding.max_future_skew_secs(),
        )
        .expect("changed lifetime"),
        GovernanceDagRequestIngressBindingV1::try_new(
            binding.scope(),
            binding.endpoint_binding(),
            binding.public_key(),
            binding.max_body_bytes(),
            binding.max_envelope_lifetime_secs(),
            binding.max_future_skew_secs() + 1,
        )
        .expect("changed future skew"),
    ];
    for variant in variants {
        assert_ne!(variant.binding_digest(), digest);
    }
}

#[tokio::test]
async fn request_receiver_accepts_real_http1_host_and_blocks_boundary_bypasses() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind request-receiver HTTP/1 fixture");
    let address = listener
        .local_addr()
        .expect("read request-receiver fixture address");
    let endpoint = format!("http://{address}/head");
    let key_pair = request_receiver_test_key_pair();
    let public_key = key_pair
        .public_key()
        .to_bytes()
        .1
        .try_into()
        .expect("Ed25519 public key width");
    let binding = request_receiver_test_binding_for_endpoint(&endpoint, public_key);
    let backend_calls = Arc::new(AtomicU64::new(0));
    let now_unix_secs = 1_700_000_000;
    let state = RequestReceiverHttp1State {
        endpoint: Arc::from(endpoint.as_str()),
        binding,
        replay_store: Arc::new(Mutex::new(
            GovernanceDagRequestAuthenticationReplayCacheV1::new(),
        )),
        backend_calls: Arc::clone(&backend_calls),
        now_unix_secs,
    };
    let router = Router::new()
        .route("/head", get(request_receiver_http1_handler))
        .with_state(state);
    let server = tokio::spawn(async move {
        axum::serve(listener, router.into_make_service())
            .await
            .expect("serve request-receiver HTTP/1 fixture");
    });
    let client = reqwest::Client::builder()
        .no_proxy()
        .http1_only()
        .build()
        .expect("construct HTTP/1 request-receiver client");
    let descriptor = request_receiver_test_descriptor(&endpoint);
    let envelope =
        request_receiver_test_envelope(&key_pair, &descriptor, now_unix_secs, [0x31; 32]);

    let accepted = request_receiver_http1_request(&client, &endpoint, &envelope)
        .send()
        .await
        .expect("send authenticated HTTP/1 request");
    assert_eq!(accepted.status(), StatusCode::NO_CONTENT);
    assert_eq!(backend_calls.load(Ordering::SeqCst), 1);

    let replay = request_receiver_http1_request(&client, &endpoint, &envelope)
        .send()
        .await
        .expect("replay authenticated HTTP/1 request");
    assert_eq!(replay.status(), StatusCode::CONFLICT);
    assert_eq!(backend_calls.load(Ordering::SeqCst), 1);

    let extension_envelope =
        request_receiver_test_envelope(&key_pair, &descriptor, now_unix_secs, [0x32; 32]);
    let extension = request_receiver_http1_request(&client, &endpoint, &extension_envelope)
        .header("x-original-url", "/bypass")
        .send()
        .await
        .expect("send unsigned semantic-extension request");
    assert_eq!(extension.status(), StatusCode::BAD_REQUEST);
    assert_eq!(backend_calls.load(Ordering::SeqCst), 1);

    let wrong_host_envelope =
        request_receiver_test_envelope(&key_pair, &descriptor, now_unix_secs, [0x33; 32]);
    let wrong_host = request_receiver_http1_request(&client, &endpoint, &wrong_host_envelope)
        .header(header::HOST, "attacker.invalid")
        .send()
        .await
        .expect("send mismatched-Host HTTP/1 request");
    assert_eq!(wrong_host.status(), StatusCode::MISDIRECTED_REQUEST);
    assert_eq!(backend_calls.load(Ordering::SeqCst), 1);

    let authority = endpoint
        .strip_prefix("http://")
        .and_then(|endpoint| endpoint.strip_suffix("/head"))
        .expect("fixture endpoint has canonical authority");
    let absolute_envelope =
        request_receiver_test_envelope(&key_pair, &descriptor, now_unix_secs, [0x34; 32]);
    let mut absolute_form = Request::builder()
        .method("GET")
        .uri(endpoint.as_str())
        .version(Version::HTTP_11)
        .header(header::HOST, authority)
        .header(header::ACCEPT, "*/*")
        .header(header::ACCEPT_ENCODING, "identity")
        .header(header::USER_AGENT, "iroha-gdag-receiver-test/1")
        .body(Vec::new())
        .expect("construct absolute-form request");
    for (name, value) in governance_dag_request_authentication_headers_v1(&absolute_envelope) {
        absolute_form.headers_mut().insert(
            http::HeaderName::from_bytes(name.as_bytes()).expect("canonical auth header name"),
            http::HeaderValue::from_str(&value).expect("canonical auth header value"),
        );
    }
    let mut absolute_replay_store = GovernanceDagRequestAuthenticationReplayCacheV1::new();
    let mut absolute_receiver =
        GovernanceDagHttpRequestReceiverV1::try_new(&endpoint, binding, &mut absolute_replay_store)
            .expect("construct absolute-form receiver");
    let absolute_verified = absolute_receiver
        .verify_http_request(absolute_form, now_unix_secs)
        .expect("matching absolute-form authority authenticates");
    assert_eq!(absolute_verified.descriptor(), &descriptor);
    assert!(absolute_verified.request().uri().scheme().is_none());
    assert!(absolute_verified.request().uri().authority().is_none());
    assert_eq!(
        absolute_verified
            .request()
            .uri()
            .path_and_query()
            .map(http::uri::PathAndQuery::as_str),
        Some("/head")
    );
    assert_eq!(
        absolute_verified
            .request()
            .headers()
            .get(header::HOST)
            .and_then(|value| value.to_str().ok()),
        Some(authority)
    );
    assert_eq!(backend_calls.load(Ordering::SeqCst), 1);

    let duplicate_envelope =
        request_receiver_test_envelope(&key_pair, &descriptor, now_unix_secs, [0x35; 32]);
    let mut duplicate_host = Request::builder()
        .method("GET")
        .uri("/head")
        .version(Version::HTTP_11)
        .header(header::HOST, authority)
        .header(header::ACCEPT, "*/*")
        .header(header::ACCEPT_ENCODING, "identity")
        .header(header::USER_AGENT, "iroha-gdag-receiver-test/1")
        .body(Vec::new())
        .expect("construct duplicate-Host request");
    duplicate_host.headers_mut().append(
        header::HOST,
        axum::http::HeaderValue::from_str(authority).expect("canonical fixture authority"),
    );
    for (name, value) in governance_dag_request_authentication_headers_v1(&duplicate_envelope) {
        duplicate_host.headers_mut().insert(
            axum::http::HeaderName::from_bytes(name.as_bytes())
                .expect("canonical auth header name"),
            axum::http::HeaderValue::from_str(&value).expect("canonical auth header value"),
        );
    }
    let mut duplicate_replay_store = GovernanceDagRequestAuthenticationReplayCacheV1::new();
    let mut duplicate_receiver = GovernanceDagHttpRequestReceiverV1::try_new(
        &endpoint,
        binding,
        &mut duplicate_replay_store,
    )
    .expect("construct duplicate-Host receiver");
    let duplicate_error = duplicate_receiver
        .verify_http_request(duplicate_host, now_unix_secs)
        .expect_err("duplicate Host must fail before backend dispatch");
    assert_eq!(
        duplicate_error,
        GovernanceDagRequestAuthenticationErrorV1::InvalidAuthority
    );
    assert_eq!(backend_calls.load(Ordering::SeqCst), 1);

    server.abort();
}

#[test]
fn request_ingress_qualification_requires_receiver_replay_and_replica_proofs() {
    let provider = GovernanceDagRuntimeProviderQualificationV1::new(7, [0xB1; 32]);
    let binding = request_ingress_test_binding();
    for (receiver, replay, replicas, expected) in [
        (
            [0; 32],
            [0xB3; 32],
            [0xB4; 32],
            GovernanceDagRequestIngressQualificationErrorV1::InvalidReceiverPolicy,
        ),
        (
            [0xB2; 32],
            [0; 32],
            [0xB4; 32],
            GovernanceDagRequestIngressQualificationErrorV1::InvalidReplayNamespace,
        ),
        (
            [0xB2; 32],
            [0xB3; 32],
            [0; 32],
            GovernanceDagRequestIngressQualificationErrorV1::InvalidReplicaSet,
        ),
    ] {
        assert_eq!(
            GovernanceDagRequestIngressQualificationV1::try_new(
                provider, binding, receiver, replay, replicas,
            ),
            Err(expected)
        );
    }
    let qualification = GovernanceDagRequestIngressQualificationV1::try_new(
        provider, binding, [0xB2; 32], [0xB3; 32], [0xB4; 32],
    )
    .expect("complete ingress proof");
    assert_eq!(
        qualification.enforcement(),
        GovernanceDagRequestIngressEnforcementV1::ExclusiveAuthenticatedReceiver
    );
    assert_eq!(
        qualification.replay_posture(),
        GovernanceDagRequestReplayPostureV1::SharedSealedAtomicConsumeUntilExpiry
    );
}

fn read_publication_snapshot_fixture(root: &Path) -> GovernancePublicationSnapshotV1 {
    let root_guard = GovernanceFilesystemRootGuard::capture_source(root)
        .expect("retain read-only publication fixture root");
    load_governance_publication_snapshot_v1(&root_guard)
        .expect("read authoritative governance publication snapshot")
        .expect("publication fixture is initialized")
}

fn read_publication_state_fixture(root: &Path) -> JsonValue {
    let snapshot = read_publication_snapshot_fixture(root);
    norito::json::from_slice(snapshot.canonical_bytes())
        .expect("decode authoritative governance publication state")
}

fn read_publication_section_fixture(root: &Path, section: &str) -> JsonValue {
    read_publication_state_fixture(root)
        .get(section)
        .cloned()
        .unwrap_or_else(|| panic!("publication state section `{section}`"))
}

fn published_source_paths_fixture(root: &Path, payload_kind: &str) -> Vec<(PathBuf, PathBuf)> {
    read_publication_section_fixture(root, "publish_index")
        .get("entries")
        .and_then(JsonValue::as_array)
        .expect("publication entries")
        .iter()
        .filter(|entry| entry.get("payload_kind").and_then(JsonValue::as_str) == Some(payload_kind))
        .map(|entry| {
            let encoded = entry
                .get("encoded_path")
                .and_then(JsonValue::as_str)
                .expect("encoded source path");
            let json = entry
                .get("json_path")
                .and_then(JsonValue::as_str)
                .expect("JSON source path");
            (root.join(encoded), root.join(json))
        })
        .collect()
}

fn only_published_source_paths(root: &Path, payload_kind: &str) -> (PathBuf, PathBuf) {
    let paths = published_source_paths_fixture(root, payload_kind);
    assert_eq!(paths.len(), 1, "expected one `{payload_kind}` publication");
    paths.into_iter().next().expect("one publication path")
}

#[test]
fn runtime_dag_decode_allocation_budget_is_scaled_and_absolutely_capped() {
    assert_eq!(
        runtime_dag_decode_allocation_limit(1),
        GOVERNANCE_RUNTIME_DAG_DECODE_MIN_ALLOCATED_BYTES_V1
    );
    let last_scaled_input = (GOVERNANCE_RUNTIME_DAG_DECODE_MAX_ALLOCATED_BYTES_V1 - 1)
        / GOVERNANCE_RUNTIME_DAG_DECODE_ALLOCATION_MULTIPLIER_V1;
    assert_eq!(
        runtime_dag_decode_allocation_limit(last_scaled_input),
        last_scaled_input * GOVERNANCE_RUNTIME_DAG_DECODE_ALLOCATION_MULTIPLIER_V1
    );
    let first_capped_input = last_scaled_input.saturating_add(1);
    assert_eq!(
        runtime_dag_decode_allocation_limit(first_capped_input),
        GOVERNANCE_RUNTIME_DAG_DECODE_MAX_ALLOCATED_BYTES_V1
    );
    assert_eq!(
        runtime_dag_decode_allocation_limit(usize::MAX),
        GOVERNANCE_RUNTIME_DAG_DECODE_MAX_ALLOCATED_BYTES_V1
    );
}

#[test]
fn runtime_dag_decode_allocation_floor_admits_one_composite_state_but_rejects_two() {
    let state = FencedPrivacyStateV1 {
        version: GOVERNANCE_FENCED_PRIVACY_STATE_VERSION_V1,
        pending: None,
        publication_cache: None,
        authoritative_head_sync: None,
    };
    let bytes = encode_governance_two_slot_value_v1(&state, "empty fenced privacy state")
        .expect("encode production empty fenced privacy state");
    assert_eq!(bytes.len(), 48, "production empty-state canonical width");
    assert_eq!(
        runtime_dag_decode_allocation_limit(bytes.len()),
        GOVERNANCE_RUNTIME_DAG_DECODE_MIN_ALLOCATED_BYTES_V1
    );
    let decoded: FencedPrivacyStateV1 =
        decode_canonical_runtime_dag(&bytes, "empty fenced privacy state")
            .expect("decode production empty fenced privacy state");
    assert_eq!(decoded, state);

    let composite = (state.clone(), state);
    let composite_bytes =
        encode_governance_two_slot_value_v1(&composite, "two empty fenced privacy states")
            .expect("encode two empty fenced privacy states");
    assert!(
        composite_bytes.len()
            <= GOVERNANCE_RUNTIME_DAG_DECODE_MIN_ALLOCATED_BYTES_V1
                / GOVERNANCE_RUNTIME_DAG_DECODE_ALLOCATION_MULTIPLIER_V1,
        "negative fixture must remain inside the bounded allocation-floor interval"
    );
    assert_eq!(
        runtime_dag_decode_allocation_limit(composite_bytes.len()),
        GOVERNANCE_RUNTIME_DAG_DECODE_MIN_ALLOCATED_BYTES_V1
    );
    let error = decode_canonical_runtime_dag::<(FencedPrivacyStateV1, FencedPrivacyStateV1)>(
        &composite_bytes,
        "two empty fenced privacy states",
    )
    .expect_err("two composite records must exceed the bounded allocation floor");
    let message = error.to_string();
    assert!(
        message.contains("cumulative allocation") && message.contains("exceeds decode limit 2048"),
        "unexpected allocation-floor diagnostic: {error}"
    );
}

#[test]
fn canonical_request_rejects_body_bound_before_consuming_headers() {
    let result = catch_unwind(AssertUnwindSafe(|| {
        GovernanceDagCanonicalRequestV1::try_from_http_parts(
            GovernanceDagAuthenticationScope::Ipfs,
            "POST",
            "https://example.invalid/api/v0/add?pin=false",
            std::iter::once_with(|| -> (&'static str, &'static [u8]) {
                panic!("oversized body must fail before headers are consumed")
            }),
            b"too-large",
            1,
        )
    }));
    let error = result
        .expect("body-bound rejection must not poll the header iterator")
        .expect_err("oversized body must fail closed");
    assert_eq!(
        error,
        "Governance DAG request body commitment is noncanonical or exceeds the configured bound"
    );
}

#[test]
fn request_partition_rejects_selected_header_count_before_vector_growth() {
    let headers = std::iter::repeat_n(
        ("accept", b"application/octet-stream".as_slice()),
        GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADERS_V1 + 1,
    )
    .chain(std::iter::once_with(|| -> (&'static str, &'static [u8]) {
        panic!("selected-header overflow must stop iterator consumption")
    }));
    let result = catch_unwind(AssertUnwindSafe(|| {
        partition_governance_dag_http_headers_v1(
            headers,
            b"",
            GovernanceDagAuthenticationHeaderDispositionV1::Retain,
        )
    }));
    assert_eq!(
        result
            .expect("selected-header overflow must not poll past its fixed bound")
            .expect_err("selected-header overflow must fail closed"),
        GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest
    );
}

#[test]
fn request_partition_rejects_selected_header_budget_before_vector_growth() {
    let oversized_value = vec![b'a'; GOVERNANCE_DAG_REQUEST_AUTH_MAX_HEADER_VALUE_BYTES_V1 + 1];
    let headers =
        std::iter::once(("accept", oversized_value.as_slice())).chain(std::iter::once_with(|| {
            panic!("selected-header byte overflow must stop iterator consumption")
        }));
    let result = catch_unwind(AssertUnwindSafe(|| {
        partition_governance_dag_http_headers_v1(
            headers,
            b"",
            GovernanceDagAuthenticationHeaderDispositionV1::Retain,
        )
    }));
    assert_eq!(
        result
            .expect("selected-header byte overflow must not poll past its fixed bound")
            .expect_err("selected-header byte overflow must fail closed"),
        GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest
    );
}

#[test]
fn request_partition_rejects_authentication_header_count_before_vector_growth() {
    let headers = std::iter::repeat_n(
        (
            GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1[0],
            b"1".as_slice(),
        ),
        GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1.len() + 1,
    )
    .chain(std::iter::once_with(|| -> (&'static str, &'static [u8]) {
        panic!("authentication-header overflow must stop iterator consumption")
    }));
    let result = catch_unwind(AssertUnwindSafe(|| {
        partition_governance_dag_http_headers_v1(
            headers,
            b"",
            GovernanceDagAuthenticationHeaderDispositionV1::Retain,
        )
    }));
    assert_eq!(
        result
            .expect("authentication-header overflow must not poll past its fixed bound")
            .expect_err("authentication-header overflow must fail closed"),
        GovernanceDagRequestAuthenticationErrorV1::DuplicateHeader
    );
}

// Keep one target-gated assertion for every ABI branch. Overlapping branches
// fail with duplicate definitions; missing branches fail to resolve the flag.
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
#[test]
fn linux_directory_open_flags_match_low_flag_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x8000);
    assert_eq!(platform_directory_only_flag(), 0x4000);
}

#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
#[test]
fn linux_directory_open_flags_match_generic_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x20000);
    assert_eq!(platform_directory_only_flag(), 0x10000);
}

#[cfg(all(
    target_os = "android",
    any(target_arch = "aarch64", target_arch = "arm")
))]
#[test]
fn android_arm_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x8000);
    assert_eq!(platform_directory_only_flag(), 0x4000);
}

#[cfg(all(
    target_os = "android",
    any(target_arch = "x86", target_arch = "x86_64")
))]
#[test]
fn android_x86_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x20000);
    assert_eq!(platform_directory_only_flag(), 0x10000);
}

#[cfg(all(target_os = "android", target_arch = "riscv64"))]
#[test]
fn android_riscv64_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x400000);
    assert_eq!(platform_directory_only_flag(), 0x200000);
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "riscv32", target_arch = "riscv64")
))]
#[test]
fn linux_riscv_directory_open_flags_remain_generic_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x20000);
    assert_eq!(platform_directory_only_flag(), 0x10000);
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[test]
fn apple_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x100);
    assert_eq!(platform_directory_only_flag(), 0x0010_0000);
}

#[cfg(target_os = "freebsd")]
#[test]
fn freebsd_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x100);
    assert_eq!(platform_directory_only_flag(), 0x0002_0000);
}

#[cfg(target_os = "dragonfly")]
#[test]
fn dragonfly_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x100);
    assert_eq!(platform_directory_only_flag(), 0x0800_0000);
}

#[cfg(target_os = "openbsd")]
#[test]
fn openbsd_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x100);
    assert_eq!(platform_directory_only_flag(), 0x0002_0000);
}

#[cfg(target_os = "netbsd")]
#[test]
fn netbsd_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x100);
    assert_eq!(platform_directory_only_flag(), 0x0020_0000);
}

const TEST_RUNTIME_DAG_SIGNER_POLICY_DIGEST: [u8; 32] = [0x71; 32];
const TEST_RUNTIME_DAG_STORE_POLICY_DIGEST: [u8; 32] = [0x73; 32];

fn test_runtime_dag_signer_qualification() -> GovernanceDagRuntimeProviderQualificationV1 {
    GovernanceDagRuntimeProviderQualificationV1::new(1, TEST_RUNTIME_DAG_SIGNER_POLICY_DIGEST)
}

#[derive(Debug)]
struct TestRuntimeDagCheckpointStoreState {
    records: [Option<GovernanceDagSealedStateRecord>; 6],
    generation_floors: [u64; 6],
}

impl Default for TestRuntimeDagCheckpointStoreState {
    fn default() -> Self {
        Self {
            records: std::array::from_fn(|_| None),
            generation_floors: [0; 6],
        }
    }
}

#[derive(Debug, Default)]
struct TestRuntimeDagCheckpointStore {
    state: Mutex<TestRuntimeDagCheckpointStoreState>,
    fail_after_next_intent_cas: AtomicBool,
    fail_before_next_checkpoint_cas: AtomicBool,
    fail_after_next_checkpoint_cas: AtomicBool,
    producer_checkpoint_load_count: AtomicU64,
    producer_checkpoint_second_load: Mutex<Option<GovernanceDagSealedStateRecord>>,
}

impl TestRuntimeDagCheckpointStore {
    const HANDLE: &'static str = "kms:governance-dag:producer-checkpoint-primary";

    const fn qualification() -> GovernanceDagRuntimeProviderQualificationV1 {
        GovernanceDagRuntimeProviderQualificationV1::new(1, TEST_RUNTIME_DAG_STORE_POLICY_DIGEST)
    }

    const fn slot_index(slot: GovernanceDagSealedStateSlot) -> usize {
        match slot {
            GovernanceDagSealedStateSlot::Checkpoint => 0,
            GovernanceDagSealedStateSlot::PublishIntent => 1,
            GovernanceDagSealedStateSlot::ProducerCheckpoint => 2,
            GovernanceDagSealedStateSlot::ProducerPublishIntent => 3,
            GovernanceDagSealedStateSlot::IpfsRequestReplay => 4,
            GovernanceDagSealedStateSlot::SignedHeadRequestReplay => 5,
        }
    }

    fn return_producer_checkpoint_on_second_load(&self, record: GovernanceDagSealedStateRecord) {
        *self
            .producer_checkpoint_second_load
            .lock()
            .expect("lock producer checkpoint race fixture") = Some(record);
        self.producer_checkpoint_load_count
            .store(0, Ordering::SeqCst);
    }
}

impl GovernanceDagSealedCheckpointStore for TestRuntimeDagCheckpointStore {
    fn handle(&self) -> &str {
        Self::HANDLE
    }

    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
        Ok(Self::qualification())
    }

    fn load(
        &self,
        slot: GovernanceDagSealedStateSlot,
    ) -> Result<Option<GovernanceDagSealedStateRecord>, String> {
        if slot == GovernanceDagSealedStateSlot::ProducerCheckpoint
            && self
                .producer_checkpoint_load_count
                .fetch_add(1, Ordering::SeqCst)
                == 1
            && let Some(record) = self
                .producer_checkpoint_second_load
                .lock()
                .map_err(|_| "poisoned".to_owned())?
                .take()
        {
            return Ok(Some(record));
        }
        let state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
        Ok(state.records[Self::slot_index(slot)].clone())
    }

    fn compare_and_swap(
        &self,
        slot: GovernanceDagSealedStateSlot,
        expected_revision: Option<[u8; 32]>,
        next: GovernanceDagSealedStateRecord,
    ) -> Result<(), String> {
        if slot == GovernanceDagSealedStateSlot::ProducerCheckpoint
            && self
                .fail_before_next_checkpoint_cas
                .swap(false, Ordering::SeqCst)
        {
            return Err("checkpoint CAS refused before install".to_owned());
        }
        let index = Self::slot_index(slot);
        let mut state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
        if state.records[index].as_ref().map(|record| record.revision) != expected_revision {
            return Err("compare-and-swap conflict".to_owned());
        }
        if next.generation <= state.generation_floors[index]
            || next.payload.is_empty()
            || !next.has_valid_revision(slot)
        {
            return Err("invalid or non-monotonic record".to_owned());
        }
        state.generation_floors[index] = next.generation;
        state.records[index] = Some(next);
        drop(state);
        if slot == GovernanceDagSealedStateSlot::ProducerPublishIntent
            && self
                .fail_after_next_intent_cas
                .swap(false, Ordering::SeqCst)
        {
            return Err("ambiguous intent CAS response".to_owned());
        }
        if slot == GovernanceDagSealedStateSlot::ProducerCheckpoint
            && self
                .fail_after_next_checkpoint_cas
                .swap(false, Ordering::SeqCst)
        {
            return Err("ambiguous checkpoint CAS response".to_owned());
        }
        Ok(())
    }

    fn delete(
        &self,
        slot: GovernanceDagSealedStateSlot,
        expected_revision: [u8; 32],
    ) -> Result<(), String> {
        let index = Self::slot_index(slot);
        let mut state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
        if state.records[index].as_ref().map(|record| record.revision) != Some(expected_revision) {
            return Err("delete conflict".to_owned());
        }
        state.records[index] = None;
        Ok(())
    }
}

const TEST_FENCED_PUBLISHER_HANDLE: &str = "hsm:governance:fenced-privacy-primary";
const TEST_FENCED_PUBLISHER_POLICY_DIGEST: [u8; 32] = [0x72; 32];
const TEST_FENCED_HEAD_READER_HANDLE: &str = TEST_FENCED_PUBLISHER_HANDLE;
const TEST_FENCED_HEAD_READER_POLICY_DIGEST: [u8; 32] = TEST_FENCED_PUBLISHER_POLICY_DIGEST;
const TEST_PRIVACY_QUERY_ID: [u8; 32] = [0x91; 32];
const TEST_PRIVACY_CYCLE_START: u64 = 1_800_000_000;
const TEST_PRIVACY_CYCLE_END: u64 = 1_800_604_800;

fn test_fenced_publisher_qualification() -> GovernanceDagRuntimeProviderQualificationV1 {
    GovernanceDagRuntimeProviderQualificationV1::new(1, TEST_FENCED_PUBLISHER_POLICY_DIGEST)
}

fn test_fenced_head_reader_qualification() -> GovernanceDagRuntimeProviderQualificationV1 {
    GovernanceDagRuntimeProviderQualificationV1::new(1, TEST_FENCED_HEAD_READER_POLICY_DIGEST)
}

type TestFencedPublications =
    BTreeMap<([u8; 32], [u8; 16]), ([u8; 32], [u8; 32], FencedTransparencyTargetHeadV1)>;

#[derive(Debug, Default)]
struct TestFencedPublisherState {
    head: Option<FencedTransparencyTargetHeadV1>,
    fencing_floor: u64,
    publications: TestFencedPublications,
    receipts: BTreeMap<
        [u8; 32],
        (
            FencedPrivacyPublicationRequestV1,
            FencedPrivacyPublicationReceiptV1,
        ),
    >,
    history: Vec<FencedTransparencyTargetHeadV1>,
    append_count: usize,
}

#[derive(Debug, Default)]
struct TestFencedPublisherPause {
    reached: bool,
    released: bool,
}

#[derive(Debug)]
struct TestFencedTransparencyPublisher {
    state: Mutex<TestFencedPublisherState>,
    pause_token: AtomicU64,
    pause: Mutex<TestFencedPublisherPause>,
    pause_changed: Condvar,
    substitute_receipt: AtomicBool,
}

#[derive(Debug)]
struct TestFencedTransparencyHeadReader {
    target: Arc<TestFencedTransparencyPublisher>,
    handle: String,
    revision: AtomicU64,
    policy_digest: [u8; 32],
    head_override: Mutex<Option<Option<FencedTransparencyTargetHeadV1>>>,
    fail_read: AtomicBool,
}

impl TestFencedTransparencyPublisher {
    fn new() -> Self {
        Self {
            state: Mutex::new(TestFencedPublisherState::default()),
            pause_token: AtomicU64::new(0),
            pause: Mutex::new(TestFencedPublisherPause::default()),
            pause_changed: Condvar::new(),
            substitute_receipt: AtomicBool::new(false),
        }
    }

    fn pause_fencing_token(&self, fencing_token: u64) {
        self.pause_token.store(fencing_token, Ordering::Release);
        *self.pause.lock().expect("fenced publisher pause") = TestFencedPublisherPause::default();
    }

    fn wait_until_paused(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut pause = self.pause.lock().expect("fenced publisher pause");
        while !pause.reached {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .expect("fenced publisher reached pause deadline");
            let (next, wait) = self
                .pause_changed
                .wait_timeout(pause, remaining)
                .expect("fenced publisher pause");
            pause = next;
            assert!(!wait.timed_out(), "fenced publisher did not pause");
        }
    }

    fn release_paused(&self) {
        self.pause.lock().expect("fenced publisher pause").released = true;
        self.pause_changed.notify_all();
    }

    fn set_substitute_receipt(&self, substitute: bool) {
        self.substitute_receipt.store(substitute, Ordering::Release);
    }

    fn append_count(&self) -> usize {
        self.state
            .lock()
            .expect("fenced publisher state")
            .append_count
    }

    fn head(&self) -> Option<FencedTransparencyTargetHeadV1> {
        self.state.lock().expect("fenced publisher state").head
    }

    fn pause_if_requested(&self, fencing_token: u64) {
        if self.pause_token.load(Ordering::Acquire) != fencing_token {
            return;
        }
        let mut pause = self.pause.lock().expect("fenced publisher pause");
        pause.reached = true;
        self.pause_changed.notify_all();
        while !pause.released {
            pause = self
                .pause_changed
                .wait(pause)
                .expect("fenced publisher pause");
        }
        self.pause_token.store(0, Ordering::Release);
    }
}

impl TestFencedTransparencyHeadReader {
    fn new(target: Arc<TestFencedTransparencyPublisher>) -> Self {
        Self {
            target,
            handle: TEST_FENCED_HEAD_READER_HANDLE.to_owned(),
            revision: AtomicU64::new(1),
            policy_digest: TEST_FENCED_HEAD_READER_POLICY_DIGEST,
            head_override: Mutex::new(None),
            fail_read: AtomicBool::new(false),
        }
    }

    fn with_handle(
        target: Arc<TestFencedTransparencyPublisher>,
        handle: impl Into<String>,
    ) -> Self {
        Self {
            handle: handle.into(),
            ..Self::new(target)
        }
    }

    fn with_binding(
        target: Arc<TestFencedTransparencyPublisher>,
        handle: impl Into<String>,
        revision: u64,
        policy_digest: [u8; 32],
    ) -> Self {
        Self {
            target,
            handle: handle.into(),
            revision: AtomicU64::new(revision),
            policy_digest,
            head_override: Mutex::new(None),
            fail_read: AtomicBool::new(false),
        }
    }

    fn set_revision(&self, revision: u64) {
        self.revision.store(revision, Ordering::Release);
    }

    fn override_head(&self, head: Option<FencedTransparencyTargetHeadV1>) {
        *self.head_override.lock().expect("head reader override") = Some(head);
    }

    fn set_fail_read(&self, fail: bool) {
        self.fail_read.store(fail, Ordering::Release);
    }
}

impl FencedTransparencyPublisherV1 for TestFencedTransparencyPublisher {
    fn handle(&self) -> &str {
        TEST_FENCED_PUBLISHER_HANDLE
    }

    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
        Ok(test_fenced_publisher_qualification())
    }

    fn compare_and_append_privacy(
        &self,
        request: &FencedPrivacyPublicationRequestV1,
    ) -> Result<FencedPrivacyPublicationReceiptV1, FencedTransparencyPublishErrorV1> {
        request.validate()?;
        self.pause_if_requested(request.fencing_token());
        let mut state = self
            .state
            .lock()
            .map_err(|_| FencedTransparencyPublishErrorV1::UnqualifiedProvider)?;
        if let Some((retained_request, receipt)) = state.receipts.get(&request.request_digest()) {
            return if retained_request == request {
                Ok(receipt.clone())
            } else {
                Err(FencedTransparencyPublishErrorV1::Rejected)
            };
        }
        if let Some((idempotency_digest, payload_digest, included_head)) = state
            .publications
            .get(&request.publication_scope())
            .copied()
        {
            if idempotency_digest != request.publication_idempotency_digest()
                || payload_digest != request.payload_digest()
            {
                return Err(FencedTransparencyPublishErrorV1::PublicationConflict);
            }
            let readback_head = state
                .head
                .ok_or(FencedTransparencyPublishErrorV1::InvalidReceipt)?;
            let receipt = FencedPrivacyPublicationReceiptV1::from_verified_existing(
                request,
                TEST_FENCED_PUBLISHER_HANDLE,
                test_fenced_publisher_qualification(),
                included_head,
                readback_head,
            )?;
            state
                .receipts
                .insert(request.request_digest(), (request.clone(), receipt.clone()));
            return Ok(receipt);
        }
        if request.fencing_token() <= state.fencing_floor {
            return Err(FencedTransparencyPublishErrorV1::StaleFencingToken);
        }
        if request.expected_authoritative_head() != state.head {
            return Err(FencedTransparencyPublishErrorV1::CompareConflict);
        }
        let receipt = FencedPrivacyPublicationReceiptV1::from_verified_append(
            request,
            TEST_FENCED_PUBLISHER_HANDLE,
            test_fenced_publisher_qualification(),
        )?;
        state.head = Some(receipt.included_head());
        state.fencing_floor = request.fencing_token();
        state.append_count += 1;
        state.history.push(receipt.included_head());
        state.publications.insert(
            request.publication_scope(),
            (
                request.publication_idempotency_digest(),
                request.payload_digest(),
                receipt.included_head(),
            ),
        );
        state
            .receipts
            .insert(request.request_digest(), (request.clone(), receipt.clone()));
        if self.substitute_receipt.load(Ordering::Acquire) {
            let mut substituted = receipt;
            substituted.head_inclusion_digest[0] ^= 0x80;
            Ok(substituted)
        } else {
            Ok(receipt)
        }
    }
}

impl FencedTransparencyAuthoritativeHeadReaderV1 for TestFencedTransparencyHeadReader {
    fn handle(&self) -> &str {
        &self.handle
    }

    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
        Ok(GovernanceDagRuntimeProviderQualificationV1::new(
            self.revision.load(Ordering::Acquire),
            self.policy_digest,
        ))
    }

    fn read_authoritative_head_with_ancestry(
        &self,
        required_ancestors: &[FencedTransparencyTargetHeadV1],
        required_publications: &[FencedTransparencyPublicationInclusionV1],
    ) -> Result<FencedTransparencyHeadAncestryProofV1, String> {
        if self.fail_read.load(Ordering::Acquire) {
            return Err("redacted test read failure".to_owned());
        }
        let observed = if let Some(head) = *self.head_override.lock().expect("head reader override")
        {
            head
        } else {
            self.target.head()
        };
        let state = self
            .target
            .state
            .lock()
            .map_err(|_| "redacted test target failure".to_owned())?;
        if observed != state.head {
            return Err("redacted test ancestry failure".to_owned());
        }
        let current_index = observed
            .map(|head| {
                state
                    .history
                    .iter()
                    .position(|candidate| *candidate == head)
                    .ok_or_else(|| "redacted test current-head proof failure".to_owned())
            })
            .transpose()?;
        for ancestor in required_ancestors {
            let ancestor_index = state
                .history
                .iter()
                .position(|candidate| candidate == ancestor)
                .ok_or_else(|| "redacted test ancestry failure".to_owned())?;
            if current_index.is_none_or(|current| ancestor_index > current) {
                return Err("redacted test ancestry failure".to_owned());
            }
        }
        for publication in required_publications {
            if !state.publications.values().any(
                |(publication_idempotency_digest, payload_digest, included_head)| {
                    *publication_idempotency_digest == publication.publication_idempotency_digest()
                        && *payload_digest == publication.payload_digest()
                        && *included_head == publication.included_head()
                },
            ) {
                return Err("redacted test publication inclusion failure".to_owned());
            }
        }
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"sorafs.test.fenced-head-ancestry-proof.v1");
        crate::fenced_privacy_digest_head(&mut hasher, observed);
        for ancestor in required_ancestors {
            crate::fenced_privacy_digest_head(&mut hasher, Some(*ancestor));
        }
        for publication in required_publications {
            hasher.update(&publication.publication_idempotency_digest());
            hasher.update(&publication.payload_digest());
            crate::fenced_privacy_digest_head(&mut hasher, Some(publication.included_head()));
        }
        FencedTransparencyHeadAncestryProofV1::try_new(
            observed,
            required_ancestors.to_vec(),
            required_publications.to_vec(),
            *hasher.finalize().as_bytes(),
        )
        .map_err(|_| "redacted test ancestry proof encoding failure".to_owned())
    }
}

fn qualified_test_fenced_publisher(
    provider: Arc<TestFencedTransparencyPublisher>,
) -> QualifiedFencedTransparencyPublisherV1 {
    let provider: Arc<dyn FencedTransparencyPublisherV1> = provider;
    QualifiedFencedTransparencyPublisherV1::try_new(
        TEST_FENCED_PUBLISHER_HANDLE.to_owned(),
        test_fenced_publisher_qualification(),
        provider,
    )
    .expect("qualify test fused publisher")
}

fn qualified_test_fenced_head_reader(
    reader: Arc<TestFencedTransparencyHeadReader>,
) -> QualifiedFencedTransparencyHeadReaderV1 {
    let reader: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1> = reader;
    QualifiedFencedTransparencyHeadReaderV1::try_new(
        TEST_FENCED_HEAD_READER_HANDLE.to_owned(),
        test_fenced_head_reader_qualification(),
        reader,
    )
    .expect("qualify test fused head reader")
}

fn test_fenced_head_reader(
    provider: Arc<TestFencedTransparencyPublisher>,
) -> Arc<TestFencedTransparencyHeadReader> {
    Arc::new(TestFencedTransparencyHeadReader::new(provider))
}

fn xor(value: &str) -> sorafs_manifest::deal::XorQuantity {
    value.parse().expect("canonical XOR quantity")
}

#[derive(Clone, Copy)]
struct SamplePrivacyReleaseSpec {
    query_id: [u8; 32],
    cycle_start_unix: u64,
    cycle_end_unix: u64,
    release_sequence: u64,
    release_record_digest: [u8; 32],
}

impl SamplePrivacyReleaseSpec {
    const fn primary() -> Self {
        Self {
            query_id: TEST_PRIVACY_QUERY_ID,
            cycle_start_unix: TEST_PRIVACY_CYCLE_START,
            cycle_end_unix: TEST_PRIVACY_CYCLE_END,
            release_sequence: 1,
            release_record_digest: [0x98; 32],
        }
    }

    const fn next() -> Self {
        let cycle_seconds = TEST_PRIVACY_CYCLE_END - TEST_PRIVACY_CYCLE_START;
        Self {
            query_id: TEST_PRIVACY_QUERY_ID,
            cycle_start_unix: TEST_PRIVACY_CYCLE_END,
            cycle_end_unix: TEST_PRIVACY_CYCLE_END + cycle_seconds,
            release_sequence: 2,
            release_record_digest: [0xA8; 32],
        }
    }
}

#[derive(Clone, Copy)]
struct SampleFinalizedAnchorSpec {
    sequence: u64,
    release_id: [u8; 16],
    record_digest: [u8; 32],
    latest_publication_block_hash: Option<[u8; 32]>,
}

fn sample_privacy_publication_for(
    spec: SamplePrivacyReleaseSpec,
) -> (ModerationLedgerCyclePublicationV1, Vec<u8>) {
    let cycle_id = crate::privacy_aggregate_cycle_id(
        spec.query_id,
        spec.cycle_start_unix,
        spec.cycle_end_unix,
    );
    let aggregate = ModerationPrivacyAggregateV1 {
        version: MODERATION_PRIVACY_AGGREGATE_VERSION_V1,
        aggregate_id: format!("sfm4c-fenced-publication-{}", spec.release_sequence),
        window_start_unix: spec.cycle_start_unix,
        window_end_unix: spec.cycle_end_unix,
        generated_at_unix: spec.cycle_end_unix,
        population_label: "fenced-population".to_owned(),
        population_digest: [0x92; 32],
        source_commitment: [0x91; 32],
        privacy: ModerationPrivacyParametersV1 {
            version: MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
            mode: ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression,
            epsilon_numerator: Some(4),
            epsilon_denominator: Some(5),
            delta_ppb: Some(0),
            per_subject_metric_cap: Some(1),
            suppression_threshold: Some(25),
        },
        noise_source: ModerationPrivacyNoiseSourceV1::ThresholdPrf(
            ModerationPrivacyThresholdPrfCommitmentV1 {
                commitment: [0x93; 32],
            },
        ),
        metrics: vec![ModerationPrivacyAggregateMetricV1 {
            key: "moderation_actions".to_owned(),
            value: 7,
            unit: "count".to_owned(),
        }],
        policy_digest: [0x94; 32],
        metadata: vec![ModerationLedgerMetadataV1 {
            key: "publisher".to_owned(),
            value: "fenced-runtime".to_owned(),
        }],
    };
    let publication = crate::NodeHandle::build_privacy_aggregate_publication(
        cycle_id,
        spec.cycle_start_unix,
        spec.cycle_end_unix,
        spec.cycle_end_unix,
        None,
        vec![aggregate],
    )
    .expect("build privacy publication");
    let encoded = norito::to_bytes(&publication).expect("encode privacy publication");
    (publication, encoded)
}

fn sample_privacy_publication() -> (ModerationLedgerCyclePublicationV1, Vec<u8>) {
    sample_privacy_publication_for(SamplePrivacyReleaseSpec::primary())
}

fn sample_privacy_authorization_for(
    spec: SamplePrivacyReleaseSpec,
    publication: &ModerationLedgerCyclePublicationV1,
    encoded: &[u8],
    fencing_token: u64,
    finalized_anchor: Option<SampleFinalizedAnchorSpec>,
) -> PrivacyPublicationAuthorizationV1 {
    let cycle_seconds = spec.cycle_end_unix - spec.cycle_start_unix;
    let window = crate::PrivacyAggregateCycleWindow {
        cycle_start_unix: spec.cycle_start_unix,
        cycle_end_unix: spec.cycle_end_unix,
        due_at_unix: spec.cycle_end_unix,
    };
    let scope = crate::TransparencyLeaderLeaseScopeV1::try_new(spec.query_id, window, [0x95; 32])
        .expect("privacy leader scope");
    assert_eq!(scope.cycle_id(), publication.block.cycle_id);
    let lease_binding = crate::TransparencyRuntimeProviderBindingV1::try_new(
        "hsm:transparency:leader-primary",
        1,
        [0x96; 32],
    )
    .expect("privacy leader provider binding");
    let mut lease_id = [0x97; 32];
    lease_id[..8].copy_from_slice(&fencing_token.to_le_bytes());
    let lease = crate::TransparencyLeaderLeaseGrantV1::try_new(
        lease_id,
        scope,
        fencing_token,
        spec.cycle_end_unix,
        spec.cycle_end_unix + 300,
        lease_binding,
    )
    .expect("privacy leader lease");
    let payload_digest = *blake3::hash(encoded).as_bytes();
    let block_hash = publication
        .block
        .block_hash()
        .expect("privacy publication block hash");
    let release = crate::transparency::PrivacyReleaseRecordV1 {
        sequence: spec.release_sequence,
        release_id: publication.block.cycle_id,
        query_id: spec.query_id,
        first_cycle_start_unix: spec.cycle_start_unix,
        cycle_seconds,
        publish_delay_seconds: 0,
        cycle_start_unix: spec.cycle_start_unix,
        cycle_end_unix: spec.cycle_end_unix,
        due_at_unix: spec.cycle_end_unix,
        private_source_digest: [0x99; 32],
        policy_digest: [0x94; 32],
        population_inventory_digest: [0x9A; 32],
        metric_schema_digest: [0x9B; 32],
        privacy: publication.privacy_aggregates[0].privacy,
        prf_request_binding: Some([0x9C; 32]),
        prf_commitment: Some([0x93; 32]),
        budget_charge_digest: None,
        publication_payload_digest: Some(payload_digest),
        published_aggregate_inventory_digest: Some([0x9D; 32]),
        previous_publication_block_hash: None,
        publication_block_hash: Some(block_hash),
        status: crate::transparency::PrivacyReleaseStatusV1::Published,
        previous_record_digest: None,
        record_digest: spec.release_record_digest,
    };
    let finalized_anchor = finalized_anchor.unwrap_or(SampleFinalizedAnchorSpec {
        sequence: spec.release_sequence,
        release_id: publication.block.cycle_id,
        record_digest: spec.release_record_digest,
        latest_publication_block_hash: Some(block_hash),
    });
    let anchor = crate::PrivacyReleaseAnchorHeadV1::try_from_parts(
        spec.query_id,
        finalized_anchor.sequence,
        finalized_anchor.release_id,
        finalized_anchor.record_digest,
        finalized_anchor.latest_publication_block_hash,
    )
    .expect("privacy finalized anchor");
    PrivacyPublicationAuthorizationV1::try_new(&lease, anchor, &release, payload_digest)
        .expect("privacy publication authorization")
}

fn sample_privacy_authorization(
    publication: &ModerationLedgerCyclePublicationV1,
    encoded: &[u8],
    fencing_token: u64,
) -> PrivacyPublicationAuthorizationV1 {
    sample_privacy_authorization_for(
        SamplePrivacyReleaseSpec::primary(),
        publication,
        encoded,
        fencing_token,
        None,
    )
}

fn sample_fenced_request(
    fencing_token: u64,
    expected_head: Option<FencedTransparencyTargetHeadV1>,
) -> FencedPrivacyPublicationRequestV1 {
    let (publication, encoded) = sample_privacy_publication();
    let authorization = sample_privacy_authorization(&publication, &encoded, fencing_token);
    FencedPrivacyPublicationRequestV1::try_new(
        authorization,
        &publication,
        encoded,
        expected_head,
        expected_head.map_or(0, |head| head.fencing_floor()),
    )
    .expect("fenced privacy request")
}

fn assert_empty_publication_authority(root: &Path) {
    let state = read_publication_state_fixture(root);
    assert_eq!(state.get("generation").and_then(JsonValue::as_u64), Some(0));
    assert_eq!(
        state
            .get("publish_index")
            .and_then(|index| index.get("entries"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        state
            .get("car_queue")
            .and_then(|queue| queue.get("segments"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        fs::read(root.join(GOVERNANCE_PUBLICATION_INITIALIZED_FILE))
            .expect("read publication initialization marker"),
        GOVERNANCE_PUBLICATION_INITIALIZED_BODY
    );
}

fn assert_no_privacy_publication_side_effects(root: &Path) {
    assert!(
        !root.join(GOVERNANCE_PUBLICATION_SOURCES_DIR).exists()
            && !root.join(GOVERNANCE_CAR_SEGMENTS_DIR).exists(),
        "privacy artifacts must remain absent"
    );
    assert_empty_publication_authority(root);
    assert!(
        !root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE).exists(),
        "legacy runtime DAG index must remain absent"
    );
    assert_eq!(
        read_fenced_privacy_head_cache(root).expect("read combined fenced privacy state"),
        None,
        "authoritative-head cache must remain logically absent"
    );
}

fn assert_fenced_privacy_pending_logically_cleared(root: &Path) {
    assert!(
        !fenced_privacy_pending_path(root).exists(),
        "the retired standalone pending journal must remain absent"
    );
    assert_eq!(
        read_fenced_privacy_pending_request(root).expect("read combined fenced privacy state"),
        None
    );
}

struct CanonicalTempDir {
    _inner: TempDir,
    path: PathBuf,
}

impl CanonicalTempDir {
    fn path(&self) -> &Path {
        &self.path
    }
}

fn tempdir() -> std::io::Result<CanonicalTempDir> {
    let inner = tempfile::tempdir()?;
    let path = inner.path().canonicalize()?;
    Ok(CanonicalTempDir {
        _inner: inner,
        path,
    })
}

fn canonical_temp_path(dir: &CanonicalTempDir) -> PathBuf {
    dir.path().to_path_buf()
}

fn sample_settlement() -> (DealSettlementV1, Vec<u8>) {
    let deal_id = [0xAB; 32];
    let provider_id = [0xCD; 32];
    let client_id = [0xEF; 32];
    let mut ledger = DealLedgerSnapshotV1 {
        version: DEAL_LEDGER_VERSION_V1,
        snapshot_id: [0; 32],
        sequence: 1,
        previous_snapshot_id: None,
        deal_id,
        terms_digest: [0xA4; 32],
        provider_id,
        client_id,
        deal_start_epoch: 1_699_999_990,
        deal_end_epoch: 1_699_999_999,
        settlement_window_epochs: 10,
        window_start_epoch: 1_699_999_990,
        window_end_epoch: 1_700_000_000,
        provider_accrual: xor("0.5"),
        client_liability: xor("0.5"),
        micropayment_credit_generated: XorQuantity::zero(),
        micropayment_credit_applied: XorQuantity::zero(),
        micropayment_credit_carry: XorQuantity::zero(),
        client_debit: xor("0.5"),
        outstanding_liability: XorQuantity::zero(),
        bond_total: xor("1"),
        bond_locked: XorQuantity::zero(),
        bond_slashed: XorQuantity::zero(),
        bond_released: xor("1"),
        window_expected_charge: xor("0.5"),
        window_micropayment_generated: XorQuantity::zero(),
        window_micropayment_applied: XorQuantity::zero(),
        window_client_debit: xor("0.5"),
        window_bond_slashed: XorQuantity::zero(),
        window_bond_released: xor("1"),
        captured_at: 1_700_000_000,
    };
    ledger.snapshot_id = ledger.derive_snapshot_id().expect("ledger id");
    let mut settlement = DealSettlementV1 {
        version: DEAL_SETTLEMENT_VERSION_V1,
        settlement_id: [0; 32],
        deal_id,
        ledger,
        status: DealSettlementStatusV1::Completed,
        settled_at: 1_700_000_000,
        audit_notes: None,
    };
    settlement.settlement_id = settlement.derive_settlement_id().expect("settlement id");
    let encoded = norito::to_bytes(&settlement).expect("encode settlement");
    (settlement, encoded)
}

fn sample_por_challenge_publication() -> (PorChallengePublicationV1, Vec<u8>) {
    let manifest_digest = [0x41; 32];
    let provider_id = [0x42; 32];
    let epoch_id = 7;
    let drand_round = 11;
    let drand_randomness = [0x43; 32];
    let seed = derive_challenge_seed(&drand_randomness, None, &manifest_digest, epoch_id);
    let challenge = PorChallengeV1 {
        version: POR_CHALLENGE_VERSION_V1,
        challenge_id: derive_challenge_id(
            &seed,
            &manifest_digest,
            &provider_id,
            epoch_id,
            drand_round,
        ),
        manifest_digest,
        provider_id,
        epoch_id,
        drand_round,
        drand_randomness,
        drand_signature: [0x44; iroha_crypto::drand::DRAND_SIGNATURE_BYTES],
        vrf_output: None,
        vrf_proof: None,
        forced: true,
        chunking_profile: "sorafs.sf1@1.0.0".to_owned(),
        seed,
        sample_tier: 1,
        sample_count: 3,
        sample_indices: vec![5, 5, 9],
        issued_at: 1_800_000_000,
        deadline_at: 1_800_000_900,
    };
    let publication =
        PorChallengePublicationV1::try_new(challenge, 1).expect("challenge publication");
    let encoded = norito::to_bytes(&publication).expect("encode challenge publication");
    (publication, encoded)
}

fn sample_por_weekly_report() -> (PorWeeklyReportV1, Vec<u8>) {
    let report = PorWeeklyReportV1 {
        version: POR_WEEKLY_REPORT_VERSION_V1,
        cycle: PorReportIsoWeek {
            year: 2026,
            week: 30,
        },
        generated_at: 1_800_604_800,
        challenges_total: 3,
        challenges_verified: 2,
        challenges_failed: 1,
        forced_challenges: 1,
        repairs_enqueued: 1,
        repairs_completed: 1,
        mean_latency_ms: Some(75),
        p95_latency_ms: Some(120),
        slashing_events: Vec::new(),
        providers_missing_vrf: vec![[0x42; 32]],
        top_offenders: Vec::new(),
        notes: None,
    };
    report.validate().expect("weekly report");
    let encoded = norito::to_bytes(&report).expect("encode weekly report");
    (report, encoded)
}

fn sample_reputation_snapshot() -> (SignedReputationSnapshotV1, Vec<u8>) {
    let metrics = ReputationProviderMetricsV1 {
        version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
        por_success_bps: 9_800,
        pdp_success_bps: 9_700,
        potr_success_bps: 9_600,
        latency_health_bps: 9_000,
        dispute_rate_bps: 100,
        token_violation_rate_bps: 50,
        repair_breach_rate_bps: 0,
    };
    let input = ReputationProviderInputV1 {
        version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
        provider_id: "provider-a".to_string(),
        metrics,
        reserve_stage: ReputationReserveStageV1::Active,
        previous_score_bps: None,
        active_dispute: false,
        slashing_event: false,
    };
    let inputs = vec![input];
    let snapshot = build_reputation_snapshot(
        [0x42; 16],
        1_800_000_000,
        ReputationWeightsV1::default(),
        &inputs,
        None,
    )
    .expect("reputation snapshot");
    let scoring_evidence = ReputationScoringEvidenceV1 {
        version: REPUTATION_SCORING_EVIDENCE_VERSION_V1,
        provider_inputs: inputs,
        trust_edges: Vec::new(),
    };
    let mut envelope = SignedReputationSnapshotV1 {
        version: SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
        policy_digest: [0xA5; 32],
        snapshot,
        scoring_evidence_digest: scoring_evidence
            .canonical_digest()
            .expect("scoring evidence digest"),
        scoring_evidence,
        signatures: Vec::new(),
    };
    let signing_key = KeyPair::try_from_seed(vec![0x5A; 32], Algorithm::Ed25519)
        .expect("derive reputation signing key");
    let signature = IrohaSignature::try_new(
        signing_key.private_key(),
        &envelope.signing_digest().expect("signing digest"),
    )
    .expect("sign reputation snapshot");
    envelope.signatures.push(ReputationSnapshotSignatureV1 {
        signer_id: "council-1".to_owned(),
        signature: signature
            .payload()
            .try_into()
            .expect("Ed25519 signature is fixed-width"),
    });
    let encoded = envelope
        .canonical_bytes()
        .expect("encode signed reputation snapshot");
    (envelope, encoded)
}

fn sample_moderation_ballot_event() -> (SoraFsModerationBallotGovernanceEventV1, Vec<u8>) {
    let event = SoraFsModerationBallotGovernanceEventV1 {
        version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
        sequence: 6,
        kind: SoraFsModerationBallotGovernanceEventKindV1::BallotTallied,
        generated_at_unix_ms: 1_800_000_030_000,
        case_id: "case-42".to_string(),
        round_id: "round-1".to_string(),
        juror_id: None,
        committed_count: 2,
        revealed_count: 2,
        challenge_count: 0,
        tally: Some(SoraFsModerationBallotGovernanceTallyV1 {
            case_id: "case-42".to_string(),
            round_id: "round-1".to_string(),
            counts: SoraFsModerationVoteCountsV1 {
                uphold: 2,
                overturn: 0,
                modify: 0,
                escalate: 0,
            },
            votes_total: 2,
            quorum: 2,
            winning_choice: Some(SoraFsModerationVoteChoiceV1::Uphold),
            contested: false,
            tallied_at_unix_ms: 1_800_000_030_000,
        }),
        challenge: None,
    };
    let encoded = norito::to_bytes(&event).expect("encode moderation ballot event");
    (event, encoded)
}

fn sample_transparency_ledger_publication() -> (ModerationLedgerCyclePublicationV1, Vec<u8>) {
    use iroha_data_model::sorafs::transparency::{
        MODERATION_LEDGER_ENTRY_VERSION_V1, ModerationLedgerEntryKindV1, ModerationLedgerEntryV1,
        ModerationLedgerMetadataV1,
    };

    let cycle_id = *b"cycle-2026-wk-03";
    let entries = [
        ModerationLedgerEntryV1 {
            version: MODERATION_LEDGER_ENTRY_VERSION_V1,
            cycle_id,
            entry_id: [0x32; 16],
            sequence: 2,
            occurred_at_unix: 1_800_000_032,
            kind: ModerationLedgerEntryKindV1::GarEnforcementReceipt,
            subject: "gar-receipt-32".to_string(),
            subject_digest: [0x32; 32],
            payload_digest: [0x33; 32],
            summary_digest: [0x34; 32],
            policy_digest: Some([0x35; 32]),
            evidence_uris: vec!["sora://transparency/32".to_string()],
            metadata: vec![ModerationLedgerMetadataV1 {
                key: "source".to_string(),
                value: "gar".to_string(),
            }],
        },
        ModerationLedgerEntryV1 {
            version: MODERATION_LEDGER_ENTRY_VERSION_V1,
            cycle_id,
            entry_id: [0x31; 16],
            sequence: 1,
            occurred_at_unix: 1_800_000_031,
            kind: ModerationLedgerEntryKindV1::ModerationAction,
            subject: "moderation-case-31".to_string(),
            subject_digest: [0x31; 32],
            payload_digest: [0x32; 32],
            summary_digest: [0x33; 32],
            policy_digest: Some([0x34; 32]),
            evidence_uris: vec!["sora://transparency/31".to_string()],
            metadata: vec![ModerationLedgerMetadataV1 {
                key: "source".to_string(),
                value: "moderation".to_string(),
            }],
        },
    ];
    let publication = ModerationLedgerCyclePublicationV1::from_entries(
        cycle_id,
        1_800_000_000,
        1_800_604_800,
        1_800_604_801,
        None,
        &entries,
    )
    .expect("transparency ledger publication");
    let encoded = norito::to_bytes(&publication).expect("encode transparency ledger publication");
    (publication, encoded)
}

fn sample_proof_token_issuance() -> (ProofTokenIssuanceV1, Vec<u8>) {
    let issuance = ProofTokenIssuanceV1 {
        version: PROOF_TOKEN_ISSUANCE_VERSION_V1,
        token_id: [0x61; 16],
        issued_at_unix: 1_800_000_030,
        expires_at_unix: Some(1_800_086_430),
        moderation_action_code: 2,
        signer_key: [0x62; 32],
        token_blake3: [0x63; 32],
        blinded_digest: [0x64; 32],
        entry_ids: vec!["denylist/global".to_string(), "gar/policy/42".to_string()],
        evidence_digest: Some([0x65; 32]),
        policy_digest: Some([0x66; 32]),
        metadata: Vec::new(),
    };
    let encoded = norito::to_bytes(&issuance).expect("encode proof-token issuance");
    (issuance, encoded)
}

fn sample_appeal_finance_report() -> (SoraFsAppealFinanceReportV1, Vec<u8>) {
    let report = SoraFsAppealFinanceReportV1 {
        version: SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
        report_id: [0x42; 16],
        case_id: "case-42".to_string(),
        round_id: Some("round-1".to_string()),
        generated_at_unix_ms: 1_800_000_031_000,
        appeal_finance_config_version: "baseline-v1".to_string(),
        evidence_bundle_digest: Some([0xA7; 32]),
        outcome: SoraFsAppealFinanceOutcomeV1::Overturn,
        deposit_xor: xor("420"),
        refund: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "refund-account".to_string(),
            amount_xor: xor("420"),
        },
        treasury: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "treasury-account".to_string(),
            amount_xor: xor("50"),
        },
        held: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "escrow-account".to_string(),
            amount_xor: xor("0"),
        },
        panel_size: 3,
        panel_reward_total_xor: xor("85"),
        rewards_paid_total_xor: xor("60"),
        rewards_forfeited_treasury_xor: xor("25"),
        juror_payouts: vec![
            SoraFsAppealFinanceJurorPayoutV1 {
                juror_id: "juror-a".to_string(),
                stipend_xor: xor("25"),
                bonus_xor: xor("5"),
                total_xor: xor("30"),
            },
            SoraFsAppealFinanceJurorPayoutV1 {
                juror_id: "juror-b".to_string(),
                stipend_xor: xor("25"),
                bonus_xor: xor("5"),
                total_xor: xor("30"),
            },
        ],
        no_show_juror_ids: vec!["juror-c".to_string()],
    };
    let encoded = norito::to_bytes(&report).expect("encode appeal finance report");
    (report, encoded)
}

fn sample_appeal_finance_weekly_rollup() -> (SoraFsAppealFinanceWeeklyRollupV1, Vec<u8>) {
    let (report, _) = sample_appeal_finance_report();
    let rollup = SoraFsAppealFinanceWeeklyRollupV1::from_reports(
        PorReportIsoWeek {
            year: 2026,
            week: 26,
        },
        1_800_000_100_000,
        &[report],
    )
    .expect("appeal finance weekly rollup");
    let encoded = norito::to_bytes(&rollup).expect("encode appeal finance weekly rollup");
    (rollup, encoded)
}

fn sample_appeal_finance_settlement_receipt() -> (SoraFsAppealFinanceSettlementReceiptV1, Vec<u8>) {
    let receipt = SoraFsAppealFinanceSettlementReceiptV1 {
        version: SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
        receipt_id: [0x52; 16],
        case_id: "case-42".to_string(),
        round_id: Some("round-1".to_string()),
        generated_at_unix_ms: 1_800_000_032_000,
        finalized_block_height: 42,
        finalized_block_hash: [0x43; 32],
        appeal_finance_config_version: "baseline-v1".to_string(),
        appeal_finance_policy_digest: [0x44; 32],
        outcome: SoraFsAppealFinanceOutcomeV1::Frivolous,
        escrow_id_hex: "11".repeat(32),
        payer_account: "payer-account".to_string(),
        destination_account: "escrow-account".to_string(),
        release_authority_account: Some("release-authority".to_string()),
        submitted_step: "drawdown_non_refund".to_string(),
        required_authority: "release-authority".to_string(),
        amount_xor: xor("420"),
        tx_hash_hex: "22".repeat(32),
        reconciliation_digest_hex: "33".repeat(32),
        reconciliation_status: "settled".to_string(),
        observed_lifecycle_status: "drawn_down".to_string(),
        observed_remaining_xor: xor("0"),
        deposit_xor: xor("420"),
        refund_xor: xor("0"),
        treasury_xor: xor("210"),
        held_xor: xor("210"),
        panel_size: 7,
        configured_signer_count: 1,
    };
    let encoded = norito::to_bytes(&receipt).expect("encode appeal finance settlement receipt");
    (receipt, encoded)
}

#[test]
fn governance_car_queue_rejects_non_producible_pending_segments() {
    let temp = tempdir().expect("tempdir");
    let root_guard =
        GovernanceFilesystemRootGuard::capture_writer(temp.path()).expect("retain governance root");
    let mut pending = JsonMap::new();
    pending.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_CAR_SEGMENT_SCHEMA),
    );
    pending.insert("status".into(), JsonValue::from("pending"));

    let error = rebuild_car_queue(JsonMap::new(), vec![JsonValue::Object(pending)])
        .expect_err("pending CAR segment must fail closed");
    assert!(error.to_string().contains("non-producible"));
    assert!(!temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE).exists());
    root_guard
        .revalidate()
        .expect("retained root remains valid");
}

fn write_car_segment_source_fixture_for_kind(
    root: &Path,
    payload_kind: &str,
    encoded: &[u8],
) -> PublishIndexEntryForCar {
    let json = br#"{"status":"ready"}"#;
    let encoded_blake3 = blake3::hash(encoded).to_hex().to_string();
    let json_blake3 = blake3::hash(json).to_hex().to_string();
    let (encoded_relative, json_relative) = governance_source_pair_relative_paths(
        payload_kind,
        u64::try_from(encoded.len()).expect("encoded length"),
        &encoded_blake3,
        u64::try_from(json.len()).expect("JSON length"),
        &json_blake3,
    )
    .expect("derive canonical source fixture paths");
    let encoded_path = root.join(&encoded_relative);
    let json_path = root.join(&json_relative);
    fs::create_dir_all(encoded_path.parent().expect("encoded source parent"))
        .expect("create CAR source directory");
    fs::write(&encoded_path, encoded).expect("write encoded CAR source");
    fs::write(&json_path, json).expect("write JSON CAR source");
    for (path, bytes) in [(&encoded_path, encoded), (&json_path, json.as_slice())] {
        let mut digest = blake3::hash(bytes).to_hex().to_string();
        digest.push('\n');
        fs::write(digest_sidecar_path_for(path), digest).expect("write CAR source sidecar");
    }
    PublishIndexEntryForCar {
        position: 0,
        newly_inserted: true,
        payload_kind: payload_kind.to_owned(),
        encoded_path: encoded_relative,
        json_path: json_relative,
        encoded_blake3,
        encoded_len: encoded.len(),
        json_blake3,
        json_len: json.len(),
    }
}

fn write_car_segment_source_fixture(root: &Path, encoded: &[u8]) -> PublishIndexEntryForCar {
    write_car_segment_source_fixture_for_kind(root, "test_payload", encoded)
}

fn publication_artifact_paths_for_fixture(
    root: &Path,
    entry: &PublishIndexEntryForCar,
) -> Vec<PathBuf> {
    let encoded = root.join(&entry.encoded_path);
    let json = root.join(&entry.json_path);
    let base =
        root.join(governance_car_segment_relative_base(entry).expect("derive fixture CAR base"));
    let car = base.with_extension("car");
    let plan = base.with_extension("plan.json");
    let manifest = base.with_extension("json");
    vec![
        encoded.clone(),
        digest_sidecar_path_for(&encoded),
        json.clone(),
        digest_sidecar_path_for(&json),
        car.clone(),
        digest_sidecar_path_for(&car),
        plan.clone(),
        digest_sidecar_path_for(&plan),
        manifest.clone(),
        digest_sidecar_path_for(&manifest),
    ]
}

fn seed_complete_uncommitted_publication_fixture(
    root: &Path,
    payload_kind: &str,
    encoded: &[u8],
    position: usize,
) -> (PublishIndexEntryForCar, Vec<(PathBuf, Vec<u8>)>) {
    let mut entry = write_car_segment_source_fixture_for_kind(root, payload_kind, encoded);
    entry.position = position;
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)
        .expect("retain publication fixture root");
    assemble_governance_car_queue(root, &root_guard, empty_governance_car_queue(), &entry)
        .expect("assemble uncommitted publication fixture");
    drop(root_guard);
    let snapshots = publication_artifact_paths_for_fixture(root, &entry)
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path).expect("snapshot uncommitted publication artifact");
            (path, bytes)
        })
        .collect();
    (entry, snapshots)
}
