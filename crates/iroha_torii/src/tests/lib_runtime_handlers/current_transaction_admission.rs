#[cfg(feature = "connect")]
async fn current_admission_http(
    app: SharedAppState,
    transaction: &SignedTransaction,
    endpoint: &str,
) -> Response {
    use iroha_version::codec::EncodeVersioned as _;
    use tower::ServiceExt as _;
    let body = if endpoint == route_catalog::pipeline::TRANSACTIONS_BATCH.path() {
        norito::to_bytes(&vec![transaction.encode_versioned()]).unwrap()
    } else if endpoint == route_catalog::pipeline::TRANSACTION_ENTRYPOINT.path() {
        TransactionEntrypoint::External(transaction.clone()).encode_versioned()
    } else {
        transaction.encode_versioned()
    };
    let router = axum::Router::new()
        .route(
            route_catalog::pipeline::TRANSACTION.path(),
            axum::routing::post(super::handler_post_transaction),
        )
        .route(
            route_catalog::pipeline::TRANSACTION_ENTRYPOINT.path(),
            axum::routing::post(super::handler_post_transaction_entrypoint),
        )
        .route(
            route_catalog::pipeline::TRANSACTIONS_BATCH.path(),
            axum::routing::post(super::handler_post_transactions_batch),
        )
        .with_state(app);
    router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(endpoint)
                .header(
                    axum::http::header::CONTENT_TYPE,
                    crate::utils::NORITO_MIME_TYPE,
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[cfg(feature = "connect")]
async fn assert_unsupported_current_admission(response: Response) {
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        torii_response_header(&response, "x-iroha-reject-code"),
        Some("unsupported_transaction_admission")
    );
    for header in [
        "x-iroha-transactions-accepted",
        "x-iroha-route",
        "preference-applied",
    ] {
        assert!(
            response.headers().get(header).is_none(),
            "rejection cannot contain {header}"
        );
    }
    let is_json = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/json"));
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let envelope: iroha_torii_shared::ErrorEnvelope = if is_json {
        norito::json::from_slice(&bytes).unwrap()
    } else {
        norito::decode_from_bytes(&bytes).unwrap()
    };
    assert_eq!(envelope.code, "unsupported_transaction_admission");
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn current_http_admission_retains_exact_canonical_input_in_local_custody() {
    use iroha_version::codec::EncodeVersioned as _;
    for endpoint in [
        route_catalog::pipeline::TRANSACTION.path(),
        route_catalog::pipeline::TRANSACTION_ENTRYPOINT.path(),
        route_catalog::pipeline::TRANSACTIONS_BATCH.path(),
    ] {
        let (app, key, _, _) = lifecycle_ordinary_fixture(true);
        let transaction = lifecycle_ordinary_transaction(
            &app,
            &key,
            vec![Log::new(Level::INFO, "current admission control".to_owned()).into()],
        );
        assert!(lifecycle_pending_wire(&app).is_empty());
        let response = current_admission_http(app.clone(), &transaction, endpoint).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(app.queue.active_len(), 1);
        assert_eq!(
            lifecycle_pending_wire(&app),
            vec![TransactionEntrypoint::External(transaction.clone()).encode_versioned()]
        );
        assert!(
            !app.state
                .has_committed_entrypoint(transaction.hash_as_entrypoint())
        );
    }
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn current_http_admission_rejects_actual_multiroute_before_local_custody() {
    let domains = [
        DomainId::try_new("first", "universal").unwrap(),
        DomainId::try_new("second", "secondary").unwrap(),
    ];
    let world = World::with(
        domains
            .iter()
            .cloned()
            .map(|id| Domain::new(id).build(&ALICE_ID)),
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
    );
    let app = native_ingress_app_with_world_and_nexus_for_test(
        world,
        multiple_dataspace_nexus_for_test(),
    );
    for domain in &domains {
        bind_domain_name_for_test(&app, &domain.to_string());
    }
    let transaction = TransactionBuilder::new(
        *app.state.network_id_ref(),
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(domains.into_iter().map(|domain| {
        iroha_data_model::isi::SetKeyValue::domain(
            domain,
            "current_admission".parse().unwrap(),
            iroha_primitives::json::Json::new(true),
        )
    }))
    .sign(ALICE_KEYPAIR.private_key());
    let plan = app
        .queue
        .route_payload_plan_with_state(transaction.payload(), &app.state)
        .unwrap();
    assert!(
        !matches!(plan, RoutingPlan::Single(_)),
        "exercise actual two-dataspace routing"
    );
    let before = lifecycle_pending_wire(&app);
    for endpoint in [
        route_catalog::pipeline::TRANSACTION.path(),
        route_catalog::pipeline::TRANSACTION_ENTRYPOINT.path(),
        route_catalog::pipeline::TRANSACTIONS_BATCH.path(),
    ] {
        assert_unsupported_current_admission(
            current_admission_http(app.clone(), &transaction, endpoint).await,
        )
        .await;
        assert_eq!(app.queue.active_len(), 0);
        assert_eq!(lifecycle_pending_wire(&app), before);
    }
}

/// Decode the error envelope of `response`, whatever format was negotiated.
#[cfg(feature = "connect")]
async fn anonymous_ballot_error_envelope(response: Response) -> iroha_torii_shared::ErrorEnvelope {
    let is_json = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/json"));
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    if is_json {
        norito::json::from_slice(&bytes).unwrap()
    } else {
        norito::decode_from_bytes(&bytes).unwrap()
    }
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn the_account_authenticated_entrypoint_route_never_carries_an_anonymous_ballot() {
    use iroha_version::codec::EncodeVersioned as _;
    use tower::ServiceExt as _;
    let (app, _, _, _) = lifecycle_ordinary_fixture(true);
    let entry = crate::anonymous_ballot_test_support::entrypoint(*app.state.network_id_ref());
    let endpoint = route_catalog::pipeline::TRANSACTION_ENTRYPOINT.path();
    let router = axum::Router::new()
        .route(
            endpoint,
            axum::routing::post(super::handler_post_transaction_entrypoint),
        )
        .with_state(app.clone());
    for (mime, body) in [
        (crate::utils::NORITO_MIME_TYPE, entry.encode_versioned()),
        (
            "application/json",
            format!(
                "{{\"version\":1,\"content\":{}}}",
                norito::json::to_json(&entry).unwrap()
            )
            .into_bytes(),
        ),
    ] {
        let response = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri(endpoint)
                    .header(axum::http::header::CONTENT_TYPE, mime)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        // The ballot has a route of its own: no token or transaction permit is spent on it.
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(
            response
                .headers()
                .get("x-iroha-transactions-accepted")
                .is_none()
        );
        let envelope = anonymous_ballot_error_envelope(response).await;
        assert_eq!(envelope.code, "anonymous_ballot_route_required");
        assert!(envelope.message.contains("/v1/gov/anonymous-ballots"));
        assert_eq!(app.queue.active_len(), 0);
        assert!(lifecycle_pending_wire(&app).is_empty());
    }
}

#[cfg(all(feature = "connect", feature = "app_api"))]
#[tokio::test]
async fn the_anonymous_ballot_route_decides_a_body_without_any_caller_credential() {
    use tower::ServiceExt as _;
    let (app, _, _, _) = lifecycle_ordinary_fixture(true);
    let submit = route_catalog::runtime_governance::GOV_ANONYMOUS_BALLOT_SUBMIT;
    let router = axum::Router::new()
        .route(
            submit.path(),
            axum::routing::post(super::handler_gov_anonymous_ballot_submit),
        )
        .with_state(app.clone());
    let TransactionEntrypoint::AnonymousBallot(envelope) =
        crate::anonymous_ballot_test_support::entrypoint(*app.state.network_id_ref())
    else {
        unreachable!("a ballot entrypoint")
    };
    let post = |mime: Option<&'static str>, body: Vec<u8>| {
        let mut request = axum::http::Request::builder()
            .method("POST")
            .uri(submit.path());
        if let Some(mime) = mime {
            request = request.header(axum::http::header::CONTENT_TYPE, mime);
        }
        router
            .clone()
            .oneshot(request.body(Body::from(body)).unwrap())
    };
    let norito = Some(crate::utils::NORITO_MIME_TYPE);

    // This root's selection installs no anonymous family: the queue's ballot partition gives
    // the well-formed envelope no route. Only the stable code leaves the node.
    let response = post(norito, envelope.as_bytes().to_vec()).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.headers()[axum::http::header::CACHE_CONTROL],
        "no-store"
    );
    let refusal = anonymous_ballot_error_envelope(response).await;
    assert_eq!(refusal.code, "anonymous_ballot_not_admitted");
    assert_eq!(refusal.message, "the anonymous ballot was not admitted");

    // A body that is not one canonical envelope frame, another media type, and a body
    // above the route's own limit.
    let response = post(norito, vec![0x5a; 512]).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        anonymous_ballot_error_envelope(response).await.code,
        "anonymous_ballot_malformed"
    );
    for mime in [None, Some("application/json")] {
        let response = post(mime, envelope.as_bytes().to_vec()).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(
            anonymous_ballot_error_envelope(response).await.code,
            "anonymous_ballot_unsupported_media_type"
        );
    }
    let response = post(norito, vec![0; app.anonymous_ballot_max_body_bytes + 1])
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        anonymous_ballot_error_envelope(response).await.code,
        "anonymous_ballot_oversize"
    );

    // Nothing became resident, no ordinary transaction permit was taken, and every ballot
    // permit came back.
    assert_eq!(app.queue.active_len(), 0);
    assert!(lifecycle_pending_wire(&app).is_empty());
    let defaults = iroha_config::parameters::actual::AnonymousBallotIngress::default();
    assert_eq!(
        app.anonymous_ballot_ingress_inflight.available_permits(),
        defaults.max_concurrent_compute_jobs.get()
    );
    assert_eq!(
        app.anonymous_ballot_max_body_bytes,
        defaults.max_body_bytes.get()
    );
    assert_eq!(
        app.transaction_ingress_compute_inflight.available_permits(),
        iroha_config::parameters::actual::TransactionIngress::default()
            .max_concurrent_compute_jobs
            .get()
    );
}

#[cfg(all(feature = "connect", feature = "app_api"))]
#[tokio::test]
async fn anonymous_family_reads_take_one_canonical_key_and_report_absent_records() {
    let (app, _, _, _) = lifecycle_ordinary_fixture(true);
    // A scope is exactly 128 lowercase hexadecimal characters; an attempt id exactly 64.
    for scope in [
        String::new(),
        "zz".repeat(64),
        "AB".repeat(64),
        "0".repeat(127),
        "0".repeat(129),
    ] {
        let error = crate::gov::handle_gov_anonymous_ballot_board_read(&app.state, &scope)
            .expect_err("a noncanonical scope");
        assert_eq!(
            error.into_response().status(),
            StatusCode::BAD_REQUEST,
            "{scope}"
        );
    }
    for id in [String::new(), "zz".repeat(32), "0".repeat(63)] {
        let error = crate::gov::handle_gov_anonymous_parliament_attempt_read(&app.state, &id)
            .expect_err("a noncanonical attempt identifier");
        assert_eq!(
            error.into_response().status(),
            StatusCode::BAD_REQUEST,
            "{id}"
        );
    }
    // This chain keeps no anonymous-family record: both reads are absent, never empty bytes.
    assert!(matches!(
        crate::gov::handle_gov_anonymous_ballot_board_read(&app.state, &"5a".repeat(64)),
        Err(super::Error::AppNotFound {
            code: "anonymous_ballot_board_not_found",
            ..
        })
    ));
    assert!(matches!(
        crate::gov::handle_gov_anonymous_parliament_attempt_read(&app.state, &"5a".repeat(32)),
        Err(super::Error::AppNotFound {
            code: "anonymous_parliament_attempt_not_found",
            ..
        })
    ));
    // A projection is canonical Norito bytes that no cache keeps.
    let response = super::anonymous_family_projection_response(vec![1, 2, 3]);
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[axum::http::header::CONTENT_TYPE],
        crate::utils::NORITO_MIME_TYPE
    );
    assert_eq!(
        response.headers()[axum::http::header::CACHE_CONTROL],
        "no-store"
    );
}

#[cfg(feature = "app_api")]
#[test]
fn anonymous_ballot_refusals_expose_only_allowlisted_stable_codes() {
    for (reason, code) in [
        (
            "anonymous_ballot_insufficient_stamp: the admission stamp does not reach 28 bits",
            "anonymous_ballot_insufficient_stamp",
        ),
        (
            "anonymous_ballot_proof_rejected: detail of the verifier",
            "anonymous_ballot_proof_rejected",
        ),
        (
            "anonymous_ballot_casting_closed",
            "anonymous_ballot_casting_closed",
        ),
        // Anything else, including a size refusal of the generic queue and a code that is a
        // nullifier status, is the one generic refusal: its text never leaves the node.
        (
            "transaction of 300000 bytes exceeds the global route",
            "anonymous_ballot_not_admitted",
        ),
        (
            "anonymous_ballot_nullifier_pending: held",
            "anonymous_ballot_not_admitted",
        ),
        ("", "anonymous_ballot_not_admitted"),
    ] {
        assert_eq!(
            super::anonymous_ballot_rejection_code(reason),
            code,
            "{reason}"
        );
    }
}

/// These controls deliberately use pristine State instead of the ready app policy fixture.
mod committed_crypto_policy_admission {
    use super::*;
    use crate::routing::MaybeTelemetry;
    use iroha_core::tx::{DecodedVersionedSignedTransaction, SignatureRejectionCode};
    use iroha_data_model::crypto_admission_policy::CryptoAdmissionPolicyV1;
    use iroha_version::codec::EncodeVersioned as _;

    fn fixture(policy: Option<CryptoAdmissionPolicyV1>) -> (Arc<IrohaState>, SignedTransaction) {
        let key = checked_torii_test_ed25519_keypair(0xD3, "committed policy admission signer");
        let authority = AccountId::new(key.public_key().clone());
        let world = World::with([], [Account::new(authority.clone()).build(&authority)], []);
        if let Some(policy) = policy {
            let mut block = world.block();
            block
                .set_parameter_for_testing(policy.into_parameter().unwrap())
                .unwrap();
            block.commit();
        }
        let state = Arc::new(IrohaState::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let transaction = TransactionBuilder::new(
            *state.network_id_ref(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "original committed policy".to_owned(),
        )])
        .sign(key.private_key());
        (state, transaction)
    }

    fn ingress(
        state: &Arc<IrohaState>,
        transaction: &SignedTransaction,
        mode: usize,
    ) -> Result<AcceptedTransaction<'static>, Error> {
        let telemetry = MaybeTelemetry::disabled();
        if mode == 0 {
            return routing::accept_transaction_for_ingress(state, transaction.clone(), &telemetry);
        }
        let decoded =
            DecodedVersionedSignedTransaction::decode_versioned(&transaction.encode_versioned())
                .unwrap();
        if mode == 1 {
            return routing::accept_decoded_signed_transaction_for_ingress(
                state, decoded, &telemetry,
            );
        }
        let mut prechecks = precheck_transaction_batch_ed25519(std::slice::from_ref(&decoded), 8);
        let precheck = prechecks.remove(0);
        assert!(precheck.single_ed25519_prechecked && precheck.precheck_rejection.is_none());
        routing::accept_decoded_signed_transaction_for_ingress_with_precheck(
            state,
            decoded,
            &telemetry,
            precheck.single_ed25519_prechecked,
            precheck.precheck_rejection,
        )
    }

    #[tokio::test]
    async fn pristine_state_crypto_policy_defers_all_ingress_variants_without_rejection() {
        let (state, transaction) = fixture(None);
        let original = transaction.encode_versioned();
        assert!(
            state
                .transaction_admission_parameters()
                .require_crypto_policy()
                .is_err()
        );
        for mode in 0..3 {
            let error = ingress(&state, &transaction, mode).expect_err("no genesis policy owner");
            assert!(
                matches!(&error, Error::TransactionAdmissionDeferred(original)
                if original.reason() == ivm::error::ExecutionDeferral::CryptoAdmissionPolicyUnavailable)
            );
            let response = error.into_response();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(response.headers()[axum::http::header::RETRY_AFTER], "1");
            assert!(response.headers().get("x-iroha-reject-code").is_none());
            assert!(
                response
                    .headers()
                    .get("x-iroha-transactions-accepted")
                    .is_none()
            );
            assert_eq!(transaction.encode_versioned(), original);
            assert!(!state.has_committed_entrypoint(transaction.hash_as_entrypoint()));
        }
    }

    #[test]
    fn explicit_committed_policy_controls_every_ingress_signature_path() {
        let (allowed, transaction) = fixture(Some(
            CryptoAdmissionPolicyV1::new(vec![iroha_crypto::Algorithm::Ed25519]).unwrap(),
        ));
        let original = transaction.encode_versioned();
        for mode in 0..3 {
            let accepted = ingress(&allowed, &transaction, mode).expect("explicit member admitted");
            assert_eq!(accepted.external(), Some(&transaction));
            assert_eq!(transaction.encode_versioned(), original);
        }
        let (denied, transaction) = fixture(Some(
            CryptoAdmissionPolicyV1::new(vec![iroha_crypto::Algorithm::Secp256k1]).unwrap(),
        ));
        for mode in 0..3 {
            match ingress(&denied, &transaction, mode).expect_err("no implicit Ed25519 membership")
            {
                Error::AcceptTransaction(AcceptTransactionFail::SignatureVerification(failure)) => {
                    assert_eq!(
                        failure.code(),
                        SignatureRejectionCode::AlgorithmNotPermitted
                    )
                }
                other => panic!("expected explicit policy denial, got {other:?}"),
            }
        }
    }

    #[test]
    fn ready_fixture_preserves_an_explicit_restrictive_policy() {
        let mut world = World::default();
        let policy =
            CryptoAdmissionPolicyV1::new(vec![iroha_crypto::Algorithm::Secp256k1]).unwrap();
        let mut block = world.block();
        block
            .set_parameter_for_testing(policy.clone().into_parameter().unwrap())
            .unwrap();
        block.commit();
        crate::test_utils::author_ready_fixture_crypto_policy(&mut world);
        let state = IrohaState::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        assert_eq!(
            state
                .transaction_admission_parameters()
                .require_crypto_policy()
                .unwrap(),
            &policy
        );
    }

    #[test]
    fn unavailable_crypto_implementation_remains_typed_through_public_error_adapters() {
        // This is an adapter control. Actual backend-absence verification requires
        // the Crypto/model minimal-feature controls; this fixed cause is not a signature verdict.
        let expected = iroha_crypto::UnsupportedImplementation {
            algorithm: iroha_crypto::Algorithm::MlDsa,
            operation: iroha_crypto::CryptoOperation::VerifySignature,
        };
        for direct in [false, true] {
            let failure = AcceptTransactionFail::CryptoImplementationUnavailable(expected);
            let error = if direct {
                Error::AcceptTransaction(failure)
            } else {
                Error::from(failure)
            };
            assert_eq!(error.status_code(), StatusCode::SERVICE_UNAVAILABLE);
            let error = error.classify_transaction_admission();
            let Error::TransactionAdmissionDeferred(original) = error else {
                panic!("unavailable implementation is unfinished local admission");
            };
            assert_eq!(
                original.reason(),
                ivm::error::ExecutionDeferral::CryptoImplementationUnavailable(expected)
            );
            assert!(original.decode_refusal().is_none());
            assert!(original.allocation_refusal().is_none());
            let response = Error::TransactionAdmissionDeferred(original).into_response();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(response.headers()[axum::http::header::RETRY_AFTER], "1");
            assert!(response.headers().get("x-iroha-reject-code").is_none());
            assert!(
                response
                    .headers()
                    .get("x-iroha-transactions-accepted")
                    .is_none()
            );
        }
    }
}
