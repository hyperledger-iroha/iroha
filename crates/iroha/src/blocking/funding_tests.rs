//! Exact native balance and cumulative funding boundary tests.
use super::*;
use crate::{
    config::Config,
    http::{HttpTransport, Response, TransportFuture, TransportRequest},
};
use iroha_version::codec::DecodeVersioned as _;
use std::{path::Path, sync::Arc};
const XOR_ASSET_DEFINITION: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
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

#[derive(Debug)]
struct BalanceTransport {
    missing: Option<&'static str>,
    foreign: bool,
}
impl HttpTransport for BalanceTransport {
    fn send_blocking(&self, request: TransportRequest) -> Result<Response<Vec<u8>>> {
        use iroha_data_model::{
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
                        AssetBalancePolicy::Global,
                        None,
                    )
                    .build(&authority),
                )),
            ),
            QueryRequest::Singular(SingularQueryBox::FindAssetById(query)) => {
                let id = AssetId::new(XOR_ASSET_DEFINITION.parse()?, authority.clone());
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
                } else {
                    id
                };
                (
                    "holding",
                    QueryResponse::Singular(SingularQueryOutputBox::Asset(Asset::new(
                        returned_id,
                        77_u32,
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
                let missing = AssetId::new(
                    XOR_ASSET_DEFINITION.parse()?,
                    if self.missing == Some("wrong-missing-holding") {
                        iroha_test_samples::BOB_ID.clone()
                    } else {
                        authority
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
            Arc::new(BalanceTransport { missing, foreign }),
        )
        .unwrap();
        let result = client.balance(&XOR_ASSET_DEFINITION.parse().unwrap());
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
        }),
    )
    .unwrap();
    let principal = BTreeMap::from([(
        XOR_ASSET_DEFINITION.parse().unwrap(),
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
        XOR_ASSET_DEFINITION.parse().unwrap(),
        Quantity::from(70_u32),
    )]);
    let first = quote(&config.account, 4, true, 8);
    let (account, sponsor) =
        aggregate_funding(&config.account, &principal, &[first.clone(), first.clone()]).unwrap();
    assert_eq!(account, principal);
    assert_eq!(sponsor[0].required, Quantity::from(8_u32));
    assert_eq!(sponsor[0].available, Quantity::from(8_u32));
    let weaker = quote(&config.account, 4, true, 7);
    assert!(
        aggregate_funding(&config.account, &principal, &[first.clone(), weaker])
            .unwrap_err()
            .to_string()
            .contains("required 8")
    );
    let mut missing_capacity = first.clone();
    missing_capacity.capacities.clear();
    assert!(aggregate_funding(&config.account, &principal, &[missing_capacity]).is_err());
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
        aggregate_funding(&config.account, &principal, &[first, revision])
            .unwrap_err()
            .to_string()
            .contains("revision")
    );
}
