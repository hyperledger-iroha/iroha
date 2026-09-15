//! Native transfer, journal, quote and recovery boundary tests.
use super::*;
use iroha::http::{HttpTransport, Response, TransportFuture, TransportRequest};
use norito::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub(crate) fn fixture_config() -> Config {
    // Published deterministic SDK fixture, never an operational wallet identity.
    let source = br#"
chain = "00000000-0000-0000-0000-000000000000"
network_id = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
torii_url = "http://127.0.0.1:8080/"
[account]
domain = "wonderland.universal"
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
private_key = "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
[transaction]
time_to_live_ms = 100000
status_timeout_ms = 1000
nonce = false
"#;
    Config::load_bytes_with_musubi_publication(Path::new("public-wallet-fixture.toml"), source)
        .unwrap()
        .0
}

#[derive(Debug, Default)]
struct Transport {
    quote_count: AtomicUsize,
    dispatch_count: AtomicUsize,
    journal: Mutex<Option<std::path::PathBuf>>,
    wrong_payer: AtomicBool,
    incompatible_submission: AtomicBool,
    alias_plan: Mutex<Option<AliasTransactionPlanV1>>,
}
impl HttpTransport for Transport {
    fn send_blocking(&self, request: TransportRequest) -> Result<Response<Vec<u8>>> {
        let (status, body) = match request.url.path() {
            "/v1/node/capabilities" => (
                200,
                json::to_vec(
                    &norito::json!({"data_model_version": (iroha::data_model::DATA_MODEL_VERSION),
                    "signed_transaction_schema_hash_hex": (if self.incompatible_submission.load(Ordering::SeqCst) { "00".repeat(16) } else { hex::encode(norito::schema::identity::frame_hash::<SignedTransaction>()) })}),
                )?,
            ),
            "/v1/aliases/setup/plan" => (
                200,
                json::to_vec(self.alias_plan.lock().unwrap().as_ref().unwrap())?,
            ),
            "/v1/fees/quote" => {
                self.quote_count.fetch_add(1, Ordering::SeqCst);
                let request: iroha_torii_shared::FeeQuoteRequest = json::from_slice(&request.body)?;
                let payload = request.payload;
                let mut intent = payload.fee_payment_intent().clone();
                if self.wrong_payer.load(Ordering::SeqCst) {
                    intent =
                        FeePaymentIntent::authority(Vec::new(), std::num::NonZeroU64::new(999));
                }
                let quote = FeeQuoteResponse {
                    intent,
                    observation: iroha_torii_shared::FeeQuoteObservation {
                        ledger_time_ms: 1,
                        next_block_height: 1,
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
                (200, json::to_vec(&quote)?)
            }
            "/v1/query" => {
                return BalanceTransport {
                    missing: None,
                    foreign: false,
                }
                .send_blocking(request);
            }
            "/v1/pipeline/transactions/status" => (404, Vec::new()),
            _ => {
                self.dispatch_count.fetch_add(1, Ordering::SeqCst);
                let path = self
                    .journal
                    .lock()
                    .unwrap()
                    .clone()
                    .expect("dispatch may occur only after preparation");
                assert!(path.join("operation.json").is_file());
                assert!(
                    path.join("submission.json").is_file(),
                    "attempt must be durable before native dispatch"
                );
                (503, b"unavailable".to_vec())
            }
        };
        Ok(Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(body)?)
    }
    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move { self.send_blocking(request) })
    }
}
fn service() -> (AccountService, Arc<Transport>) {
    let config = fixture_config();
    let transport = Arc::new(Transport::default());
    let client = Client::with_http_transport(config.clone(), transport.clone()).unwrap();
    (AccountService { config, client }, transport)
}
fn request() -> TransferRequest {
    TransferRequest {
        destination: iroha_test_samples::BOB_ID.clone(),
        amount: Quantity::from(3_u32),
        fee_payment: FeePaymentIntent::authority(Vec::new(), None),
    }
}

#[test]
fn prepare_quotes_exact_transfer_and_persists_before_any_dispatch() {
    let (service, transport) = service();
    let _profile = ChainDiscriminantGuard::enter(service.config.account_chain_discriminant);
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("transfer");
    let result = service.prepare_transfer(&request(), &path).unwrap();
    assert_eq!(result.status, OperationStatus::Prepared);
    assert!(!result.status.is_complete());
    assert!(result.data["deadline_ms"].as_u64().unwrap() > current_unix_ms().unwrap());
    assert_eq!(transport.quote_count.load(Ordering::SeqCst), 1);
    assert_eq!(transport.dispatch_count.load(Ordering::SeqCst), 0);
    let record: TransactionJournal = Journal::open(&path).unwrap().read_operation().unwrap();
    let transaction = record.verify(&service.config).unwrap();
    assert_eq!(
        transaction.hash().to_string(),
        result.data["transaction_hash"].as_str().unwrap()
    );
    let public = std::fs::read_to_string(path.join("operation.json")).unwrap();
    for secret in ["private_key", "onboarding_token", "basic_auth"] {
        assert!(!public.contains(secret));
    }
    assert!(
        service.prepare_transfer(&request(), &path).is_err(),
        "preparation cannot overwrite an earlier signed operation"
    );
}

#[test]
fn transferred_amount_destination_fee_and_network_are_bound_to_exact_wire() {
    let (service, _) = service();
    let _profile = ChainDiscriminantGuard::enter(service.config.account_chain_discriminant);
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("transfer");
    service.prepare_transfer(&request(), &path).unwrap();
    let record: TransactionJournal = Journal::open(&path).unwrap().read_operation().unwrap();
    let mut changed = record.clone();
    if let NativeOperation::Transfer { amount, .. } = &mut changed.operation {
        *amount = Quantity::from(4_u32);
    }
    assert!(changed.verify(&service.config).is_err());
    let mut changed = record.clone();
    if let NativeOperation::Transfer { destination, .. } = &mut changed.operation {
        *destination = service.config.account.clone();
    }
    assert!(changed.verify(&service.config).is_err());
    let mut foreign = service.config.clone();
    foreign.network_id =
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"foreign wallet network"),
        ));
    assert!(record.verify(&foreign).is_err());
    let mut changed = record.clone();
    changed.requested_fee = FeePaymentIntent::authority(Vec::new(), std::num::NonZeroU64::new(1));
    assert!(changed.verify(&service.config).is_err());
    let mut changed = record.clone();
    changed.deadline_ms += 1;
    assert!(changed.verify(&service.config).is_err());
    let mut changed = record;
    changed.signed_transaction_hex = changed.signed_transaction_hex.to_uppercase();
    assert!(changed.verify(&service.config).is_err());
}

#[test]
fn ambiguous_submission_is_never_repeated_and_resume_is_read_only() {
    let (service, transport) = service();
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("transfer");
    service.prepare_transfer(&request(), &path).unwrap();
    *transport.journal.lock().unwrap() = Some(path.clone());
    let outcome = service
        .submit(&path, NativeOperationKind::Transfer)
        .unwrap();
    assert_eq!(outcome.status, OperationStatus::Pending);
    assert!(outcome.data["deadline_ms"].as_u64().is_some());
    let attempted = transport.dispatch_count.load(Ordering::SeqCst);
    assert!(attempted > 0);
    assert_eq!(
        service
            .submit(&path, NativeOperationKind::Transfer)
            .unwrap()
            .status,
        OperationStatus::Pending
    );
    assert_eq!(
        service
            .resume(&path, NativeOperationKind::Transfer)
            .unwrap()
            .status,
        OperationStatus::Pending
    );
    assert_eq!(transport.dispatch_count.load(Ordering::SeqCst), attempted);
    assert!(!path.join("applied.json").exists());
}

#[test]
fn hostile_quote_and_invalid_transfer_fail_before_journal_or_dispatch() {
    let (service, transport) = service();
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("transfer");
    let mut invalid = request();
    invalid.amount = Quantity::from(0_u32);
    assert!(service.prepare_transfer(&invalid, &path).is_err());
    assert_eq!(transport.quote_count.load(Ordering::SeqCst), 0);
    invalid.amount = Quantity::from(78_u32);
    let insufficient = service
        .prepare_transfer(&invalid, &path)
        .unwrap_err()
        .to_string();
    assert!(insufficient.contains("required 78"), "{insufficient}");
    assert!(!path.exists());
    transport.wrong_payer.store(true, Ordering::SeqCst);
    assert!(service.prepare_transfer(&request(), &path).is_err());
    assert!(!path.exists());
    assert_eq!(transport.dispatch_count.load(Ordering::SeqCst), 0);
}

#[test]
fn xor_balance_uses_the_canonical_native_account_holding() {
    let (service, _) = service();
    let balance = service.xor_balance().unwrap();
    assert_eq!(balance.amount, Quantity::from(77_u32));
    assert_eq!(
        balance.asset_definition,
        XOR_ASSET_DEFINITION.parse().unwrap()
    );
}

#[test]
fn account_context_and_public_endpoint_are_validated_without_io() {
    let mut config = fixture_config();
    assert!(AccountService::new(config.clone()).is_ok());
    config.account = iroha_test_samples::BOB_ID.clone();
    assert!(AccountService::new(config).is_err());
    for url in [
        "https://user:secret@example.com",
        "https://example.com?token=secret",
        "https://example.com#fragment",
    ] {
        assert!(iroha::account_bootstrap::validate_endpoint(&url.parse().unwrap()).is_err());
    }
    assert!(
        OperationReport {
            status: OperationStatus::Prepared,
            data: Value::Null
        }
        .require_complete()
        .is_err()
    );
    assert!(
        OperationReport {
            status: OperationStatus::Applied,
            data: Value::Null
        }
        .require_complete()
        .is_ok()
    );
}

#[derive(Debug)]
struct BalanceTransport {
    missing: Option<&'static str>,
    foreign: bool,
}
impl HttpTransport for BalanceTransport {
    fn send_blocking(&self, request: TransportRequest) -> Result<Response<Vec<u8>>> {
        use iroha::data_model::{
            Registrable as _,
            account::Account,
            asset::{Asset, AssetBalancePolicy, AssetDefinition},
            query::{
                QueryRequest, QueryResponse, SignedQuery, SingularQueryBox, SingularQueryOutputBox,
            },
        };
        if request.url.path() == "/v1/node/capabilities" {
            return Ok(Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(json::to_vec(
                    &norito::json!({"data_model_version": (iroha::data_model::DATA_MODEL_VERSION)}),
                )?)?);
        }
        assert_eq!(request.url.path(), "/v1/query");
        let signed = SignedQuery::decode_all_versioned(&request.body)?;
        signed.verify_signature()?;
        let authority = signed.authority().clone();
        let (name, output) = match signed.request() {
            QueryRequest::Singular(SingularQueryBox::FindAccountById(_)) => (
                "account",
                SingularQueryOutputBox::Account(
                    Account::new(if self.foreign {
                        iroha_test_samples::BOB_ID.clone()
                    } else {
                        authority.clone()
                    })
                    .build(&authority),
                ),
            ),
            QueryRequest::Singular(SingularQueryBox::FindAssetDefinitionById(_)) => (
                "definition",
                SingularQueryOutputBox::AssetDefinition(
                    AssetDefinition::numeric(
                        XOR_ASSET_DEFINITION.parse()?,
                        "XOR",
                        AssetBalancePolicy::Global,
                        None,
                    )
                    .build(&authority),
                ),
            ),
            QueryRequest::Singular(SingularQueryBox::FindAssetById(query)) => (
                "holding",
                SingularQueryOutputBox::Asset(Asset::new(query.asset_id().clone(), 77_u32)),
            ),
            _ => panic!("unexpected wallet query"),
        };
        if self.missing == Some(name) {
            return Ok(Response::builder()
                .status(404)
                .header("Content-Type", "application/x-norito")
                .body(norito::to_bytes(&iroha_torii_shared::ErrorEnvelope::new(
                    "query_validation_failed",
                    "fixture missing holding",
                ))?)?);
        }
        Ok(Response::builder()
            .status(200)
            .header("Content-Type", "application/x-norito")
            .body(norito::to_bytes(&QueryResponse::Singular(output))?)?)
    }
    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move { self.send_blocking(request) })
    }
}

#[test]
fn typed_command_mismatch_rejects_saved_transfer_before_dispatch() {
    let (service, transport) = service();
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("transfer");
    service.prepare_transfer(&request(), &path).unwrap();
    assert!(
        service
            .submit(&path, NativeOperationKind::AliasSetup)
            .is_err()
    );
    assert!(
        service
            .resume(&path, NativeOperationKind::AliasSetup)
            .is_err()
    );
    assert_eq!(transport.dispatch_count.load(Ordering::SeqCst), 0);
    assert!(!path.join("submission.json").exists());
}

fn alias_fixture(
    config: &Config,
    noop: bool,
    amount: u32,
) -> (AliasSetupPlanRequestV1, AliasTransactionPlanV1) {
    use iroha::data_model::{alias_setup::*, isi::alias_setup::EnsureAlias};
    use iroha_model_base::{domain::DomainId, topology::DataSpaceId};
    let intent = AliasIntentV1::Domain(AliasDomainIntentV1 {
        domain: ResolvedDomainV1::new(
            DomainId::try_new("developer", "universal").unwrap(),
            DataSpaceId::UNIVERSAL,
        ),
        owner: config.account.clone(),
    });
    let guard = AliasQuoteGuardV1 {
        expected_policy_version: 1,
        expected_payment_asset: XOR_ASSET_DEFINITION.parse().unwrap(),
        max_amount: Quantity::from(amount),
        valid_until_ms: current_unix_ms().unwrap() + 60_000,
    };
    let ensure = EnsureAlias::new(
        intent.clone(),
        AliasLeaseAcquisitionV1::new(1, None),
        guard.clone(),
    );
    let (wire_id, framed_payload) =
        iroha::data_model::isi::framed_instruction_payload(&InstructionBox::from(ensure.clone()))
            .unwrap();
    let plan = AliasTransactionPlanV1::new(AliasTransactionPlanBodyV1 {
        version: AliasTransactionPlanBodyV1::VERSION,
        authority: config.account.clone(),
        network_id: config.network_id,
        anchor: AliasPlanAnchorV1 {
            block_height: 1,
            block_hash: iroha_crypto::Hash::new(b"native-wallet-alias-fixture"),
        },
        resources: vec![AliasPlanResourceV1 {
            intent: intent.clone(),
            disposition: if noop {
                AliasPlanDispositionV1::NoOp
            } else {
                AliasPlanDispositionV1::Create
            },
            quote: (!noop).then(|| AliasLeaseQuoteV1 {
                target: intent.target(),
                pricing_class: 0,
                exact_amount: Quantity::from(amount),
                guard: guard.clone(),
                expires_at_ms: 10,
                grace_expires_at_ms: 20,
                redemption_expires_at_ms: 30,
            }),
            instruction_index: Some(0),
        }],
        instructions: vec![AliasFramedInstructionV1 {
            wire_id: wire_id.to_owned(),
            framed_payload,
        }],
        totals_by_asset: if noop {
            vec![]
        } else {
            vec![AliasAssetTotalV1 {
                payment_asset: guard.expected_payment_asset.clone(),
                amount: Quantity::from(amount),
            }]
        },
        warnings: vec![],
        blockers: vec![],
        valid_until_ms: guard.valid_until_ms - 1,
    });
    (AliasSetupPlanRequestV1::new(vec![ensure]), plan)
}
#[test]
fn existing_alias_returns_verified_already_present_without_signing_or_journal() {
    let (service, transport) = service();
    let (request, plan) = alias_fixture(&service.config, true, 5);
    *transport.alias_plan.lock().unwrap() = Some(plan);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("alias");
    let report = service
        .prepare_alias(&request, FeePaymentIntent::authority(vec![], None), &path)
        .unwrap();
    assert_eq!(report.status, OperationStatus::AlreadyPresent);
    assert_eq!(report.data["transaction_hash"], Value::Null);
    assert_eq!(report.data["journal"], Value::Null);
    assert!(!path.exists());
    assert_eq!(transport.quote_count.load(Ordering::SeqCst), 0);
    assert_eq!(transport.dispatch_count.load(Ordering::SeqCst), 0);
    let invalid_fee = FeePaymentIntent::sponsor(
        iroha::data_model::nexus::FeeSponsorProgramId::new(
            service.config.account.clone(),
            "invalid".parse().unwrap(),
        ),
        0,
        vec![],
        None,
    );
    assert!(service.prepare_alias(&request, invalid_fee, &path).is_err());
    assert!(!path.exists());
}
#[test]
fn alias_creation_preserves_the_exact_plan_and_checks_its_rent_before_signing() {
    for amount in [5_u32, 78] {
        let (service, transport) = service();
        let (request, plan) = alias_fixture(&service.config, false, amount);
        *transport.alias_plan.lock().unwrap() = Some(plan.clone());
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("alias");
        let result =
            service.prepare_alias(&request, FeePaymentIntent::authority(vec![], None), &path);
        if amount == 5 {
            assert_eq!(result.unwrap().status, OperationStatus::Prepared);
            let journal = Journal::open(&path).unwrap();
            let record: TransactionJournal = journal.read_operation().unwrap();
            assert_eq!(record.operation.kind(), NativeOperationKind::AliasSetup);
            let NativeOperation::AliasSetup {
                request: retained_request,
                plan: retained_plan,
            } = &record.operation
            else {
                panic!("alias operation required")
            };
            assert_eq!(retained_request, &request);
            assert_eq!(retained_plan, &plan);
            record.verify(&service.config).unwrap();
        } else {
            let error = result.unwrap_err().to_string();
            assert!(error.contains("required 78"), "{error}");
            assert!(!path.exists());
        }
        assert_eq!(transport.dispatch_count.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn incompatible_submission_surface_preserves_an_unattempted_operation() {
    let (service, transport) = service();
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("transfer");
    transport
        .incompatible_submission
        .store(true, Ordering::SeqCst);
    assert!(service.prepare_transfer(&request(), &path).is_err());
    assert!(!path.exists());
    transport
        .incompatible_submission
        .store(false, Ordering::SeqCst);
    service.prepare_transfer(&request(), &path).unwrap();
    transport
        .incompatible_submission
        .store(true, Ordering::SeqCst);
    let error = service
        .submit(&path, NativeOperationKind::Transfer)
        .unwrap_err();
    assert!(error.to_string().contains("unattempted"));
    assert!(!path.join("submission.json").exists());
    assert_eq!(transport.dispatch_count.load(Ordering::SeqCst), 0);
}
