//! Exact native balance and cumulative funding boundary tests.
use super::*;
use crate::{
    config::Config,
    http::{HttpTransport, Response, TransportFuture, TransportRequest},
};
use iroha_version::codec::DecodeVersioned as _;
use std::{path::Path, sync::Arc};
const XOR_ASSET_DEFINITION: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
pub fn fixture_config() -> Config {
    // Published deterministic SDK fixture, never an operational wallet identity.
    let source = br#"
chain = "00000000-0000-0000-0000-000000000000"
network_id = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
torii_url = "http://127.0.0.1:8080/"
[account]
chain_discriminant = 753
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

#[derive(Debug)]
struct BalanceTransport {
    missing: Option<&'static str>,
    foreign: bool,
    policy: AssetBalancePolicy,
    holdings: BTreeMap<AssetBalanceScope, u32>,
    requests: std::sync::atomic::AtomicUsize,
}
impl Default for BalanceTransport {
    fn default() -> Self {
        Self {
            missing: None,
            foreign: false,
            policy: AssetBalancePolicy::Global,
            holdings: BTreeMap::from([(AssetBalanceScope::Global, 77)]),
            requests: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}
impl HttpTransport for BalanceTransport {
    fn send_blocking(&self, request: TransportRequest) -> Result<Response<Vec<u8>>> {
        use iroha_data_model::{
            Registrable as _,
            account::Account,
            asset::{Asset, AssetDefinition},
            query::{
                QueryRequest, QueryResponse, SignedQuery, SingularQueryBox, SingularQueryOutputBox,
            },
        };

        self.requests
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if request.url.path() == "/v1/node/capabilities" {
            return Ok(Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(json::to_vec(
                    &norito::json!({"data_model_version": (iroha_data_model::DATA_MODEL_VERSION)}),
                )?)?);
        }
        assert_eq!(request.url.path(), "/v1/query");
        let signed = SignedQuery::decode_all_versioned(&request.body)?;
        signed.verify_signature()?;
        let authority = signed.authority().clone();
        let (name, output) = match signed.request() {
            QueryRequest::Singular(SingularQueryBox::FindAccountById(_)) => (
                "account",
                QueryResponse::Singular(SingularQueryOutputBox::Account(
                    Account::new(if self.foreign {
                        iroha_test_samples::BOB_ID.clone()
                    } else {
                        authority.clone()
                    })
                    .build(&authority),
                )),
            ),
            QueryRequest::Singular(SingularQueryBox::FindAssetDefinitionById(_)) => (
                "definition",
                QueryResponse::Singular(SingularQueryOutputBox::AssetDefinition(
                    AssetDefinition::numeric(
                        XOR_ASSET_DEFINITION.parse()?,
                        "XOR",
                        self.policy,
                        None,
                    )
                    .build(&authority),
                )),
            ),
            QueryRequest::Singular(SingularQueryBox::FindAssetById(query)) => {
                let id = AssetId::with_scope(
                    XOR_ASSET_DEFINITION.parse()?,
                    authority.clone(),
                    *query.asset_id().scope(),
                );
                let amount = *self
                    .holdings
                    .get(id.scope())
                    .expect("only the exact requested bucket may be read");
                assert_eq!(
                    query.asset_id(),
                    &id,
                    "the signed query must bind the exact holding"
                );
                let returned_id = if self.missing == Some("foreign-holding") {
                    AssetId::new(
                        XOR_ASSET_DEFINITION.parse()?,
                        iroha_test_samples::BOB_ID.clone(),
                    )
                } else if self.missing == Some("foreign-scope") {
                    AssetId::with_scope(
                        id.definition().clone(),
                        authority.clone(),
                        AssetBalanceScope::Dataspace(iroha_model_base::topology::DataSpaceId::new(
                            41,
                        )),
                    )
                } else {
                    id
                };
                (
                    "holding",
                    QueryResponse::Singular(SingularQueryOutputBox::Asset(Asset::new(
                        returned_id,
                        amount,
                    ))),
                )
            }
            _ => panic!("unexpected wallet query"),
        };
        if self.missing == Some("malformed-holding") && name == "holding" {
            return Ok(Response::builder()
                .status(404)
                .header("Content-Type", "text/html")
                .body(b"<html>missing ingress route</html>".to_vec())?);
        }
        if (name != "holding" && self.missing == Some(name))
            || (name == "holding"
                && matches!(
                    self.missing,
                    Some(
                        "holding"
                            | "missing-query"
                            | "wrong-missing-holding"
                            | "missing-details-holding"
                            | "wrong-scope-missing"
                    )
                ))
        {
            let typed_absence = name == "holding" && self.missing != Some("missing-query");
            let mut envelope = iroha_torii_shared::ErrorEnvelope::new(
                if typed_absence {
                    "query_asset_not_found"
                } else {
                    "query_validation_failed"
                },
                "fixture missing holding",
            );
            if typed_absence && self.missing != Some("missing-details-holding") {
                let QueryRequest::Singular(SingularQueryBox::FindAssetById(query)) =
                    signed.request()
                else {
                    panic!("typed asset absence requires an asset query")
                };
                let missing = AssetId::with_scope(
                    XOR_ASSET_DEFINITION.parse()?,
                    if self.missing == Some("wrong-missing-holding") {
                        iroha_test_samples::BOB_ID.clone()
                    } else {
                        authority
                    },
                    if self.missing == Some("wrong-scope-missing") {
                        AssetBalanceScope::Dataspace(iroha_model_base::topology::DataSpaceId::new(
                            41,
                        ))
                    } else {
                        *query.asset_id().scope()
                    },
                );
                envelope = envelope.with_details(iroha_torii_shared::ErrorDetails {
                    query_asset_not_found: Some(missing),
                    ..iroha_torii_shared::ErrorDetails::default()
                });
            }
            return Ok(Response::builder()
                .status(404)
                .header("Content-Type", "application/x-norito")
                .body(norito::to_bytes(&envelope)?)?);
        }
        Ok(Response::builder()
            .status(200)
            .header("Content-Type", "application/x-norito")
            .body(norito::to_bytes(&output)?)?)
    }
    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move { self.send_blocking(request) })
    }
}

#[test]
fn balance_reads_exact_holding_and_only_matching_typed_absence_becomes_zero() {
    for (missing, foreign, expected) in [
        (None, false, Some(77_u32)),
        (Some("holding"), false, Some(0)),
        (Some("account"), false, None),
        (Some("definition"), false, None),
        (Some("malformed-holding"), false, None),
        (Some("missing-query"), false, None),
        (Some("foreign-holding"), false, None),
        (Some("wrong-missing-holding"), false, None),
        (Some("missing-details-holding"), false, None),
        (None, true, None),
    ] {
        let config = fixture_config();
        let client = Client::with_http_transport(
            config.clone(),
            Arc::new(BalanceTransport {
                missing,
                foreign,
                ..BalanceTransport::default()
            }),
        )
        .unwrap();
        let result = client.balance(&AssetId::new(
            XOR_ASSET_DEFINITION.parse().unwrap(),
            config.account.clone(),
        ));
        if let Some(amount) = expected {
            let report = result.unwrap();
            assert_eq!(report.amount, Quantity::from(amount));
            assert_eq!(
                report.to_json().unwrap()["amount"].as_str(),
                Some(amount.to_string().as_str())
            );
        } else {
            assert!(result.is_err());
        }
    }
}

fn quote(authority: &AccountId, amount: u32, sponsored: bool, capacity: u32) -> FeeQuoteResponse {
    use iroha_data_model::transaction::{FeeChargeKind, FeeChargeLimit, FeePaymentIntent};
    use iroha_torii_shared::{
        FeeQuoteCapacity, FeeQuoteComponent, FeeQuoteDecision, FeeQuoteObservation,
    };
    let asset = XOR_ASSET_DEFINITION.parse::<AssetDefinitionId>().unwrap();
    let limits = vec![FeeChargeLimit::new(
        FeeChargeKind::Nexus,
        asset.clone(),
        Quantity::from(amount),
    )];
    let program = FeeSponsorProgramId::new(authority.clone(), "test".parse().unwrap());
    FeeQuoteResponse {
        intent: if sponsored {
            FeePaymentIntent::sponsor(program.clone(), 1, limits, None)
        } else {
            FeePaymentIntent::authority(limits, None)
        },
        observation: FeeQuoteObservation {
            ledger_time_ms: 1,
            next_block_height: 1,
            route_dataspace_id: iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        },
        components: vec![FeeQuoteComponent {
            kind: FeeChargeKind::Nexus,
            asset_definition_id: asset.clone(),
            max_amount: Quantity::from(amount),
        }],
        capacities: if sponsored {
            vec![FeeQuoteCapacity {
                asset_definition_id: asset,
                vault_balance: Quantity::from(capacity + 1),
                reserve_floor: Quantity::from(1_u32),
                block_remaining: Quantity::from(amount),
                program_epoch_remaining: Quantity::from(capacity),
                beneficiary_epoch_remaining: Quantity::from(capacity),
            }]
        } else {
            vec![]
        },
        decision: FeeQuoteDecision::Accepted {
            debit_source: if sponsored {
                FeeDebitSource::SponsorProgram(program)
            } else {
                FeeDebitSource::Account(authority.clone())
            },
            program_revision: sponsored.then_some(1),
        },
    }
}
#[test]
fn principal_and_all_authority_stages_must_fit_the_exact_available_balance() {
    let config = fixture_config();
    let client = Client::with_http_transport(
        config.clone(),
        Arc::new(BalanceTransport {
            missing: None,
            foreign: false,
            ..BalanceTransport::default()
        }),
    )
    .unwrap();
    let principal = BTreeMap::from([(
        AssetId::new(
            XOR_ASSET_DEFINITION.parse().unwrap(),
            config.account.clone(),
        ),
        Quantity::from(70_u32),
    )]);
    let quotes = [
        quote(&config.account, 3, false, 0),
        quote(&config.account, 4, false, 0),
    ];
    let report = client.check_funding(&principal, &quotes).unwrap();
    assert_eq!(report[0].required, Quantity::from(77_u32));
    assert_eq!(report[0].available, Quantity::from(77_u32));
    let error = client
        .check_funding(
            &principal,
            &[quotes[0].clone(), quotes[1].clone(), quotes[0].clone()],
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("required 80"), "{error}");
    assert!(error.contains("available 77"), "{error}");
    let mut foreign = quotes[0].clone();
    foreign.decision = iroha_torii_shared::FeeQuoteDecision::Accepted {
        debit_source: FeeDebitSource::Account(iroha_test_samples::BOB_ID.clone()),
        program_revision: None,
    };
    assert!(client.check_funding(&principal, &[foreign]).is_err());
}
#[test]
fn sponsor_stages_share_vault_and_epoch_capacity_but_principal_remains_account_paid() {
    let config = fixture_config();
    let principal = BTreeMap::from([(
        AssetId::new(
            XOR_ASSET_DEFINITION.parse().unwrap(),
            config.account.clone(),
        ),
        Quantity::from(70_u32),
    )]);
    let policies = BTreeMap::from([(
        XOR_ASSET_DEFINITION.parse().unwrap(),
        AssetBalancePolicy::Global,
    )]);
    let first = quote(&config.account, 4, true, 8);
    let (account, sponsor) = aggregate_funding(
        &config.account,
        &principal,
        &[first.clone(), first.clone()],
        &policies,
    )
    .unwrap();
    assert_eq!(account, principal);
    assert_eq!(sponsor[0].required, Quantity::from(8_u32));
    assert_eq!(sponsor[0].available, Quantity::from(8_u32));
    let weaker = quote(&config.account, 4, true, 7);
    assert!(
        aggregate_funding(
            &config.account,
            &principal,
            &[first.clone(), weaker],
            &policies
        )
        .unwrap_err()
        .to_string()
        .contains("required 8")
    );
    let mut missing_capacity = first.clone();
    missing_capacity.capacities.clear();
    assert!(
        aggregate_funding(&config.account, &principal, &[missing_capacity], &policies).is_err()
    );
    let mut revision = first.clone();
    let (id, _) = revision.intent.sponsor_program().unwrap();
    revision.intent = iroha_data_model::transaction::FeePaymentIntent::sponsor(
        id.clone(),
        2,
        revision.intent.charge_limits().to_vec(),
        None,
    );
    let iroha_torii_shared::FeeQuoteDecision::Accepted {
        program_revision, ..
    } = &mut revision.decision;
    *program_revision = Some(2);
    assert!(
        aggregate_funding(&config.account, &principal, &[first, revision], &policies)
            .unwrap_err()
            .to_string()
            .contains("revision")
    );
}

#[test]
fn restricted_balance_preserves_full_scope_and_exact_absence() {
    let config = fixture_config();
    let scope =
        AssetBalanceScope::Dataspace(iroha_model_base::topology::DataSpaceId::new(u64::MAX));
    let asset = AssetId::with_scope(
        XOR_ASSET_DEFINITION.parse().unwrap(),
        config.account.clone(),
        scope,
    );
    for (missing, expected) in [
        (None, Some(77_u32)),
        (Some("holding"), Some(0)),
        (Some("foreign-scope"), None),
        (Some("wrong-scope-missing"), None),
        (Some("wrong-missing-holding"), None),
        (Some("malformed-holding"), None),
        (Some("missing-details-holding"), None),
    ] {
        let transport = Arc::new(BalanceTransport {
            missing,
            policy: AssetBalancePolicy::DataspaceRestricted,
            holdings: BTreeMap::from([(scope, 77)]),
            ..BalanceTransport::default()
        });
        let client = Client::with_http_transport(config.clone(), transport).unwrap();
        let result = client.balance(&asset);
        if let Some(expected) = expected {
            let report = result.unwrap();
            assert_eq!(report.asset_id, asset);
            assert_eq!(report.amount, Quantity::from(expected));
            let exact: AssetId =
                json::from_value(report.to_json().unwrap()["asset_id"].clone()).unwrap();
            assert_eq!(exact, asset);
        } else {
            assert!(
                result.is_err(),
                "{missing:?} must not become a scoped balance"
            );
        }
    }
}

#[test]
fn balance_policy_scope_mismatches_and_foreign_principal_are_rejected() {
    let config = fixture_config();
    let definition = XOR_ASSET_DEFINITION.parse::<AssetDefinitionId>().unwrap();
    for (policy, scope) in [
        (
            AssetBalancePolicy::Global,
            AssetBalanceScope::Dataspace(iroha_model_base::topology::DataSpaceId::new(u64::MAX)),
        ),
        (
            AssetBalancePolicy::DataspaceRestricted,
            AssetBalanceScope::Global,
        ),
    ] {
        let transport = Arc::new(BalanceTransport {
            policy,
            holdings: BTreeMap::new(),
            ..BalanceTransport::default()
        });
        let client = Client::with_http_transport(config.clone(), transport).unwrap();
        let asset = AssetId::with_scope(definition.clone(), config.account.clone(), scope);
        assert!(
            client
                .balance(&asset)
                .unwrap_err()
                .to_string()
                .contains("policy")
        );
        assert!(
            client
                .check_funding(&BTreeMap::from([(asset, 1_u32.into())]), &[])
                .unwrap_err()
                .to_string()
                .contains("policy")
        );
    }
    let transport = Arc::new(BalanceTransport::default());
    let client = Client::with_http_transport(config.clone(), transport.clone()).unwrap();
    let before = transport.requests.load(std::sync::atomic::Ordering::SeqCst);
    let foreign = AssetId::new(definition, iroha_test_samples::BOB_ID.clone());
    assert!(client.balance(&foreign).is_err());
    assert!(
        client
            .check_funding(&BTreeMap::from([(foreign, 1_u32.into())]), &[])
            .is_err()
    );
    assert_eq!(
        transport.requests.load(std::sync::atomic::Ordering::SeqCst),
        before,
        "foreign principal must fail before any network read"
    );
}

#[test]
fn restricted_fee_routes_use_disjoint_buckets_and_never_borrow_other_scope_funds() {
    use iroha_model_base::topology::DataSpaceId;
    let config = fixture_config();
    let first_ds = DataSpaceId::new(u64::MAX);
    let second_ds = DataSpaceId::new(u64::MAX - 1);
    let first_scope = AssetBalanceScope::Dataspace(first_ds);
    let second_scope = AssetBalanceScope::Dataspace(second_ds);
    let asset = AssetId::with_scope(
        XOR_ASSET_DEFINITION.parse().unwrap(),
        config.account.clone(),
        first_scope,
    );
    let principal = BTreeMap::from([(asset, Quantity::from(10_u32))]);
    let mut first = quote(&config.account, 3, false, 0);
    first.observation.route_dataspace_id = first_ds;
    let mut second = quote(&config.account, 4, false, 0);
    second.observation.route_dataspace_id = second_ds;
    let transport = Arc::new(BalanceTransport {
        policy: AssetBalancePolicy::DataspaceRestricted,
        holdings: BTreeMap::from([(first_scope, 13), (second_scope, 1000)]),
        ..BalanceTransport::default()
    });
    let client = Client::with_http_transport(config.clone(), transport).unwrap();
    let result = client
        .check_funding(&principal, &[first.clone(), second.clone()])
        .unwrap();
    assert_eq!(result.len(), 2);
    let first_result = result
        .iter()
        .find(|item| item.balance_scope == first_scope)
        .unwrap();
    let second_result = result
        .iter()
        .find(|item| item.balance_scope == second_scope)
        .unwrap();
    assert_eq!(first_result.required, Quantity::from(13_u32));
    assert_eq!(second_result.required, Quantity::from(4_u32));
    let encoded = json::to_value(first_result).unwrap();
    assert_eq!(
        json::from_value::<AssetBalanceScope>(encoded["balance_scope"].clone()).unwrap(),
        first_scope
    );
    let error = client
        .check_funding(&principal, &[first.clone(), second, first])
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("required 16") && error.contains("available 13"),
        "{error}"
    );
    assert!(
        error.contains(&u64::MAX.to_string()),
        "failure identifies the exact bucket: {error}"
    );
}

#[test]
fn global_fee_routes_share_one_bucket_and_restricted_sponsors_fail() {
    use iroha_model_base::topology::DataSpaceId;
    let config = fixture_config();
    let principal = BTreeMap::from([(
        AssetId::new(
            XOR_ASSET_DEFINITION.parse().unwrap(),
            config.account.clone(),
        ),
        70_u32.into(),
    )]);
    let mut first = quote(&config.account, 3, false, 0);
    first.observation.route_dataspace_id = DataSpaceId::new(u64::MAX);
    let mut second = quote(&config.account, 4, false, 0);
    second.observation.route_dataspace_id = DataSpaceId::new(u64::MAX - 1);
    let client =
        Client::with_http_transport(config.clone(), Arc::new(BalanceTransport::default())).unwrap();
    let result = client.check_funding(&principal, &[first, second]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].balance_scope, AssetBalanceScope::Global);
    assert_eq!(result[0].required, Quantity::from(77_u32));
    let sponsored = quote(&config.account, 4, true, 8);
    let client = Client::with_http_transport(
        config,
        Arc::new(BalanceTransport {
            policy: AssetBalancePolicy::DataspaceRestricted,
            holdings: BTreeMap::new(),
            ..BalanceTransport::default()
        }),
    )
    .unwrap();
    assert!(
        client
            .check_funding(&BTreeMap::new(), &[sponsored])
            .unwrap_err()
            .to_string()
            .contains("registered global asset policy")
    );
}
