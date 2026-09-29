//! Canonical CLI read requests, authority selection, and response projections.

use super::*;

#[derive(Debug)]
struct CanonicalReadTransport {
    requests: std::sync::Mutex<Vec<iroha::http::TransportRequest>>,
    responses: std::sync::Mutex<std::collections::VecDeque<iroha::http::Response<Vec<u8>>>>,
}
impl iroha::http::HttpTransport for CanonicalReadTransport {
    fn send_blocking(
        &self,
        request: iroha::http::TransportRequest,
    ) -> Result<iroha::http::Response<Vec<u8>>> {
        self.requests.lock().unwrap().push(request);
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| eyre!("unexpected extra canonical read request"))
    }
    fn send(&self, request: iroha::http::TransportRequest) -> iroha::http::TransportFuture<'_> {
        Box::pin(async move { self.send_blocking(request) })
    }
}
struct CanonicalReadContext {
    config: Config,
    client: Client,
    i18n: Localizer,
    output: Option<String>,
    operator_key_pair: Option<KeyPair>,
}
impl RunContext for CanonicalReadContext {
    fn config(&self) -> &Config {
        &self.config
    }
    fn transaction_metadata(&self) -> Option<&Metadata> {
        None
    }
    fn input_instructions(&self) -> bool {
        false
    }
    fn output_instructions(&self) -> bool {
        false
    }
    fn i18n(&self) -> &Localizer {
        &self.i18n
    }
    fn print_data<T: JsonSerialize + ?Sized>(&mut self, data: &T) -> Result<()> {
        self.output = Some(norito::json::to_json(data)?);
        Ok(())
    }
    fn println(&mut self, data: impl std::fmt::Display) -> Result<()> {
        self.output = Some(data.to_string());
        Ok(())
    }
    fn client_from_config(&self) -> Result<Client> {
        Ok(self.client.clone())
    }
    fn operator_key_pair(&self) -> Option<&KeyPair> {
        self.operator_key_pair.as_ref()
    }
}
fn canonical_read_context(
    responses: Vec<iroha::http::Response<Vec<u8>>>,
) -> (CanonicalReadContext, std::sync::Arc<CanonicalReadTransport>) {
    let config = fallback_config();
    let transport = std::sync::Arc::new(CanonicalReadTransport {
        requests: std::sync::Mutex::new(Vec::new()),
        responses: std::sync::Mutex::new(responses.into()),
    });
    let client = Client::builder(config.clone())
        .http_transport(transport.clone())
        .build()
        .expect("canonical read client");
    (
        CanonicalReadContext {
            config,
            client,
            i18n: Localizer::new(Bundle::Cli, Language::English),
            output: None,
            operator_key_pair: None,
        },
        transport,
    )
}
#[test]
fn transaction_get_uses_exact_authenticated_details_and_preserves_rejection() {
    use iroha::data_model::{
        query::{
            CommittedTransaction, CommittedTxFilters, QueryRequest, SignedQuery,
            dsl::CompoundPredicate,
        },
        transaction::{TransactionResult, error::TransactionRejectionReason},
    };
    use iroha_version::codec::DecodeVersioned;
    use norito::codec::Decode;

    let config = fallback_config();
    let signed = TransactionBuilder::new(
        config.network_id,
        config.account.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .try_sign(config.key_pair.private_key())
    .expect("sign exact transaction fixture");
    let hash = signed.hash_as_entrypoint();
    let result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
        ValidationFail::NotPermitted("fixture contract permission denied".to_owned()),
    )));
    let output = iroha::data_model::block::execution_output::ExecutionOutputV1::Network(
        iroha::data_model::block::execution_output::NetworkExecutionOutputV1 {
            input_index: 0,
            result,
            completions: Vec::new(),
        },
    );
    let transaction = CommittedTransaction {
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"exact CLI transaction block")),
        entrypoint_hash: hash,
        entrypoint_proof: iroha_crypto::MerkleProof::from_audit_path(0, Vec::new()),
        entrypoint: TransactionEntrypoint::External(signed),
        output_hash: iroha_crypto::HashOf::new(&output),
        output_proof: iroha_crypto::MerkleProof::from_audit_path(0, Vec::new()),
        output,
    };
    for mismatched_hash in [false, true] {
        let details = iroha_torii_shared::PipelineTransactionDetailsResponse {
            hash: if mismatched_hash {
                HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
                    b"other entrypoint",
                ))
                .to_string()
            } else {
                hash.to_string()
            },
            transaction: transaction.clone(),
        };
        let capabilities = iroha::http::Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body(
                format!(
                    "{{\"data_model_version\":{}}}",
                    iroha::data_model::DATA_MODEL_VERSION
                )
                .into_bytes(),
            )
            .unwrap();
        let response = iroha::http::Response::builder()
            .status(200)
            .header("content-type", "application/x-norito")
            .body(norito::to_bytes(&details).unwrap())
            .unwrap();
        let (mut context, transport) = canonical_read_context(vec![capabilities, response]);
        let hash_literal = hash.to_string();
        let outcome = Args::try_parse_from(["iroha", "tx", "get", "--hash", &hash_literal])
            .unwrap()
            .command
            .run(&mut context);
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 2, "one capability read and one exact query");
        assert_eq!(requests[1].method, iroha::http::Method::POST);
        assert_eq!(requests[1].url.path(), "/v1/pipeline/transactions/details");
        let query = SignedQuery::decode_all_versioned(&requests[1].body).unwrap();
        query.verify_signature().unwrap();
        assert_eq!(query.authority(), &config.account);
        let QueryRequest::Start(query) = query.request() else {
            panic!("transaction get must sign an exact transaction-details query");
        };
        let (_, predicate, _, _) = query.parts();
        let predicate =
            CompoundPredicate::<CommittedTransaction>::decode(&mut std::io::Cursor::new(predicate))
                .unwrap();
        assert_eq!(
            predicate.committed_tx_filters(),
            Some(CommittedTxFilters {
                entry_eq: Some(hash),
                ..CommittedTxFilters::default()
            })
        );
        if mismatched_hash {
            assert!(outcome.is_err(), "a substituted proof must fail");
            assert!(context.output.is_none());
        } else {
            outcome.expect("rejected transactions still have readable details");
            assert_eq!(
                context.output,
                Some(norito::json::to_json(&transaction).unwrap())
            );
        }
    }
}
fn effective_permission_page(names: &[&str]) -> iroha::http::Response<Vec<u8>> {
    let items: Vec<_> = names
        .iter()
        .map(|name| {
            Permission::new(
                (*name).to_owned(),
                iroha_primitives::json::Json::from(norito::json!({})),
            )
        })
        .collect();
    let body = format!(
        "{{\"items\":{},\"total\":{}}}",
        norito::json::to_json(&items).unwrap(),
        items.len()
    );
    iroha::http::Response::builder()
        .status(200)
        .header("content-type", "application/json; charset=utf-8")
        .header("x-iroha-account-permission-semantics", "effective-v1")
        .header("x-iroha-fanout-routes-attempted", "2")
        .header("x-iroha-fanout-routes-succeeded", "2")
        .header("x-iroha-fanout-routes-failed", "0")
        .header("x-iroha-fanout-routes-denied", "0")
        .header("x-iroha-fanout-routes-unavailable", "0")
        .header("x-iroha-fanout-routes-not-found", "0")
        .body(body.into_bytes())
        .unwrap()
}
#[test]
fn account_permission_list_reads_complete_effective_fanout_before_global_pagination() {
    for bounded in [false, true] {
        // Pages are merged per route, so a page can exceed --fetch-size and a permission
        // may occur on different pages in different dataspaces. `total` is page-local.
        let (mut context, transport) = canonical_read_context(vec![
            effective_permission_page(&["CanC", "CanA", "CanB"]),
            effective_permission_page(&["CanC", "CanD"]),
            effective_permission_page(&["CanE"]),
        ]);
        let account = context.config.account.to_string();
        let mut argv = vec![
            "iroha",
            "account",
            "permission",
            "list",
            "--id",
            account.as_str(),
            "--fetch-size",
            "2",
        ];
        if bounded {
            argv.extend(["--offset", "1", "--limit", "2"]);
        }
        Args::try_parse_from(argv)
            .unwrap()
            .command
            .run(&mut context)
            .unwrap();
        let permissions: Vec<Permission> =
            norito::json::from_json(context.output.as_deref().unwrap()).unwrap();
        let names: Vec<_> = permissions.iter().map(Permission::name).collect();
        assert_eq!(
            names,
            if bounded {
                vec!["CanB", "CanC"]
            } else {
                vec!["CanA", "CanB", "CanC", "CanD", "CanE"]
            }
        );
        let mut expected_url = context.config.torii_api_url.clone();
        expected_url.set_path(&format!("/v1/accounts/{account}/permissions"));
        let requests = transport.requests.lock().unwrap();
        assert_eq!(
            requests.len(),
            3,
            "oversized and saturated union pages must continue; the final short page must stop"
        );
        for (index, request) in requests.iter().enumerate() {
            assert_eq!(request.method, iroha::http::Method::GET);
            assert_eq!(request.url.path(), expected_url.path());
            let params: std::collections::BTreeMap<_, _> = request.url.query_pairs().collect();
            assert_eq!(params.get("limit").map(|v| v.as_ref()), Some("2"));
            assert_eq!(params.get("offset").unwrap(), &(index * 2).to_string());
            assert_eq!(params.get("count_mode").map(|v| v.as_ref()), Some("exact"));
            for name in ["x-iroha-account", "x-iroha-signature"] {
                assert!(
                    request
                        .headers
                        .iter()
                        .any(|(key, value)| key.as_str() == name && !value.is_empty())
                );
            }
        }
    }

    // The default 500-row request already uses the native fetch budget. A
    // complete short page must retain its rows without probing offset 500.
    let (mut context, transport) =
        canonical_read_context(vec![effective_permission_page(&["CanA"])]);
    let account = context.config.account.to_string();
    Args::try_parse_from([
        "iroha",
        "account",
        "permission",
        "list",
        "--id",
        account.as_str(),
    ])
    .unwrap()
    .command
    .run(&mut context)
    .expect("a complete nonempty short page must succeed without an empty probe");
    let permissions: Vec<Permission> =
        norito::json::from_json(context.output.as_deref().unwrap()).unwrap();
    assert_eq!(
        permissions.iter().map(Permission::name).collect::<Vec<_>>(),
        vec!["CanA"]
    );
    let requests = transport.requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        1,
        "the default short page must not trigger another HTTP request"
    );
    let params: std::collections::BTreeMap<_, _> = requests[0].url.query_pairs().collect();
    assert_eq!(params.get("limit").map(|v| v.as_ref()), Some("500"));
    assert_eq!(params.get("offset").map(|v| v.as_ref()), Some("0"));
    assert_eq!(params.get("count_mode").map(|v| v.as_ref()), Some("exact"));
}
#[test]
fn account_permission_list_rejects_partial_or_non_effective_pages_without_output() {
    for damage in 0..5 {
        let mut damaged = effective_permission_page(&["CanB"]);
        match damage {
            0 => {
                damaged
                    .headers_mut()
                    .remove("x-iroha-account-permission-semantics");
            }
            1 => {
                damaged
                    .headers_mut()
                    .insert("x-iroha-fanout-routes-failed", "1".parse().unwrap());
            }
            2 => {
                damaged
                    .headers_mut()
                    .remove("x-iroha-fanout-routes-succeeded");
            }
            3 => {
                *damaged.body_mut() = br#"{"items":[],"total":1}"#.to_vec();
            }
            4 => {
                *damaged.status_mut() = iroha::http::StatusCode::CONFLICT;
            }
            _ => unreachable!(),
        }
        let (mut context, transport) =
            canonical_read_context(vec![effective_permission_page(&["CanA"]), damaged]);
        let account = context.config.account.to_string();
        let result = Args::try_parse_from([
            "iroha",
            "account",
            "permission",
            "list",
            "--id",
            account.as_str(),
            "--fetch-size",
            "1",
        ])
        .unwrap()
        .command
        .run(&mut context);
        assert!(result.is_err(), "damage {damage} must fail");
        assert!(
            context.output.is_none(),
            "no partial permission set may escape"
        );
        assert_eq!(transport.requests.lock().unwrap().len(), 2);
    }
}
#[test]
fn account_permission_list_rejects_zero_pagination_before_http() {
    for flag in ["--limit", "--fetch-size"] {
        let (mut context, transport) = canonical_read_context(Vec::new());
        let account = context.config.account.to_string();
        let error = Args::try_parse_from([
            "iroha",
            "account",
            "permission",
            "list",
            "--id",
            account.as_str(),
            flag,
            "0",
        ])
        .unwrap()
        .command
        .run(&mut context)
        .expect_err("zero pagination rejected");
        assert!(error.to_string().contains("must be positive"));
        assert!(transport.requests.lock().unwrap().is_empty());
    }
}
#[test]
fn account_permission_list_propagates_server_page_cap_rejection() {
    // The permission handler's enforce_app_pagination rejects an oversized explicit
    // limit; it does not silently clamp the per-route stride to its configured cap.
    let response = iroha::http::Response::builder()
        .status(400)
        .header("x-iroha-reject-code", "invalid_pagination")
        .body(Vec::new())
        .unwrap();
    let (mut context, transport) = canonical_read_context(vec![response]);
    let account = context.config.account.to_string();
    let oversized = u64::MAX.to_string();
    let error = Args::try_parse_from([
        "iroha",
        "account",
        "permission",
        "list",
        "--id",
        account.as_str(),
        "--fetch-size",
        oversized.as_str(),
    ])
    .unwrap()
    .command
    .run(&mut context)
    .expect_err(
        "server page cap rejection must not return a partial set or retry with a guessed stride",
    );
    assert!(error.to_string().contains("HTTP 400"));
    assert!(context.output.is_none());
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let params: std::collections::BTreeMap<_, _> = requests[0].url.query_pairs().collect();
    assert_eq!(params.get("limit").unwrap(), &oversized);
    assert_eq!(params.get("offset").map(|value| value.as_ref()), Some("0"));
}
#[test]
fn ledger_asset_get_uses_exact_singular_query_and_preserves_missing_asset_diagnostic() {
    use iroha::data_model::asset::AssetBalanceScope;
    use iroha::data_model::query::{
        QueryRequest, QueryResponse, SignedQuery, SingularQueryBox, SingularQueryOutputBox,
    };
    use iroha_version::codec::DecodeVersioned;

    for scope in [
        AssetBalanceScope::Global,
        AssetBalanceScope::Dataspace(DataSpaceId::new(3)),
    ] {
        for missing in [false, true] {
            let account = fallback_config().account;
            let definition = AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "coin".parse().unwrap(),
            );
            let id = AssetId::with_scope(definition.clone(), account.clone(), scope);
            let asset = Asset::new(id.clone(), 77_u32);
            let capabilities = iroha::http::Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(
                    format!(
                        "{{\"data_model_version\":{}}}",
                        iroha::data_model::DATA_MODEL_VERSION
                    )
                    .into_bytes(),
                )
                .unwrap();
            let response = if missing {
                let failure = iroha::data_model::query::error::QueryExecutionFail::Find(
                    iroha::data_model::query::error::FindError::Asset(Box::new(id.clone())),
                );
                let envelope = iroha_torii_shared::ErrorEnvelope::new(
                    "query_validation_failed",
                    failure.to_string(),
                );
                iroha::http::Response::builder()
                    .status(404)
                    .header("content-type", "application/x-norito")
                    .body(norito::to_bytes(&envelope).unwrap())
                    .unwrap()
            } else {
                iroha::http::Response::builder()
                    .status(200)
                    .header("content-type", "application/x-norito")
                    .body(
                        norito::to_bytes(&QueryResponse::Singular(SingularQueryOutputBox::Asset(
                            asset.clone(),
                        )))
                        .unwrap(),
                    )
                    .unwrap()
            };
            let (mut context, transport) = canonical_read_context(vec![capabilities, response]);
            let definition_literal = definition.to_string();
            let account_literal = account.to_string();
            let scope_literal = match scope {
                AssetBalanceScope::Global => "global",
                AssetBalanceScope::Dataspace(_) => "dataspace:3",
            };
            let result = Args::try_parse_from([
                "iroha",
                "ledger",
                "asset",
                "get",
                "--definition",
                &definition_literal,
                "--account",
                &account_literal,
                "--scope",
                scope_literal,
            ])
            .unwrap()
            .command
            .run(&mut context);
            let requests = transport.requests.lock().unwrap();
            assert_eq!(
                requests.len(),
                2,
                "only capability and singular query requests"
            );
            assert_eq!(requests[1].method, iroha::http::Method::POST);
            assert_eq!(requests[1].url.path(), "/v1/query");
            let signed = SignedQuery::decode_all_versioned(&requests[1].body).unwrap();
            signed.verify_signature().unwrap();
            assert_eq!(signed.authority(), &account);
            let QueryRequest::Singular(SingularQueryBox::FindAssetById(query)) = signed.request()
            else {
                panic!("asset get must submit the singular query, never an iterable scan");
            };
            assert_eq!(query.asset_id(), &id);
            if missing {
                let error = result.expect_err("singular missing asset must remain an error");
                let expected = iroha::data_model::query::error::QueryExecutionFail::Find(
                    iroha::data_model::query::error::FindError::Asset(Box::new(id.clone())),
                )
                .to_string();
                let Some(iroha::query::QueryError::Http {
                    status,
                    code,
                    message,
                }) = error.downcast_ref::<iroha::query::QueryError>()
                else {
                    panic!("expected typed HTTP query error, got {error:?}");
                };
                assert_eq!(*status, iroha::http::StatusCode::NOT_FOUND);
                assert_eq!(code, "query_validation_failed");
                assert_eq!(message, &expected);
                let rendered = format!("{error:#}");
                assert!(rendered.contains("HTTP 404"));
                assert!(rendered.contains("query_validation_failed"));
                assert!(rendered.contains(&expected));
                assert!(!rendered.contains("live query store"));
                assert!(context.output.is_none());
            } else {
                result.unwrap();
                let found: Asset =
                    norito::json::from_json(context.output.as_deref().unwrap()).unwrap();
                assert_eq!(found, asset);
            }
        }
    }
}

#[test]
fn transaction_submission_receipt_hash_roundtrips_through_status_cli() {
    use iroha::data_model::{nexus::FeeDebitSource, transaction::TransactionBuilder};
    use iroha_torii_shared::{FeeQuoteDecision, FeeQuoteObservation};

    let config = fallback_config();
    let fee_payment = FeePaymentIntent::authority(Vec::new(), None);
    let transaction = TransactionBuilder::new(
        config.network_id,
        config.account.clone(),
        fee_payment.clone(),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "submission receipt fixture".to_owned(),
    )])
    .try_sign(config.key_pair.private_key())
    .expect("signed submission fixture");
    let hash = transaction.hash();
    let fee_quote = FeeQuoteResponse {
        intent: fee_payment,
        observation: FeeQuoteObservation {
            ledger_time_ms: 1,
            next_block_height: 2,
            route_dataspace_id: DataSpaceId::UNIVERSAL,
        },
        components: Vec::new(),
        capacities: Vec::new(),
        decision: FeeQuoteDecision::Accepted {
            debit_source: FeeDebitSource::Account(config.account),
            program_revision: None,
        },
    };
    let receipt = json_utils::json_object(
        transaction_submission_receipt_fields(hash, &transaction, &fee_quote).unwrap(),
    )
    .unwrap();
    let encoded = norito::json::to_json(&receipt).unwrap();
    let exported: json::Value = norito::json::from_json(&encoded).unwrap();
    let exported_hash = exported.get("hash").and_then(json::Value::as_str).unwrap();
    assert_eq!(exported_hash, hex::encode(hash.as_ref()));
    assert_eq!(exported_hash.len(), 64);
    assert!(
        exported_hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    let args = Args::try_parse_from(["iroha", "tx", "status", "--hash", exported_hash, "--wait"])
        .expect("the exported receipt hash must be accepted directly by tx status");
    let Command::Tx(transaction::Command::Status(status)) = args.command else {
        panic!("expected tx status");
    };
    assert_eq!(status.hash, hash);
    assert!(status.wait.wait);
    assert_eq!(
        exported.get("transaction"),
        Some(&json_utils::json_value(&transaction).unwrap())
    );
    assert_eq!(
        exported.get("fee_quote"),
        Some(&json_utils::json_value(&fee_quote).unwrap())
    );
    let checked_network = json_utils::json_value(&config.network_id).unwrap();
    assert!(checked_network.as_str().unwrap().starts_with("hash:"));
    assert_eq!(
        norito::json::from_value::<NetworkId>(checked_network).unwrap(),
        config.network_id
    );
    let checked_hash = json_utils::json_value(&hash).unwrap();
    assert!(
        Args::try_parse_from([
            "iroha",
            "tx",
            "status",
            "--hash",
            checked_hash.as_str().unwrap(),
        ])
        .is_err(),
        "the raw transaction locator parser must not gain a checked-literal fallback"
    );
}

fn diagnostics_fixture() -> iroha::data_model::block::consensus::SumeragiDiagnosticsStatus {
    use iroha::data_model::block::consensus::{SumeragiDiagnosticsStatus, SumeragiLaneGovernance};
    SumeragiDiagnosticsStatus {
        tx_queue_depth: 7,
        tx_queue_capacity: 20,
        tx_queue_retained_bytes: 3_072,
        tx_queue_max_retained_bytes: 4_096,
        tx_queue_saturated: true,
        tx_queue_saturated_by_count: false,
        tx_queue_saturated_by_bytes: true,
        tx_queue_saturated_by_age: false,
        tx_queue_oldest_queued_age_ms: 1_250,
        npos: None,
        lane_governance_sealed_total: 1,
        lane_governance_sealed_aliases: vec!["missing-manifest".to_owned()],
        lane_governance: [false, true]
            .into_iter()
            .enumerate()
            .map(|(index, ready)| SumeragiLaneGovernance {
                lane_id: iroha_model_base::topology::LaneId::new(
                    u32::try_from(index).expect("two fixture lanes"),
                ),
                alias: if ready { "ready" } else { "missing-manifest" }.to_owned(),
                governance: Some("operator-fixture".to_owned()),
                manifest_required: true,
                manifest_ready: ready,
                manifest_path: None,
                validator_ids: Vec::new(),
                quorum: None,
                protected_namespaces: Vec::new(),
                runtime_upgrade: None,
            })
            .collect(),
    }
}

fn diagnostics_context() -> (CanonicalReadContext, std::sync::Arc<CanonicalReadTransport>) {
    let response = iroha::http::Response::builder()
        .status(200)
        .header("content-type", "application/x-norito")
        .body(norito::encode_canonical(&diagnostics_fixture()).unwrap())
        .unwrap();
    let (mut context, transport) = canonical_read_context(vec![response]);
    // An embedded SDK key must never replace the command's explicit authority.
    let mut builder = context.client.to_builder();
    builder.operator_key_pair = Some(fixture_key_pair(0x72));
    context.client = builder.build().unwrap();
    (context, transport)
}

fn assert_missing_operator_authority(arguments: &[&str]) {
    let (mut context, transport) = diagnostics_context();
    let error = Args::try_parse_from(arguments)
        .unwrap()
        .command
        .run(&mut context)
        .expect_err("operator reads require the configured CLI operator authority");
    assert!(error.to_string().contains("--operator-private-key-file"));
    assert!(context.output.is_none());
    assert!(transport.requests.lock().unwrap().is_empty());
    assert_eq!(transport.responses.lock().unwrap().len(), 1);
}

fn assert_exact_operator_request(
    context: &CanonicalReadContext,
    transport: &CanonicalReadTransport,
    operator: &KeyPair,
) {
    use base64::Engine as _;
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1, "no capability probe or request replay");
    let request = &requests[0];
    assert_eq!(request.method, iroha::http::Method::GET);
    assert_eq!(request.url.path(), "/v1/sumeragi/diagnostics");
    assert!(request.url.query().is_none());
    assert!(request.body.is_empty());
    let header = |name: &str| {
        let values: Vec<_> = request
            .headers
            .iter()
            .filter(|(key, _)| key == name)
            .collect();
        assert_eq!(values.len(), 1, "exactly one {name}");
        values[0].1.to_str().unwrap()
    };
    let public_key: iroha::crypto::PublicKey =
        header("x-iroha-operator-public-key").parse().unwrap();
    assert_eq!(&public_key, operator.public_key());
    assert_ne!(&public_key, context.config.key_pair.public_key());
    assert_ne!(&public_key, fixture_key_pair(0x72).public_key());
    let signature = iroha::crypto::Signature::from_bytes(
        &base64::engine::general_purpose::STANDARD
            .decode(header("x-iroha-operator-signature"))
            .unwrap(),
    );
    let message = Client::operator_network_request_message(
        &context.config.network_id,
        &request.method,
        &request.url,
        &request.body,
        header("x-iroha-operator-timestamp-ms").parse().unwrap(),
        header("x-iroha-operator-nonce"),
    )
    .unwrap();
    signature.verify(operator.public_key(), &message).unwrap();
    assert!(
        signature
            .verify(context.config.key_pair.public_key(), &message)
            .is_err()
    );
    for name in [
        "x-iroha-account",
        "x-iroha-signature",
        "x-iroha-witness",
        "authorization",
    ] {
        assert!(!request.headers.iter().any(|(key, _)| key == name));
    }
    assert!(transport.responses.lock().unwrap().is_empty());
}

#[test]
fn sumeragi_diagnostics_requires_explicit_operator_before_io() {
    assert_missing_operator_authority(&["iroha", "ops", "sumeragi", "diagnostics"]);
}

#[test]
fn nexus_lane_report_requires_explicit_operator_before_io() {
    assert_missing_operator_authority(&["iroha", "app", "nexus", "lane-report"]);
}

#[test]
fn sumeragi_diagnostics_signs_with_cli_operator_and_preserves_typed_response() {
    let (mut context, transport) = diagnostics_context();
    let operator = fixture_key_pair(0x73);
    context.operator_key_pair = Some(operator.clone());
    Args::try_parse_from(["iroha", "ops", "sumeragi", "diagnostics"])
        .unwrap()
        .command
        .run(&mut context)
        .unwrap();
    let actual: iroha::data_model::block::consensus::SumeragiDiagnosticsStatus =
        norito::json::from_json(context.output.as_deref().unwrap()).unwrap();
    assert_eq!(actual, diagnostics_fixture());
    assert_exact_operator_request(&context, &transport, &operator);
}

#[test]
fn nexus_lane_report_signs_with_cli_operator_and_preserves_sealed_failure() {
    let (mut context, transport) = diagnostics_context();
    let operator = fixture_key_pair(0x74);
    context.operator_key_pair = Some(operator.clone());
    let error = Args::try_parse_from([
        "iroha",
        "app",
        "nexus",
        "lane-report",
        "--only-missing",
        "--fail-on-sealed",
    ])
    .unwrap()
    .command
    .run(&mut context)
    .expect_err("the authenticated report must still reject sealed lanes");
    assert!(error.to_string().contains("1 lane(s) still sealed"));
    let actual: norito::json::Value =
        norito::json::from_json(context.output.as_deref().unwrap()).unwrap();
    assert_eq!(
        actual
            .get("sealed_total")
            .and_then(norito::json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        actual.get("sealed_aliases"),
        Some(&norito::json::to_value(&vec!["missing-manifest"]).unwrap())
    );
    assert_eq!(
        actual.get("lanes"),
        Some(
            &norito::json::to_value(&vec![diagnostics_fixture().lane_governance.remove(0)])
                .unwrap()
        )
    );
    assert_exact_operator_request(&context, &transport, &operator);
}
