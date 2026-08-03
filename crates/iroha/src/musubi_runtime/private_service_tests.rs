// Private HTTP service test body included from the parent module.
#[test]
fn authorization_is_domain_bound_and_verifiable() {
    let (client, _) = client();
    let runtime = AuthenticatedMusubiPublicationRuntimeClientV1::from_iroha_client(
        &client,
        Duration::from_secs(5),
    )
    .expect("runtime client");
    let operation_id = [0x44; 32];
    let digest = [0x55; 32];
    let authorization = runtime
        .authorization(
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            operation_id,
            digest,
            1_000,
        )
        .expect("authorization");
    authorization
        .verify(
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            operation_id,
            digest,
            1_001,
        )
        .expect("verify exact authorization");
    assert!(
        authorization
            .verify(
                MusubiPublicationRuntimeOperationV1::ProviderReadback,
                operation_id,
                digest,
                1_001,
            )
            .is_err()
    );

    let mut substituted = authorization.clone();
    substituted.payload.domain = [0x99; 32];
    assert!(substituted.payload.validate().is_err());
    assert!(
        substituted.approvals[0]
            .signature
            .verify_hash(
                &substituted.approvals[0].public_key,
                HashOf::new(&substituted.payload),
            )
            .is_err()
    );
}

#[test]
fn authorization_verifier_enforces_fixed_clock_skew_bound() {
    let (client, _) = client();
    let runtime = AuthenticatedMusubiPublicationRuntimeClientV1::from_iroha_client(
        &client,
        Duration::from_secs(5),
    )
    .expect("runtime client");
    let operation = MusubiPublicationRuntimeOperationV1::StorageCoordination;
    let operation_id = [0x46; 32];
    let digest = [0x57; 32];
    let current_time_ms = 1_000;
    let issued_at_ms = current_time_ms + MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1;
    let authorization = runtime
        .authorization(operation, operation_id, digest, issued_at_ms)
        .expect("future-skew authorization");

    authorization
        .verify_with_clock_skew(
            operation,
            operation_id,
            digest,
            current_time_ms,
            MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1,
        )
        .expect("the exact protocol skew boundary is accepted");

    for (verification_time_ms, skew_ms) in [
        (
            current_time_ms,
            MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1 + 1,
        ),
        (0, 0),
    ] {
        let error = authorization
            .verify_with_clock_skew(
                operation,
                operation_id,
                digest,
                verification_time_ms,
                skew_ms,
            )
            .expect_err("an invalid verifier clock policy must fail closed");
        assert_eq!(error.code(), "MUSUBI_RUNTIME_CLOCK_INVALID");
        assert_eq!(
            error.class(),
            MusubiPublicationRuntimeTransportFailureClassV1::Permanent
        );
    }
}

#[test]
fn publisher_authorization_accepts_exact_multisig_quorum_and_rejects_bad_sets() {
    let operation = MusubiPublicationRuntimeOperationV1::StorageCoordination;
    let operation_id = [0x45; 32];
    let digest = [0x56; 32];
    let runtime = threshold_authorization_runtime(ThresholdAuthorizationSigningBehavior::Correct);
    let authorization = runtime
        .authorization(operation, operation_id, digest, 1_000)
        .expect("2-of-3 publisher authorization");
    assert_eq!(authorization.approvals.len(), 2);
    authorization
        .verify(operation, operation_id, digest, 1_001)
        .expect("verify multisig publisher authorization");

    for (behavior, code, class) in [
        (
            ThresholdAuthorizationSigningBehavior::BelowThreshold,
            "MUSUBI_RUNTIME_AUTHORIZATION_THRESHOLD_UNMET",
            MusubiPublicationRuntimeTransportFailureClassV1::Permanent,
        ),
        (
            ThresholdAuthorizationSigningBehavior::Duplicate,
            "MUSUBI_RUNTIME_AUTHORIZATION_APPROVALS_INVALID",
            MusubiPublicationRuntimeTransportFailureClassV1::Permanent,
        ),
        (
            ThresholdAuthorizationSigningBehavior::Unsorted,
            "MUSUBI_RUNTIME_AUTHORIZATION_APPROVALS_INVALID",
            MusubiPublicationRuntimeTransportFailureClassV1::Permanent,
        ),
        (
            ThresholdAuthorizationSigningBehavior::OverApprovalBound,
            "MUSUBI_RUNTIME_AUTHORIZATION_APPROVALS_INVALID",
            MusubiPublicationRuntimeTransportFailureClassV1::Permanent,
        ),
        (
            ThresholdAuthorizationSigningBehavior::WrongPayload,
            "MUSUBI_RUNTIME_AUTHORIZATION_SIGNATURE_INVALID",
            MusubiPublicationRuntimeTransportFailureClassV1::Permanent,
        ),
        (
            ThresholdAuthorizationSigningBehavior::RetryableFailure,
            "MUSUBI_RUNTIME_AUTHORIZATION_SIGNER_UNAVAILABLE",
            MusubiPublicationRuntimeTransportFailureClassV1::Retryable,
        ),
        (
            ThresholdAuthorizationSigningBehavior::PermanentFailure,
            "MUSUBI_RUNTIME_AUTHORIZATION_SIGNING_FAILED",
            MusubiPublicationRuntimeTransportFailureClassV1::Permanent,
        ),
    ] {
        let error = threshold_authorization_runtime(behavior)
            .authorization(operation, operation_id, digest, 1_000)
            .expect_err("invalid approval set must fail before transport");
        assert_eq!(error.code(), code);
        assert_eq!(error.class(), class);
    }
}

#[test]
fn publisher_authorization_counts_weight_and_revalidates_decoded_policy() {
    let operation = MusubiPublicationRuntimeOperationV1::StorageCoordination;
    let operation_id = [0x47; 32];
    let digest = [0x58; 32];
    let key_pairs: Vec<_> = (0_u8..3)
        .map(|index| {
            KeyPair::try_from_seed(vec![0x70 + index; 32], Algorithm::Ed25519)
                .expect("derive weighted publisher key")
        })
        .collect();
    let members = key_pairs
        .iter()
        .zip([3_u16, 1, 1])
        .map(|(key_pair, weight)| {
            MultisigMember::new(key_pair.public_key().clone(), weight)
                .expect("weighted publisher member")
        })
        .collect();
    let publisher = AccountId::new_multisig(
        MultisigPolicy::new(3, members).expect("weighted publisher policy"),
    );
    let payload = MusubiPublicationRuntimeAuthorizationPayloadV1 {
        domain: AUTH_DOMAIN_V1,
        version: 1,
        operation,
        operation_id,
        chain_id: ChainId::from("musubi-runtime-weighted-test"),
        publisher,
        request_digest: digest,
        issued_at_ms: 1_000,
        expires_at_ms: 2_000,
    };
    let authorization_for = |indices: &[usize]| {
        let mut approvals: Vec<_> = indices
            .iter()
            .map(|index| MusubiPublicationRuntimeAuthorizationApprovalV1 {
                public_key: key_pairs[*index].public_key().clone(),
                signature: SignatureOf::try_new(key_pairs[*index].private_key(), &payload)
                    .expect("weighted publisher signature"),
            })
            .collect();
        approvals.sort_by(|left, right| left.public_key.cmp(&right.public_key));
        MusubiPublicationRuntimeAuthorizationV1 {
            payload: payload.clone(),
            approvals,
        }
    };

    authorization_for(&[0])
        .verify(operation, operation_id, digest, 1_001)
        .expect("one weight-three member meets threshold three");
    let error = authorization_for(&[1, 2])
        .verify(operation, operation_id, digest, 1_001)
        .expect_err("two weight-one members remain below threshold three");
    assert_eq!(error.code(), "MUSUBI_RUNTIME_AUTHORIZATION_THRESHOLD_UNMET");

    let valid_policy = MultisigPolicy::new(
        1,
        vec![MultisigMember::new(key_pairs[0].public_key().clone(), 1).expect("publisher member")],
    )
    .expect("valid policy fixture");
    let mut unchecked_json =
        norito::json::to_value(&valid_policy).expect("serialize policy fixture");
    *unchecked_json
        .as_object_mut()
        .and_then(|object| object.get_mut("threshold"))
        .expect("policy threshold field") = norito::json::Value::from(0_u64);
    let unchecked_policy: MultisigPolicy = norito::json::from_value(unchecked_json)
        .expect("generic decoding materializes unchecked policy fields");
    let mut malformed = payload;
    malformed.publisher = AccountId::new_multisig(unchecked_policy);
    let malformed = MusubiPublicationRuntimeAuthorizationV1 {
        approvals: vec![MusubiPublicationRuntimeAuthorizationApprovalV1 {
            public_key: key_pairs[0].public_key().clone(),
            signature: SignatureOf::try_new(key_pairs[0].private_key(), &malformed)
                .expect("malformed-policy fixture signature"),
        }],
        payload: malformed,
    };
    let error = malformed
        .verify(operation, operation_id, digest, 1_001)
        .expect_err("a structurally invalid decoded controller must fail closed");
    assert_eq!(
        error.code(),
        "MUSUBI_RUNTIME_AUTHORIZATION_CONTROLLER_UNSUPPORTED"
    );
}

#[test]
fn software_publisher_authorizer_requires_one_matching_controller_key() {
    let keys: Vec<_> = (0_u8..2)
        .map(|index| {
            KeyPair::try_from_seed(vec![0xe0_u8 + index; 32], Algorithm::Ed25519)
                .expect("derive publisher key")
        })
        .collect();
    let members = keys
        .iter()
        .map(|key_pair| {
            MultisigMember::new(key_pair.public_key().clone(), 1).expect("publisher member")
        })
        .collect();
    let publisher =
        AccountId::new_multisig(MultisigPolicy::new(2, members).expect("publisher policy"));
    let error =
        SoftwareMusubiPublicationRuntimeAuthorizationSignerV1::new(publisher, keys[0].clone())
            .expect_err("software adapter must not pretend to collect a threshold");
    assert_eq!(
        error.code(),
        "MUSUBI_RUNTIME_MULTISIG_AUTH_PROVIDER_REQUIRED"
    );
}

#[test]
fn private_service_urls_reject_credentials_redirect_primitives_and_retired_upload() {
    for valid in [
        "https://seed.example/",
        "https://seed.example/private/",
        "https://127.0.0.1:8443/",
    ] {
        validate_publication_service_base_url(&Url::parse(valid).expect("valid URL"))
            .expect("accepted private HTTPS base");
    }
    for invalid in [
        "http://seed.example/",
        "https://user:secret@seed.example/",
        "https://seed.example/path",
        "https://seed.example/?token=secret",
        "https://seed.example/#fragment",
        "https://seed.example/v1/sorafs/upload/",
    ] {
        assert!(
            validate_publication_service_base_url(
                &Url::parse(invalid).expect("syntactically valid URL")
            )
            .is_err(),
            "accepted {invalid}"
        );
    }
}

#[test]
fn request_digest_binds_operation_length_and_body() {
    let body = b"canonical request";
    let digest = request_digest(
        MusubiPublicationRuntimeOperationV1::StorageCoordination,
        body,
    )
    .expect("bounded digest input");
    assert_ne!(
        digest,
        request_digest(MusubiPublicationRuntimeOperationV1::ProviderReadback, body)
            .expect("bounded digest input")
    );
    assert_ne!(
        digest,
        request_digest(
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            b"canonical request!"
        )
        .expect("bounded digest input")
    );
}

#[test]
fn seed_ingress_carries_exact_metadata_and_authorization_with_raw_car_body() {
    let (mut client, _) = client();
    client.headers.insert(
        "Authorization".to_owned(),
        "Basic must-not-cross-boundary".to_owned(),
    );
    client.headers.insert(
        "x-platform-secret".to_owned(),
        "must-not-cross-boundary".to_owned(),
    );
    let runtime = AuthenticatedMusubiPublicationRuntimeClientV1::from_iroha_client(
        &client,
        Duration::from_secs(5),
    )
    .expect("runtime client");
    let car = b"raw canonical SoraFS CAR bytes".to_vec();
    let operation_id = [0x31; 32];
    let stage_request = MusubiSeedIngressStageRequestV1 {
        version: 1,
        operation_id,
        binding: MusubiSeedIngressReceiptBindingV1 {
            chain_id: client.chain.clone(),
            genesis_block_hash: [0x32; 32],
            publisher: client.account.clone(),
            ingress_broker: client.account.clone(),
            seed_provider: ProviderId::new([0x33; 32]),
            semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0x34; 32]),
            archive_id: ArchiveId::new([0x35; 32]),
            car_body_digest: MusubiContentDigestV1::new(*blake3::hash(&car).as_bytes()),
            car_body_length: u64::try_from(car.len()).expect("fixture CAR length fits u64"),
            nonce: [0x36; 32],
        },
    };
    stage_request.validate().expect("valid stage metadata");
    let metadata = norito::encode_canonical(&stage_request).expect("encode stage metadata");
    let digest = request_digest(MusubiPublicationRuntimeOperationV1::SeedIngress, &metadata)
        .expect("bounded metadata digest");
    let authorization = runtime
        .authorization(
            MusubiPublicationRuntimeOperationV1::SeedIngress,
            operation_id,
            digest,
            1_000,
        )
        .expect("authorize exact metadata");
    let endpoint = publication_route(
        &Url::parse("https://seed.example/private/").expect("base URL"),
        SEED_INGRESS_ROUTE,
    )
    .expect("stage endpoint");
    let prepared = runtime
        .prepare_request(
            endpoint,
            APPLICATION_SORAFS_CAR,
            &authorization,
            Some(&metadata),
            car.clone(),
        )
        .expect("prepare seed-ingress request");

    assert_eq!(
        prepared.body().and_then(reqwest::blocking::Body::as_bytes),
        Some(car.as_slice())
    );
    assert!(prepared.headers().get("Authorization").is_none());
    assert!(prepared.headers().get("x-platform-secret").is_none());
    assert_eq!(
        prepared.url().path(),
        "/private/v1/musubi/publication/seed-ingress"
    );
    let encoded_metadata = prepared
        .headers()
        .get(SEED_INGRESS_METADATA_HEADER)
        .expect("typed metadata header")
        .to_str()
        .expect("ASCII metadata header");
    assert!(
        prepared
            .headers()
            .get(SEED_INGRESS_METADATA_HEADER)
            .expect("typed metadata header")
            .is_sensitive()
    );
    assert_eq!(
        encoded_metadata,
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&metadata)
    );
    let decoded_metadata = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded_metadata)
        .expect("decode metadata header");
    let decoded_request: MusubiSeedIngressStageRequestV1 =
        norito::decode_canonical(&decoded_metadata).expect("decode canonical metadata");
    assert_eq!(decoded_request, stage_request);

    let encoded_authorization = prepared
        .headers()
        .get(AUTHORIZATION_HEADER)
        .expect("authorization header")
        .to_str()
        .expect("ASCII authorization header");
    assert!(
        prepared
            .headers()
            .get(AUTHORIZATION_HEADER)
            .expect("authorization header")
            .is_sensitive()
    );
    let authorization_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded_authorization)
        .expect("decode authorization header");
    let decoded_authorization: MusubiPublicationRuntimeAuthorizationV1 =
        norito::decode_canonical(&authorization_bytes).expect("decode authorization");
    decoded_authorization
        .verify(
            MusubiPublicationRuntimeOperationV1::SeedIngress,
            operation_id,
            digest,
            1_001,
        )
        .expect("authorization binds exact metadata");
}

#[test]
fn private_service_constructs_and_verifies_exact_external_signer_payload() {
    let mut fixture = private_service_fixture(false);
    let broker_key = KeyPair::try_from_seed(
        b"musubi-publication-runtime-broker-test".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("derive broker key");
    let (provider, observed) = TestReceiptSigningProvider::new(
        fixture.request.binding.ingress_broker.clone(),
        broker_key,
        TestSigningBehavior::Correct,
    );
    fixture.service.receipt_signer = Box::new(provider);
    fixture.service.seed_ingress = Box::new(RecordingSeedIngress {
        provider: fixture.request.binding.seed_provider,
        calls: Arc::clone(&fixture.calls),
        fail_first: false,
        clock_after_stage: Some((Arc::clone(&fixture.clock), 1_901)),
    });

    let metadata = fixture.metadata.clone();
    let car = fixture.car.clone();
    let authorization = authorization_header(&fixture.runtime, &fixture.request, &metadata, 900);
    let response = seed_http_response(&mut fixture, &authorization, &metadata, &car, 901);
    assert_eq!(response.status, 200);
    let receipt: MusubiSeedIngressReceiptV1 =
        norito::decode_canonical_with_limits(&response.body, RESPONSE_DECODE_LIMITS)
            .expect("externally signed receipt");
    receipt
        .verify(&fixture.request.binding, 1_901)
        .expect("service verifies exact external approval");
    let observed = observed.lock().expect("observed signing payloads");
    assert_eq!(observed.as_slice(), &[receipt.payload.clone()]);
    assert_eq!(receipt.payload.binding, fixture.request.binding);
    assert_eq!(receipt.payload.issued_at_ms, 1_901);
    assert_eq!(receipt.payload.expires_at_ms, 61_901);
}

#[test]
fn private_service_rejects_invalid_or_unavailable_external_signer_and_retries_exactly() {
    for (behavior, expected_status, expected_retryable) in [
        (TestSigningBehavior::WrongPayload, 422, false),
        (TestSigningBehavior::WrongController, 422, false),
        (TestSigningBehavior::DuplicateApproval, 422, false),
        (TestSigningBehavior::RetryableFailure, 503, true),
    ] {
        let mut fixture = private_service_fixture(false);
        let broker_key = KeyPair::try_from_seed(
            b"musubi-publication-runtime-broker-test".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("derive broker key");
        let (provider, _) = TestReceiptSigningProvider::new(
            fixture.request.binding.ingress_broker.clone(),
            broker_key.clone(),
            behavior,
        );
        fixture.service.receipt_signer = Box::new(provider);
        let metadata = fixture.metadata.clone();
        let car = fixture.car.clone();
        let authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 10_000);
        let failed = seed_http_response(&mut fixture, &authorization, &metadata, &car, 10_001);
        let failure = decode_service_error(&failed);
        assert_eq!(failed.status, expected_status);
        assert_eq!(
            failure.code,
            MusubiPublicationServiceErrorCodeV1::ReceiptSigningUnavailable
        );
        assert_eq!(failure.retryable, expected_retryable);
        assert_eq!(*fixture.calls.lock().expect("seed calls"), 1);

        let (correct, _) = TestReceiptSigningProvider::new(
            fixture.request.binding.ingress_broker.clone(),
            broker_key,
            TestSigningBehavior::Correct,
        );
        fixture.service.receipt_signer = Box::new(correct);
        let fresh = authorization_header(&fixture.runtime, &fixture.request, &metadata, 10_002);
        let retried = seed_http_response(&mut fixture, &fresh, &metadata, &car, 10_003);
        assert_eq!(
            retried.status, 200,
            "exact signer retry must leave its tombstone usable"
        );
        assert_eq!(*fixture.calls.lock().expect("seed calls"), 2);
    }

    let mut fixture = private_service_fixture(false);
    let broker_key = KeyPair::try_from_seed(
        b"musubi-publication-runtime-broker-test".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("derive broker key");
    let (slow_provider, _) = TestReceiptSigningProvider::new(
        fixture.request.binding.ingress_broker.clone(),
        broker_key.clone(),
        TestSigningBehavior::Correct,
    );
    fixture.service.receipt_signer =
        Box::new(slow_provider.with_clock_after_signing(Arc::clone(&fixture.clock), 70_001));
    let metadata = fixture.metadata.clone();
    let car = fixture.car.clone();
    let authorization = authorization_header(&fixture.runtime, &fixture.request, &metadata, 10_000);
    let expired = seed_http_response(&mut fixture, &authorization, &metadata, &car, 10_001);
    let expired_error = decode_service_error(&expired);
    assert_eq!(expired.status, 503);
    assert_eq!(
        expired_error.code,
        MusubiPublicationServiceErrorCodeV1::ReceiptSigningUnavailable
    );
    assert!(expired_error.retryable);

    let (correct, _) = TestReceiptSigningProvider::new(
        fixture.request.binding.ingress_broker.clone(),
        broker_key,
        TestSigningBehavior::Correct,
    );
    fixture.service.receipt_signer = Box::new(correct);
    let fresh = authorization_header(&fixture.runtime, &fixture.request, &metadata, 70_002);
    let retried = seed_http_response(&mut fixture, &fresh, &metadata, &car, 70_003);
    assert_eq!(retried.status, 200);
    assert_eq!(*fixture.calls.lock().expect("seed calls"), 2);
}

#[test]
fn private_service_constructor_binds_signer_and_seed_backend_identities() {
    let broker_key = KeyPair::try_from_seed(
        b"musubi-publication-constructor-broker".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("derive broker key");
    let broker = AccountId::new(broker_key.public_key().clone());
    let provider = ProviderId::new([0xa1; 32]);
    let config = MusubiPublicationServiceConfigurationV1 {
        chain_id: ChainId::from("musubi-constructor-test"),
        genesis_block_hash: [0xa2; 32],
        ingress_broker: broker.clone(),
        seed_provider: provider,
        max_future_clock_skew_ms: 1_000,
        receipt_lifetime_ms: 60_000,
    };
    let journal_binding = MusubiPublicationServiceJournalBindingV1::from_configuration(&config);

    let wrong_broker_key = KeyPair::try_from_seed(
        b"musubi-publication-constructor-wrong-broker".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("derive wrong broker key");
    let wrong_broker = AccountId::new(wrong_broker_key.public_key().clone());
    let wrong_signer =
        SoftwareMusubiSeedIngressReceiptSignerV1::new(wrong_broker, wrong_broker_key)
            .expect("internally consistent wrong signer");
    let (signer_mismatch_clock, _) = TestPublicationClock::new(1_000);
    let signer_mismatch = MusubiPublicationPrivateServiceV1::new(
        config.clone(),
        Box::new(signer_mismatch_clock),
        Box::new(wrong_signer),
        Box::new(
            InMemoryMusubiPublicationServiceJournalV1::new(journal_binding.clone(), 2, 4)
                .expect("journal"),
        ),
        Box::new(RecordingSeedIngress {
            provider,
            calls: Arc::new(Mutex::new(0)),
            fail_first: false,
            clock_after_stage: None,
        }),
        Box::new(UnusedStorage),
        Box::new(UnusedReadback),
    );
    assert!(matches!(
        signer_mismatch,
        Err(MusubiPublicationServiceErrorCodeV1::IdentityMismatch)
    ));

    let signer = SoftwareMusubiSeedIngressReceiptSignerV1::new(broker, broker_key)
        .expect("matching software signer");
    let signer_debug = format!("{signer:?}");
    assert!(!signer_debug.contains("key_pair"));
    assert!(!signer_debug.contains("private"));
    let (provider_mismatch_clock, _) = TestPublicationClock::new(1_000);
    let provider_mismatch = MusubiPublicationPrivateServiceV1::new(
        config,
        Box::new(provider_mismatch_clock),
        Box::new(signer),
        Box::new(
            InMemoryMusubiPublicationServiceJournalV1::new(journal_binding, 2, 4).expect("journal"),
        ),
        Box::new(RecordingSeedIngress {
            provider: ProviderId::new([0xa3; 32]),
            calls: Arc::new(Mutex::new(0)),
            fail_first: false,
            clock_after_stage: None,
        }),
        Box::new(UnusedStorage),
        Box::new(UnusedReadback),
    );
    assert!(matches!(
        provider_mismatch,
        Err(MusubiPublicationServiceErrorCodeV1::IdentityMismatch)
    ));
}

#[test]
fn private_service_verifies_bounded_threshold_signing_provider() {
    let mut fixture = threshold_private_service_fixture(ThresholdSigningBehavior::Correct);
    let metadata = fixture.metadata.clone();
    let car = fixture.car.clone();
    let authorization = authorization_header(&fixture.runtime, &fixture.request, &metadata, 1_000);
    let response = seed_http_response(&mut fixture, &authorization, &metadata, &car, 1_001);
    assert_eq!(response.status, 200);
    let receipt: MusubiSeedIngressReceiptV1 =
        norito::decode_canonical_with_limits(&response.body, RESPONSE_DECODE_LIMITS)
            .expect("threshold receipt");
    receipt
        .verify(&fixture.request.binding, 1_001)
        .expect("2-of-3 broker receipt");
    assert_eq!(receipt.approvals.len(), 2);

    for behavior in [
        ThresholdSigningBehavior::BelowThreshold,
        ThresholdSigningBehavior::Empty,
        ThresholdSigningBehavior::Unsorted,
        ThresholdSigningBehavior::OverApprovalBound,
    ] {
        let mut fixture = threshold_private_service_fixture(behavior);
        let metadata = fixture.metadata.clone();
        let car = fixture.car.clone();
        let authorization =
            authorization_header(&fixture.runtime, &fixture.request, &metadata, 2_000);
        let response = seed_http_response(&mut fixture, &authorization, &metadata, &car, 2_001);
        let error = decode_service_error(&response);
        assert_eq!(response.status, 422);
        assert_eq!(
            error.code,
            MusubiPublicationServiceErrorCodeV1::ReceiptSigningUnavailable
        );
        assert!(!error.retryable);
    }
}

#[test]
fn private_service_rejects_broker_quorum_larger_than_receipt_bound() {
    let key_pairs: Vec<_> =
        (0_u8..u8::try_from(MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1 + 1)
            .expect("approval bound fits u8"))
            .map(|index| {
                KeyPair::try_from_seed(vec![index.saturating_add(1); 32], Algorithm::Ed25519)
                    .expect("derive impossible broker key")
            })
            .collect();
    let members = key_pairs
        .iter()
        .map(|key_pair| {
            MultisigMember::new(key_pair.public_key().clone(), 1).expect("impossible broker member")
        })
        .collect();
    let broker = AccountId::new_multisig(
        MultisigPolicy::new(
            u16::try_from(MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1 + 1)
                .expect("approval bound fits u16"),
            members,
        )
        .expect("65-of-65 account policy is structurally valid"),
    );
    let provider = ProviderId::new([0xb5; 32]);
    let config = MusubiPublicationServiceConfigurationV1 {
        chain_id: ChainId::from("musubi-impossible-broker-test"),
        genesis_block_hash: [0xb6; 32],
        ingress_broker: broker.clone(),
        seed_provider: provider,
        max_future_clock_skew_ms: 1_000,
        receipt_lifetime_ms: 60_000,
    };
    let journal_binding = MusubiPublicationServiceJournalBindingV1::from_configuration(&config);
    let signer = ThresholdReceiptSigningProvider {
        broker,
        key_pairs,
        behavior: ThresholdSigningBehavior::Correct,
    };
    let (clock, _) = TestPublicationClock::new(1_000);
    let service = MusubiPublicationPrivateServiceV1::new(
        config,
        Box::new(clock),
        Box::new(signer),
        Box::new(ConflictJournal {
            binding: journal_binding,
        }),
        Box::new(RecordingSeedIngress {
            provider,
            calls: Arc::new(Mutex::new(0)),
            fail_first: false,
            clock_after_stage: None,
        }),
        Box::new(UnusedStorage),
        Box::new(UnusedReadback),
    );
    assert!(matches!(
        service,
        Err(MusubiPublicationServiceErrorCodeV1::IdentityMismatch)
    ));
}

#[test]
fn private_service_rejects_trusted_clock_regression() {
    let mut fixture = private_service_fixture(false);
    let metadata = fixture.metadata.clone();
    let car = fixture.car.clone();
    let authorization = authorization_header(&fixture.runtime, &fixture.request, &metadata, 3_000);
    let first = seed_http_response(&mut fixture, &authorization, &metadata, &car, 3_001);
    assert_eq!(first.status, 200);

    let regressed_authorization =
        authorization_header(&fixture.runtime, &fixture.request, &metadata, 2_999);
    let regressed = seed_http_response(
        &mut fixture,
        &regressed_authorization,
        &metadata,
        &car,
        3_000,
    );
    let error = decode_service_error(&regressed);
    assert_eq!(regressed.status, 503);
    assert_eq!(
        error.code,
        MusubiPublicationServiceErrorCodeV1::TrustedClockUnavailable
    );
    assert!(error.retryable);

    let recovered_authorization =
        authorization_header(&fixture.runtime, &fixture.request, &metadata, 3_002);
    let recovered = seed_http_response(
        &mut fixture,
        &recovered_authorization,
        &metadata,
        &car,
        3_003,
    );
    assert_eq!(recovered.status, 200);
    assert_eq!(recovered.body, first.body);
    assert_eq!(*fixture.calls.lock().expect("seed calls"), 1);
}

#[test]
fn private_service_classifies_receipt_expiry_overflow_as_clock_failure() {
    let mut fixture = private_service_fixture(false);
    fixture.service.seed_ingress = Box::new(RecordingSeedIngress {
        provider: fixture.request.binding.seed_provider,
        calls: Arc::clone(&fixture.calls),
        fail_first: false,
        clock_after_stage: Some((Arc::clone(&fixture.clock), u64::MAX.saturating_sub(59_999))),
    });
    let metadata = fixture.metadata.clone();
    let car = fixture.car.clone();
    let authorization = authorization_header(&fixture.runtime, &fixture.request, &metadata, 4_000);
    let response = seed_http_response(&mut fixture, &authorization, &metadata, &car, 4_001);
    let error = decode_service_error(&response);
    assert_eq!(response.status, 503);
    assert_eq!(
        error.code,
        MusubiPublicationServiceErrorCodeV1::TrustedClockUnavailable
    );
    assert!(!error.retryable);
    assert_eq!(*fixture.calls.lock().expect("seed calls"), 1);
}

#[test]
fn private_service_returns_one_broker_receipt_and_reuses_exact_completed_operation() {
    let mut fixture = private_service_fixture(false);
    let metadata = fixture.metadata.clone();
    let car = fixture.car.clone();
    let first_authorization =
        authorization_header(&fixture.runtime, &fixture.request, &metadata, 1_000);
    let first = seed_http_response(&mut fixture, &first_authorization, &metadata, &car, 1_001);
    assert_eq!(first.status, 200);
    let receipt: MusubiSeedIngressReceiptV1 =
        norito::decode_canonical_with_limits(&first.body, RESPONSE_DECODE_LIMITS)
            .expect("signed receipt");
    receipt
        .verify(&fixture.request.binding, 1_001)
        .expect("receipt binds the exact staged CAR");

    let retry_authorization =
        authorization_header(&fixture.runtime, &fixture.request, &metadata, 1_002);
    let retry = seed_http_response(&mut fixture, &retry_authorization, &metadata, &car, 1_003);
    assert_eq!(retry.status, 200);
    assert_eq!(retry.body, first.body);
    assert_eq!(*fixture.calls.lock().expect("seed calls"), 1);

    let mut substituted = fixture.request.clone();
    substituted.binding.archive_id = ArchiveId::new([0x7a; 32]);
    substituted.binding.nonce = [0x7b; 32];
    let substituted_metadata =
        norito::encode_canonical(&substituted).expect("substituted metadata");
    let substituted_authorization =
        authorization_header(&fixture.runtime, &substituted, &substituted_metadata, 1_004);
    let conflict = seed_http_response(
        &mut fixture,
        &substituted_authorization,
        &substituted_metadata,
        &car,
        1_005,
    );
    assert_eq!(conflict.status, 422);
    assert_eq!(
        decode_service_error(&conflict).code,
        MusubiPublicationServiceErrorCodeV1::OperationConflict
    );
    assert_eq!(*fixture.calls.lock().expect("seed calls"), 1);

    let refresh_authorization =
        authorization_header(&fixture.runtime, &fixture.request, &metadata, 62_000);
    let refreshed = seed_http_response(
        &mut fixture,
        &refresh_authorization,
        &metadata,
        &car,
        62_001,
    );
    assert_eq!(refreshed.status, 200);
    assert_ne!(refreshed.body, first.body);
    let refreshed_receipt: MusubiSeedIngressReceiptV1 =
        norito::decode_canonical_with_limits(&refreshed.body, RESPONSE_DECODE_LIMITS)
            .expect("refreshed receipt");
    refreshed_receipt
        .verify(&fixture.request.binding, 62_001)
        .expect("refreshed receipt is live and exact");
    assert_eq!(*fixture.calls.lock().expect("seed calls"), 2);
}

#[test]
fn private_service_rejects_consumed_authorization_but_accepts_fresh_retry() {
    let mut fixture = private_service_fixture(true);
    let metadata = fixture.metadata.clone();
    let car = fixture.car.clone();
    let authorization = authorization_header(&fixture.runtime, &fixture.request, &metadata, 2_000);
    let failed = seed_http_response(&mut fixture, &authorization, &metadata, &car, 2_001);
    let failed_error = decode_service_error(&failed);
    assert_eq!(failed.status, 503);
    assert_eq!(
        failed_error.code,
        MusubiPublicationServiceErrorCodeV1::SeedIngressUnavailable
    );
    assert!(failed_error.retryable);

    let replay = seed_http_response(&mut fixture, &authorization, &metadata, &car, 2_002);
    assert_eq!(replay.status, 401);
    assert_eq!(
        decode_service_error(&replay).code,
        MusubiPublicationServiceErrorCodeV1::AuthorizationReplay
    );
    assert_eq!(*fixture.calls.lock().expect("seed calls"), 1);

    let mut substituted = fixture.request.clone();
    substituted.binding.nonce = [0x7c; 32];
    let substituted_metadata =
        norito::encode_canonical(&substituted).expect("substituted metadata");
    let substituted_authorization =
        authorization_header(&fixture.runtime, &substituted, &substituted_metadata, 2_003);
    let conflict = seed_http_response(
        &mut fixture,
        &substituted_authorization,
        &substituted_metadata,
        &car,
        2_004,
    );
    assert_eq!(conflict.status, 422);
    assert_eq!(
        decode_service_error(&conflict).code,
        MusubiPublicationServiceErrorCodeV1::OperationConflict
    );
    assert_eq!(*fixture.calls.lock().expect("seed calls"), 1);

    let fresh = authorization_header(&fixture.runtime, &fixture.request, &metadata, 2_005);
    let success = seed_http_response(&mut fixture, &fresh, &metadata, &car, 2_006);
    assert_eq!(success.status, 200);
    assert_eq!(*fixture.calls.lock().expect("seed calls"), 2);
}

#[test]
fn private_service_rejects_expiry_signer_metadata_and_car_substitution() {
    let mut fixture = private_service_fixture(false);
    let metadata = fixture.metadata.clone();
    let car = fixture.car.clone();

    let expired_authorization =
        authorization_header(&fixture.runtime, &fixture.request, &metadata, 3_000);
    let expired = seed_http_response(
        &mut fixture,
        &expired_authorization,
        &metadata,
        &car,
        33_001,
    );
    assert_eq!(
        decode_service_error(&expired).code,
        MusubiPublicationServiceErrorCodeV1::AuthorizationExpired
    );

    let future_authorization =
        authorization_header(&fixture.runtime, &fixture.request, &metadata, 50_000);
    let future = seed_http_response(&mut fixture, &future_authorization, &metadata, &car, 33_002);
    assert_eq!(
        decode_service_error(&future).code,
        MusubiPublicationServiceErrorCodeV1::AuthorizationExpired
    );

    let valid_authorization =
        authorization_header(&fixture.runtime, &fixture.request, &metadata, 33_003);
    let mut substituted_request = fixture.request.clone();
    substituted_request.binding.nonce = [0x77; 32];
    let substituted_metadata =
        norito::encode_canonical(&substituted_request).expect("substituted metadata");
    let substituted = seed_http_response(
        &mut fixture,
        &valid_authorization,
        &substituted_metadata,
        &car,
        33_004,
    );
    assert_eq!(
        decode_service_error(&substituted).code,
        MusubiPublicationServiceErrorCodeV1::AuthorizationInvalid
    );

    let mut wrong_body = car.clone();
    wrong_body[0] ^= 0x01;
    let wrong_body_response = seed_http_response(
        &mut fixture,
        &valid_authorization,
        &metadata,
        &wrong_body,
        33_004,
    );
    assert_eq!(
        decode_service_error(&wrong_body_response).code,
        MusubiPublicationServiceErrorCodeV1::CarBodyMismatch
    );

    let authorization_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&valid_authorization)
        .expect("decode authorization");
    let mut wrong_signer: MusubiPublicationRuntimeAuthorizationV1 =
        norito::decode_canonical(&authorization_bytes).expect("authorization");
    let attacker = KeyPair::try_from_seed(
        b"musubi-publication-runtime-attacker".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("attacker key");
    wrong_signer.approvals[0] = MusubiPublicationRuntimeAuthorizationApprovalV1 {
        public_key: attacker.public_key().clone(),
        signature: SignatureOf::try_new(attacker.private_key(), &wrong_signer.payload)
            .expect("attacker signature"),
    };
    let wrong_signer = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(norito::encode_canonical(&wrong_signer).expect("wrong-signer authorization"));
    let wrong_signer_response =
        seed_http_response(&mut fixture, &wrong_signer, &metadata, &car, 33_004);
    assert_eq!(
        decode_service_error(&wrong_signer_response).code,
        MusubiPublicationServiceErrorCodeV1::AuthorizationInvalid
    );
    assert_eq!(*fixture.calls.lock().expect("seed calls"), 0);
}

#[test]
fn private_service_rejects_noncanonical_headers_and_nonexact_routes() {
    let mut fixture = private_service_fixture(false);
    let metadata = fixture.metadata.clone();
    let car = fixture.car.clone();
    let authorization = authorization_header(&fixture.runtime, &fixture.request, &metadata, 6_000);
    let encoded_metadata = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&metadata);
    fixture.clock.store(6_001, Ordering::SeqCst);

    let route = fixture
        .service
        .handle(MusubiPublicationPrivateHttpRequestV1 {
            method: "POST",
            path: "/v1/musubi/publication/seed-ingress/extra",
            content_type: APPLICATION_SORAFS_CAR,
            authorization: Some(&authorization),
            seed_ingress_metadata: Some(&encoded_metadata),
            body: &car,
        });
    assert_eq!(route.status, 404);
    assert_eq!(
        decode_service_error(&route).code,
        MusubiPublicationServiceErrorCodeV1::RouteNotFound
    );

    let method = fixture
        .service
        .handle(MusubiPublicationPrivateHttpRequestV1 {
            method: "GET",
            path: MUSUBI_PUBLICATION_SEED_INGRESS_PATH_V1,
            content_type: APPLICATION_SORAFS_CAR,
            authorization: Some(&authorization),
            seed_ingress_metadata: Some(&encoded_metadata),
            body: &car,
        });
    assert_eq!(method.status, 405);
    assert_eq!(
        decode_service_error(&method).code,
        MusubiPublicationServiceErrorCodeV1::MethodInvalid
    );

    let padded_authorization = format!("{authorization}=");
    let noncanonical = fixture
        .service
        .handle(MusubiPublicationPrivateHttpRequestV1 {
            method: "POST",
            path: MUSUBI_PUBLICATION_SEED_INGRESS_PATH_V1,
            content_type: APPLICATION_SORAFS_CAR,
            authorization: Some(&padded_authorization),
            seed_ingress_metadata: Some(&encoded_metadata),
            body: &car,
        });
    assert_eq!(noncanonical.status, 401);
    assert_eq!(
        decode_service_error(&noncanonical).code,
        MusubiPublicationServiceErrorCodeV1::AuthorizationInvalid
    );

    let malformed_control = fixture
        .service
        .handle(MusubiPublicationPrivateHttpRequestV1 {
            method: "POST",
            path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
            content_type: APPLICATION_NORITO,
            authorization: Some(&authorization),
            seed_ingress_metadata: None,
            body: b"not norito",
        });
    assert_eq!(malformed_control.status, 422);
    assert_eq!(
        decode_service_error(&malformed_control).code,
        MusubiPublicationServiceErrorCodeV1::RequestInvalid
    );
}

#[test]
fn private_service_authenticates_control_requests_before_embedded_signatures() {
    let mut fixture = control_service_fixture(false, false);
    let attacker = KeyPair::try_from_seed(
        b"musubi-publication-control-signature-attacker".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("attacker key");

    let mut storage_request = fixture.storage_request.clone();
    let receipt_hash = storage_request.staging_receipt.payload.signing_hash();
    storage_request.staging_receipt.approvals[0].signature =
        SignatureOf::try_from_hash(attacker.private_key(), receipt_hash)
            .expect("mismatched receipt signature");
    let storage_body = norito::encode_canonical(&storage_request).expect("storage request bytes");
    fixture.clock.store(2_001, Ordering::SeqCst);
    let anonymous_storage = fixture
        .service
        .handle(MusubiPublicationPrivateHttpRequestV1 {
            method: "POST",
            path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
            content_type: APPLICATION_NORITO,
            authorization: None,
            seed_ingress_metadata: None,
            body: &storage_body,
        });
    assert_eq!(anonymous_storage.status, 401);
    assert_eq!(
        decode_service_error(&anonymous_storage).code,
        MusubiPublicationServiceErrorCodeV1::AuthorizationInvalid
    );

    let storage_authorization = control_authorization_header(
        &fixture.runtime,
        MusubiPublicationRuntimeOperationV1::StorageCoordination,
        storage_request.operation_id,
        &storage_body,
        2_000,
    );
    let authenticated_storage = fixture
        .service
        .handle(MusubiPublicationPrivateHttpRequestV1 {
            method: "POST",
            path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
            content_type: APPLICATION_NORITO,
            authorization: Some(&storage_authorization),
            seed_ingress_metadata: None,
            body: &storage_body,
        });
    assert_eq!(authenticated_storage.status, 422);
    assert_eq!(
        decode_service_error(&authenticated_storage).code,
        MusubiPublicationServiceErrorCodeV1::RequestInvalid
    );

    let valid_storage_body =
        norito::encode_canonical(&fixture.storage_request).expect("valid storage request bytes");
    let valid_storage_authorization = control_authorization_header(
        &fixture.runtime,
        MusubiPublicationRuntimeOperationV1::StorageCoordination,
        fixture.storage_request.operation_id,
        &valid_storage_body,
        2_002,
    );
    fixture.clock.store(2_003, Ordering::SeqCst);
    let valid_storage = fixture
        .service
        .handle(MusubiPublicationPrivateHttpRequestV1 {
            method: "POST",
            path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
            content_type: APPLICATION_NORITO,
            authorization: Some(&valid_storage_authorization),
            seed_ingress_metadata: None,
            body: &valid_storage_body,
        });
    assert_eq!(valid_storage.status, 200);

    let mut readback_request = fixture.readback_request.clone();
    let attestation = &mut readback_request.location.provider_attestations[0];
    let attestation_hash = attestation.payload.signing_hash();
    attestation.approvals[0].signature =
        SignatureOf::try_from_hash(attacker.private_key(), attestation_hash)
            .expect("mismatched provider signature");
    let readback_body =
        norito::encode_canonical(&readback_request).expect("readback request bytes");
    fixture.clock.store(3_001, Ordering::SeqCst);
    let anonymous_readback = fixture
        .service
        .handle(MusubiPublicationPrivateHttpRequestV1 {
            method: "POST",
            path: MUSUBI_PUBLICATION_PROVIDER_READBACK_PATH_V1,
            content_type: APPLICATION_NORITO,
            authorization: None,
            seed_ingress_metadata: None,
            body: &readback_body,
        });
    assert_eq!(anonymous_readback.status, 401);
    assert_eq!(
        decode_service_error(&anonymous_readback).code,
        MusubiPublicationServiceErrorCodeV1::AuthorizationInvalid
    );

    let readback_authorization = control_authorization_header(
        &fixture.runtime,
        MusubiPublicationRuntimeOperationV1::ProviderReadback,
        readback_request.operation_id,
        &readback_body,
        3_000,
    );
    let authenticated_readback = fixture
        .service
        .handle(MusubiPublicationPrivateHttpRequestV1 {
            method: "POST",
            path: MUSUBI_PUBLICATION_PROVIDER_READBACK_PATH_V1,
            content_type: APPLICATION_NORITO,
            authorization: Some(&readback_authorization),
            seed_ingress_metadata: None,
            body: &readback_body,
        });
    assert_eq!(authenticated_readback.status, 422);
    assert_eq!(
        decode_service_error(&authenticated_readback).code,
        MusubiPublicationServiceErrorCodeV1::RequestInvalid
    );
}

#[test]
fn authenticated_staging_receipt_failures_have_exact_deadletter_reasons() {
    let mut invalid_fixture = control_service_fixture(false, false);
    let attacker = KeyPair::try_from_seed(
        b"musubi-publication-invalid-receipt-observer".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("attacker key");
    let mut invalid_request = invalid_fixture.storage_request.clone();
    let receipt_hash = invalid_request.staging_receipt.payload.signing_hash();
    invalid_request.staging_receipt.approvals[0].signature =
        SignatureOf::try_from_hash(attacker.private_key(), receipt_hash)
            .expect("mismatched receipt signature");
    let invalid_body =
        norito::encode_canonical(&invalid_request).expect("invalid receipt request bytes");
    let invalid_authorization = control_authorization_header(
        &invalid_fixture.runtime,
        MusubiPublicationRuntimeOperationV1::StorageCoordination,
        invalid_request.operation_id,
        &invalid_body,
        2_000,
    );
    let invalid_error = invalid_fixture
        .service
        .handle_storage_coordination(
            MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
                content_type: APPLICATION_NORITO,
                authorization: Some(&invalid_authorization),
                seed_ingress_metadata: None,
                body: &invalid_body,
            },
            2_001,
        )
        .expect_err("authenticated invalid staging receipt");
    assert_eq!(
        invalid_error.telemetry,
        Some(MusubiPublicationServiceTelemetryEventV1::IngestDeadletter(
            MusubiIngestDeadletterReasonV1::ReceiptInvalid,
        ))
    );

    let mut future_fixture = control_service_fixture(false, false);
    future_fixture.service.config.max_future_clock_skew_ms = 1;
    let future_body = norito::encode_canonical(&future_fixture.storage_request)
        .expect("future receipt request bytes");
    let future_authorization = control_authorization_header(
        &future_fixture.runtime,
        MusubiPublicationRuntimeOperationV1::StorageCoordination,
        future_fixture.storage_request.operation_id,
        &future_body,
        2,
    );
    let future_error = future_fixture
        .service
        .handle_storage_coordination(
            MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
                content_type: APPLICATION_NORITO,
                authorization: Some(&future_authorization),
                seed_ingress_metadata: None,
                body: &future_body,
            },
            2,
        )
        .expect_err("authenticated future-skewed staging receipt");
    assert_eq!(
        future_error.telemetry,
        Some(MusubiPublicationServiceTelemetryEventV1::IngestDeadletter(
            MusubiIngestDeadletterReasonV1::ReceiptInvalid,
        ))
    );
}

#[test]
fn storage_coordination_accepts_an_expired_receipt_for_the_exact_finalized_archive() {
    let mut fixture = control_service_fixture(false, false);
    let body = norito::encode_canonical(&fixture.storage_request)
        .expect("expired finalized archive request bytes");
    let authorization = control_authorization_header(
        &fixture.runtime,
        MusubiPublicationRuntimeOperationV1::StorageCoordination,
        fixture.storage_request.operation_id,
        &body,
        120_000,
    );
    let response = fixture
        .service
        .handle_storage_coordination(
            MusubiPublicationPrivateHttpRequestV1 {
                method: "POST",
                path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
                content_type: APPLICATION_NORITO,
                authorization: Some(&authorization),
                seed_ingress_metadata: None,
                body: &body,
            },
            120_001,
        )
        .expect("finalized archive outlives its registration receipt");
    let decoded: MusubiStorageCoordinationResponseV1 =
        norito::decode_canonical_with_limits(&response, RESPONSE_DECODE_LIMITS)
            .expect("storage response");
    assert_eq!(decoded, fixture.storage_response);
}

#[test]
fn private_service_accepts_exact_storage_and_provider_readback_evidence() {
    let mut fixture = control_service_fixture(false, false);
    let storage_body =
        norito::encode_canonical(&fixture.storage_request).expect("storage request bytes");
    let storage_authorization = control_authorization_header(
        &fixture.runtime,
        MusubiPublicationRuntimeOperationV1::StorageCoordination,
        fixture.storage_request.operation_id,
        &storage_body,
        2_000,
    );
    fixture.clock.store(2_001, Ordering::SeqCst);
    let storage = fixture
        .service
        .handle(MusubiPublicationPrivateHttpRequestV1 {
            method: "POST",
            path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
            content_type: APPLICATION_NORITO,
            authorization: Some(&storage_authorization),
            seed_ingress_metadata: None,
            body: &storage_body,
        });
    assert_eq!(storage.status, 200);
    let storage_response: MusubiStorageCoordinationResponseV1 =
        norito::decode_canonical_with_limits(&storage.body, RESPONSE_DECODE_LIMITS)
            .expect("storage response");
    assert_eq!(storage_response, fixture.storage_response);

    let readback_body =
        norito::encode_canonical(&fixture.readback_request).expect("readback request bytes");
    let readback_authorization = control_authorization_header(
        &fixture.runtime,
        MusubiPublicationRuntimeOperationV1::ProviderReadback,
        fixture.readback_request.operation_id,
        &readback_body,
        3_000,
    );
    fixture.clock.store(3_001, Ordering::SeqCst);
    let readback = fixture
        .service
        .handle(MusubiPublicationPrivateHttpRequestV1 {
            method: "POST",
            path: MUSUBI_PUBLICATION_PROVIDER_READBACK_PATH_V1,
            content_type: APPLICATION_NORITO,
            authorization: Some(&readback_authorization),
            seed_ingress_metadata: None,
            body: &readback_body,
        });
    assert_eq!(readback.status, 200);
    let readback_response: MusubiProviderReadbackResponseV1 =
        norito::decode_canonical_with_limits(&readback.body, RESPONSE_DECODE_LIMITS)
            .expect("readback response");
    assert_eq!(readback_response, fixture.readback_response);
}

#[cfg(unix)]
#[test]
fn durable_readback_journal_separates_replacement_and_renewal_targets() {
    let mut fixture = control_service_fixture(false, false);
    let root = tempfile::tempdir().expect("durable readback journal root");
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private journal root permissions");
    let binding =
        MusubiPublicationServiceJournalBindingV1::from_configuration(&fixture.service.config);
    let limits = DurableMusubiPublicationServiceJournalLimitsV1::new(
        16,
        64,
        MAX_CONTROL_RESPONSE_BYTES_U64,
        MAX_CONTROL_RESPONSE_BYTES_U64 + 1024 * 1024,
    )
    .expect("durable journal limits");
    fixture.service.journal = Box::new(
        DurableMusubiPublicationServiceJournalV1::initialize(root.path(), binding.clone(), limits)
            .expect("initialize durable readback journal"),
    );
    let calls = Arc::new(Mutex::new(Vec::new()));
    fixture.service.readback = Box::new(RecordingExactReadback {
        calls: Arc::clone(&calls),
    });

    let initial = fixture.readback_request.clone();
    let initial_response = control_readback_response(&mut fixture, &initial, 3_000);
    assert_eq!(initial_response.status, 200);
    assert_eq!(calls.lock().expect("readback calls").len(), 1);

    let fallback = InMemoryMusubiPublicationServiceJournalV1::new(binding.clone(), 1, 1)
        .expect("temporary journal");
    let durable = std::mem::replace(&mut fixture.service.journal, Box::new(fallback));
    drop(durable);
    fixture.service.journal = Box::new(
        DurableMusubiPublicationServiceJournalV1::open(root.path(), binding.clone(), limits)
            .expect("reopen durable readback journal"),
    );

    let cached = control_readback_response(&mut fixture, &initial, 3_100);
    assert_eq!(cached.status, 200);
    assert_eq!(cached.body, initial_response.body);
    assert_eq!(calls.lock().expect("cached readback calls").len(), 1);

    let mut replacement = initial.clone();
    replacement.location.location_id = MusubiArchiveLocationIdV1::new([0xe1; 32]);
    replacement.location.finalized_height += 1;
    assert_ne!(
        provider_readback_target(&initial.location, initial.provider),
        provider_readback_target(&replacement.location, replacement.provider)
    );
    let replacement_response = control_readback_response(&mut fixture, &replacement, 3_200);
    assert_eq!(replacement_response.status, 200);

    let mut renewal = replacement.clone();
    renewal.location.revision += 1;
    renewal.location.finalized_height += 1;
    renewal.location.renew_after_epoch += 1;
    renewal.location.expires_at_epoch += 1;
    assert_ne!(
        provider_readback_target(&replacement.location, replacement.provider),
        provider_readback_target(&renewal.location, renewal.provider)
    );
    let renewal_response = control_readback_response(&mut fixture, &renewal, 3_300);
    assert_eq!(renewal_response.status, 200);
    assert_eq!(calls.lock().expect("replacement readback calls").len(), 3);

    let fallback = InMemoryMusubiPublicationServiceJournalV1::new(binding.clone(), 1, 1)
        .expect("second temporary journal");
    let durable = std::mem::replace(&mut fixture.service.journal, Box::new(fallback));
    drop(durable);
    fixture.service.journal = Box::new(
        DurableMusubiPublicationServiceJournalV1::open(root.path(), binding, limits)
            .expect("reopen durable journal with all readback targets"),
    );

    let cached_replacement = control_readback_response(&mut fixture, &replacement, 3_400);
    assert_eq!(cached_replacement.status, 200);
    assert_eq!(cached_replacement.body, replacement_response.body);
    let cached_renewal = control_readback_response(&mut fixture, &renewal, 3_500);
    assert_eq!(cached_renewal.status, 200);
    assert_eq!(cached_renewal.body, renewal_response.body);
    assert_eq!(
        calls.lock().expect("durable cached readback calls").len(),
        3
    );

    let mut substituted_same_tuple = renewal.clone();
    substituted_same_tuple.location.renew_after_epoch += 1;
    assert_eq!(
        provider_readback_target(
            &substituted_same_tuple.location,
            substituted_same_tuple.provider
        ),
        provider_readback_target(&renewal.location, renewal.provider)
    );
    let conflict = control_readback_response(&mut fixture, &substituted_same_tuple, 3_600);
    assert_eq!(conflict.status, 422);
    assert_eq!(
        decode_service_error(&conflict).code,
        MusubiPublicationServiceErrorCodeV1::OperationConflict
    );
    assert_eq!(calls.lock().expect("conflicting readback calls").len(), 3);
}

#[test]
fn storage_response_rejects_a_refreshed_receipt_after_registration() {
    let fixture = control_service_fixture(false, false);
    let broker_key = KeyPair::try_from_seed(
        b"musubi-publication-control-broker".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("broker key");
    let mut request = fixture.storage_request.clone();
    let payload = MusubiSeedIngressReceiptPayloadV1 {
        version: 1,
        binding: request.staging_receipt.payload.binding.clone(),
        issued_at_ms: request.staging_receipt.payload.expires_at_ms + 1,
        expires_at_ms: request.staging_receipt.payload.expires_at_ms + 60_001,
    };
    request.staging_receipt = MusubiSeedIngressReceiptV1 {
        approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
            public_key: broker_key.public_key().clone(),
            signature: SignatureOf::try_from_hash(broker_key.private_key(), payload.signing_hash())
                .expect("refreshed receipt signature"),
        }],
        payload,
    };
    assert_ne!(
        fixture.storage_response.archive.staging_receipt,
        request.staging_receipt
    );
    assert!(fixture.storage_response.validate_for(&request).is_err());

    request.staging_receipt.payload.binding.nonce = [0xee; 32];
    request.staging_receipt.approvals[0].signature = SignatureOf::try_from_hash(
        broker_key.private_key(),
        request.staging_receipt.payload.signing_hash(),
    )
    .expect("different-operation receipt signature");
    assert!(fixture.storage_response.validate_for(&request).is_err());
}

#[test]
fn storage_response_requires_replication_quorum_and_exact_lock_digest() {
    let fixture = control_service_fixture(false, false);
    let mut below_quorum = fixture.storage_response.clone();
    let MusubiStorageLocationDispositionV1::NeedsRegistration {
        provider_attestations,
        ..
    } = &mut below_quorum.disposition
    else {
        panic!("fixture requires location registration")
    };
    provider_attestations.truncate(usize::from(MUSUBI_MIN_HEALTHY_REPLICAS_V1).saturating_sub(1));
    assert!(below_quorum.validate_for(&fixture.storage_request).is_err());

    let mut wrong_lock = fixture.storage_request.clone();
    wrong_lock.verification_lock_digest = MusubiVerificationLockDigestV1::new([0xee; 32]);
    assert!(fixture.storage_response.validate_for(&wrong_lock).is_err());
}

#[test]
fn storage_location_generations_have_distinct_bounded_journal_targets() {
    let first = storage_generation_target(1);
    let last = storage_generation_target(
        u8::try_from(MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1)
            .expect("location generation bound fits u8"),
    );
    assert_ne!(first, last);
    assert!(valid_storage_generation_target(first));
    assert!(valid_storage_generation_target(last));
    assert!(!valid_storage_generation_target([0; 32]));
    assert!(!valid_storage_generation_target(storage_generation_target(
        u8::try_from(MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1 + 1)
            .expect("one past the bound fits u8"),
    )));
    let mut malformed = first;
    malformed[31] = 1;
    assert!(!valid_storage_generation_target(malformed));
}

#[test]
fn storage_response_never_reuses_a_prior_location_generation() {
    let fixture = control_service_fixture(false, false);
    let retired_location_id = fixture.storage_response.location_id;
    let mut replacement_request = fixture.storage_request.clone();
    replacement_request.generation = 2;
    replacement_request.prior_location_ids = vec![retired_location_id];
    replacement_request
        .validate()
        .expect("a sorted second generation is structurally valid");
    assert!(
        fixture
            .storage_response
            .validate_for(&replacement_request)
            .is_err(),
        "the coordinator cannot return a retired stable identity"
    );

    let mut replacement = fixture.storage_response;
    replacement.location_id = MusubiArchiveLocationIdV1::new([0xee; 32]);
    replacement
        .validate_for(&replacement_request)
        .expect("a never-before-used replacement identity remains valid");

    let mut unsorted_third = replacement_request;
    unsorted_third.generation = 3;
    unsorted_third.prior_location_ids = vec![
        MusubiArchiveLocationIdV1::new([0xff; 32]),
        retired_location_id,
    ];
    assert!(unsorted_third.validate().is_err());
}

#[test]
fn storage_request_binds_immutable_registration_without_freezing_location_state() {
    let fixture = control_service_fixture(false, false);

    let mut wrong_height = fixture.storage_request.clone();
    wrong_height
        .finalized_registration
        .registration
        .registered_at_height += 1;
    wrong_height.validate().expect(
        "a different internally valid projection is digest-bound but must be authorized upstream",
    );
    let exact = norito::encode_canonical(&fixture.storage_request).expect("exact request");
    let substituted = norito::encode_canonical(&wrong_height).expect("substituted request");
    assert_ne!(
        request_digest(
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            &exact,
        )
        .expect("exact request digest"),
        request_digest(
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            &substituted,
        )
        .expect("substituted request digest")
    );
    assert!(
        fixture
            .storage_response
            .validate_for(&wrong_height)
            .is_err()
    );

    let mut wrong_registrant = fixture.storage_request.clone();
    let other = KeyPair::try_from_seed(
        b"musubi-storage-authoritative-record-substitution".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("other registrant key");
    wrong_registrant
        .finalized_registration
        .registration
        .registered_by = AccountId::new(other.public_key().clone());
    assert!(wrong_registrant.validate().is_err());

    let mut missing_transaction = fixture.storage_request.clone();
    missing_transaction.finalized_registration.transaction_hash = [0; 32];
    assert!(missing_transaction.validate().is_err());

    let mut wrong_lock = fixture.storage_request.clone();
    wrong_lock.verification_lock_digest = MusubiVerificationLockDigestV1::new([0xee; 32]);
    wrong_lock
        .validate()
        .expect("a nonzero lock digest remains structurally valid");
    assert!(fixture.storage_response.validate_for(&wrong_lock).is_err());
    wrong_lock.verification_lock_digest = MusubiVerificationLockDigestV1::new([0; 32]);
    assert!(wrong_lock.validate().is_err());

    let mut pre_registration_snapshot = fixture.storage_request.clone();
    pre_registration_snapshot
        .finalized_registration
        .snapshot
        .finalized_height = pre_registration_snapshot
        .finalized_registration
        .registration
        .registered_at_height
        .saturating_sub(1);
    assert!(pre_registration_snapshot.validate().is_err());

    let mut later_current_archive = fixture.storage_response;
    later_current_archive.archive.location_revision = 2;
    later_current_archive.archive.location_ids = vec![MusubiArchiveLocationIdV1::new([
        0xdd; 32,
    ])];
    let MusubiStorageLocationDispositionV1::NeedsRegistration {
        expected_location_revision,
        ..
    } = &mut later_current_archive.disposition
    else {
        panic!("fixture requires location registration")
    };
    *expected_location_revision = 2;
    later_current_archive
        .validate_for(&fixture.storage_request)
        .expect("a later finalized location directory preserves registration evidence");
}

#[test]
fn private_service_rejects_substituted_storage_and_readback_backend_evidence() {
    let mut fixture = control_service_fixture(true, true);
    let storage_body =
        norito::encode_canonical(&fixture.storage_request).expect("storage request bytes");
    let storage_authorization = control_authorization_header(
        &fixture.runtime,
        MusubiPublicationRuntimeOperationV1::StorageCoordination,
        fixture.storage_request.operation_id,
        &storage_body,
        4_000,
    );
    fixture.clock.store(4_001, Ordering::SeqCst);
    let storage = fixture
        .service
        .handle(MusubiPublicationPrivateHttpRequestV1 {
            method: "POST",
            path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
            content_type: APPLICATION_NORITO,
            authorization: Some(&storage_authorization),
            seed_ingress_metadata: None,
            body: &storage_body,
        });
    assert_eq!(storage.status, 422);
    assert_eq!(
        decode_service_error(&storage).code,
        MusubiPublicationServiceErrorCodeV1::BackendResponseInvalid
    );

    let readback_body =
        norito::encode_canonical(&fixture.readback_request).expect("readback request bytes");
    let readback_authorization = control_authorization_header(
        &fixture.runtime,
        MusubiPublicationRuntimeOperationV1::ProviderReadback,
        fixture.readback_request.operation_id,
        &readback_body,
        5_000,
    );
    fixture.clock.store(5_001, Ordering::SeqCst);
    let readback = fixture
        .service
        .handle(MusubiPublicationPrivateHttpRequestV1 {
            method: "POST",
            path: MUSUBI_PUBLICATION_PROVIDER_READBACK_PATH_V1,
            content_type: APPLICATION_NORITO,
            authorization: Some(&readback_authorization),
            seed_ingress_metadata: None,
            body: &readback_body,
        });
    assert_eq!(readback.status, 422);
    assert_eq!(
        decode_service_error(&readback).code,
        MusubiPublicationServiceErrorCodeV1::BackendResponseInvalid
    );
}

#[test]
fn restored_journal_preserves_completed_idempotency_and_replay_state() {
    let (client, _) = client();
    let binding = MusubiPublicationOperationBindingV1 {
        operation_id: [0x81; 32],
        chain_id: client.chain,
        genesis_block_hash: [0x80; 32],
        publisher: client.account,
        archive_id: ArchiveId::new([0x82; 32]),
        car_body_digest: MusubiContentDigestV1::new([0x83; 32]),
        car_body_length: 99,
    };
    let journal_binding = MusubiPublicationServiceJournalBindingV1 {
        chain_id: binding.chain_id.clone(),
        genesis_block_hash: binding.genesis_block_hash,
        ingress_broker: binding.publisher.clone(),
        seed_provider: ProviderId::new([0x88; 32]),
    };
    let mut journal =
        InMemoryMusubiPublicationServiceJournalV1::new(journal_binding, 8, 8).expect("journal");
    let attempt = MusubiPublicationJournalAttemptV1 {
        key: MusubiPublicationIdempotencyKeyV1 {
            operation: MusubiPublicationRuntimeOperationV1::SeedIngress,
            operation_id: binding.operation_id,
            target: [0; 32],
        },
        binding: binding.clone(),
        request_digest: [0x84; 32],
        authorization_digest: [0x85; 32],
        authorization_expires_at_ms: 20_000,
    };
    assert_eq!(
        journal.begin(&attempt, 10_000).expect("reserve"),
        MusubiPublicationJournalBeginV1::Execute
    );
    journal
        .commit(attempt.key, attempt.request_digest, b"canonical response")
        .expect("commit");

    // Cloning models restoring the same durable records into a replacement service process.
    let mut restored = journal.clone();
    let mut retry = attempt.clone();
    retry.authorization_digest = [0x86; 32];
    assert_eq!(
        restored.begin(&retry, 10_001).expect("cached retry"),
        MusubiPublicationJournalBeginV1::Cached(b"canonical response".to_vec())
    );
    assert_eq!(
        restored.begin(&retry, 10_001).expect("cached replay"),
        MusubiPublicationJournalBeginV1::Cached(b"canonical response".to_vec())
    );

    let mut conflict = retry.clone();
    conflict.binding.archive_id = ArchiveId::new([0x87; 32]);
    assert_eq!(
        restored.begin(&conflict, 10_002),
        Err(MusubiPublicationServiceJournalErrorV1::Conflict)
    );

    let mut reset_conflict = retry.clone();
    reset_conflict.binding.genesis_block_hash = [0x8a; 32];
    assert_eq!(
        restored.begin(&reset_conflict, 10_002),
        Err(MusubiPublicationServiceJournalErrorV1::Invalid)
    );

    let replay = MusubiPublicationJournalAttemptV1 {
        key: MusubiPublicationIdempotencyKeyV1 {
            operation: MusubiPublicationRuntimeOperationV1::ProviderReadback,
            operation_id: binding.operation_id,
            target: [0x88; 32],
        },
        binding,
        request_digest: [0x89; 32],
        authorization_digest: attempt.authorization_digest,
        authorization_expires_at_ms: attempt.authorization_expires_at_ms,
    };
    assert_eq!(
        restored.begin(&replay, 10_003),
        Err(MusubiPublicationServiceJournalErrorV1::Replay)
    );
}
