// Real final-canary preparation/submission over disposable loopback HTTP.
// The server only simulates ledger responses; no daemon or deployment is used.

fn exercise_final_canary_deadline(applied: bool) {
    use iroha::data_model::{
        query::CommittedTransaction,
        transaction::{DataTriggerSequence, TransactionPayload, TransactionResult},
    };
    use iroha_torii_shared::{
        ErrorDetails, ErrorEnvelope, PIPELINE_TRANSACTION_STATUS_NOT_FOUND_CODE,
        PipelineTransactionStatusNotFoundV1,
    };
    let _chain = ChainDiscriminantGuard::enter(DEFAULT_CHAIN_DISCRIMINANT);
    let retained = Arc::new(Mutex::new(None::<SignedTransaction>));
    let server_transaction = Arc::clone(&retained);
    let quoted = Arc::new(Mutex::new(None::<TransactionPayload>));
    let server_quoted = Arc::clone(&quoted);
    let observations = AtomicUsize::new(0);
    let server = spawn_mock_http(32, move |request| match path_only(&request.path) {
        "/v1/fees/quote" => {
            let body: Value = json::from_str(&request.body).unwrap();
            let payload: TransactionPayload =
                json::from_value(body.get("payload").unwrap().clone()).unwrap();
            *server_quoted.lock().unwrap() = Some(payload.clone());
            // Fee latency must not create a new timestamp or extend the quoted lifetime.
            thread::sleep(Duration::from_millis(100));
            let quote = FeeQuoteResponse {
                intent: payload.fee_payment_intent().clone(),
                observation: iroha_torii_shared::FeeQuoteObservation {
                    ledger_time_ms: 1,
                    next_block_height: 2,
                    route_dataspace_id: iroha_model_base::topology::DataSpaceId::UNIVERSAL,
                },
                components: Vec::new(),
                capacities: Vec::new(),
                decision: iroha_torii_shared::FeeQuoteDecision::Accepted {
                    debit_source: iroha::data_model::nexus::FeeDebitSource::Account(
                        payload.authority().clone(),
                    ),
                    program_revision: None,
                },
            };
            quote.validate_for_draft(&payload).unwrap();
            MockResponse::json(200, json::to_value(&quote).unwrap())
        }
        "/v1/node/capabilities" => MockResponse::json(
            200,
            norito::json!({
                "data_model_version": (iroha::data_model::DATA_MODEL_VERSION),
                "signed_transaction_schema_hash_hex": (hex::encode(norito::schema::identity::frame_hash::<SignedTransaction>()))
            }),
        ),
        "/v1/pipeline/transactions/status" => {
            let transaction = server_transaction.lock().unwrap().clone().unwrap();
            assert_eq!(request.method, "GET");
            assert_eq!(
                request.path,
                format!(
                    "/v1/pipeline/transactions/status?hash={}&scope=global",
                    transaction.hash()
                )
            );
            if observations.fetch_add(1, Ordering::SeqCst) == 0 {
                thread::sleep(Duration::from_millis(100));
                // Only Torii's exact hash-and-scope absence permits the initial POST.
                let absence = ErrorEnvelope::new(
                    PIPELINE_TRANSACTION_STATUS_NOT_FOUND_CODE,
                    "Missing status.",
                )
                .with_details(ErrorDetails {
                    pipeline_transaction_status_not_found: Some(
                        PipelineTransactionStatusNotFoundV1::new(&transaction.hash(), "global"),
                    ),
                    ..ErrorDetails::default()
                });
                MockResponse::json(404, json::to_value(&absence).unwrap())
            } else {
                prepared_status_response(
                    &transaction,
                    if applied { "Applied" } else { "Queued" },
                    if applied { "state" } else { "queue" },
                )
            }
        }
        "/v1/pipeline/transactions" => {
            assert_eq!(request.method, "POST");
            let transaction = server_transaction.lock().unwrap().clone().unwrap();
            assert_eq!(
                request.header_values("content-length"),
                [transaction
                    .encode_wire_v1()
                    .unwrap()
                    .len()
                    .to_string()
                    .as_str()]
            );
            thread::sleep(Duration::from_millis(100));
            MockResponse::text(202, "")
        }
        "/v1/pipeline/transactions/details" => {
            assert!(applied, "pending status must not request committed proof");
            let transaction = server_transaction.lock().unwrap().clone().unwrap();
            let result = TransactionResult::new(Ok(DataTriggerSequence::default()));
            let output = iroha::data_model::block::execution_output::ExecutionOutputV1::Network(
                iroha::data_model::block::execution_output::NetworkExecutionOutputV1 {
                    input_index: 0,
                    result,
                    completions: Vec::new(),
                },
            );
            let details = iroha_torii_shared::PipelineTransactionDetailsResponse {
                hash: transaction.hash_as_entrypoint().to_string(),
                transaction: CommittedTransaction {
                    block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                        b"final canary deadline block",
                    )),
                    entrypoint_hash: transaction.hash_as_entrypoint(),
                    entrypoint_proof: iroha_crypto::MerkleProof::from_audit_path(0, Vec::new()),
                    entrypoint: TransactionEntrypoint::External(transaction),
                    output_hash: iroha_crypto::HashOf::new(&output),
                    output_proof: iroha_crypto::MerkleProof::from_audit_path(0, Vec::new()),
                    output,
                },
            };
            MockResponse {
                status: 200,
                content_type: "application/x-norito",
                headers: Vec::new(),
                body: norito::to_bytes(&details).unwrap(),
            }
        }
        other => panic!("unexpected canary request {other}"),
    });
    let mut config = crate::fallback_config();
    config.chain = DEFAULT_CHAIN_ID.into();
    config.account_chain_discriminant = DEFAULT_CHAIN_DISCRIMINANT;
    config.account = AccountId::new(config.key_pair.public_key().clone());
    config.torii_api_url = Url::parse(&server.base_url).unwrap();
    let mut args = fixture_write_canary_args(WriteCanaryOperation::FinalCanary);
    args.public_root = server.base_url.clone();
    args.timeout_secs = 3;
    // Reproduce a later operation with less than the default 120-second TTL left.
    args.execution_expires_at_unix_ms = current_unix_ms().unwrap() + 10_000;
    let binding = args.binding().unwrap();
    let fee = FeePaymentIntent::authority(Vec::new(), None);
    let envelope = prepare_final_canary_operation(
        &config,
        &args,
        &server.base_url,
        &binding,
        fee.clone(),
        Instant::now() + Duration::from_secs(10),
    )
    .unwrap();
    let wire = hex::decode(envelope.operation.signed_transaction_wire_hex().unwrap()).unwrap();
    let validated = validate_prepared_operation(
        &config,
        PreparedOperationPolicyContext::Current(&args),
        &binding,
        &server.base_url,
        envelope,
        Some(wire),
        &fee,
        PreparedLifetimeCheck::LiveForward,
    )
    .unwrap();
    let quoted = quoted.lock().unwrap().clone().unwrap();
    assert_eq!(validated.transaction().unwrap().payload(), &quoted);
    assert_eq!(
        quoted.creation_time_ms + quoted.time_to_live_ms.unwrap().get(),
        binding.execution_expires_at_unix_ms,
    );
    assert!(quoted.time_to_live_ms.unwrap().get() < DEFAULT_WRITE_TTL_MS);
    *retained.lock().unwrap() = Some(validated.transaction().unwrap().clone());
    let budget = if applied {
        Duration::from_secs(2)
    } else {
        Duration::from_millis(900)
    };
    let started = Instant::now();
    let outcome = submit_exact_prepared_operation(
        &config,
        &args,
        &server.base_url,
        &validated,
        &fee,
        started + budget,
    )
    .unwrap();
    let elapsed = started.elapsed();
    if applied {
        assert_eq!(
            outcome,
            PreparedRecoveryClassification::Applied {
                block_height: Some(2),
                evidence: validated
                    .transaction()
                    .unwrap()
                    .hash_as_entrypoint()
                    .to_string(),
            }
        );
        assert!(elapsed < budget);
    } else {
        assert!(matches!(
            outcome,
            PreparedRecoveryClassification::Pending { .. }
        ));
        assert!(
            elapsed >= budget && elapsed < budget + Duration::from_secs(1),
            "the initial read and POST must consume the same original deadline: {elapsed:?}"
        );
    }
    let requests = finish_mock(server);
    assert_eq!(
        requests
            .iter()
            .filter(|request| path_only(&request.path) == "/v1/pipeline/transactions")
            .count(),
        1,
        "confirmation must never replay the exact POST"
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| path_only(&request.path) == "/v1/node/capabilities")
            .count(),
        1,
        "submission and proof share the same compatibility context"
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| path_only(&request.path) == "/v1/pipeline/transactions/details")
            .count(),
        usize::from(applied)
    );
}

#[test]
fn final_canary_submit_uses_original_deadline_after_initial_read_and_post() {
    exercise_final_canary_deadline(false);
}

#[test]
fn final_canary_submit_verifies_exact_proof_without_replaying_post() {
    exercise_final_canary_deadline(true);
}

#[test]
fn final_canary_expired_window_rejects_before_fee_quote_or_dispatch() {
    let _chain = ChainDiscriminantGuard::enter(DEFAULT_CHAIN_DISCRIMINANT);
    let server = spawn_mock_http(1, |_| panic!("expired authorization must not reach HTTP"));
    let mut config = crate::fallback_config();
    config.chain = DEFAULT_CHAIN_ID.into();
    config.account_chain_discriminant = DEFAULT_CHAIN_DISCRIMINANT;
    config.account = AccountId::new(config.key_pair.public_key().clone());
    config.torii_api_url = Url::parse(&server.base_url).unwrap();
    let mut args = fixture_write_canary_args(WriteCanaryOperation::FinalCanary);
    args.public_root = server.base_url.clone();
    args.execution_expires_at_unix_ms = 1;
    let binding = args.binding().unwrap();
    let error = prepare_final_canary_operation(
        &config,
        &args,
        &server.base_url,
        &binding,
        FeePaymentIntent::authority(Vec::new(), None),
        Instant::now() + Duration::from_secs(10),
    )
    .err()
    .expect("expired signed window must reject preparation");
    assert!(format!("{error:#}").contains("no remaining signed execution window"));
    assert!(finish_mock(server).is_empty());
}

#[test]
fn faucet_preparation_deadline_stops_http_and_cpu_work_before_dispatch() {
    let expired = Instant::now();
    let config = crate::fallback_config();
    let server = spawn_mock_http(1, |_| panic!("expired preparation must not fetch a puzzle"));
    let error = solve_account_faucet_claim(
        &server.base_url,
        &config.account,
        &config.network_id,
        expired,
    )
    .unwrap_err();
    assert!(prepared_request_timed_out(&error));
    assert!(finish_mock(server).is_empty());
    let params = ScryptParams::new(1, 1, 1, 32).unwrap();
    let error = solve_faucet_pow(&[0; 32], &params, 1, expired).unwrap_err();
    assert!(prepared_request_timed_out(&error));
    assert!(!prepared_request_timed_out(&eyre!(
        "malformed authenticated proof"
    )));
}
