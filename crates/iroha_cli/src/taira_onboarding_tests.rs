//! Focused tests for ordinary bootstrap authorization, exact envelopes, and durable recovery.
use super::*;
use clap::Parser as _;

#[derive(clap::Parser)]
struct TestCli {
    #[command(subcommand)]
    command: AccountCommand,
}

fn vector(name: &str) -> Value {
    let fixture: Value = json::from_str(include_str!(
        "../../../fixtures/prepared_transactions/prepared_transaction_signature_v1.json"
    ))
    .unwrap();
    fixture["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["name"].as_str() == Some(name))
        .unwrap()
        .clone()
}

fn onboarding_fixture(proof_required: bool) -> (Config, OperationJournalV1) {
    let fixture = vector("onboarding_prepared");
    let prepared: AccountOnboardingPreparedTransactionV1 =
        json::from_value(fixture["response"].clone()).unwrap();
    let mut config = crate::fallback_config();
    config.network_id = json::from_value(fixture["network_id"].clone()).unwrap();
    config.account_chain_discriminant = 0x02f1;
    config.account = AccountId::parse_encoded(&prepared.account_id).unwrap();
    let mut binding = prepared.binding.clone();
    let fee = prepared.fee_payment.clone();
    let issuer = prepared.receipt.body.authority.clone();
    let request = prepared.receipt.body.request.clone();
    let receipt = prepared.receipt.clone();
    let response = if proof_required {
        let proof: AccountOnboardingProofRequiredPrepareResponseV1 =
            json::from_value(vector("onboarding_proof_required")["response"].clone()).unwrap();
        binding = proof.binding.clone();
        OnboardingResponseV1::ProofRequired(Box::new(proof))
    } else {
        OnboardingResponseV1::Prepared(Box::new(prepared))
    };
    let operation = new_operation(
        &config,
        binding,
        fee,
        OperationV1::Onboarding(Box::new(OnboardingV1 {
            issuer,
            request,
            receipt,
            response,
        })),
    );
    (config, operation)
}

fn faucet_fixture() -> (Config, OperationJournalV1) {
    let fixture = vector("faucet_prepared");
    let prepared: AccountFaucetPreparedTransactionV1 =
        json::from_value(fixture["response"].clone()).unwrap();
    let mut config = crate::fallback_config();
    config.network_id = json::from_value(fixture["network_id"].clone()).unwrap();
    config.account_chain_discriminant = 0x02f1;
    config.account = AccountId::parse_encoded(&prepared.account_id).unwrap();
    let operation = new_operation(
        &config,
        prepared.binding.clone(),
        prepared.fee_payment.clone(),
        OperationV1::Faucet(Box::new(FaucetV1 {
            issuer: AccountId::parse_encoded(fixture["signer_account_id"].as_str().unwrap())
                .unwrap(),
            asset_definition: prepared.asset_definition_id.parse().unwrap(),
            amount: prepared.amount.clone(),
            claim: prepared.claim.clone(),
            prepared,
        })),
    );
    (config, operation)
}

#[test]
fn public_account_cli_requires_explicit_phase_and_independent_trust() {
    let prefix = [
        "account",
        "onboard",
        "prepare",
        "--alias",
        "coffee@universal",
        "--issuer",
        "operator",
        "--token-file",
        "/runtime/token",
        "--journal",
        "/runtime/coffee",
    ];
    let parsed = TestCli::try_parse_from(prefix).unwrap();
    let AccountCommand::Onboard(OnboardCommand::Prepare(args)) = parsed.command else {
        panic!("prepare variant");
    };
    assert_eq!(args.prepare.expires_in_secs, 120);
    assert!(args.prepare.request_id.is_none());
    assert!(TestCli::try_parse_from(["account", "onboard"]).is_err());
    assert!(
        TestCli::try_parse_from([
            "account",
            "onboard",
            "prepare",
            "--alias",
            "coffee@universal",
            "--journal",
            "/runtime/coffee"
        ])
        .is_err()
    );
    assert!(
        TestCli::try_parse_from([
            "account",
            "onboard",
            "resume",
            "--journal",
            "/runtime/coffee"
        ])
        .is_ok()
    );
    assert!(
        TestCli::try_parse_from([
            "account",
            "onboard",
            "submit",
            "--journal",
            "/runtime/coffee"
        ])
        .is_err()
    );
    assert!(
        TestCli::try_parse_from([
            "account",
            "faucet",
            "prepare",
            "--journal",
            "/runtime/coffee",
            "--issuer",
            "operator",
            "--asset-definition",
            DEFAULT_GAS_ASSET_ID,
            "--amount",
            "10"
        ])
        .is_ok()
    );
    assert!(
        TestCli::try_parse_from([
            "account",
            "faucet",
            "resume",
            "--journal",
            "/runtime/coffee",
            "--submit"
        ])
        .is_err()
    );
}

#[test]
fn request_identity_is_fresh_bounded_and_canonical() {
    let mut args = PrepareArgs {
        journal: JournalArgs {
            journal: PathBuf::from("unused"),
            timeout_secs: 30,
        },
        request_id: None,
        expires_in_secs: 120,
    };
    let (first, deadline) = args.identity(1_000).unwrap();
    assert_eq!(deadline, 121_000);
    assert_eq!(validate_request_id(&first).unwrap(), first);
    assert_ne!(args.identity(1_000).unwrap().0, first);
    args.request_id = Some("ab".repeat(32));
    assert_eq!(args.identity(1_000).unwrap().0, "ab".repeat(32));
    args.request_id = Some("AB".repeat(32));
    assert!(args.identity(1_000).is_err());
    args.request_id = None;
    assert!(args.identity(u64::MAX).is_err());
    args.expires_in_secs = 0;
    assert!(args.identity(1_000).is_err());
}

#[test]
fn onboarding_verifies_real_sdk_fixture_and_rejects_substitution() {
    let _profile = ChainDiscriminantGuard::enter(0x02f1);
    let (config, operation) = onboarding_fixture(false);
    assert!(operation.verify(&config, "onboarding").unwrap().is_some());
    assert!(operation.verify(&config, "faucet").is_err());
    let mut wrong_context = config.clone();
    wrong_context.account_chain_discriminant = 369;
    assert!(operation.verify(&wrong_context, "onboarding").is_err());
    wrong_context = config.clone();
    wrong_context.torii_api_url = Url::parse("https://other.invalid").unwrap();
    assert!(operation.verify(&wrong_context, "onboarding").is_err());
    let mut changed = operation.clone();
    changed.binding.request_id = "dd".repeat(32);
    assert!(changed.verify(&config, "onboarding").is_err());
    let mut changed = operation.clone();
    let OperationV1::Onboarding(onboarding) = &mut changed.operation else {
        unreachable!()
    };
    onboarding.issuer = AccountId::new(KeyPair::random().public_key().clone());
    assert!(changed.verify(&config, "onboarding").is_err());
    let mut changed = operation.clone();
    let OperationV1::Onboarding(onboarding) = &mut changed.operation else {
        unreachable!()
    };
    onboarding
        .request
        .permissions
        .push("CanRegisterSmartContractCode".to_owned());
    assert!(changed.verify(&config, "onboarding").is_err());
    let mut changed = operation.clone();
    let OperationV1::Onboarding(onboarding) = &mut changed.operation else {
        unreachable!()
    };
    let OnboardingResponseV1::Prepared(prepared) = &mut onboarding.response else {
        unreachable!()
    };
    prepared.signed_transaction_wire_hex.push_str("00");
    assert!(changed.verify(&config, "onboarding").is_err());
}

#[test]
fn proof_required_retains_authenticated_result_without_inventing_a_transaction() {
    let _profile = ChainDiscriminantGuard::enter(0x02f1);
    let (config, operation) = onboarding_fixture(true);
    assert!(operation.verify(&config, "onboarding").unwrap().is_none());
    let OperationV1::Onboarding(onboarding) = &operation.operation else {
        unreachable!()
    };
    assert!(verify_trusted_issuer(&onboarding.receipt, &onboarding.issuer).is_ok());
    assert_eq!(
        canonical_issuer(&onboarding.issuer.to_string()).unwrap(),
        onboarding.issuer
    );
    assert!(canonical_issuer("not-an-account").is_err());
}

#[test]
fn faucet_requires_exact_independent_policy_and_signed_target() {
    let _profile = ChainDiscriminantGuard::enter(0x02f1);
    let (config, operation) = faucet_fixture();
    assert!(operation.verify(&config, "faucet").unwrap().is_some());
    let mut changed = operation.clone();
    let OperationV1::Faucet(faucet) = &mut changed.operation else {
        unreachable!()
    };
    faucet.amount = 6_u64.into();
    assert!(changed.verify(&config, "faucet").is_err());
    let mut changed = operation.clone();
    let OperationV1::Faucet(faucet) = &mut changed.operation else {
        unreachable!()
    };
    faucet.issuer = AccountId::new(KeyPair::random().public_key().clone());
    assert!(changed.verify(&config, "faucet").is_err());
    let mut changed = operation.clone();
    let OperationV1::Faucet(faucet) = &mut changed.operation else {
        unreachable!()
    };
    faucet.claim.account_id = faucet.issuer.to_string();
    assert!(changed.verify(&config, "faucet").is_err());
}

#[test]
#[cfg(unix)]
fn immutable_journal_roundtrips_public_evidence_and_reuses_one_submission_marker() {
    let _profile = ChainDiscriminantGuard::enter(0x02f1);
    let (config, operation) = onboarding_fixture(false);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("onboarding");
    let journal = Journal::create(&path).unwrap();
    journal.write_operation(&operation).unwrap();
    assert!(journal.write_operation(&operation).is_err());
    journal.record_submission(&operation).unwrap();
    let marker = fs::read(path.join("submission.json")).unwrap();
    journal.record_submission(&operation).unwrap();
    assert_eq!(fs::read(path.join("submission.json")).unwrap(), marker);
    let mut changed = operation.clone();
    changed.binding.request_id = "ed".repeat(32);
    assert!(journal.record_submission(&changed).is_err());
    drop(journal);
    let loaded = Journal::open(&path).unwrap().read_operation().unwrap();
    assert_eq!(loaded, operation);
    assert!(loaded.verify(&config, "onboarding").unwrap().is_some());
    let text = fs::read_to_string(path.join("operation.json")).unwrap();
    for secret_field in ["private_key", "token_file", "token_fd", "onboarding_token"] {
        assert!(!text.contains(secret_field));
    }
}

#[test]
fn token_selection_and_request_timeout_reject_invalid_inputs_before_io() {
    assert!(
        TokenArgs {
            token_file: None,
            token_fd: None
        }
        .read()
        .is_err()
    );
    assert!(
        TokenArgs {
            token_file: Some(PathBuf::from("unused")),
            token_fd: Some(3)
        }
        .read()
        .is_err()
    );
    assert!(operation_client(&crate::fallback_config(), 0).is_err());
    assert!(operation_client(&crate::fallback_config(), 301).is_err());
}

#[derive(Debug)]
struct RecoveryTransport {
    expected: SignedTransaction,
    returned: SignedTransaction,
    status: &'static str,
    source: &'static str,
    calls: std::sync::atomic::AtomicUsize,
}

impl iroha::http::HttpTransport for RecoveryTransport {
    fn send_blocking(
        &self,
        request: iroha::http::TransportRequest,
    ) -> Result<iroha::http::Response<Vec<u8>>> {
        use iroha::data_model::{
            query::CommittedTransaction,
            transaction::{DataTriggerSequence, TransactionResult},
        };
        if request.url.path() == "/v1/pipeline/transactions/details" {
            assert_eq!(request.method, iroha::http::Method::POST);
            assert!(
                !request.body.is_empty(),
                "exact details use the SDK's signed canonical query"
            );
        } else {
            assert_eq!(request.method, iroha::http::Method::GET);
            assert!(request.body.is_empty());
        }
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let (content_type, body) = match request.url.path() {
            "/v1/pipeline/transactions/status" => {
                assert!(
                    request
                        .url
                        .query_pairs()
                        .any(|(key, value)| key == "hash"
                            && value == self.expected.hash().to_string())
                );
                let response = PipelineTransactionStatusResponse::new(
                    hex::encode(self.expected.hash().as_ref()),
                    iroha_torii_shared::PipelineTransactionStatus {
                        kind: self.status.to_owned(),
                        block_height: (self.status == "Applied").then_some(2),
                    },
                    "global".to_owned(),
                    self.source.to_owned(),
                );
                ("application/json", json::to_vec(&response)?)
            }
            "/v1/node/capabilities" => (
                "application/json",
                json::to_vec(
                    &norito::json!({"data_model_version": (iroha::data_model::DATA_MODEL_VERSION)}),
                )?,
            ),
            "/v1/pipeline/transactions/details" => {
                let result = TransactionResult::new(Ok(DataTriggerSequence::default()));
                let output = iroha::data_model::block::execution_output::ExecutionOutputV1::Network(
                    iroha::data_model::block::execution_output::NetworkExecutionOutputV1 {
                        input_index: 0,
                        result,
                        completions: Vec::new(),
                    },
                );
                let details = iroha_torii_shared::PipelineTransactionDetailsResponse {
                    hash: self.expected.hash_as_entrypoint().to_string(),
                    transaction: CommittedTransaction {
                        block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                            b"bootstrap recovery test block",
                        )),
                        entrypoint_hash: self.returned.hash_as_entrypoint(),
                        entrypoint_proof: iroha_crypto::MerkleProof::from_audit_path(0, Vec::new()),
                        entrypoint: TransactionEntrypoint::External(self.returned.clone()),
                        output_hash: iroha_crypto::HashOf::new(&output),
                        output_proof: iroha_crypto::MerkleProof::from_audit_path(0, Vec::new()),
                        output,
                    },
                };
                ("application/x-norito", norito::to_bytes(&details)?)
            }
            path => panic!("unexpected recovery endpoint: {path}"),
        };
        Ok(iroha::http::Response::builder()
            .status(200)
            .header("Content-Type", content_type)
            .body(body)?)
    }

    fn send(&self, request: iroha::http::TransportRequest) -> iroha::http::TransportFuture<'_> {
        Box::pin(async move { self.send_blocking(request) })
    }
}

#[test]
fn recovery_is_read_only_and_requires_state_resolved_exact_committed_wire() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let _profile = ChainDiscriminantGuard::enter(0x02f1);
    let (config, operation) = onboarding_fixture(false);
    let transaction = operation.verify(&config, "onboarding").unwrap().unwrap();
    let (other_config, other) = faucet_fixture();
    let other_transaction = other.verify(&other_config, "faucet").unwrap().unwrap();
    for (status, source, wrong_wire, expected) in [
        ("Queued", "state", false, Some("Pending")),
        ("Applied", "cache", false, Some("Pending")),
        ("Applied", "state", false, Some("Applied")),
        ("Applied", "state", true, None),
        ("Rejected", "state", false, Some("Rejected")),
        ("FutureState", "state", false, None),
    ] {
        let transport = Arc::new(RecoveryTransport {
            expected: transaction.clone(),
            returned: if wrong_wire {
                other_transaction.clone()
            } else {
                transaction.clone()
            },
            status,
            source,
            calls: AtomicUsize::new(0),
        });
        let mut client_config = crate::fallback_config();
        client_config.network_id = config.network_id;
        client_config.torii_api_url = Url::parse("http://127.0.0.1:1").unwrap();
        let client = IrohaClient::builder(client_config)
            .http_transport(transport.clone())
            .build()
            .unwrap();
        let result = observe(&client, &operation, Some(&transaction));
        match expected {
            Some(expected) => assert_eq!(result.unwrap().status, expected),
            None => assert!(result.is_err()),
        }
        assert!(transport.calls.load(Ordering::SeqCst) >= 1);
    }
}

#[test]
fn public_endpoint_projection_cannot_persist_embedded_credentials() {
    validate_public_endpoint(&Url::parse("https://taira.sora.org/").unwrap()).unwrap();
    for url in [
        "https://user:secret@example.invalid/",
        "https://example.invalid/?token=secret",
        "https://example.invalid/#secret",
        "file:///runtime/secret",
    ] {
        assert!(validate_public_endpoint(&Url::parse(url).unwrap()).is_err());
    }
}

#[test]
fn saved_operation_success_requires_verified_completion() {
    for status in ["Applied", "AlreadyPresent"] {
        assert!(require_completed(status).is_ok());
    }
    for status in [
        "Absent",
        "Pending",
        "ProofRequired",
        "Rejected",
        "AliasConflict",
        "Expired",
    ] {
        assert!(require_completed(status).is_err());
    }
}

struct ReportContext {
    config: Config,
    format: CliOutputFormat,
    i18n: iroha_i18n::Localizer,
    lines: Vec<String>,
    documents: Vec<Value>,
}

impl ReportContext {
    fn new(config: Config, format: CliOutputFormat) -> Self {
        Self {
            config,
            format,
            i18n: iroha_i18n::Localizer::new(
                iroha_i18n::Bundle::Cli,
                iroha_i18n::Language::English,
            ),
            lines: Vec::new(),
            documents: Vec::new(),
        }
    }
}

impl RunContext for ReportContext {
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
    fn i18n(&self) -> &iroha_i18n::Localizer {
        &self.i18n
    }
    fn output_format(&self) -> CliOutputFormat {
        self.format
    }
    fn print_data<T: JsonSerialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.documents.push(json::to_value(value)?);
        Ok(())
    }
    fn println(&mut self, value: impl std::fmt::Display) -> Result<()> {
        self.lines.push(value.to_string());
        Ok(())
    }
}

#[test]
#[cfg(unix)]
fn preparation_report_exposes_exact_review_inputs_without_runtime_secrets() {
    let _profile = ChainDiscriminantGuard::enter(0x02f1);
    let (config, operation) = onboarding_fixture(false);
    let root = tempfile::tempdir().unwrap();
    let journal = Journal::create(&root.path().join("operation")).unwrap();
    let mut text = ReportContext::new(config.clone(), CliOutputFormat::Text);
    report(&mut text, &journal, &operation, "Prepared", None).unwrap();
    let rendered = text.lines.join("\n");
    for field in [
        "Prepared",
        "Transaction:",
        "Alias:",
        "Issuer:",
        "Journal:",
        "onboard submit",
    ] {
        assert!(rendered.contains(field));
    }
    for exact_field in [
        format!("Network: {}", operation.network_id),
        format!("Address profile: {}", operation.chain_discriminant),
        format!(
            "Expires at (Unix ms): {}",
            operation.binding.execution_expires_at_unix_ms
        ),
        format!(
            "Fee intent: {}",
            json::to_string(&operation.fee_payment).unwrap()
        ),
    ] {
        assert!(rendered.contains(&exact_field), "missing {exact_field}");
    }
    let mut json = ReportContext::new(config, CliOutputFormat::Json);
    report(&mut json, &journal, &operation, "Prepared", None).unwrap();
    assert_eq!(json.documents.len(), 1);
    assert_eq!(json.documents[0]["status"].as_str(), Some("Prepared"));
    assert!(json.documents[0]["issuer"].as_str().is_some());
    assert_eq!(
        json.documents[0]["network_id"].as_str(),
        Some(operation.network_id.to_string().as_str())
    );
    assert_eq!(
        json.documents[0]["chain_discriminant"].as_u64(),
        Some(u64::from(operation.chain_discriminant))
    );
    assert_eq!(
        json.documents[0]["expires_at_unix_ms"].as_u64(),
        Some(operation.binding.execution_expires_at_unix_ms)
    );
    assert_eq!(
        json.documents[0]["fee_payment"],
        json::to_value(&operation.fee_payment).unwrap()
    );
    assert!(
        json.documents[0]["permissions_requested"]
            .as_array()
            .is_some()
    );
    assert!(json.lines.is_empty());
    for forbidden in [
        "private_key",
        "onboarding_token",
        "signed_transaction_wire_hex",
    ] {
        assert!(!rendered.contains(forbidden));
        assert!(
            !json::to_string(&json.documents[0])
                .unwrap()
                .contains(forbidden)
        );
    }
}

#[test]
fn command_rejects_another_chain_before_creating_or_opening_journals() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("never-created");
    let command = AccountCommand::Faucet(FaucetCommand::Resume(JournalArgs {
        journal: path.clone(),
        timeout_secs: 30,
    }));
    let mut context = ReportContext::new(crate::fallback_config(), CliOutputFormat::Json);
    context.config.account_chain_discriminant = 753;
    assert!(command.run(&mut context).is_err());
    assert!(!path.exists());
    assert!(context.documents.is_empty());
}
