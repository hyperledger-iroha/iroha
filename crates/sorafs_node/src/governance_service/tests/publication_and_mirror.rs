use axum::extract::RawQuery;
use std::collections::HashMap;
#[derive(Clone)]
struct IpnsMockState {
    resolutions: Arc<Mutex<VecDeque<String>>>,
    bodies: Arc<HashMap<String, Vec<u8>>>,
    publish_count: Arc<AtomicU64>,
}
fn raw_query_arg(raw: Option<&str>) -> Option<&str> {
    raw?.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == "arg").then_some(value)
    })
}
async fn mock_ipns_resolve(
    State(state): State<IpnsMockState>,
    RawQuery(_raw): RawQuery,
) -> Response {
    let cid = state.resolutions.lock().await.pop_front();
    match cid {
        Some(cid) => test_response(StatusCode::OK, format!(r#"{{"Path":"/ipfs/{cid}"}}"#)),
        None => test_response(StatusCode::NOT_FOUND, "{}"),
    }
}
async fn mock_ipns_publish(State(state): State<IpnsMockState>) -> Response {
    state.publish_count.fetch_add(1, AtomicOrdering::SeqCst);
    test_response(StatusCode::OK, "{}")
}
async fn mock_ipns_cat(State(state): State<IpnsMockState>, RawQuery(raw): RawQuery) -> Response {
    let Some(cid) = raw_query_arg(raw.as_deref()) else {
        return test_response(StatusCode::BAD_REQUEST, Body::empty());
    };
    match state.bodies.get(cid) {
        Some(bytes) => test_response(StatusCode::OK, bytes.clone()),
        None => test_response(StatusCode::NOT_FOUND, Body::empty()),
    }
}
fn mock_ipns_router(state: IpnsMockState) -> Router {
    Router::new()
        .route("/api/v0/name/resolve", post(mock_ipns_resolve))
        .route("/api/v0/name/publish", post(mock_ipns_publish))
        .route("/api/v0/cat", post(mock_ipns_cat))
        .with_state(state)
}
#[derive(Clone)]
struct IpnsResolveFailureState {
    status: StatusCode,
    publish_count: Arc<AtomicU64>,
}
async fn mock_ipns_resolve_failure(State(state): State<IpnsResolveFailureState>) -> Response {
    test_response(state.status, "{}")
}
async fn mock_ipns_publish_after_failure(State(state): State<IpnsResolveFailureState>) -> Response {
    state.publish_count.fetch_add(1, AtomicOrdering::SeqCst);
    test_response(StatusCode::OK, "{}")
}
fn mock_ipns_resolve_failure_router(state: IpnsResolveFailureState) -> Router {
    Router::new()
        .route("/api/v0/name/resolve", post(mock_ipns_resolve_failure))
        .route(
            "/api/v0/name/publish",
            post(mock_ipns_publish_after_failure),
        )
        .with_state(state)
}
#[tokio::test]
async fn ipns_publication_rejects_pre_post_movement_and_readback_drift() {
    let initial = PublicHead::Present {
        bytes: b"old".to_vec(),
        token: TEST_CID_OLD.to_owned(),
    };
    let cases = [
        (
            VecDeque::from([TEST_CID_ATTACKER.to_owned()]),
            HashMap::from([(TEST_CID_ATTACKER.to_owned(), b"attacker".to_vec())]),
        ),
        (
            VecDeque::from([TEST_CID_OLD.to_owned(), TEST_CID_ATTACKER.to_owned()]),
            HashMap::from([
                (TEST_CID_OLD.to_owned(), b"old".to_vec()),
                (TEST_CID_ATTACKER.to_owned(), b"attacker".to_vec()),
            ]),
        ),
        (
            VecDeque::from([TEST_CID_OLD.to_owned(), TEST_CID_NEW.to_owned()]),
            HashMap::from([
                (TEST_CID_OLD.to_owned(), b"old".to_vec()),
                (TEST_CID_NEW.to_owned(), b"wrong".to_vec()),
            ]),
        ),
    ];
    for (resolutions, bodies) in cases {
        let state = IpnsMockState {
            resolutions: Arc::new(Mutex::new(resolutions)),
            bodies: Arc::new(bodies),
            publish_count: Arc::new(AtomicU64::new(0)),
        };
        let (endpoint, task) = spawn_router(mock_ipns_router(state), "/").await;
        assert!(
            publish_ipns_head(
                &endpoint,
                IpnsHeadPublishRequest {
                    name: "test-name",
                    key_name: "test-key",
                    head_cid: TEST_CID_NEW,
                    bytes: b"new",
                    initial: &initial,
                    allow_bootstrap: false,
                    max_response_bytes: 1024,
                },
            )
            .await
            .is_err()
        );
        task.abort();
    }
}
#[test]
fn ipns_absence_profile_is_narrow_and_exact() {
    assert!(is_authenticated_ipns_absence(StatusCode::NOT_FOUND, b"{}"));
    assert!(is_authenticated_ipns_absence(
        StatusCode::INTERNAL_SERVER_ERROR,
        br#"{"Message":"could not resolve name","Code":0,"Type":"error"}"#
    ));
    assert!(!is_authenticated_ipns_absence(
        StatusCode::INTERNAL_SERVER_ERROR,
        br#"{"Message":"routing unavailable","Code":0,"Type":"error"}"#
    ));
    assert!(!is_authenticated_ipns_absence(
        StatusCode::INTERNAL_SERVER_ERROR,
        br#"{"Message":"could not resolve name","Code":0,"Type":"error","Retry":true}"#
    ));
    assert!(!is_authenticated_ipns_absence(
        StatusCode::TOO_MANY_REQUESTS,
        br#"{"Message":"could not resolve name","Code":0,"Type":"error"}"#
    ));
}
#[tokio::test]
async fn ipns_resolution_errors_never_authorize_bootstrap_publication() {
    for status in [
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
        StatusCode::TOO_MANY_REQUESTS,
        StatusCode::INTERNAL_SERVER_ERROR,
        StatusCode::SERVICE_UNAVAILABLE,
    ] {
        let publish_count = Arc::new(AtomicU64::new(0));
        let state = IpnsResolveFailureState {
            status,
            publish_count: publish_count.clone(),
        };
        let (endpoint, task) = spawn_router(mock_ipns_resolve_failure_router(state), "/").await;
        let error = publish_ipns_head(
            &endpoint,
            IpnsHeadPublishRequest {
                name: "test-name",
                key_name: "test-key",
                head_cid: TEST_CID_NEW,
                bytes: b"new",
                initial: &PublicHead::Missing,
                allow_bootstrap: true,
                max_response_bytes: 1024,
            },
        )
        .await
        .expect_err("authenticated resolver failure must fail closed");
        assert!(error.to_string().contains(status.as_str()));
        assert_eq!(
            publish_count.load(AtomicOrdering::SeqCst),
            0,
            "resolver failure must not be reclassified as authenticated absence"
        );
        task.abort();
    }
}
#[tokio::test]
async fn service_rejects_in_process_sealed_checkpoint_rollback() {
    let root = secure_temp_dir();
    let view = runtime_boundary_view(root.path());
    let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let store = test_checkpoint_store(provider.clone());
    let source = signed_source(1, 0x6A, 1_800_000_000);
    let mut checkpoint = checkpoint_from_source(&source);
    checkpoint.generation = 2;
    save_checkpoint(&store, None, &checkpoint).expect("seed generation-two checkpoint");
    let mut service = Service::from_view(
        view.clone(),
        test_runtime_providers(&view, provider.clone()),
    )
    .await
    .expect("initialize service at generation two");
    let original_record = provider
        .load(GovernanceDagSealedStateSlot::Checkpoint)
        .expect("load original checkpoint record")
        .expect("original checkpoint exists");
    let mut equivocated = checkpoint.clone();
    equivocated.published_at_unix = equivocated.published_at_unix.saturating_add(1);
    provider
        .inner
        .lock()
        .expect("lock test checkpoint store")
        .checkpoint = Some(GovernanceDagSealedStateRecord::new(
        GovernanceDagSealedStateSlot::Checkpoint,
        equivocated.generation,
        norito::to_bytes(&equivocated).expect("encode same-generation equivocation"),
    ));
    let error = service
        .reconcile_once()
        .await
        .expect_err("same-generation checkpoint rewrite must fail closed");
    assert!(error.to_string().contains("equivocated"));
    provider
        .inner
        .lock()
        .expect("restore original checkpoint")
        .checkpoint = Some(original_record);
    let mut rolled_back = checkpoint;
    rolled_back.generation = 1;
    let payload = norito::to_bytes(&rolled_back).expect("encode rollback checkpoint");
    let record =
        GovernanceDagSealedStateRecord::new(GovernanceDagSealedStateSlot::Checkpoint, 1, payload);
    provider
        .inner
        .lock()
        .expect("lock test checkpoint store")
        .checkpoint = Some(record);
    let error = service
        .reconcile_once()
        .await
        .expect_err("checkpoint rollback must fail before source/network work");
    assert!(error.to_string().contains("rolled back"));
    provider
        .inner
        .lock()
        .expect("lock test checkpoint store")
        .checkpoint = None;
    let error = service
        .reconcile_once()
        .await
        .expect_err("checkpoint deletion must fail before source/network work");
    assert!(error.to_string().contains("removed the active checkpoint"));
}
#[tokio::test]
async fn dropping_service_withdraws_every_retained_mirror_reader() {
    let root = secure_temp_dir();
    let view = runtime_boundary_view(root.path());
    let service = Service::from_view(
        view.clone(),
        test_runtime_providers(
            &view,
            Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE)),
        ),
    )
    .await
    .expect("construct supervised service");
    let reader = service.mirror_reader.clone();
    reader.mark_ready();
    drop(service);
    let error = reader
        .read()
        .expect_err("a reader retained past service shutdown must be unavailable");
    assert!(matches!(error, GovernanceDagServiceError::Unavailable(_)));
}
#[tokio::test]
async fn pinned_endpoint_rejects_requests_outside_qualified_url_boundary() {
    let ipfs_provider = Arc::new(TestAuthenticator::new(
        TEST_IPFS_AUTH_HANDLE,
        "qualified-url-ipfs",
    ));
    let ipfs_router = Router::new().route("/api/health", get(|| async { "ok" }));
    let (ipfs, ipfs_task) = spawn_router_with_authenticator(
        ipfs_router,
        "/api/",
        GovernanceDagAuthenticationScope::Ipfs,
        ipfs_provider,
    )
    .await;
    let allowed = ipfs.url.join("v0/add").expect("same-prefix Kubo URL");
    assert!(ipfs.request(Method::POST, allowed).is_ok());
    let sibling = ipfs
        .url
        .join("/api-shadow/v0/add")
        .expect("same-origin sibling URL");
    assert!(ipfs.request(Method::POST, sibling.clone()).is_err());
    let bypass = ipfs.client.request(Method::POST, sibling);
    let error = ipfs
        .execute(bypass, "unqualified URL test request failed")
        .await
        .err()
        .expect("execute must recheck a builder created outside PinnedEndpoint::request");
    assert!(error.to_string().contains("qualified ingress endpoint"));
    let cross_origin =
        Url::parse("http://example.com/api/v0/add").expect("canonical cross-origin test URL");
    assert!(ipfs.request(Method::POST, cross_origin).is_err());
    let encoded_separator = ipfs
        .url
        .join("v0/%2F..%2Fadmin")
        .expect("encoded-separator test URL");
    assert!(ipfs.request(Method::POST, encoded_separator).is_err());
    ipfs_task.abort();
    let head_provider = Arc::new(TestAuthenticator::new(
        TEST_HEAD_AUTH_HANDLE,
        "qualified-url-head",
    ));
    let head_router = Router::new().route("/head", get(|| async { "head" }));
    let (head, head_task) = spawn_router_with_authenticator(
        head_router,
        "/head",
        GovernanceDagAuthenticationScope::SignedHead,
        head_provider,
    )
    .await;
    assert!(head.request(Method::GET, head.url.clone()).is_ok());
    let mut altered_head = head.url.clone();
    altered_head.set_query(Some("generation=1"));
    assert!(head.request(Method::GET, altered_head).is_err());
    head_task.abort();
}
#[tokio::test]
async fn hardened_http_refuses_redirect_header_body_and_encoding_attacks() {
    let redirect_router = Router::new()
        .route(
            "/redirect",
            get(|| async { Redirect::temporary("/target") }),
        )
        .route("/target", get(|| async { "followed" }));
    let (redirect, redirect_task) = spawn_router(redirect_router, "/").await;
    let mut redirect_url = redirect.url.clone();
    redirect_url.set_path("/redirect");
    let request = redirect
        .request(Method::GET, redirect_url)
        .expect("build redirect request");
    let response = redirect
        .execute(request, "redirect test request failed")
        .await
        .expect("receive redirect response");
    assert!(response.status().is_redirection());
    redirect_task.abort();
    let router = Router::new()
        .route("/headers", get(response_header_bomb))
        .route("/body", get(response_body_bomb))
        .route("/gzip", get(response_gzip));
    let (endpoint, task) = spawn_router(router, "/").await;
    let mut headers_url = endpoint.url.clone();
    headers_url.set_path("/headers");
    let request = endpoint
        .request(Method::GET, headers_url)
        .expect("build header request");
    let response = endpoint
        .execute(request, "header-bound test request failed")
        .await
        .expect("receive header response");
    assert!(read_bounded_response(response, 1024).await.is_err());
    let mut body_url = endpoint.url.clone();
    body_url.set_path("/body");
    let request = endpoint
        .request(Method::GET, body_url)
        .expect("build body request");
    let response = endpoint
        .execute(request, "body-bound test request failed")
        .await
        .expect("receive body response");
    assert!(read_bounded_response(response, 16).await.is_err());
    let mut gzip_url = endpoint.url.clone();
    gzip_url.set_path("/gzip");
    let request = endpoint
        .request(Method::GET, gzip_url)
        .expect("build gzip request");
    let response = endpoint
        .execute(request, "encoding test request failed")
        .await
        .expect("receive gzip response");
    assert!(read_bounded_response(response, 16).await.is_err());
    task.abort();
}
#[tokio::test]
async fn ipfs_publication_rejects_malformed_cid_missing_pin_and_wrong_readback() {
    let cases = [
        MockIpfsState {
            add_body: Arc::new(b"not-json".to_vec()),
            cat_body: Arc::new(b"payload".to_vec()),
            pin_present: true,
        },
        MockIpfsState {
            add_body: Arc::new(br#"{"Hash":"bad/cid"}"#.to_vec()),
            cat_body: Arc::new(b"payload".to_vec()),
            pin_present: true,
        },
        MockIpfsState {
            add_body: Arc::new(format!(r#"{{"Hash":"{TEST_CID_PAYLOAD}"}}"#).into_bytes()),
            cat_body: Arc::new(b"payload".to_vec()),
            pin_present: false,
        },
        MockIpfsState {
            add_body: Arc::new(format!(r#"{{"Hash":"{TEST_CID_BLOCK}"}}"#).into_bytes()),
            cat_body: Arc::new(b"payload".to_vec()),
            pin_present: true,
        },
        MockIpfsState {
            add_body: Arc::new(format!(r#"{{"Hash":"{TEST_CID_PAYLOAD}"}}"#).into_bytes()),
            cat_body: Arc::new(b"different".to_vec()),
            pin_present: true,
        },
    ];
    for state in cases {
        let (endpoint, task) = spawn_router(mock_ipfs_router(state), "/").await;
        let result = ipfs_add_verified(&endpoint, "block.to", b"payload", 1024, 1024).await;
        assert!(result.is_err());
        task.abort();
    }
    let valid = MockIpfsState {
        add_body: Arc::new(format!(r#"{{"Hash":"{TEST_CID_PAYLOAD}"}}"#).into_bytes()),
        cat_body: Arc::new(b"payload".to_vec()),
        pin_present: true,
    };
    let (endpoint, task) = spawn_router(mock_ipfs_router(valid), "/").await;
    assert_eq!(
        ipfs_add_verified(&endpoint, "block.to", b"payload", 1024, 1024)
            .await
            .expect("valid mock IPFS publication"),
        TEST_CID_PAYLOAD
    );
    task.abort();
    let large = Arc::new(vec![0xA5; 5 * IPFS_UNIXFS_CHUNK_BYTES + 7]);
    let large_cid =
        canonical_ipfs_file_cid(&large).expect("large test object fits the fixed UnixFS profile");
    let valid_large = MockIpfsState {
        add_body: Arc::new(format!(r#"{{"Hash":"{large_cid}"}}"#).into_bytes()),
        cat_body: large.clone(),
        pin_present: true,
    };
    let (endpoint, task) = spawn_router(mock_ipfs_router(valid_large), "/").await;
    assert_eq!(
        ipfs_add_verified(
            &endpoint,
            "large-block.to",
            &large,
            large.len() as u64,
            1024,
        )
        .await
        .expect("multi-chunk publication ignores the control-response cap for CAT"),
        large_cid
    );
    task.abort();
}
fn ipip_499_chacha20_bytes(seed: &[u8], length: usize) -> Vec<u8> {
    let key = iroha_crypto::sha256(seed);
    let mut initial = [0_u32; 16];
    initial[..4].copy_from_slice(&[0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574]);
    for (word, bytes) in initial[4..12].iter_mut().zip(key.chunks_exact(4)) {
        *word = u32::from_le_bytes(bytes.try_into().expect("SHA-256 word"));
    }
    let mut output = vec![0_u8; length];
    for (counter, output_block) in output.chunks_mut(64).enumerate() {
        initial[12] = u32::try_from(counter).expect("test stream counter fits u32");
        let mut state = initial;
        for _ in 0..10 {
            chacha20_quarter_round(&mut state, 0, 4, 8, 12);
            chacha20_quarter_round(&mut state, 1, 5, 9, 13);
            chacha20_quarter_round(&mut state, 2, 6, 10, 14);
            chacha20_quarter_round(&mut state, 3, 7, 11, 15);
            chacha20_quarter_round(&mut state, 0, 5, 10, 15);
            chacha20_quarter_round(&mut state, 1, 6, 11, 12);
            chacha20_quarter_round(&mut state, 2, 7, 8, 13);
            chacha20_quarter_round(&mut state, 3, 4, 9, 14);
        }
        for (word, original) in state.iter_mut().zip(initial) {
            *word = word.wrapping_add(original);
        }
        let mut encoded = [0_u8; 64];
        for (slot, word) in encoded.chunks_exact_mut(4).zip(state) {
            slot.copy_from_slice(&word.to_le_bytes());
        }
        output_block.copy_from_slice(&encoded[..output_block.len()]);
    }
    output
}
fn chacha20_quarter_round(
    state: &mut [u32; 16],
    a_index: usize,
    b_index: usize,
    c_index: usize,
    d_index: usize,
) {
    let mut a = state[a_index];
    let mut b = state[b_index];
    let mut c = state[c_index];
    let mut d = state[d_index];
    a = a.wrapping_add(b);
    d = (d ^ a).rotate_left(16);
    c = c.wrapping_add(d);
    b = (b ^ c).rotate_left(12);
    a = a.wrapping_add(b);
    d = (d ^ a).rotate_left(8);
    c = c.wrapping_add(d);
    b = (b ^ c).rotate_left(7);
    state[a_index] = a;
    state[b_index] = b;
    state[c_index] = c;
    state[d_index] = d;
}
#[test]
fn fixed_unixfs_profile_matches_ipip_499_chunk_boundary_vectors() {
    const SMALL_CID: &str = "bafkreifzjut3te2nhyekklss27nh3k72ysco7y32koao5eei66wof36n5e";
    const AT_CHUNK_CID: &str = "bafkreiacndfy443ter6qr2tmbbdhadvxxheowwf75s6zehscklu6ezxmta";
    const OVER_CHUNK_CID: &str = "bafybeigmix7t42i6jacydtquhet7srwvgpizfg7gjbq7627d35mjomtu64";
    assert_eq!(
        canonical_ipfs_file_cid(b"hello world").as_deref(),
        Some(SMALL_CID)
    );
    let bytes = ipip_499_chacha20_bytes(b"chunk-v1-seed", IPFS_UNIXFS_CHUNK_BYTES + 1);
    assert_eq!(
        canonical_ipfs_file_cid(&bytes[..IPFS_UNIXFS_CHUNK_BYTES]).as_deref(),
        Some(AT_CHUNK_CID)
    );
    assert_eq!(
        canonical_ipfs_file_cid(&bytes).as_deref(),
        Some(OVER_CHUNK_CID)
    );
    let mut tampered = bytes;
    *tampered.last_mut().expect("non-empty fixture") ^= 1;
    assert!(validate_ipfs_cid_for_bytes(OVER_CHUNK_CID, &tampered).is_err());
}
#[test]
fn canonical_ipfs_cid_is_derived_from_exact_payload_bytes() {
    assert_eq!(canonical_raw_sha256_cid(b"payload"), TEST_CID_PAYLOAD);
    assert_eq!(
        validate_ipfs_cid_for_bytes(TEST_CID_PAYLOAD, b"payload")
            .expect("canonical CID commits to the exact bytes"),
        TEST_CID_PAYLOAD
    );
    assert!(
        validate_ipfs_cid_for_bytes(TEST_CID_PAYLOAD, b"payload-tampered").is_err(),
        "a canonical but substituted CID must not authenticate different bytes"
    );
}
#[test]
fn ipfs_multipart_body_is_deterministic_bounded_and_cloneable() {
    let payload = b"\0payload\r\n";
    let name = "governance-head.to";
    let (boundary, body) =
        canonical_ipfs_multipart_body(name, payload).expect("construct canonical multipart body");
    let (replayed_boundary, replayed_body) =
        canonical_ipfs_multipart_body(name, payload).expect("replay canonical multipart body");
    assert_eq!(boundary, replayed_boundary);
    assert_eq!(body, replayed_body);
    assert_eq!(
        body.len(),
        payload.len()
            + ipfs_multipart_wire_overhead(boundary.len(), name.len())
                .expect("exact multipart framing overhead")
    );
    assert!(boundary.len() <= 70);
    assert!(body.starts_with(format!("--{boundary}\r\n").as_bytes()));
    assert!(body.ends_with(format!("\r\n--{boundary}--\r\n").as_bytes()));
    assert!(
        body.windows(b"\0payload\r\n".len())
            .any(|window| window == b"\0payload\r\n")
    );
    assert!(canonical_ipfs_multipart_body("../escape", b"payload").is_err());
    let object_max = payload.len() as u64;
    let authenticated_wire_max = authenticated_ipfs_wire_body_max_bytes(object_max)
        .expect("derive authenticated multipart wire bound");
    let max_boundary_len = IPFS_MULTIPART_BOUNDARY_PREFIX.len() + 1 + 32 + 3;
    let max_overhead =
        ipfs_multipart_wire_overhead(max_boundary_len, IPFS_MULTIPART_FILENAME_MAX_BYTES)
            .expect("derive maximum multipart framing overhead") as u64;
    assert_eq!(authenticated_wire_max - object_max, max_overhead);
    assert!(body.len() as u64 > object_max);
    assert!(body.len() as u64 <= authenticated_wire_max);
    assert!(authenticated_ipfs_wire_body_max_bytes(u64::MAX).is_err());
    let request = Client::new()
        .post("https://example.invalid/api/v0/add")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body.clone());
    assert!(
        request.try_clone().is_some(),
        "the final multipart request must remain inspectable by the authenticator"
    );
    let request = request.build().expect("finalize multipart request");
    assert!(
        canonical_outbound_request_descriptor(
            &request,
            GovernanceDagAuthenticationScope::Ipfs,
            object_max,
        )
        .is_err(),
        "the object ceiling must not be reused as the multipart wire ceiling"
    );
    canonical_outbound_request_descriptor(
        &request,
        GovernanceDagAuthenticationScope::Ipfs,
        authenticated_wire_max,
    )
    .expect("the checked multipart wire ceiling admits the exact final body");
}
#[tokio::test]
async fn signed_head_authenticator_receives_final_body_and_cas_headers() {
    let cases = [
        (
            SignedHeadInner {
                bytes: Some(b"old".to_vec()),
                etag: "\"v1\"".to_owned(),
                ..SignedHeadInner::default()
            },
            PublicHead::Present {
                bytes: b"old".to_vec(),
                token: "\"v1\"".to_owned(),
            },
            header::IF_MATCH,
            HeaderValue::from_static("\"v1\""),
            false,
        ),
        (
            SignedHeadInner::default(),
            PublicHead::Missing,
            header::IF_NONE_MATCH,
            HeaderValue::from_static("*"),
            true,
        ),
    ];
    for (inner, current, condition, condition_value, allow_bootstrap) in cases {
        let provider = Arc::new(FinalRequestAuthenticator::new(
            b"new",
            condition,
            condition_value,
        ));
        let (endpoint, _state, task) =
            spawn_signed_head_with_authenticator(inner, provider.clone()).await;
        let installed = put_signed_http_head(&endpoint, b"new", &current, allow_bootstrap, 1024)
            .await
            .expect("authenticate and install the final conditional request");
        assert!(matches!(
            installed,
            PublicHead::Present { bytes, .. } if bytes == b"new"
        ));
        assert!(
            provider.observed_put.load(AtomicOrdering::SeqCst),
            "authenticator must observe the body and conditional headers before execution"
        );
        task.abort();
    }
}
#[tokio::test]
async fn signed_head_cas_rejects_conflict_bootstrap_and_readback_drift() {
    for status in [StatusCode::CONFLICT, StatusCode::PRECONDITION_FAILED] {
        let (endpoint, _state, task) = spawn_signed_head(SignedHeadInner {
            bytes: Some(b"old".to_vec()),
            etag: "\"v1\"".to_owned(),
            put_status: Some(status),
            ..SignedHeadInner::default()
        })
        .await;
        let current = PublicHead::Present {
            bytes: b"old".to_vec(),
            token: "\"v1\"".to_owned(),
        };
        assert!(
            put_signed_http_head(&endpoint, b"new", &current, false, 1024)
                .await
                .is_err()
        );
        task.abort();
    }
    let (endpoint, state, task) = spawn_signed_head(SignedHeadInner::default()).await;
    assert!(
        put_signed_http_head(&endpoint, b"new", &PublicHead::Missing, false, 1024)
            .await
            .is_err()
    );
    assert_eq!(state.0.lock().await.put_count, 0);
    task.abort();
    let (endpoint, _state, task) = spawn_signed_head(SignedHeadInner {
        bytes: Some(b"old".to_vec()),
        etag: "\"v1\"".to_owned(),
        readback_override: Some(b"attacker".to_vec()),
        ..SignedHeadInner::default()
    })
    .await;
    let current = PublicHead::Present {
        bytes: b"old".to_vec(),
        token: "\"v1\"".to_owned(),
    };
    assert!(
        put_signed_http_head(&endpoint, b"new", &current, false, 1024)
            .await
            .is_err()
    );
    task.abort();
}
#[tokio::test]
async fn signed_head_read_rejects_duplicate_entity_tags() {
    let (endpoint, _state, task) = spawn_signed_head(SignedHeadInner {
        bytes: Some(b"head".to_vec()),
        etag: "\"v1\"".to_owned(),
        duplicate_etag: true,
        ..SignedHeadInner::default()
    })
    .await;
    let error = fetch_signed_http_head(&endpoint, 1024)
        .await
        .expect_err("multiple ETag fields must not define an ambiguous CAS token");
    assert!(error.to_string().contains("single canonical strong ETag"));
    task.abort();
}
#[test]
fn mirror_index_exposes_only_signed_submission_provenance() {
    let source = signed_finance_source(0x39, 1_800_000_000);
    let checkpoint = checkpoint_from_source(&source);
    let mirror = mirror_index_value(
        &source,
        &checkpoint.mirror_blocks,
        &checkpoint.archive_head,
        checkpoint.generation,
        &checkpoint.head_ipfs_cid,
        checkpoint.published_at_unix,
    )
    .expect("build attributed mirror index");
    let entry = mirror
        .get("blocks")
        .and_then(JsonValue::as_array)
        .and_then(|blocks| blocks.first())
        .expect("attributed mirror block");
    let signed = source.blocks[0]
        .block
        .node
        .submission_provenance
        .as_ref()
        .expect("signed submission provenance");
    assert_eq!(
        entry
            .get("submission_publisher_account_digest_hex")
            .and_then(JsonValue::as_str),
        Some(hex::encode(signed.publisher_account_digest).as_str())
    );
    assert_eq!(
        entry.get("submission_origin").and_then(JsonValue::as_str),
        Some(signed.origin.label())
    );
    let internal_source = signed_source(1, 0x38, 1_800_000_000);
    let internal_checkpoint = checkpoint_from_source(&internal_source);
    let internal_mirror = mirror_index_value(
        &internal_source,
        &internal_checkpoint.mirror_blocks,
        &internal_checkpoint.archive_head,
        internal_checkpoint.generation,
        &internal_checkpoint.head_ipfs_cid,
        internal_checkpoint.published_at_unix,
    )
    .expect("build internal-producer mirror index");
    let internal_entry = internal_mirror
        .get("blocks")
        .and_then(JsonValue::as_array)
        .and_then(|blocks| blocks.first())
        .expect("internal mirror block");
    assert_eq!(
        internal_entry.get("submission_publisher_account_digest_hex"),
        Some(&JsonValue::Null)
    );
    assert_eq!(
        internal_entry.get("submission_origin"),
        Some(&JsonValue::Null)
    );
}
#[test]
fn mirror_two_slot_store_hard_cut_rejects_legacy_authority_without_cleanup() {
    for legacy_name in [
        LEGACY_MIRROR_INDEX_FILE,
        LEGACY_MIRROR_INDEX_SIDECAR_FILE,
        LEGACY_MIRROR_RECOVERY_QUARANTINE_DIR,
        ".governance-service-recovery-quarantine-v1.tmp-bad",
        ".governance-service-recovery-quarantine-v1.retained-v1-0000",
        ".governance-service-recovery-quarantine-v1.retained-v1-bad",
        "..governance-service-recovery-quarantine-v1.tmp-bad",
        "..governance-service-recovery-quarantine-v1.retained-v1-bad",
        ".mirror-index.json.tmp-42000-1",
        ".mirror-index.json.tmp-bad",
        ".mirror-index.json.retained-v1-0000",
        ".mirror-index.json.retained-v1-bad",
        ".mirror-index.json.blake3.tmp-42000-2",
        ".mirror-index.json.blake3.tmp-bad",
        ".mirror-index.json.blake3.retained-v1-0000",
        ".mirror-index.json.blake3.retained-v1-bad",
    ] {
        let dir = secure_temp_dir();
        let source = signed_source(2, 0x3a, 1_800_000_000);
        let config = test_runtime_config(&source, dir.path());
        let legacy_path = config.state_root_guard.root().join(legacy_name);
        fs::write(&legacy_path, b"legacy-sentinel-must-remain")
            .expect("seed retired mirror authority");
        let error =
            open_mirror_index_store(&config).expect_err("legacy mirror authority must fail closed");
        assert!(
            error.to_string().contains("legacy mirror authority"),
            "unexpected error for `{legacy_name}`: {error}"
        );
        assert_eq!(
            fs::read(&legacy_path).expect("read preserved legacy mirror authority"),
            b"legacy-sentinel-must-remain"
        );
        assert!(
            !config
                .state_root_guard
                .root()
                .join(MIRROR_INDEX_STORE_NAME)
                .exists(),
            "legacy rejection must happen before typed-store initialization"
        );
    }
}
#[test]
fn mirror_two_slot_payload_rejects_truncation_and_metadata_drift() {
    let dir = secure_temp_dir();
    let source = signed_source(2, 0x3b, 1_800_000_000);
    let config = test_runtime_config(&source, dir.path());
    let store = open_mirror_index_store(&config).expect("open mirror two-slot store");
    let mut checkpoint = checkpoint_from_source(&source);
    checkpoint.generation = 2;
    let mirror = mirror_index_value(
        &source,
        &checkpoint.mirror_blocks,
        &checkpoint.archive_head,
        checkpoint.generation,
        &checkpoint.head_ipfs_cid,
        checkpoint.published_at_unix,
    )
    .expect("build test mirror");
    let canonical = json::to_json_pretty(&mirror)
        .expect("encode test mirror")
        .into_bytes();
    checkpoint.mirror_blake3 = blake3_array(&canonical);
    let recovered = verify_or_recover_mirror_index_store(&config, &store, &checkpoint, &source)
        .expect("empty hard-cut store recovers from checkpoint");
    assert_eq!(recovered, mirror);
    let payload = MirrorIndexStorePayloadV1::committed(checkpoint.generation, [0; 32], canonical)
        .expect("construct canonical typed mirror");
    let encoded = encode_mirror_index_store_payload(&payload).expect("encode typed mirror");
    assert!(
        decode_mirror_index_store_payload(&encoded[..encoded.len() / 2]).is_err(),
        "a truncated typed payload must fail closed"
    );
    for (field, replacement) in [
        ("schema", JsonValue::from("wrong.schema")),
        ("generation", JsonValue::from(99_u64)),
    ] {
        let mut drifted = mirror.clone();
        drifted
            .as_object_mut()
            .expect("mirror object")
            .insert(field.into(), replacement);
        let bytes = json::to_json_pretty(&drifted)
            .expect("encode drifted mirror")
            .into_bytes();
        assert!(
            MirrorIndexStorePayloadV1::committed(checkpoint.generation, [0; 32], bytes).is_err(),
            "typed metadata must reject {field} drift"
        );
    }
    let mut head_drift = mirror.clone();
    head_drift
        .get_mut("head")
        .and_then(JsonValue::as_object_mut)
        .expect("head object")
        .insert(
            "head_block_cid_hex".into(),
            JsonValue::from("00".repeat(32)),
        );
    let head_drift_bytes = json::to_json_pretty(&head_drift)
        .expect("encode head-drifted mirror")
        .into_bytes();
    let head_drift_payload =
        MirrorIndexStorePayloadV1::committed(checkpoint.generation, [0; 32], head_drift_bytes)
            .expect("head drift remains internally canonical");
    let (snapshot, _) = load_mirror_index_store(&config, &store).expect("load typed mirror");
    compare_and_swap_mirror_index_store(&config, &store, &snapshot, &head_drift_payload)
        .expect("install internally canonical drift for verification test");
    let mut matching_digest_checkpoint = checkpoint.clone();
    matching_digest_checkpoint.mirror_blake3 = head_drift_payload.mirror_blake3;
    assert!(verify_mirror_index_store(&config, &store, &matching_digest_checkpoint).is_err());
    let repaired = verify_or_recover_mirror_index_store(&config, &store, &checkpoint, &source)
        .expect("checkpoint authority repairs a same-generation derived-cache drift");
    assert_eq!(repaired, mirror);
    let mut stale_mirror = mirror.clone();
    stale_mirror
        .as_object_mut()
        .expect("stale mirror object")
        .insert("generation".into(), JsonValue::from(1_u64));
    let stale_bytes = json::to_json_pretty(&stale_mirror)
        .expect("encode stale mirror")
        .into_bytes();
    let stale_payload = MirrorIndexStorePayloadV1::committed(1, [0; 32], stale_bytes)
        .expect("construct prior-generation derived mirror");
    let (snapshot, _) = load_mirror_index_store(&config, &store).expect("load repaired mirror");
    compare_and_swap_mirror_index_store(&config, &store, &snapshot, &stale_payload)
        .expect("represent an offline instance at the preceding local generation");
    assert_eq!(
        verify_or_recover_mirror_index_store(&config, &store, &checkpoint, &source)
            .expect("offline local mirror catches up from the authoritative checkpoint"),
        mirror
    );
    let mut ahead_mirror = mirror.clone();
    ahead_mirror
        .as_object_mut()
        .expect("ahead mirror object")
        .insert("generation".into(), JsonValue::from(3_u64));
    let ahead_bytes = json::to_json_pretty(&ahead_mirror)
        .expect("encode ahead mirror")
        .into_bytes();
    let ahead_payload = MirrorIndexStorePayloadV1::committed(3, [0; 32], ahead_bytes)
        .expect("construct ahead-generation derived mirror");
    let (snapshot, _) = load_mirror_index_store(&config, &store).expect("load caught-up mirror");
    compare_and_swap_mirror_index_store(&config, &store, &snapshot, &ahead_payload)
        .expect("represent a local generation ahead of authority");
    assert!(
        verify_or_recover_mirror_index_store(&config, &store, &checkpoint, &source).is_err(),
        "a local mirror ahead of sealed authority must fail closed"
    );
}
#[test]
fn mirror_read_handle_returns_only_checkpoint_coherent_typed_bytes() {
    let dir = secure_temp_dir();
    let source = signed_source(2, 0x3d, 1_800_000_000);
    let config = test_runtime_config(&source, dir.path());
    let mirror_store = open_mirror_index_store(&config).expect("open mirror store");
    let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let checkpoint_store = test_checkpoint_store(Arc::clone(&provider));
    let mut checkpoint = checkpoint_from_source(&source);
    let mirror = mirror_index_value(
        &source,
        &checkpoint.mirror_blocks,
        &checkpoint.archive_head,
        checkpoint.generation,
        &checkpoint.head_ipfs_cid,
        checkpoint.published_at_unix,
    )
    .expect("build mirror value");
    let canonical_bytes = json::to_json_pretty(&mirror)
        .expect("encode canonical mirror")
        .into_bytes();
    checkpoint.mirror_blake3 = blake3_array(&canonical_bytes);
    let (empty_snapshot, _) =
        load_mirror_index_store(&config, &mirror_store).expect("load empty mirror store");
    let committed_payload = MirrorIndexStorePayloadV1::committed(
        checkpoint.generation,
        [0; 32],
        canonical_bytes.clone(),
    )
    .expect("construct committed mirror payload");
    compare_and_swap_mirror_index_store(
        &config,
        &mirror_store,
        &empty_snapshot,
        &committed_payload,
    )
    .expect("commit mirror payload");
    let (committed_snapshot, committed_readback) =
        load_mirror_index_store(&config, &mirror_store).expect("reload committed mirror");
    assert_eq!(committed_readback, committed_payload);
    let checkpoint_revision =
        save_checkpoint(&checkpoint_store, None, &checkpoint).expect("seal checkpoint");
    let state_inventory_before = config
        .state_root_guard
        .rooted_directory()
        .child_names_bounded(SOURCE_ENTRY_HARD_CAP)
        .expect("inventory state before reader construction");
    let handle = GovernanceDagMirrorReadHandleV1::try_new(&config, checkpoint_store.clone(), false)
        .expect("construct coherent mirror reader");
    handle.mark_ready();
    assert_eq!(
        handle.binding().source_root_digest(),
        runtime_dag_producer_root_digest(&config.source_dir).expect("derive source digest")
    );
    assert_eq!(
        handle.binding().producer_signer_handle(),
        TEST_PRODUCER_SIGNER_HANDLE
    );
    assert_eq!(
        handle.binding().checkpoint_store_handle(),
        TEST_CHECKPOINT_STORE_HANDLE
    );
    let observed = handle
        .read()
        .expect("read coherent mirror capability")
        .expect("coherent checkpoint has a committed mirror snapshot");
    assert_eq!(observed.canonical_bytes(), canonical_bytes);
    assert_eq!(
        observed.mirror_store_identity(),
        (
            committed_snapshot.generation(),
            committed_snapshot.record_digest()
        )
    );
    assert_eq!(
        observed.checkpoint_identity().generation(),
        checkpoint.generation
    );
    assert_eq!(
        observed.checkpoint_identity().revision(),
        checkpoint_revision
    );
    assert_eq!(
        load_mirror_index_store(&config, &mirror_store)
            .expect("reload writer mirror after read")
            .0,
        committed_snapshot,
        "reader construction and read must not mutate either slot"
    );
    assert_eq!(
        config
            .state_root_guard
            .rooted_directory()
            .child_names_bounded(SOURCE_ENTRY_HARD_CAP)
            .expect("inventory state after reader read"),
        state_inventory_before,
        "reader construction and read must not create state"
    );
    let mut checkpoint_b = checkpoint.clone();
    checkpoint_b.generation = checkpoint
        .generation
        .checked_add(1)
        .expect("test checkpoint generation has successor");
    provider.return_checkpoint_on_second_load(GovernanceDagSealedStateRecord::new(
        GovernanceDagSealedStateSlot::Checkpoint,
        checkpoint_b.generation,
        norito::to_bytes(&checkpoint_b).expect("encode raced checkpoint"),
    ));
    let error = handle
        .read()
        .expect_err("A/B checkpoint race must fail closed");
    assert!(error.to_string().contains("checkpoint changed during read"));
    let intent = intent_from_source(&source);
    provider.return_intent_on_second_load(GovernanceDagSealedStateRecord::new(
        GovernanceDagSealedStateSlot::PublishIntent,
        intent.generation,
        norito::to_bytes(&intent).expect("encode raced intent"),
    ));
    let error = handle.read().expect_err("A/B intent race must fail closed");
    assert!(error.to_string().contains("intent changed during read"));
    let active_intent_revision =
        save_publish_intent(&checkpoint_store, None, &intent).expect("seal active intent");
    let error = handle
        .read()
        .expect_err("active intent must make mirror reads fail closed");
    assert!(error.to_string().contains("active sealed publish intent"));
    provider
        .qualification_revision
        .store(2, AtomicOrdering::SeqCst);
    let error = handle
        .read()
        .expect_err("provider qualification drift must invalidate reader");
    assert!(error.to_string().contains("identity or policy changed"));
    provider
        .qualification_revision
        .store(1, AtomicOrdering::SeqCst);
    delete_publish_intent(&checkpoint_store, Some(active_intent_revision))
        .expect("clear active test intent");
    let (current, _) =
        load_mirror_index_store(&config, &mirror_store).expect("load mirror before corruption");
    compare_and_swap_mirror_index_store(
        &config,
        &mirror_store,
        &current,
        &MirrorIndexStorePayloadV1::empty(),
    )
    .expect("commit internally valid but checkpoint-incoherent mirror");
    let error = handle
        .read()
        .expect_err("typed mirror corruption must fail closed");
    assert!(error.to_string().contains("no committed index"));
    #[cfg(unix)]
    {
        let state_root = config.state_root_guard.root().to_path_buf();
        let displaced = state_root.with_extension("displaced-reader-root");
        fs::rename(&state_root, &displaced).expect("displace retained state root");
        fs::create_dir(&state_root).expect("install substituted state root");
        fs::set_permissions(&state_root, fs::Permissions::from_mode(0o700))
            .expect("secure substituted state root");
        let error = handle
            .read()
            .expect_err("state-root substitution must invalidate reader");
        assert!(error.to_string().contains("state root"));
    }
}
#[test]
fn mirror_read_handle_never_initializes_an_absent_store() {
    let dir = secure_temp_dir();
    let source = signed_source(1, 0x3e, 1_800_000_000);
    let config = test_runtime_config(&source, dir.path());
    let before = config
        .state_root_guard
        .rooted_directory()
        .child_names_bounded(1)
        .expect("inventory pristine state root");
    assert!(before.is_empty());
    let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let checkpoint_store = test_checkpoint_store(provider);
    let error = GovernanceDagMirrorReadHandleV1::try_new(&config, checkpoint_store, true)
        .expect_err("reader must not initialize an absent mirror store");
    assert!(matches!(error, GovernanceDagServiceError::Filesystem(_)));
    assert_eq!(
        config
            .state_root_guard
            .rooted_directory()
            .child_names_bounded(1)
            .expect("inventory state after rejected reader"),
        before,
        "read capability construction must not create an init lock, directory, or slot"
    );
}
#[test]
fn mirror_read_handle_install_readiness_requires_the_existing_typed_store() {
    let dir = secure_temp_dir();
    let source = signed_source(1, 0x3f, 1_800_000_000);
    let config = test_runtime_config(&source, dir.path());
    let mirror_store = open_mirror_index_store(&config).expect("initialize typed mirror store");
    drop(mirror_store);
    let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let handle =
        GovernanceDagMirrorReadHandleV1::try_new(&config, test_checkpoint_store(provider), true)
            .expect("construct genesis mirror reader from an existing empty store");
    handle
        .assert_install_ready()
        .expect("an existing canonical empty mirror store is install-ready at genesis");
    let bootstrap_inventory = config
        .state_root_guard
        .rooted_directory()
        .child_names_bounded(SOURCE_ENTRY_HARD_CAP)
        .expect("inventory bootstrap mirror state");
    assert!(
        handle
            .read()
            .expect("read authenticated bootstrap mirror state")
            .is_none(),
        "an empty mirror with no sealed checkpoint is authenticated bootstrap, not corruption"
    );
    assert_eq!(
        config
            .state_root_guard
            .rooted_directory()
            .child_names_bounded(SOURCE_ENTRY_HARD_CAP)
            .expect("inventory mirror state after bootstrap read"),
        bootstrap_inventory,
        "bootstrap reads must not initialize or mutate mirror state"
    );
    let mut store_directories = Vec::new();
    for entry in fs::read_dir(config.state_root_guard.root()).expect("list mirror state root") {
        let entry = entry.expect("read mirror state entry");
        if entry
            .file_type()
            .expect("inspect mirror state entry")
            .is_dir()
        {
            store_directories.push(entry.path());
        }
    }
    assert_eq!(
        store_directories.len(),
        1,
        "fresh mirror state has exactly one typed-store directory"
    );
    fs::remove_dir_all(&store_directories[0]).expect("remove typed mirror store fixture");
    let error = handle
        .assert_install_ready()
        .expect_err("a mirror capability whose typed store disappeared must not install");
    assert!(matches!(error, GovernanceDagServiceError::Filesystem(_)));
}
#[test]
fn node_handle_installs_real_mirror_reader_once_across_preexisting_clones() {
    let dir = secure_temp_dir();
    let source = signed_source(1, 0x40, 1_800_000_000);
    let service_config = test_runtime_config(&source, dir.path());
    let mirror_store =
        open_mirror_index_store(&service_config).expect("initialize typed mirror store");
    drop(mirror_store);
    let checkpoint_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let signer = Arc::new(PublisherTestSigner {
        handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
        peer_id: TEST_PRODUCER_PEER_ID.as_bytes().to_vec(),
        signer: TestSigner::new(0x40),
    });
    let node_config = StorageConfig::builder()
        .enabled(true)
        .data_dir(dir.path().join("node-storage"))
        .governance_dir(Some(service_config.source_dir.clone()))
        .governance_dag_publisher_peer_id(Some(TEST_PRODUCER_PEER_ID.to_owned()))
        .governance_dag_signer_handle(Some(TEST_PRODUCER_SIGNER_HANDLE.to_owned()))
        .governance_dag_signer_qualification(Some(TEST_PRODUCER_SIGNER_QUALIFICATION))
        .governance_dag_publisher_public_key_hex(Some(hex::encode(signer.public_key())))
        .governance_dag_checkpoint_store_handle(Some(TEST_CHECKPOINT_STORE_HANDLE.to_owned()))
        .governance_dag_checkpoint_store_qualification(Some(TEST_STORE_QUALIFICATION))
        .build();
    let mut node = NodeHandle::try_new_with_runtime_deps(
        node_config,
        NodeRuntimeDeps::default()
            .with_governance_dag_signer(signer)
            .with_governance_dag_checkpoint_store(checkpoint_provider.clone()),
    )
    .expect("start node with the same retained Governance DAG providers");
    let mut clone_created_before_install = node.clone();
    let mismatch_root = dir.path().join("mismatched-reader");
    let mismatch_config = test_runtime_config(&source, &mismatch_root);
    let mismatch_store =
        open_mirror_index_store(&mismatch_config).expect("initialize mismatched mirror store");
    drop(mismatch_store);
    let mismatched_reader = GovernanceDagMirrorReadHandleV1::try_new(
        &mismatch_config,
        test_checkpoint_store(checkpoint_provider.clone()),
        true,
    )
    .expect("construct valid reader bound to the wrong producer root");
    let error = node
        .install_governance_dag_mirror_read_handle(mismatched_reader)
        .expect_err("a reader for another producer root must not install");
    assert!(error.to_string().contains("does not match"));
    assert!(
        node.governance_dag_mirror_snapshot()
            .expect("failed installation leaves the shared slot readable")
            .is_none(),
        "failed preflight must not consume or populate the installation slot"
    );
    let reader = GovernanceDagMirrorReadHandleV1::try_new(
        &service_config,
        test_checkpoint_store(checkpoint_provider.clone()),
        true,
    )
    .expect("construct reader for the node's retained producer root");
    node.install_governance_dag_mirror_read_handle(reader.clone())
        .expect("install the authenticated mirror reader exactly once");
    assert!(
        node.governance_dag_mirror_snapshot()
            .expect("node reads authenticated bootstrap mirror state")
            .is_none()
    );
    assert!(
        clone_created_before_install
            .governance_dag_mirror_snapshot()
            .expect("preexisting clone observes the installed reader")
            .is_none()
    );
    let mut checkpoint = checkpoint_from_source(&source);
    let mirror = mirror_index_value(
        &source,
        &checkpoint.mirror_blocks,
        &checkpoint.archive_head,
        checkpoint.generation,
        &checkpoint.head_ipfs_cid,
        checkpoint.published_at_unix,
    )
    .expect("build checkpoint-coherent mirror");
    let canonical_bytes = json::to_json_pretty(&mirror)
        .expect("encode checkpoint-coherent mirror")
        .into_bytes();
    checkpoint.mirror_blake3 = blake3_array(&canonical_bytes);
    let mirror_store =
        open_mirror_index_store(&service_config).expect("reopen typed mirror writer");
    let (empty_snapshot, empty_payload) =
        load_mirror_index_store(&service_config, &mirror_store).expect("load bootstrap mirror");
    assert!(empty_payload.is_empty());
    let committed_payload = MirrorIndexStorePayloadV1::committed(
        checkpoint.generation,
        [0; 32],
        canonical_bytes.clone(),
    )
    .expect("construct checkpoint-coherent typed mirror payload");
    compare_and_swap_mirror_index_store(
        &service_config,
        &mirror_store,
        &empty_snapshot,
        &committed_payload,
    )
    .expect("commit typed mirror payload");
    save_checkpoint(
        &test_checkpoint_store(checkpoint_provider),
        None,
        &checkpoint,
    )
    .expect("seal matching service checkpoint");
    reader.mark_ready();
    let node_snapshot = node
        .governance_dag_mirror_snapshot()
        .expect("node reads the installed checkpoint-coherent mirror")
        .expect("checkpointed mirror is available");
    assert_eq!(node_snapshot.canonical_bytes(), canonical_bytes);
    let clone_snapshot = clone_created_before_install
        .governance_dag_mirror_snapshot()
        .expect("preexisting clone reads the checkpoint-coherent mirror")
        .expect("checkpointed mirror is visible across clones");
    assert_eq!(clone_snapshot.canonical_bytes(), canonical_bytes);
    let error = clone_created_before_install
        .install_governance_dag_mirror_read_handle(reader)
        .expect_err("the shared installation slot must reject a second reader");
    assert!(error.to_string().contains("already installed"));
}
