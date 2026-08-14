#[test]
fn sealed_http_receiver_accepts_canonical_ipfs_and_signed_head_requests() {
    let now = 1_700_000_000;
    let store = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let cases = [
        (
            GovernanceDagAuthenticationScope::Ipfs,
            "POST",
            "https://example.invalid/api/v0/add?pin=false",
            vec![
                ("accept-encoding", "identity"),
                ("content-type", "application/vnd.ipld.raw"),
            ],
            b"canonical-block".as_slice(),
            [0x41; 32],
        ),
        (
            GovernanceDagAuthenticationScope::SignedHead,
            "PUT",
            "https://example.invalid/governance/head",
            vec![
                ("accept-encoding", "identity"),
                ("content-type", "application/vnd.iroha.norito"),
                ("if-match", "\"generation-7\""),
            ],
            b"canonical-head".as_slice(),
            [0x42; 32],
        ),
    ];
    for (scope, method, url, selected, body, nonce) in cases {
        let receiver = test_sealed_http_receiver(scope, store.clone());
        assert_eq!(receiver.scope(), scope);
        assert_eq!(receiver.max_body_bytes(), 1024 * 1024);
        assert_eq!(
            receiver.checkpoint_store_handle(),
            TEST_CHECKPOINT_STORE_HANDLE
        );
        assert_eq!(
            receiver.checkpoint_store_qualification(),
            TEST_STORE_QUALIFICATION
        );
        let (expected, headers) =
            sealed_receiver_request_parts(scope, method, url, &selected, body, now, nonce);
        let observed = receiver
            .verify_http_request(
                method,
                url,
                headers
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.as_slice())),
                body,
                now,
            )
            .expect("canonical sealed ingress request");
        assert_eq!(observed, expected);
    }
    let inner = store.inner.lock().expect("lock sealed replay state");
    assert!(inner.ipfs_request_replay.is_some());
    assert!(inner.signed_head_request_replay.is_some());
}
#[test]
fn sealed_http_receiver_constructor_rejects_missing_or_unqualified_store() {
    let policy = test_request_auth_policy(test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE));
    let construct =
        |max_body_bytes,
         handle: &str,
         qualification,
         store: Option<Arc<dyn GovernanceDagSealedCheckpointStore>>| {
            GovernanceDagSealedHttpRequestReceiverV1::try_new(
                GovernanceDagAuthenticationScope::Ipfs,
                max_body_bytes,
                policy,
                handle,
                qualification,
                store,
            )
        };
    assert!(
        construct(
            1024,
            TEST_CHECKPOINT_STORE_HANDLE,
            TEST_STORE_QUALIFICATION,
            None,
        )
        .expect_err("missing sealed store")
        .to_string()
        .contains("was not injected")
    );
    assert!(
        construct(
            0,
            TEST_CHECKPOINT_STORE_HANDLE,
            TEST_STORE_QUALIFICATION,
            Some(Arc::new(
                TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE,)
            )),
        )
        .expect_err("zero request bound")
        .to_string()
        .contains("must be non-zero")
    );
    assert!(
        construct(
            1024,
            TEST_CHECKPOINT_STORE_HANDLE,
            TEST_STORE_QUALIFICATION,
            Some(Arc::new(TestSealedStore::new(
                "kms:governance/checkpoint:substituted",
            ))),
        )
        .expect_err("substituted sealed store")
        .to_string()
        .contains("does not match configured handle")
    );
    assert!(
        construct(
            1024,
            "test://governance/checkpoint",
            TEST_STORE_QUALIFICATION,
            Some(Arc::new(TestSealedStore::new(
                "test://governance/checkpoint",
            ))),
        )
        .expect_err("test-marked sealed store")
        .to_string()
        .contains("test-marked")
    );
    let stale = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    stale
        .qualification_refuse
        .store(true, AtomicOrdering::SeqCst);
    let error = construct(
        1024,
        TEST_CHECKPOINT_STORE_HANDLE,
        TEST_STORE_QUALIFICATION,
        Some(stale),
    )
    .expect_err("stale sealed store");
    assert!(
        error
            .to_string()
            .contains("unavailable, stale, or unqualified")
    );
    assert!(!error.to_string().contains("kms_access_token"));
}
#[test]
fn sealed_http_receiver_rejects_duplicate_across_receiver_instances() {
    let now = 1_700_000_000;
    let store = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let first = test_sealed_http_receiver(GovernanceDagAuthenticationScope::Ipfs, store.clone());
    let second = test_sealed_http_receiver(GovernanceDagAuthenticationScope::Ipfs, store.clone());
    let (request, headers) = sealed_receiver_request_parts(
        GovernanceDagAuthenticationScope::Ipfs,
        "POST",
        "https://example.invalid/api/v0/pin/add?arg=cid&recursive=true",
        &[("accept-encoding", "identity")],
        b"",
        now,
        [0x43; 32],
    );
    let verify = |receiver: &GovernanceDagSealedHttpRequestReceiverV1| {
        receiver.verify_http_request(
            request.method(),
            request.canonical_url(),
            headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_slice())),
            b"",
            now,
        )
    };
    assert_eq!(verify(&first).expect("first receiver accepts"), request);
    let replay = verify(&second).expect_err("second receiver rejects durable replay");
    assert!(replay.to_string().contains("replay was rejected"));
    assert_eq!(store.replay_cas_calls.load(AtomicOrdering::SeqCst), 1);
}
#[test]
fn sealed_http_receiver_cas_race_accepts_exactly_one_replica() {
    let now = 1_700_000_000;
    let barrier = Arc::new(Barrier::new(2));
    let store = Arc::new(
        TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE).with_replay_load_barrier(barrier),
    );
    let request_parts = sealed_receiver_request_parts(
        GovernanceDagAuthenticationScope::Ipfs,
        "POST",
        "https://example.invalid/api/v0/pin/add?arg=cid&recursive=true",
        &[("accept-encoding", "identity")],
        b"",
        now,
        [0x44; 32],
    );
    let mut threads = Vec::new();
    for _ in 0..2 {
        let receiver =
            test_sealed_http_receiver(GovernanceDagAuthenticationScope::Ipfs, store.clone());
        let (request, headers) = request_parts.clone();
        threads.push(std::thread::spawn(move || {
            receiver.verify_http_request(
                request.method(),
                request.canonical_url(),
                headers
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.as_slice())),
                b"",
                now,
            )
        }));
    }
    let results = threads
        .into_iter()
        .map(|thread| thread.join().expect("join sealed receiver replica"))
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
    assert_eq!(store.replay_cas_calls.load(AtomicOrdering::SeqCst), 2);
}
#[test]
fn sealed_http_receiver_rejects_request_failures_before_state_mutation() {
    let now = 1_700_000_000;
    let store = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let receiver = test_sealed_http_receiver(GovernanceDagAuthenticationScope::Ipfs, store.clone());
    let (request, valid_headers) = sealed_receiver_request_parts(
        GovernanceDagAuthenticationScope::Ipfs,
        "POST",
        "https://example.invalid/api/v0/add?pin=false",
        &[("accept-encoding", "identity")],
        b"body",
        now,
        [0x45; 32],
    );
    let verify = |method: &str,
                  url: &str,
                  headers: &[(String, Vec<u8>)],
                  body: &[u8],
                  verification_time: u64| {
        receiver.verify_http_request(
            method,
            url,
            headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_slice())),
            body,
            verification_time,
        )
    };
    let mut missing = valid_headers.clone();
    missing.remove(0);
    assert!(
        verify(
            request.method(),
            request.canonical_url(),
            &missing,
            b"body",
            now
        )
        .is_err()
    );
    assert!(verify("PUT", request.canonical_url(), &valid_headers, b"body", now,).is_err());
    assert!(
        verify(
            request.method(),
            "https://example.invalid/api/v0/add?pin=true",
            &valid_headers,
            b"body",
            now,
        )
        .is_err()
    );
    assert!(
        verify(
            request.method(),
            request.canonical_url(),
            &valid_headers,
            b"tampered-body",
            now,
        )
        .is_err()
    );
    assert!(
        verify(
            request.method(),
            request.canonical_url(),
            &valid_headers,
            b"body",
            now + 16,
        )
        .is_err()
    );
    let wrong_key_envelope = signed_test_request_auth_envelope(
        TEST_HEAD_AUTH_HANDLE,
        &request,
        now,
        now + 15,
        [0x46; 32],
    );
    let mut wrong_key_headers = request_auth_header_fields(&wrong_key_envelope);
    wrong_key_headers.extend([
        ("accept-encoding".to_owned(), b"identity".to_vec()),
        ("content-length".to_owned(), b"4".to_vec()),
    ]);
    assert!(
        verify(
            request.method(),
            request.canonical_url(),
            &wrong_key_headers,
            b"body",
            now,
        )
        .is_err()
    );
    let mut tampered_signature = valid_headers.clone();
    let signature = tampered_signature
        .iter_mut()
        .find(|(name, _value)| name == "x-sorafs-governance-auth-signature")
        .expect("signature header is present");
    signature.1[0] = if signature.1[0] == b'0' { b'1' } else { b'0' };
    assert!(
        verify(
            request.method(),
            request.canonical_url(),
            &tampered_signature,
            b"body",
            now,
        )
        .is_err()
    );
    let mut unknown_header = valid_headers.clone();
    unknown_header.push((
        "x-sorafs-governance-auth-extension".to_owned(),
        b"1".to_vec(),
    ));
    assert!(
        verify(
            request.method(),
            request.canonical_url(),
            &unknown_header,
            b"body",
            now,
        )
        .is_err()
    );
    let head_receiver =
        test_sealed_http_receiver(GovernanceDagAuthenticationScope::SignedHead, store.clone());
    assert!(
        head_receiver
            .verify_http_request(
                request.method(),
                request.canonical_url(),
                valid_headers
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.as_slice())),
                b"body",
                now,
            )
            .is_err()
    );
    assert_eq!(store.replay_cas_calls.load(AtomicOrdering::SeqCst), 0);
    let inner = store.inner.lock().expect("lock unchanged sealed state");
    assert!(inner.ipfs_request_replay.is_none());
    assert!(inner.signed_head_request_replay.is_none());
}
#[test]
fn sealed_http_receiver_fails_closed_on_store_drift_and_readback_divergence() {
    let now = 1_700_000_000;
    for failure in ["cas-drift", "readback-divergence"] {
        let store = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        match failure {
            "cas-drift" => store
                .drift_during_replay_cas
                .store(true, AtomicOrdering::SeqCst),
            "readback-divergence" => store
                .diverge_replay_readback
                .store(true, AtomicOrdering::SeqCst),
            _ => unreachable!(),
        }
        let receiver =
            test_sealed_http_receiver(GovernanceDagAuthenticationScope::Ipfs, store.clone());
        let (request, headers) = sealed_receiver_request_parts(
            GovernanceDagAuthenticationScope::Ipfs,
            "POST",
            "https://example.invalid/api/v0/pin/add?arg=cid&recursive=true",
            &[("accept-encoding", "identity")],
            b"",
            now,
            if failure == "cas-drift" {
                [0x47; 32]
            } else {
                [0x48; 32]
            },
        );
        let error = receiver
            .verify_http_request(
                request.method(),
                request.canonical_url(),
                headers
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.as_slice())),
                b"",
                now,
            )
            .expect_err("store ambiguity cannot authorize ingress");
        assert!(error.to_string().contains("replay store is unavailable"));
        assert_eq!(store.replay_cas_calls.load(AtomicOrdering::SeqCst), 1);
    }
}
#[test]
fn sealed_http_receiver_rejects_corrupt_and_noncanonical_replay_payloads() {
    let now = 1_700_000_000;
    let mut noncanonical = norito::to_bytes(&RequestAuthReplayStateV1 {
        version: REQUEST_AUTH_REPLAY_STATE_VERSION_V1,
        entries: Vec::new(),
    })
    .expect("encode canonical empty replay state");
    noncanonical.push(0);
    for payload in [vec![0xff], noncanonical] {
        let store = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        {
            let mut inner = store.inner.lock().expect("lock replay state");
            inner.ipfs_request_replay = Some(GovernanceDagSealedStateRecord::new(
                GovernanceDagSealedStateSlot::IpfsRequestReplay,
                1,
                payload,
            ));
            inner.ipfs_request_replay_generation_floor = 1;
        }
        let receiver =
            test_sealed_http_receiver(GovernanceDagAuthenticationScope::Ipfs, store.clone());
        let (request, headers) = sealed_receiver_request_parts(
            GovernanceDagAuthenticationScope::Ipfs,
            "GET",
            "https://example.invalid/api/v0/cat?arg=cid",
            &[("accept-encoding", "identity")],
            b"",
            now,
            [0x49; 32],
        );
        let error = receiver
            .verify_http_request(
                request.method(),
                request.canonical_url(),
                headers
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.as_slice())),
                b"",
                now,
            )
            .expect_err("invalid sealed payload cannot authorize ingress");
        assert!(error.to_string().contains("replay store is unavailable"));
        assert_eq!(store.replay_cas_calls.load(AtomicOrdering::SeqCst), 0);
    }
}
#[test]
fn sealed_http_receiver_prunes_expiry_without_evicting_live_capacity() {
    let now = 1_700_000_000;
    let entries = (0..GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_CACHE_CAPACITY_V1)
        .map(|index| RequestAuthReplayEntryV1 {
            nonce: {
                let mut nonce = [0; 32];
                nonce[24..].copy_from_slice(&(index as u64 + 1).to_be_bytes());
                nonce
            },
            expires_at_unix_secs: now + 20,
        })
        .collect::<Vec<_>>();
    let store = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    {
        let payload = norito::to_bytes(&RequestAuthReplayStateV1 {
            version: REQUEST_AUTH_REPLAY_STATE_VERSION_V1,
            entries: entries.clone(),
        })
        .expect("encode full live replay state");
        let mut inner = store.inner.lock().expect("lock replay state");
        inner.ipfs_request_replay = Some(GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::IpfsRequestReplay,
            1,
            payload,
        ));
        inner.ipfs_request_replay_generation_floor = 1;
    }
    let receiver = test_sealed_http_receiver(GovernanceDagAuthenticationScope::Ipfs, store.clone());
    let (request, headers) = sealed_receiver_request_parts(
        GovernanceDagAuthenticationScope::Ipfs,
        "GET",
        "https://example.invalid/api/v0/cat?arg=cid",
        &[("accept-encoding", "identity")],
        b"",
        now,
        [0xff; 32],
    );
    let full = receiver
        .verify_http_request(
            request.method(),
            request.canonical_url(),
            headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_slice())),
            b"",
            now,
        )
        .expect_err("live replay capacity must not evict");
    assert!(full.to_string().contains("bounded capacity"));
    assert_eq!(store.replay_cas_calls.load(AtomicOrdering::SeqCst), 0);
    {
        let mut expired = entries;
        expired[0].expires_at_unix_secs = now;
        let payload = norito::to_bytes(&RequestAuthReplayStateV1 {
            version: REQUEST_AUTH_REPLAY_STATE_VERSION_V1,
            entries: expired,
        })
        .expect("encode replay state with one expired entry");
        let mut inner = store.inner.lock().expect("lock replay state");
        inner.ipfs_request_replay = Some(GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::IpfsRequestReplay,
            2,
            payload,
        ));
        inner.ipfs_request_replay_generation_floor = 2;
    }
    receiver
        .verify_http_request(
            request.method(),
            request.canonical_url(),
            headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_slice())),
            b"",
            now,
        )
        .expect("one expired entry makes bounded room");
    assert_eq!(store.replay_cas_calls.load(AtomicOrdering::SeqCst), 1);
    let observed = store
        .inner
        .lock()
        .expect("lock committed replay state")
        .ipfs_request_replay
        .clone()
        .expect("committed replay state");
    let state = decode_request_auth_replay_state(
        &observed,
        GovernanceDagSealedStateSlot::IpfsRequestReplay,
        now,
    )
    .expect("decode committed replay state");
    assert_eq!(
        state.entries.len(),
        GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_CACHE_CAPACITY_V1
    );
    assert!(
        state
            .entries
            .iter()
            .all(|entry| entry.expires_at_unix_secs > now)
    );
    assert!(state.entries.iter().any(|entry| entry.nonce == [0xff; 32]));
}
#[test]
fn inbound_request_auth_accepts_canonical_ipfs_and_head_operations() {
    let now = 1_700_000_000;
    let ipfs_policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
        test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE),
        30,
        5,
    )
    .expect("valid IPFS receiver policy");
    let head_policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
        test_request_auth_public_key(TEST_HEAD_AUTH_HANDLE),
        30,
        5,
    )
    .expect("valid signed-head receiver policy");
    let cases = vec![
        (
            GovernanceDagAuthenticationScope::Ipfs,
            "GET",
            "https://example.invalid/api/v0/cat?arg=cid",
            vec![("accept-encoding", b"identity".as_slice())],
            b"".as_slice(),
            TEST_IPFS_AUTH_HANDLE,
            [0x31; 32],
        ),
        (
            GovernanceDagAuthenticationScope::Ipfs,
            "POST",
            "https://example.invalid/api/v0/add?pin=false",
            vec![
                (
                    "content-type",
                    b"multipart/form-data;boundary=gdag".as_slice(),
                ),
                ("accept-encoding", b"identity".as_slice()),
            ],
            b"canonical-block".as_slice(),
            TEST_IPFS_AUTH_HANDLE,
            [0x32; 32],
        ),
        (
            GovernanceDagAuthenticationScope::SignedHead,
            "GET",
            "https://example.invalid/governance/head",
            vec![
                ("if-none-match", b"\"v7\"".as_slice()),
                ("accept-encoding", b"identity".as_slice()),
            ],
            b"".as_slice(),
            TEST_HEAD_AUTH_HANDLE,
            [0x33; 32],
        ),
        (
            GovernanceDagAuthenticationScope::SignedHead,
            "PUT",
            "https://example.invalid/governance/head",
            vec![
                ("if-match", b"\"v7\"".as_slice()),
                ("content-type", b"application/vnd.iroha.norito".as_slice()),
                ("accept-encoding", b"identity".as_slice()),
            ],
            b"canonical-head".as_slice(),
            TEST_HEAD_AUTH_HANDLE,
            [0x34; 32],
        ),
    ];
    let backend_calls = AtomicU64::new(0);
    let mut ipfs_replay_cache = GovernanceDagRequestAuthenticationReplayCacheV1::new();
    let mut head_replay_cache = GovernanceDagRequestAuthenticationReplayCacheV1::new();
    for (scope, method, url, headers, body, handle, nonce) in cases {
        let request = GovernanceDagCanonicalRequestV1::try_from_http_parts(
            scope,
            method,
            url,
            headers,
            body,
            1024 * 1024,
        )
        .expect("canonical inbound request");
        let envelope = signed_test_request_auth_envelope(handle, &request, now, now + 15, nonce);
        let mut headers = request_auth_header_fields(&envelope);
        headers.push((
            "content-length".to_owned(),
            body.len().to_string().into_bytes(),
        ));
        let (policy, replay_cache) = match scope {
            GovernanceDagAuthenticationScope::Ipfs => (&ipfs_policy, &mut ipfs_replay_cache),
            GovernanceDagAuthenticationScope::SignedHead => (&head_policy, &mut head_replay_cache),
        };
        verify_request_before_test_backend(
            &request,
            &headers,
            body,
            scope,
            policy,
            now,
            replay_cache,
            &backend_calls,
        )
        .expect("verified request reaches the test backend");
    }
    assert_eq!(backend_calls.load(AtomicOrdering::SeqCst), 4);
}
#[test]
fn inbound_request_auth_header_mapping_is_an_exact_hard_cut() {
    let now = 1_700_000_000;
    let request = canonical_test_request(
        GovernanceDagAuthenticationScope::Ipfs,
        "POST",
        "https://example.invalid/api/v0/pin/add?arg=cid&recursive=true",
        &[("accept-encoding", "identity")],
        b"",
    );
    let envelope = signed_test_request_auth_envelope(
        TEST_IPFS_AUTH_HANDLE,
        &request,
        now,
        now + 15,
        [0xab; 32],
    );
    let canonical = request_auth_header_fields(&envelope);
    let parsed = parse_governance_dag_request_authentication_headers_v1(
        canonical
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_slice()))
            .chain(std::iter::once(("accept-encoding", b"identity".as_slice()))),
    )
    .expect("ignore ordinary headers and parse the exact auth header set");
    assert_eq!(parsed, envelope);
    let policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
        test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE),
        30,
        5,
    )
    .expect("valid IPFS receiver policy");
    let zero_bound_error = GovernanceDagRequestIngressBindingV1::try_new(
        GovernanceDagAuthenticationScope::Ipfs,
        governance_dag_request_ingress_endpoint_binding_v1(
            GovernanceDagAuthenticationScope::Ipfs,
            "https://example.invalid/",
        )
        .expect("canonical zero-bound test endpoint"),
        policy.public_key(),
        0,
        policy.max_envelope_lifetime_secs(),
        policy.max_future_skew_secs(),
    )
    .expect_err("ingress binding must reject a zero body ceiling");
    assert_eq!(
        zero_bound_error,
        crate::GovernanceDagRequestIngressQualificationErrorV1::InvalidRequestBodyLimit
    );
    let backend_calls = AtomicU64::new(0);
    let mut missing = canonical.clone();
    missing.remove(0);
    let cases = [
        (
            missing,
            GovernanceDagRequestAuthenticationErrorV1::MissingHeader,
        ),
        (
            {
                let mut fields = canonical.clone();
                fields.push(canonical[0].clone());
                fields
            },
            GovernanceDagRequestAuthenticationErrorV1::DuplicateHeader,
        ),
        (
            {
                let mut fields = canonical.clone();
                fields.push(("x-sorafs-governance-auth-key".to_owned(), vec![b'a'; 64]));
                fields
            },
            GovernanceDagRequestAuthenticationErrorV1::UnknownHeader,
        ),
        (
            {
                let mut fields = canonical.clone();
                fields.push((
                    "x-sorafs-governance-auth-extension".to_owned(),
                    b"1".to_vec(),
                ));
                fields
            },
            GovernanceDagRequestAuthenticationErrorV1::UnknownHeader,
        ),
    ];
    for (fields, expected) in cases {
        let error = verify_request_before_test_backend(
            &request,
            &fields,
            b"",
            GovernanceDagAuthenticationScope::Ipfs,
            &policy,
            now,
            &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
            &backend_calls,
        )
        .expect_err("noncanonical header map must stop before backend dispatch");
        assert_eq!(error, expected);
    }
    for unexpected_name in [
        "cache-control",
        "x-request-id",
        "x-http-method-override",
        "x-original-url",
        "forwarded",
    ] {
        let mut fields = canonical.clone();
        fields.push((unexpected_name.to_owned(), b"semantic-extension".to_vec()));
        let error = verify_request_before_test_backend(
            &request,
            &fields,
            b"",
            GovernanceDagAuthenticationScope::Ipfs,
            &policy,
            now,
            &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
            &backend_calls,
        )
        .expect_err("unsigned semantic headers must stop before backend dispatch");
        assert_eq!(
            error,
            GovernanceDagRequestAuthenticationErrorV1::UnexpectedHeader
        );
    }
    let unavailable_error = verify_request_before_test_backend(
        &request,
        &canonical,
        b"",
        GovernanceDagAuthenticationScope::Ipfs,
        &policy,
        now,
        &mut UnavailableTestReplayStore,
        &backend_calls,
    )
    .expect_err("unavailable shared replay state must stop before backend dispatch");
    assert_eq!(
        unavailable_error,
        GovernanceDagRequestAuthenticationErrorV1::ReplayStoreUnavailable
    );
    for (index, value) in [
        (0, b"01".to_vec()),
        (1, b"IPFS".to_vec()),
        (2, b"01".to_vec()),
        (4, "AA".repeat(32).into_bytes()),
        (5, b"00".to_vec()),
    ] {
        let mut fields = canonical.clone();
        fields[index].1 = value;
        let error = verify_request_before_test_backend(
            &request,
            &fields,
            b"",
            GovernanceDagAuthenticationScope::Ipfs,
            &policy,
            now,
            &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
            &backend_calls,
        )
        .expect_err("noncanonical header value must stop before backend dispatch");
        assert_eq!(
            error,
            GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader
        );
    }
    assert_eq!(
        backend_calls.load(AtomicOrdering::SeqCst),
        0,
        "no header-mapping failure may reach the backend"
    );
}
#[test]
fn inbound_request_auth_binds_every_request_part_before_backend_dispatch() {
    let now = 1_700_000_000;
    let request = canonical_test_request(
        GovernanceDagAuthenticationScope::SignedHead,
        "PUT",
        "https://example.invalid/governance/head",
        &[
            ("accept-encoding", "identity"),
            ("content-type", "application/vnd.iroha.norito"),
            ("if-match", "\"v7\""),
        ],
        b"head-v7",
    );
    let envelope = signed_test_request_auth_envelope(
        TEST_HEAD_AUTH_HANDLE,
        &request,
        now,
        now + 15,
        [0x61; 32],
    );
    let headers = request_auth_header_fields(&envelope);
    let policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
        test_request_auth_public_key(TEST_HEAD_AUTH_HANDLE),
        30,
        5,
    )
    .expect("valid signed-head receiver policy");
    let tampered = [
        canonical_test_request(
            GovernanceDagAuthenticationScope::Ipfs,
            "PUT",
            "https://example.invalid/governance/head",
            &[
                ("accept-encoding", "identity"),
                ("content-type", "application/vnd.iroha.norito"),
                ("if-match", "\"v7\""),
            ],
            b"head-v7",
        ),
        canonical_test_request(
            GovernanceDagAuthenticationScope::SignedHead,
            "POST",
            "https://example.invalid/governance/head",
            &[
                ("accept-encoding", "identity"),
                ("content-type", "application/vnd.iroha.norito"),
                ("if-match", "\"v7\""),
            ],
            b"head-v7",
        ),
        canonical_test_request(
            GovernanceDagAuthenticationScope::SignedHead,
            "PUT",
            "https://example.invalid/governance/head-v8",
            &[
                ("accept-encoding", "identity"),
                ("content-type", "application/vnd.iroha.norito"),
                ("if-match", "\"v7\""),
            ],
            b"head-v7",
        ),
        canonical_test_request(
            GovernanceDagAuthenticationScope::SignedHead,
            "PUT",
            "https://example.invalid/governance/head",
            &[
                ("accept-encoding", "identity"),
                ("content-type", "application/vnd.iroha.norito"),
                ("if-match", "\"v6\""),
            ],
            b"head-v7",
        ),
        canonical_test_request(
            GovernanceDagAuthenticationScope::SignedHead,
            "PUT",
            "https://example.invalid/governance/head",
            &[
                ("accept-encoding", "identity"),
                ("content-type", "application/vnd.iroha.norito"),
            ],
            b"head-v7",
        ),
        canonical_test_request(
            GovernanceDagAuthenticationScope::SignedHead,
            "PUT",
            "https://example.invalid/governance/head",
            &[
                ("accept-encoding", "identity"),
                ("content-type", "application/vnd.iroha.norito"),
                ("if-match", "\"v7\""),
            ],
            b"HEAD-v7",
        ),
        GovernanceDagCanonicalRequestV1::try_new(
            GovernanceDagAuthenticationScope::SignedHead,
            "PUT",
            "https://example.invalid/governance/head",
            request.selected_headers().to_vec(),
            request.body_length().saturating_add(1),
            request.body_blake3(),
            1024 * 1024,
        )
        .expect("canonical body-length tamper descriptor"),
    ];
    let backend_calls = AtomicU64::new(0);
    for (index, tampered_request) in tampered.iter().enumerate() {
        let body = if index == 5 {
            b"HEAD-v7".as_slice()
        } else {
            b"head-v7".as_slice()
        };
        let error = verify_request_before_test_backend(
            tampered_request,
            &headers,
            body,
            GovernanceDagAuthenticationScope::SignedHead,
            &policy,
            now,
            &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
            &backend_calls,
        )
        .expect_err("tampered request must stop before backend dispatch");
        assert_eq!(
            error,
            GovernanceDagRequestAuthenticationErrorV1::RequestMismatch
        );
    }
    let error = verify_request_before_test_backend(
        &request,
        &headers,
        b"head-v7",
        GovernanceDagAuthenticationScope::Ipfs,
        &policy,
        now,
        &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
        &backend_calls,
    )
    .expect_err("wrong receiver scope must stop before backend dispatch");
    assert_eq!(
        error,
        GovernanceDagRequestAuthenticationErrorV1::RequestMismatch
    );
    let wrong_key_policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
        test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE),
        30,
        5,
    )
    .expect("alternate valid receiver key");
    let error = verify_request_before_test_backend(
        &request,
        &headers,
        b"head-v7",
        GovernanceDagAuthenticationScope::SignedHead,
        &wrong_key_policy,
        now,
        &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
        &backend_calls,
    )
    .expect_err("wrong pinned key must stop before backend dispatch");
    assert_eq!(
        error,
        GovernanceDagRequestAuthenticationErrorV1::RequestMismatch
    );
    assert_eq!(
        backend_calls.load(AtomicOrdering::SeqCst),
        0,
        "no binding failure may reach the backend"
    );
}
#[test]
fn inbound_request_auth_rejects_time_nonce_signature_and_replay_failures() {
    let now = 1_700_000_000;
    let request = canonical_test_request(
        GovernanceDagAuthenticationScope::Ipfs,
        "GET",
        "https://example.invalid/api/v0/cat?arg=cid",
        &[("accept-encoding", "identity")],
        b"",
    );
    let policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
        test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE),
        30,
        5,
    )
    .expect("valid IPFS receiver policy");
    let backend_calls = AtomicU64::new(0);
    for (issued_at, expires_at, nonce) in [
        (now - 20, now - 1, [0x71; 32]),
        (now + 6, now + 16, [0x72; 32]),
        (now, now + 31, [0x73; 32]),
    ] {
        let envelope = signed_test_request_auth_envelope(
            TEST_IPFS_AUTH_HANDLE,
            &request,
            issued_at,
            expires_at,
            nonce,
        );
        let error = verify_request_before_test_backend(
            &request,
            &request_auth_header_fields(&envelope),
            b"",
            GovernanceDagAuthenticationScope::Ipfs,
            &policy,
            now,
            &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
            &backend_calls,
        )
        .expect_err("invalid timing must stop before backend dispatch");
        assert_eq!(
            error,
            GovernanceDagRequestAuthenticationErrorV1::InvalidTiming
        );
    }
    let valid = signed_test_request_auth_envelope(
        TEST_IPFS_AUTH_HANDLE,
        &request,
        now,
        now + 15,
        [0x74; 32],
    );
    let mut zero_nonce_headers = request_auth_header_fields(&valid);
    zero_nonce_headers[4].1 = "00".repeat(32).into_bytes();
    let error = verify_request_before_test_backend(
        &request,
        &zero_nonce_headers,
        b"",
        GovernanceDagAuthenticationScope::Ipfs,
        &policy,
        now,
        &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
        &backend_calls,
    )
    .expect_err("zero nonce must stop before backend dispatch");
    assert_eq!(
        error,
        GovernanceDagRequestAuthenticationErrorV1::MalformedEnvelope
    );
    let mut bad_signature_headers = request_auth_header_fields(&valid);
    let mut invalid_signature = valid.signature();
    invalid_signature[32..].fill(0);
    bad_signature_headers[7].1 = hex::encode(invalid_signature).into_bytes();
    let error = verify_request_before_test_backend(
        &request,
        &bad_signature_headers,
        b"",
        GovernanceDagAuthenticationScope::Ipfs,
        &policy,
        now,
        &mut GovernanceDagRequestAuthenticationReplayCacheV1::new(),
        &backend_calls,
    )
    .expect_err("invalid signature must stop before backend dispatch");
    assert_eq!(
        error,
        GovernanceDagRequestAuthenticationErrorV1::SignatureVerification
    );
    assert_eq!(backend_calls.load(AtomicOrdering::SeqCst), 0);
    let headers = request_auth_header_fields(&valid);
    let mut replay_cache = GovernanceDagRequestAuthenticationReplayCacheV1::new();
    verify_request_before_test_backend(
        &request,
        &headers,
        b"",
        GovernanceDagAuthenticationScope::Ipfs,
        &policy,
        now,
        &mut replay_cache,
        &backend_calls,
    )
    .expect("first nonce use reaches backend");
    let error = verify_request_before_test_backend(
        &request,
        &headers,
        b"",
        GovernanceDagAuthenticationScope::Ipfs,
        &policy,
        now,
        &mut replay_cache,
        &backend_calls,
    )
    .expect_err("replayed nonce must stop before backend dispatch");
    assert_eq!(error, GovernanceDagRequestAuthenticationErrorV1::Replay);
    assert_eq!(
        backend_calls.load(AtomicOrdering::SeqCst),
        1,
        "replay rejection must not invoke the backend again"
    );
    let second = signed_test_request_auth_envelope(
        TEST_IPFS_AUTH_HANDLE,
        &request,
        now,
        now + 15,
        [0x75; 32],
    );
    let mut bounded_cache = GovernanceDagRequestAuthenticationReplayCacheV1::try_with_capacity(1)
        .expect("one-entry replay cache");
    let capacity_backend_calls = AtomicU64::new(0);
    verify_request_before_test_backend(
        &request,
        &headers,
        b"",
        GovernanceDagAuthenticationScope::Ipfs,
        &policy,
        now,
        &mut bounded_cache,
        &capacity_backend_calls,
    )
    .expect("first live nonce fits bounded cache");
    let error = verify_request_before_test_backend(
        &request,
        &request_auth_header_fields(&second),
        b"",
        GovernanceDagAuthenticationScope::Ipfs,
        &policy,
        now,
        &mut bounded_cache,
        &capacity_backend_calls,
    )
    .expect_err("full live replay cache must fail closed");
    assert_eq!(
        error,
        GovernanceDagRequestAuthenticationErrorV1::ReplayCacheFull
    );
    assert_eq!(capacity_backend_calls.load(AtomicOrdering::SeqCst), 1);
}
#[test]
fn inbound_receiver_rejects_framing_before_replay_consumption_or_dispatch() {
    let now = 1_700_000_000;
    let body = b"canonical-head";
    let request = canonical_test_request(
        GovernanceDagAuthenticationScope::SignedHead,
        "PUT",
        "https://example.invalid/governance/head",
        &[
            ("accept-encoding", "identity"),
            ("content-type", "application/vnd.iroha.norito"),
            ("if-match", "\"v7\""),
        ],
        body,
    );
    let envelope = signed_test_request_auth_envelope(
        TEST_HEAD_AUTH_HANDLE,
        &request,
        now,
        now + 15,
        [0x76; 32],
    );
    let mut ambiguous_headers = request_auth_header_fields(&envelope);
    ambiguous_headers.push(("transfer-encoding".to_owned(), b"chunked".to_vec()));
    let policy = GovernanceDagRequestAuthenticationPolicyV1::try_new(
        test_request_auth_public_key(TEST_HEAD_AUTH_HANDLE),
        30,
        5,
    )
    .expect("valid signed-head receiver policy");
    let mut replay_cache = GovernanceDagRequestAuthenticationReplayCacheV1::new();
    let backend_calls = AtomicU64::new(0);
    let error = verify_request_before_test_backend(
        &request,
        &ambiguous_headers,
        body,
        GovernanceDagAuthenticationScope::SignedHead,
        &policy,
        now,
        &mut replay_cache,
        &backend_calls,
    )
    .expect_err("ambiguous framing must stop before verification and dispatch");
    assert_eq!(
        error,
        GovernanceDagRequestAuthenticationErrorV1::InvalidFraming
    );
    assert_eq!(backend_calls.load(AtomicOrdering::SeqCst), 0);
    verify_request_before_test_backend(
        &request,
        &request_auth_header_fields(&envelope),
        body,
        GovernanceDagAuthenticationScope::SignedHead,
        &policy,
        now,
        &mut replay_cache,
        &backend_calls,
    )
    .expect("same nonce remains usable after pre-verification framing rejection");
    assert_eq!(backend_calls.load(AtomicOrdering::SeqCst), 1);
}
#[test]
fn canonical_request_hard_cut_rejects_credentials_aliases_and_bounds() {
    assert!(
        GovernanceDagCanonicalRequestHeaderV1::try_new("authorization", "Bearer must-not-escape")
            .is_err()
    );
    assert!(GovernanceDagCanonicalRequestHeaderV1::try_new("cookie", "session=secret").is_err());
    assert!(GovernanceDagCanonicalRequestHeaderV1::try_new("content-type", " value").is_err());
    let duplicate = vec![
        GovernanceDagCanonicalRequestHeaderV1::try_new("accept-encoding", "identity")
            .expect("first header"),
        GovernanceDagCanonicalRequestHeaderV1::try_new("accept-encoding", "identity")
            .expect("duplicate header"),
    ];
    assert!(
        GovernanceDagCanonicalRequestV1::try_new(
            GovernanceDagAuthenticationScope::Ipfs,
            "POST",
            "https://example.invalid/api/v0/cat?arg=cid",
            duplicate,
            0,
            blake3_array(b""),
            1024,
        )
        .is_err()
    );
    assert!(
        GovernanceDagCanonicalRequestV1::try_new(
            GovernanceDagAuthenticationScope::Ipfs,
            "GET",
            "https://example.invalid/",
            Vec::new(),
            0,
            [0x55; 32],
            1024,
        )
        .is_err()
    );
    assert!(
        GovernanceDagCanonicalRequestV1::try_new(
            GovernanceDagAuthenticationScope::Ipfs,
            "PATCH",
            "https://example.invalid/",
            Vec::new(),
            0,
            blake3_array(b""),
            1024,
        )
        .is_err()
    );
    assert!(
        GovernanceDagCanonicalRequestV1::try_new(
            GovernanceDagAuthenticationScope::Ipfs,
            "POST",
            "https://example.invalid/",
            Vec::new(),
            1025,
            blake3_array(&[0; 1025]),
            1024,
        )
        .is_err()
    );
    for noncanonical_url in [
        "/api/v0/cat?arg=cid",
        "https://user@example.invalid/api/v0/cat?arg=cid",
        "https://example.invalid/api/v0/cat?z=1&a=2",
        "https://example.invalid/api/v0/cat?arg=%2f",
        "https://example.invalid/api/%41",
        "https://example.invalid/api/v0/cat?arg=cid#fragment",
    ] {
        assert!(
            GovernanceDagCanonicalRequestV1::try_new(
                GovernanceDagAuthenticationScope::Ipfs,
                "GET",
                noncanonical_url,
                Vec::new(),
                0,
                blake3_array(b""),
                1024,
            )
            .is_err(),
            "{noncanonical_url} must fail the canonical URL hard cut"
        );
    }
    assert!(
        GovernanceDagRequestAuthenticationEnvelopeV1::try_new(
            &canonical_test_request(
                GovernanceDagAuthenticationScope::Ipfs,
                "GET",
                "https://example.invalid/",
                &[],
                b"",
            ),
            0,
            1,
            [0; 32],
            [0; 32],
            [0; 64],
        )
        .is_err()
    );
    let client = Client::builder().no_proxy().build().expect("test client");
    let credential_request = client
        .get("https://example.invalid/")
        .header(header::AUTHORIZATION, "Bearer must-not-escape")
        .build()
        .expect("build credential-bearing request");
    assert!(
        canonical_outbound_request_descriptor(
            &credential_request,
            GovernanceDagAuthenticationScope::Ipfs,
            1024,
        )
        .is_err()
    );
    let unsorted_query = client
        .get("https://example.invalid/?z=1&a=2")
        .build()
        .expect("build noncanonical query request");
    assert!(
        canonical_outbound_request_descriptor(
            &unsorted_query,
            GovernanceDagAuthenticationScope::Ipfs,
            1024,
        )
        .is_err()
    );
}
#[test]
fn outbound_descriptor_binds_selected_headers_and_rejects_unsigned_semantics() {
    let body = b"canonical-body";
    let baseline_headers = [
        ("accept-encoding", b"identity".as_slice()),
        ("content-type", b"application/vnd.iroha.norito".as_slice()),
    ];
    let baseline = canonicalize_governance_dag_outbound_http_request_v1(
        GovernanceDagAuthenticationScope::SignedHead,
        "PUT",
        "https://example.invalid/governance/head",
        baseline_headers,
        body,
        1024,
    )
    .expect("canonical baseline descriptor");
    for unexpected_name in [
        "cache-control",
        "x-request-id",
        "x-http-method-override",
        "x-original-url",
        "forwarded",
    ] {
        let headers = [
            ("accept-encoding", b"identity".as_slice()),
            ("content-type", b"application/vnd.iroha.norito".as_slice()),
            ("content-length", b"14".as_slice()),
            (unexpected_name, b"semantic-extension".as_slice()),
        ];
        assert_eq!(
            canonicalize_governance_dag_outbound_http_request_v1(
                GovernanceDagAuthenticationScope::SignedHead,
                "PUT",
                "https://example.invalid/governance/head",
                headers,
                body,
                1024,
            ),
            Err(GovernanceDagRequestAuthenticationErrorV1::UnexpectedHeader)
        );
    }
    let changed_selected = canonicalize_governance_dag_outbound_http_request_v1(
        GovernanceDagAuthenticationScope::SignedHead,
        "PUT",
        "https://example.invalid/governance/head",
        [
            ("accept-encoding", b"gzip".as_slice()),
            ("content-type", b"application/vnd.iroha.norito".as_slice()),
            ("content-length", b"14".as_slice()),
        ],
        body,
        1024,
    )
    .expect("alternate selected public header remains canonical");
    assert_ne!(
        changed_selected.request_digest(),
        baseline.request_digest(),
        "a selected public header must change the signed request digest"
    );
}
#[test]
fn outbound_descriptor_rejects_credentials_auth_prefixes_and_ambiguous_framing() {
    for forbidden_name in [
        "authorization",
        "Proxy-Authorization",
        "cookie",
        "x-api-key",
        "x-auth-token",
        "x-sorafs-governance-auth-version",
        "X-Sorafs-Governance-Auth-Extension",
    ] {
        let error = canonicalize_governance_dag_outbound_http_request_v1(
            GovernanceDagAuthenticationScope::Ipfs,
            "GET",
            "https://example.invalid/api/v0/cat?arg=cid",
            [(forbidden_name, b"must-not-pass".as_slice())],
            b"",
            1024,
        )
        .expect_err("credential and authentication-prefix headers must fail closed");
        assert_eq!(
            error,
            GovernanceDagRequestAuthenticationErrorV1::ForbiddenHeader,
            "unexpected rejection for {forbidden_name}"
        );
    }
    let framing_cases = [
        vec![("content-length", b"13".as_slice())],
        vec![("content-length", b"014".as_slice())],
        vec![
            ("content-length", b"14".as_slice()),
            ("content-length", b"14".as_slice()),
        ],
        vec![("content-length", b"14, 14".as_slice())],
        vec![("Content-Length", b"14".as_slice())],
        vec![("transfer-encoding", b"chunked".as_slice())],
        vec![("Transfer-Encoding", b"identity".as_slice())],
    ];
    for headers in framing_cases {
        let error = canonicalize_governance_dag_outbound_http_request_v1(
            GovernanceDagAuthenticationScope::SignedHead,
            "PUT",
            "https://example.invalid/governance/head",
            headers,
            b"canonical-body",
            1024,
        )
        .expect_err("ambiguous HTTP framing must fail closed");
        assert_eq!(
            error,
            GovernanceDagRequestAuthenticationErrorV1::InvalidFraming
        );
    }
}
#[test]
fn public_auth_headers_preserve_final_body_and_conditional_headers() {
    let client = Client::builder().no_proxy().build().expect("test client");
    let mut request = client
        .put("https://example.invalid/governance/head")
        .header(header::ACCEPT_ENCODING, "identity")
        .header(header::CONTENT_TYPE, "application/vnd.iroha.norito")
        .header(header::IF_MATCH, "\"v7\"")
        .body(b"canonical-head".to_vec())
        .build()
        .expect("build final signed-head PUT");
    let descriptor = canonical_outbound_request_descriptor(
        &request,
        GovernanceDagAuthenticationScope::SignedHead,
        1024,
    )
    .expect("canonical final signed-head descriptor");
    let now = current_unix_timestamp_seconds();
    let envelope = signed_test_request_auth_envelope(
        TEST_HEAD_AUTH_HANDLE,
        &descriptor,
        now,
        now + 15,
        [0x44; 32],
    );
    attach_request_authentication_headers(&mut request, &envelope)
        .expect("attach fixed public authentication headers");
    assert_eq!(
        request
            .body()
            .and_then(reqwest::Body::as_bytes)
            .expect("byte body"),
        b"canonical-head"
    );
    assert_eq!(
        request.headers().get(header::IF_MATCH),
        Some(&HeaderValue::from_static("\"v7\""))
    );
    assert!(request.headers().get(header::AUTHORIZATION).is_none());
    assert!(request.headers().get(header::COOKIE).is_none());
    for name in GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1 {
        assert!(
            request.headers().contains_key(name),
            "missing fixed public request-auth header {name}"
        );
    }
}
#[tokio::test]
async fn authenticated_execute_discards_response_after_qualification_drift() {
    let provider = Arc::new(TestAuthenticator::new(
        TEST_IPFS_AUTH_HANDLE,
        "in-flight-secret-token",
    ));
    let router = Router::new()
        .route("/drift", get(mock_authenticator_drift))
        .with_state(provider.clone());
    let (endpoint, task) = spawn_router_with_authenticator(
        router,
        "/drift",
        GovernanceDagAuthenticationScope::Ipfs,
        provider,
    )
    .await;
    let request = endpoint
        .request(Method::GET, endpoint.url.clone())
        .expect("construct drift request");
    let error = match endpoint.execute(request, "drift request failed").await {
        Ok(_) => panic!("post-execute policy drift must discard the response"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("ingress qualification changed"));
    assert!(!error.to_string().contains("in-flight-secret-token"));
    task.abort();
}
#[tokio::test]
async fn authenticated_response_discards_body_when_qualification_drifts_before_eof() {
    let provider = Arc::new(TestAuthenticator::new(
        TEST_IPFS_AUTH_HANDLE,
        "response-lifetime-secret",
    ));
    let router = Router::new().route("/body", get(|| async { "authenticated-body" }));
    let (endpoint, task) = spawn_router_with_authenticator(
        router,
        "/body",
        GovernanceDagAuthenticationScope::Ipfs,
        provider.clone(),
    )
    .await;
    let request = endpoint
        .request(Method::GET, endpoint.url.clone())
        .expect("construct authenticated body request");
    let response = endpoint
        .execute(request, "authenticated body request failed")
        .await
        .expect("receive response under the original qualification");
    provider
        .qualification_revision
        .store(2, AtomicOrdering::SeqCst);
    let error = read_bounded_response(response, 1024)
        .await
        .expect_err("qualification drift before completed consumption must discard the body");
    assert!(error.to_string().contains("ingress qualification changed"));
    assert!(!error.to_string().contains("response-lifetime-secret"));
    task.abort();
}
#[tokio::test]
async fn runtime_registry_injection_reaches_startup_with_exact_bindings() {
    let root = secure_temp_dir();
    let view = runtime_boundary_view(root.path());
    let state_dir = view
        .service
        .state_dir
        .clone()
        .expect("test state directory");
    let request_auth_max_body_bytes =
        authenticated_ipfs_wire_body_max_bytes(view.service.max_request_bytes.0)
            .expect("derive authenticated wire-body binding");
    let registry = Arc::new(TestRuntimeProviderRegistry::returning(
        test_runtime_providers(
            &view,
            Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE)),
        ),
    ));
    let runtime_registry: Arc<dyn GovernanceDagServiceRuntimeProviderRegistryV1> = registry.clone();
    let providers = resolve_runtime_registry_providers(&view, Some(runtime_registry))
        .expect("registry resolves the configured providers");
    let _service = Service::from_view(view.clone(), providers)
        .await
        .expect("registry providers reach qualified service startup");
    let observed = registry
        .observed_bindings
        .lock()
        .expect("lock observed registry bindings")
        .clone()
        .expect("registry was called");
    assert_eq!(observed.ipfs_authenticator_handle(), TEST_IPFS_AUTH_HANDLE);
    assert_eq!(
        observed.ipfs_authenticator_qualification(),
        TEST_AUTH_QUALIFICATION
    );
    assert_eq!(
        observed.head_authenticator_handle(),
        Some(TEST_HEAD_AUTH_HANDLE)
    );
    assert_eq!(
        observed.head_authenticator_qualification(),
        Some(TEST_AUTH_QUALIFICATION)
    );
    assert_eq!(
        observed.ipfs_request_ingress_binding().max_body_bytes(),
        request_auth_max_body_bytes
    );
    assert_eq!(
        observed
            .head_request_ingress_binding()
            .expect("signed-head ingress binding")
            .max_body_bytes(),
        view.service.max_request_bytes.0
    );
    assert_eq!(
        observed.checkpoint_store_handle(),
        TEST_CHECKPOINT_STORE_HANDLE
    );
    assert_eq!(
        observed.checkpoint_store_qualification(),
        TEST_STORE_QUALIFICATION
    );
    assert!(state_dir.exists());
}
#[test]
fn embedding_launcher_preflight_qualifies_adapters_without_opening_state() {
    let root = secure_temp_dir();
    let view = runtime_boundary_view(root.path());
    let state_dir = view
        .service
        .state_dir
        .clone()
        .expect("test state directory");
    validate_governance_dag_service_runtime_providers(
        &view,
        &test_runtime_providers(
            &view,
            Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE)),
        ),
    )
    .expect("qualify the exact deployment adapter set");
    assert!(
        !state_dir.exists(),
        "provider-only launcher preflight must not open mutable state"
    );
    let error = validate_governance_dag_service_runtime_providers(
        &view,
        &GovernanceDagServiceRuntimeProviders::default(),
    )
    .expect_err("missing providers must fail launcher preflight");
    assert!(error.to_string().contains("no runtime provider"));
    assert!(!state_dir.exists());
    let error = validate_governance_dag_service_runtime_providers(
        &view,
        &test_runtime_providers(
            &view,
            Arc::new(TestSealedStore::new("kms:governance/checkpoint:test")),
        ),
    )
    .expect_err("test-marked provider must fail launcher preflight");
    assert!(error.to_string().contains("test-marked"));
    assert!(!state_dir.exists());
}
#[test]
fn ipns_runtime_bindings_omit_and_reject_signed_head_provider() {
    let root = secure_temp_dir();
    let mut view = runtime_boundary_view(root.path());
    view.service.head_mode = "ipns".to_owned();
    view.service.signed_head_url = None;
    view.service.ipns_name = Some("k51qzi5uqu5dl-governance".to_owned());
    view.service.ipns_key_name = Some("governance-publisher".to_owned());
    view.service.head_authenticator_handle = None;
    view.service.head_authenticator_revision = None;
    view.service.head_authenticator_policy_digest = None;
    view.service.head_request_auth_public_key = None;
    let bindings = runtime_provider_bindings(&view).expect("derive IPNS runtime bindings");
    assert_eq!(bindings.head_authenticator_handle(), None);
    assert_eq!(bindings.head_authenticator_qualification(), None);
    assert_eq!(bindings.head_request_ingress_binding(), None);
    let store = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let providers = test_runtime_providers(&view, store);
    validate_governance_dag_service_runtime_providers(&view, &providers)
        .expect("IPNS mode requires only IPFS authentication and sealed checkpoint providers");
    let substituted = GovernanceDagServiceRuntimeProviders {
        head_authenticator: Some(Arc::new(TestAuthenticator::new(
            TEST_HEAD_AUTH_HANDLE,
            "must-not-be-used",
        ))),
        ..providers
    };
    let error = validate_governance_dag_service_runtime_providers(&view, &substituted)
        .expect_err("a signed-head provider must fail closed in IPNS mode");
    assert!(error.to_string().contains("must be absent in IPNS mode"));
}
#[tokio::test]
async fn prepare_reconciles_initial_state_without_publication() {
    let root = secure_temp_dir();
    let mut view = runtime_boundary_view(root.path());
    let mut source = signed_source(1, 0x6d, current_unix_timestamp_seconds().saturating_sub(1));
    materialize_source_snapshot(
        view.source_dir.as_deref().expect("test source directory"),
        &mut source,
    );
    let publisher_key_hex = hex::encode(&source.head.head_signature.public_key);
    view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
    view.service.publisher_public_key_hex = Some(publisher_key_hex);
    view.service.allow_head_bootstrap = true;
    let (head_endpoint, head_state, task) = spawn_signed_head(SignedHeadInner::default()).await;
    view.service.signed_head_url = Some(head_endpoint.url.to_string());
    let checkpoint_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    seed_producer_checkpoint(
        &checkpoint_provider,
        view.source_dir.as_deref().expect("test source directory"),
        &source,
    );
    let runner = prepare_governance_dag_service_from_view(
        view.clone(),
        test_runtime_providers(&view, checkpoint_provider.clone()),
    )
    .await
    .expect("empty authenticated state may prepare for an allowed bootstrap");
    assert!(runner.service.checkpoint.is_none());
    assert!(runner.service.intent.is_none());
    assert_eq!(head_state.0.lock().await.put_count, 0);
    drop(runner);
    let checkpoint = checkpoint_from_source(&source);
    save_checkpoint(
        &test_checkpoint_store(checkpoint_provider.clone()),
        None,
        &checkpoint,
    )
    .expect("seed authenticated checkpoint");
    let error = prepare_governance_dag_service_from_view(
        view.clone(),
        test_runtime_providers(&view, checkpoint_provider),
    )
    .await
    .err()
    .expect("a missing public head cannot satisfy an existing checkpoint");
    assert!(error.to_string().contains("public head disappeared"));
    assert_eq!(
        head_state.0.lock().await.put_count,
        0,
        "prepare must not repair or publish the public head"
    );
    task.abort();
}
#[tokio::test]
async fn prepare_recovers_empty_typed_mirror_but_keeps_reader_unready() {
    let root = secure_temp_dir();
    let mut view = runtime_boundary_view(root.path());
    let mut source = signed_source(1, 0x70, current_unix_timestamp_seconds().saturating_sub(1));
    materialize_source_snapshot(
        view.source_dir.as_deref().expect("test source directory"),
        &mut source,
    );
    let publisher_key_hex = hex::encode(&source.head.head_signature.public_key);
    view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
    view.service.publisher_public_key_hex = Some(publisher_key_hex);
    let (head_endpoint, head_state, task) = spawn_signed_head(SignedHeadInner {
        bytes: Some(source.head_bytes.clone()),
        etag: "\"v1\"".to_owned(),
        ..SignedHeadInner::default()
    })
    .await;
    view.service.signed_head_url = Some(head_endpoint.url.to_string());
    let checkpoint_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    seed_producer_checkpoint(
        &checkpoint_provider,
        view.source_dir.as_deref().expect("test source directory"),
        &source,
    );
    let checkpoint = checkpoint_with_canonical_mirror(&source);
    let checkpoint_revision = save_checkpoint(
        &test_checkpoint_store(checkpoint_provider.clone()),
        None,
        &checkpoint,
    )
    .expect("seed authenticated checkpoint");
    let runner = prepare_governance_dag_service_from_view(
        view.clone(),
        test_runtime_providers(&view, checkpoint_provider),
    )
    .await
    .expect("prepare must recover an empty typed mirror");
    let mirror = verify_mirror_index_store(
        &runner.service.config,
        &runner.service.mirror_store,
        &checkpoint,
    )
    .expect("prepare installs a checkpoint-coherent mirror");
    let canonical_bytes = json::to_json_pretty(&mirror)
        .expect("encode recovered mirror")
        .into_bytes();
    assert_eq!(blake3_array(&canonical_bytes), checkpoint.mirror_blake3);
    assert_eq!(
        runner.service.checkpoint_revision,
        Some(checkpoint_revision)
    );
    let error = runner
        .mirror_read_handle()
        .read()
        .expect_err("prepare must keep mirror reads unavailable until the first full audit");
    assert!(
        matches!(error, GovernanceDagServiceError::Unavailable(_)),
        "unexpected pre-audit mirror-read error: {error}"
    );
    assert_eq!(
        head_state.0.lock().await.put_count,
        0,
        "startup mirror recovery must not publish the public head"
    );
    drop(runner);
    task.abort();
}
#[tokio::test]
async fn prepare_repairs_nonempty_checkpoint_incoherent_derived_mirror() {
    let root = secure_temp_dir();
    let mut view = runtime_boundary_view(root.path());
    let mut source = signed_source(1, 0x71, current_unix_timestamp_seconds().saturating_sub(1));
    materialize_source_snapshot(
        view.source_dir.as_deref().expect("test source directory"),
        &mut source,
    );
    let publisher_key_hex = hex::encode(&source.head.head_signature.public_key);
    view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
    view.service.publisher_public_key_hex = Some(publisher_key_hex);
    let (head_endpoint, head_state, task) = spawn_signed_head(SignedHeadInner {
        bytes: Some(source.head_bytes.clone()),
        etag: "\"v1\"".to_owned(),
        ..SignedHeadInner::default()
    })
    .await;
    view.service.signed_head_url = Some(head_endpoint.url.to_string());
    let checkpoint_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    seed_producer_checkpoint(
        &checkpoint_provider,
        view.source_dir.as_deref().expect("test source directory"),
        &source,
    );
    let checkpoint = checkpoint_with_canonical_mirror(&source);
    save_checkpoint(
        &test_checkpoint_store(checkpoint_provider.clone()),
        None,
        &checkpoint,
    )
    .expect("seed authenticated checkpoint");
    let service = Service::from_view(
        view.clone(),
        test_runtime_providers(&view, checkpoint_provider.clone()),
    )
    .await
    .expect("open service state for mismatch fixture");
    let mut drifted_mirror = mirror_index_value(
        &source,
        &checkpoint.mirror_blocks,
        &checkpoint.archive_head,
        checkpoint.generation,
        &checkpoint.head_ipfs_cid,
        checkpoint.published_at_unix,
    )
    .expect("build canonical mirror fixture");
    drifted_mirror
        .get_mut("head")
        .and_then(JsonValue::as_object_mut)
        .expect("mirror head object")
        .insert(
            "head_block_cid_hex".into(),
            JsonValue::from("00".repeat(32)),
        );
    let drifted_payload = MirrorIndexStorePayloadV1::committed(
        checkpoint.generation,
        [0; 32],
        json::to_json_pretty(&drifted_mirror)
            .expect("encode internally canonical drifted mirror")
            .into_bytes(),
    )
    .expect("construct internally valid drifted mirror payload");
    let (empty_snapshot, empty_payload) =
        load_mirror_index_store(&service.config, &service.mirror_store)
            .expect("load empty typed mirror");
    assert!(empty_payload.is_empty());
    compare_and_swap_mirror_index_store(
        &service.config,
        &service.mirror_store,
        &empty_snapshot,
        &drifted_payload,
    )
    .expect("install nonempty checkpoint-incoherent mirror fixture");
    drop(service);
    let runner = prepare_governance_dag_service_from_view(
        view.clone(),
        test_runtime_providers(&view, checkpoint_provider),
    )
    .await
    .expect("sealed checkpoint repairs a stale or corrupt local derived mirror");
    assert_eq!(
        verify_mirror_index_store(
            &runner.service.config,
            &runner.service.mirror_store,
            &checkpoint,
        )
        .expect("read repaired checkpoint-coherent mirror"),
        mirror_index_value(
            &source,
            &checkpoint.mirror_blocks,
            &checkpoint.archive_head,
            checkpoint.generation,
            &checkpoint.head_ipfs_cid,
            checkpoint.published_at_unix,
        )
        .expect("rebuild expected derived mirror")
    );
    assert_eq!(
        head_state.0.lock().await.put_count,
        0,
        "derived-cache recovery must not republish the public head"
    );
    drop(runner);
    task.abort();
}
#[tokio::test]
async fn prepare_rejects_source_conflicting_publish_intent_before_publication() {
    let root = secure_temp_dir();
    let mut view = runtime_boundary_view(root.path());
    let now = current_unix_timestamp_seconds().saturating_sub(1);
    let mut source = signed_source(1, 0x6e, now);
    materialize_source_snapshot(
        view.source_dir.as_deref().expect("test source directory"),
        &mut source,
    );
    let publisher_key_hex = hex::encode(&source.head.head_signature.public_key);
    view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
    view.service.publisher_public_key_hex = Some(publisher_key_hex);
    view.service.allow_head_bootstrap = true;
    let (head_endpoint, head_state, task) = spawn_signed_head(SignedHeadInner::default()).await;
    view.service.signed_head_url = Some(head_endpoint.url.to_string());
    let checkpoint_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    seed_producer_checkpoint(
        &checkpoint_provider,
        view.source_dir.as_deref().expect("test source directory"),
        &source,
    );
    let conflicting_source = signed_source(1, 0x6f, now);
    save_publish_intent(
        &test_checkpoint_store(checkpoint_provider.clone()),
        None,
        &intent_from_source(&conflicting_source),
    )
    .expect("seed independently valid but source-conflicting intent");
    let error = prepare_governance_dag_service_from_view(
        view.clone(),
        test_runtime_providers(&view, checkpoint_provider),
    )
    .await
    .err()
    .expect("prepare must reconcile the durable intent against the source");
    assert!(
        error.to_string().contains("source forked")
            || error.to_string().contains("incompatible with the source")
            || error
                .to_string()
                .contains("not an authenticated source prefix"),
        "unexpected source-conflicting intent error: {error}"
    );
    assert_eq!(head_state.0.lock().await.put_count, 0);
    task.abort();
}
#[tokio::test]
async fn sealed_producer_intent_blocks_all_publication_io_before_checkpoint_commit() {
    let root = secure_temp_dir();
    let mut view = runtime_boundary_view(root.path());
    let now = current_unix_timestamp_seconds().saturating_sub(2);
    let previous_source = signed_source(1, 0x70, now);
    let mut visible_uncommitted_source = signed_source(2, 0x70, now);
    let source_dir = view.source_dir.as_deref().expect("test source directory");
    materialize_source_snapshot(source_dir, &mut visible_uncommitted_source);
    let publisher_key_hex = hex::encode(&visible_uncommitted_source.head.head_signature.public_key);
    view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
    view.service.publisher_public_key_hex = Some(publisher_key_hex);
    let request_count = Arc::new(AtomicU64::new(0));
    let router = Router::new()
        .fallback(any(count_unexpected_publication_io))
        .with_state(request_count.clone());
    let (endpoint, task) = spawn_router(router, "/").await;
    view.service.ipfs_api_url = Some(endpoint.url.to_string());
    view.service.signed_head_url = Some(endpoint.url.to_string());
    let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let producer_checkpoint = seed_producer_checkpoint(&provider, source_dir, &previous_source);
    let intent = GovernanceDagSealedStateRecord::new(
        GovernanceDagSealedStateSlot::ProducerPublishIntent,
        producer_checkpoint.generation.saturating_add(1),
        vec![0xA5],
    );
    provider
        .compare_and_swap(
            GovernanceDagSealedStateSlot::ProducerPublishIntent,
            None,
            intent,
        )
        .expect("pause producer after sealing its intent");
    let mut service = Service::from_view(view.clone(), test_runtime_providers(&view, provider))
        .await
        .expect("construct service without performing public I/O");
    let error = service
        .reconcile_once()
        .await
        .expect_err("uncommitted producer transaction must block reconciliation");
    assert!(error.to_string().contains("active sealed publish intent"));
    assert_eq!(
        request_count.load(AtomicOrdering::SeqCst),
        0,
        "service must perform no Kubo or public-head I/O before producer checkpoint commit"
    );
    task.abort();
}
#[tokio::test]
async fn incomplete_service_intent_suffix_fails_before_all_publication_io() {
    let root = secure_temp_dir();
    let mut view = runtime_boundary_view(root.path());
    let mut source = signed_source(2, 0x7c, current_unix_timestamp_seconds().saturating_sub(2));
    let source_dir = view.source_dir.as_deref().expect("test source directory");
    materialize_source_snapshot(source_dir, &mut source);
    let publisher_key_hex = hex::encode(&source.head.head_signature.public_key);
    view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
    view.service.publisher_public_key_hex = Some(publisher_key_hex);
    let request_count = Arc::new(AtomicU64::new(0));
    let router = Router::new()
        .fallback(any(count_unexpected_publication_io))
        .with_state(request_count.clone());
    let (endpoint, task) = spawn_router(router, "/").await;
    view.service.ipfs_api_url = Some(endpoint.url.to_string());
    view.service.signed_head_url = Some(endpoint.url.to_string());
    let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    seed_producer_checkpoint(&provider, source_dir, &source);
    let mut incomplete_intent = intent_from_source(&source);
    incomplete_intent.blocks.remove(0);
    incomplete_intent.blocks[0].ipfs_cid = None;
    incomplete_intent.head_ipfs_cid = None;
    save_publish_intent(
        &test_checkpoint_store(provider.clone()),
        None,
        &incomplete_intent,
    )
    .expect("seal internally contiguous but incomplete service intent fixture");
    let mut service = Service::from_view(view.clone(), test_runtime_providers(&view, provider))
        .await
        .expect("construct service without performing public I/O");
    let error = service
        .reconcile_once()
        .await
        .expect_err("incomplete unpublished suffix must fail before publication");
    assert!(
        error
            .to_string()
            .contains("complete unpublished source suffix")
    );
    assert_eq!(
        request_count.load(AtomicOrdering::SeqCst),
        0,
        "invalid service intent must perform no Kubo or public-head I/O"
    );
    task.abort();
}
#[tokio::test]
async fn substituted_producer_binding_fails_before_all_publication_io() {
    let request_count = Arc::new(AtomicU64::new(0));
    let router = Router::new()
        .fallback(any(count_unexpected_publication_io))
        .with_state(request_count.clone());
    let (endpoint, task) = spawn_router(router, "/").await;
    for substitution in ["handle", "revision", "policy", "peer", "key"] {
        let root = secure_temp_dir();
        let mut view = runtime_boundary_view(root.path());
        let mut source = signed_source(1, 0x78, current_unix_timestamp_seconds().saturating_sub(1));
        let source_dir = view.source_dir.as_deref().expect("test source directory");
        materialize_source_snapshot(source_dir, &mut source);
        let publisher_key_hex = hex::encode(&source.head.head_signature.public_key);
        view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
        view.service.publisher_public_key_hex = Some(publisher_key_hex);
        view.service.ipfs_api_url = Some(endpoint.url.to_string());
        view.service.signed_head_url = Some(endpoint.url.to_string());
        let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        let mut checkpoint = producer_checkpoint_from_source(source_dir, &source);
        match substitution {
            "handle" => {
                checkpoint.signer_handle = "hsm:governance/source-signer:alternate".to_owned();
            }
            "revision" => checkpoint.signer_revision = 2,
            "policy" => checkpoint.signer_policy_digest = [0x84; 32],
            "peer" => checkpoint.publisher_peer_id = b"12D3KooWGovernanceAlternate".to_vec(),
            "key" => checkpoint.publisher_public_key = [0x55; 32],
            _ => unreachable!("enumerated producer substitution"),
        }
        let record = GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::ProducerCheckpoint,
            checkpoint.block_count.saturating_add(1),
            norito::to_bytes(&checkpoint).expect("encode substituted producer checkpoint"),
        );
        provider
            .compare_and_swap(
                GovernanceDagSealedStateSlot::ProducerCheckpoint,
                None,
                record,
            )
            .expect("seed substituted producer checkpoint");
        let mut service = Service::from_view(view.clone(), test_runtime_providers(&view, provider))
            .await
            .expect("construct service without performing public I/O");
        let error = service
            .reconcile_once()
            .await
            .expect_err("producer binding substitution must fail closed");
        assert!(
            error.to_string().contains("identity or generation"),
            "unexpected {substitution} substitution error: {error}"
        );
        assert_eq!(
            request_count.load(AtomicOrdering::SeqCst),
            0,
            "{substitution} substitution reached Kubo or public-head I/O"
        );
    }
    task.abort();
}
#[cfg(unix)]
#[tokio::test]
async fn replaced_source_or_state_root_fails_before_all_publication_io() {
    let request_count = Arc::new(AtomicU64::new(0));
    let router = Router::new()
        .fallback(any(count_unexpected_publication_io))
        .with_state(request_count.clone());
    let (endpoint, task) = spawn_router(router, "/").await;
    for replaced_role in ["source", "state"] {
        let root = secure_temp_dir();
        let mut view = runtime_boundary_view(root.path());
        let mut source = signed_source(1, 0x79, current_unix_timestamp_seconds().saturating_sub(1));
        let source_dir = view
            .source_dir
            .clone()
            .expect("test source directory is configured");
        let state_dir = view
            .service
            .state_dir
            .clone()
            .expect("test state directory is configured");
        materialize_source_snapshot(&source_dir, &mut source);
        let publisher_key_hex = hex::encode(&source.head.head_signature.public_key);
        view.producer_publisher_public_key_hex = Some(publisher_key_hex.clone());
        view.service.publisher_public_key_hex = Some(publisher_key_hex);
        view.service.ipfs_api_url = Some(endpoint.url.to_string());
        view.service.signed_head_url = Some(endpoint.url.to_string());
        let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        seed_producer_checkpoint(&provider, &source_dir, &source);
        let mut service = Service::from_view(view.clone(), test_runtime_providers(&view, provider))
            .await
            .expect("construct service with pinned source and state roots");
        let replaced = if replaced_role == "source" {
            source_dir
        } else {
            state_dir
        };
        let detached = root.path().join(format!("{replaced_role}.detached"));
        fs::rename(&replaced, &detached).expect("detach pinned service root");
        fs::create_dir(&replaced).expect("create replacement service root");
        fs::set_permissions(&replaced, fs::Permissions::from_mode(0o700))
            .expect("secure replacement service root");
        let marker = replaced.join("must-remain");
        fs::write(&marker, replaced_role.as_bytes()).expect("seed replacement marker");
        let error = service
            .reconcile_once()
            .await
            .expect_err("root replacement must fail before publication I/O");
        assert!(
            error.to_string().contains("root identity changed")
                || error.to_string().contains("changed identity")
                || error.to_string().contains("changed"),
            "unexpected {replaced_role} replacement error: {error}"
        );
        assert_eq!(
            fs::read(&marker).expect("replacement marker remains"),
            replaced_role.as_bytes()
        );
        assert_eq!(
            request_count.load(AtomicOrdering::SeqCst),
            0,
            "{replaced_role} replacement reached Kubo or public-head I/O"
        );
    }
    task.abort();
}
#[tokio::test]
async fn service_rejects_configured_provider_qualification_substitution_before_state_access() {
    let root = secure_temp_dir();
    let mut view = runtime_boundary_view(root.path());
    let state_dir = view
        .service
        .state_dir
        .clone()
        .expect("test state directory");
    view.service.ipfs_authenticator_policy_digest = Some([0x99; 32]);
    let error = Service::from_view(
        view.clone(),
        test_runtime_providers(
            &view,
            Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE)),
        ),
    )
    .await
    .err()
    .expect("substituted configured provider qualification must fail");
    assert!(
        error
            .to_string()
            .contains("qualification or ingress binding does not match configuration")
    );
    assert!(
        !state_dir.exists(),
        "qualification substitution must fail before mutable state is opened"
    );
}
#[tokio::test]
async fn runtime_registry_failures_precede_service_state() {
    let root = secure_temp_dir();
    let view = runtime_boundary_view(root.path());
    let state_dir = view
        .service
        .state_dir
        .clone()
        .expect("test state directory");
    let missing = resolve_runtime_registry_providers(&view, None)
        .expect_err("missing registry must fail closed");
    assert!(matches!(
        missing,
        GovernanceDagServiceLauncherError::MissingRuntimeProviderRegistry
    ));
    assert!(!state_dir.exists());
    let stale_registry: Arc<dyn GovernanceDagServiceRuntimeProviderRegistryV1> =
        Arc::new(TestRuntimeProviderRegistry::failing(
            GovernanceDagServiceRuntimeProviderRegistryErrorV1::StaleOrRevoked,
        ));
    let stale = resolve_runtime_registry_providers(&view, Some(stale_registry))
        .expect_err("stale registry must fail closed");
    assert!(matches!(
        stale,
        GovernanceDagServiceLauncherError::RuntimeProviderRegistry(
            GovernanceDagServiceRuntimeProviderRegistryErrorV1::StaleOrRevoked
        )
    ));
    assert!(!state_dir.exists());
    let default_registry: Arc<dyn GovernanceDagServiceRuntimeProviderRegistryV1> = Arc::new(
        TestRuntimeProviderRegistry::returning(GovernanceDagServiceRuntimeProviders::default()),
    );
    let providers = resolve_runtime_registry_providers(&view, Some(default_registry))
        .expect("registry may return an incomplete set for service qualification to reject");
    let error = Service::from_view(view.clone(), providers)
        .await
        .err()
        .expect("empty provider set must fail startup");
    assert!(error.to_string().contains("no runtime provider"));
    assert!(!state_dir.exists());
    for provider_handle in [
        "kms:governance/checkpoint:other",
        "kms:governance/checkpoint:test",
    ] {
        let registry: Arc<dyn GovernanceDagServiceRuntimeProviderRegistryV1> =
            Arc::new(TestRuntimeProviderRegistry::returning(
                test_runtime_providers(&view, Arc::new(TestSealedStore::new(provider_handle))),
            ));
        let providers = resolve_runtime_registry_providers(&view, Some(registry))
            .expect("registry returns provider for startup qualification");
        let error = Service::from_view(view.clone(), providers)
            .await
            .err()
            .expect("substituted or test provider must fail startup");
        if provider_handle.ends_with(":test") {
            assert!(error.to_string().contains("test-marked"));
        } else {
            assert!(error.to_string().contains("does not match"));
        }
        assert!(!state_dir.exists());
    }
}
#[tokio::test]
async fn service_fails_closed_when_runtime_providers_are_missing_or_mismatched() {
    let root = secure_temp_dir();
    let view = runtime_boundary_view(root.path());
    let state_dir = view
        .service
        .state_dir
        .clone()
        .expect("test state directory");
    let error = Service::from_view(
        view.clone(),
        GovernanceDagServiceRuntimeProviders::default(),
    )
    .await
    .err()
    .expect("missing sealed store must fail");
    assert!(error.to_string().contains("no runtime provider"));
    assert!(
        !state_dir.exists(),
        "missing provider must fail before mutable state is opened"
    );
    let mismatched_store = Arc::new(TestSealedStore::new("kms:governance/checkpoint:other"));
    let error = Service::from_view(
        view.clone(),
        GovernanceDagServiceRuntimeProviders {
            checkpoint_store: Some(mismatched_store),
            ..GovernanceDagServiceRuntimeProviders::default()
        },
    )
    .await
    .err()
    .expect("mismatched sealed store handle must fail");
    assert!(error.to_string().contains("does not match"));
    assert!(
        !state_dir.exists(),
        "substituted provider must fail before mutable state is opened"
    );
    let checkpoint_store = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let error = Service::from_view(
        view.clone(),
        GovernanceDagServiceRuntimeProviders {
            checkpoint_store: Some(checkpoint_store.clone()),
            ..GovernanceDagServiceRuntimeProviders::default()
        },
    )
    .await
    .err()
    .expect("missing IPFS authenticator must fail");
    assert!(error.to_string().contains("IPFS authentication"));
    let error = Service::from_view(
        view,
        GovernanceDagServiceRuntimeProviders {
            checkpoint_store: Some(checkpoint_store),
            ipfs_authenticator: Some(Arc::new(TestAuthenticator::new(
                TEST_IPFS_AUTH_HANDLE,
                "test-only-ipfs",
            ))),
            head_authenticator: None,
        },
    )
    .await
    .err()
    .expect("missing signed-head authenticator must fail");
    assert!(error.to_string().contains("signed-head authentication"));
    assert!(!state_dir.exists());
}
#[tokio::test]
async fn service_rejects_stale_providers_before_state_access() {
    let root = secure_temp_dir();
    let view = runtime_boundary_view(root.path());
    let state_dir = view
        .service
        .state_dir
        .clone()
        .expect("test state directory");
    let stale_store = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    stale_store
        .qualification_refuse
        .store(true, AtomicOrdering::SeqCst);
    let error = Service::from_view(view.clone(), test_runtime_providers(&view, stale_store))
        .await
        .err()
        .expect("stale sealed store must fail startup");
    let rendered = error.to_string();
    assert!(rendered.contains("stale"));
    assert!(!rendered.contains("must-never-escape"));
    assert!(
        !state_dir.exists(),
        "stale provider must fail before mutable state is opened"
    );
    let stale_ipfs = Arc::new(TestAuthenticator::new(
        TEST_IPFS_AUTH_HANDLE,
        "test-only-ipfs",
    ));
    stale_ipfs
        .qualification_refuse
        .store(true, AtomicOrdering::SeqCst);
    let error = Service::from_view(
        view.clone(),
        GovernanceDagServiceRuntimeProviders {
            checkpoint_store: Some(Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE))),
            ipfs_authenticator: Some(stale_ipfs),
            head_authenticator: Some(Arc::new(TestAuthenticator::new(
                TEST_HEAD_AUTH_HANDLE,
                "test-only-head",
            ))),
        },
    )
    .await
    .err()
    .expect("stale IPFS authenticator must fail startup");
    assert!(error.to_string().contains("stale"));
    assert!(!state_dir.exists());
    let stale_head = Arc::new(TestAuthenticator::new(
        TEST_HEAD_AUTH_HANDLE,
        "test-only-head",
    ));
    stale_head
        .qualification_refuse
        .store(true, AtomicOrdering::SeqCst);
    let error = Service::from_view(
        view.clone(),
        GovernanceDagServiceRuntimeProviders {
            checkpoint_store: Some(Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE))),
            ipfs_authenticator: Some(Arc::new(TestAuthenticator::new(
                TEST_IPFS_AUTH_HANDLE,
                "test-only-ipfs",
            ))),
            head_authenticator: Some(stale_head),
        },
    )
    .await
    .err()
    .expect("stale signed-head authenticator must fail startup");
    assert!(error.to_string().contains("stale"));
    assert!(!state_dir.exists());
}
#[tokio::test]
async fn service_rejects_test_marked_provider_before_state_access() {
    let root = secure_temp_dir();
    let view = runtime_boundary_view(root.path());
    let state_dir = view
        .service
        .state_dir
        .clone()
        .expect("test state directory");
    let mut test_marked_view = view;
    test_marked_view.service.checkpoint_store_handle =
        Some("kms:governance/checkpoint:test".to_owned());
    let error = Service::from_view(
        test_marked_view,
        GovernanceDagServiceRuntimeProviders {
            checkpoint_store: Some(Arc::new(TestSealedStore::new(
                "kms:governance/checkpoint:test",
            ))),
            ipfs_authenticator: Some(Arc::new(TestAuthenticator::new(
                TEST_IPFS_AUTH_HANDLE,
                "test-only-ipfs",
            ))),
            head_authenticator: Some(Arc::new(TestAuthenticator::new(
                TEST_HEAD_AUTH_HANDLE,
                "test-only-head",
            ))),
        },
    )
    .await
    .err()
    .expect("test-marked provider handle must fail startup");
    assert!(error.to_string().contains("test-marked"));
    assert!(!state_dir.exists());
}
