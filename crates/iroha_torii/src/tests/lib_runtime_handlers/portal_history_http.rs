//! Signed portal history requests traverse production authentication, routing and owner growth.

use super::*;
use axum::{
    Router,
    middleware::{Next, from_fn_with_state},
};
use iroha_core::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
use iroha_data_model::{
    asset::AssetBalancePolicy,
    isi::{InstructionBox, Log, Mint, Register, Transfer},
    level::Level,
};
use iroha_executor_data_model::isi::multisig::{
    DEFAULT_MULTISIG_TTL_MS, MultisigApprove, MultisigProposalId, MultisigProposalNonce,
    MultisigPropose, MultisigRegister, MultisigSpec,
};
use iroha_torii_shared::list_query::{ListQuery, field};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU16,
    sync::Mutex,
};

struct HistoryFixture {
    app: SharedAppState,
    signer: KeyPair,
    recipient: AccountId,
    multisig: AccountId,
    first_height: u64,
    height: u64,
    transactions: BTreeSet<String>,
}

fn populated_history() -> HistoryFixture {
    let signer = checked_torii_test_ed25519_keypair(0x51, "portal history signer");
    let sender = AccountId::new(signer.public_key().clone());
    let recipient = AccountId::new(
        checked_torii_test_ed25519_keypair(0x52, "portal history recipient")
            .public_key()
            .clone(),
    );
    let anchor = AccountId::new(
        checked_torii_test_ed25519_keypair(0x53, "portal history multisig anchor")
            .public_key()
            .clone(),
    );
    let domain =
        iroha_model_base::domain::DomainId::try_new("portal_history", "universal").unwrap();
    let definition =
        AssetDefinitionId::derive_from_components(domain.clone(), "coin".parse().unwrap());
    let mut config = TestChainConfig::new(World::new(), 1_000);
    let genesis_key = config.genesis_key.clone();
    config.genesis_instructions = vec![
        Register::domain(Domain::new(domain.clone())).into(),
        Register::account(Account::new(sender.clone())).into(),
        Register::account(Account::new(recipient.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(
            definition.clone(),
            "Portal history coin",
            AssetBalancePolicy::Global,
            Some(domain.clone()),
        ))
        .into(),
    ];
    let prepared = CertifiedTestChain::prepare(config).unwrap();
    let validator = prepared.validator_keys[0].clone();
    let mut chain = CertifiedTestChain::from_prepared(prepared).unwrap();
    let spec = MultisigSpec::new(
        BTreeMap::from([(sender.clone(), 1)]),
        NonZeroU16::new(1).unwrap(),
        NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).unwrap(),
    );
    let register = chain.sign(
        &genesis_key,
        [MultisigRegister::with_account(anchor, domain, spec).into()],
        2_000,
    );
    assert_eq!(chain.commit_at(2_000, vec![register]), [true]);
    let multisig = chain
        .state()
        .view()
        .world()
        .accounts_iter()
        .find(|account| account.id().multisig_policy().is_some())
        .unwrap()
        .id()
        .clone();
    let holding = AssetId::new(definition.clone(), sender);
    let multisig_holding = AssetId::new(definition, multisig.clone());
    let funding = chain.sign(
        &genesis_key,
        [
            Mint::asset_quantity(1_000_u32, holding.clone()).into(),
            Mint::asset_quantity(10_u32, multisig_holding.clone()).into(),
        ],
        3_000,
    );
    assert_eq!(chain.commit_at(3_000, vec![funding]), [true]);
    let instructions = vec![InstructionBox::from(Transfer::asset_quantity(
        multisig_holding,
        7_u32,
        recipient.clone(),
    ))];
    let nonce = MultisigProposalNonce::new([0x54; 32]).unwrap();
    let proposal_id = MultisigProposalId::derive(&chain.network_id(), &multisig, &nonce);
    let instructions_hash = HashOf::new(&instructions);
    let multisig_execution = vec![
        MultisigPropose::new(multisig.clone(), nonce, instructions, None).into(),
        MultisigApprove::new(multisig.clone(), proposal_id, instructions_hash).into(),
    ];
    // A real producer obeys the frozen FASTPQ source cap as well as the nominal
    // transaction count. The unchanged bootstrap policy reserves possible Pipeline
    // callbacks and permits eleven Network inputs per carrier, not 512.
    let parameters = chain.state().view().world().parameters().block();
    let batch_size = usize::try_from(
        u64::from(
            parameters
                .fastpq_source()
                .maximum_network_inputs(parameters.execution_output())
                .unwrap(),
        )
        .min(parameters.max_transactions().get()),
    )
    .unwrap();
    assert!(batch_size > 0);
    let first_height = chain.height() + 1;
    let mut hashes = BTreeSet::new();
    let indices = (0..202).collect::<Vec<_>>();
    // Put the short remainder first so the latest batch has the full admitted count.
    for (batch_index, batch) in indices.rchunks(batch_size).rev().enumerate() {
        let time_ms = 4_000 + 1_000 * u64::try_from(batch_index).unwrap();
        let transactions = batch
            .iter()
            .map(|&index| {
                let instructions = if index == 201 {
                    multisig_execution.clone()
                } else {
                    let padding = if index >= 202usize.saturating_sub(batch_size) {
                        32 * 1024
                    } else {
                        0
                    };
                    vec![
                        Transfer::asset_quantity(holding.clone(), 1_u32, recipient.clone()).into(),
                        // The latest carrier really exceeds the first 256 KiB source
                        // phase, without changing any ledger or query capacity policy.
                        Log::new(
                            Level::DEBUG,
                            format!("portal-history-{index}:{}", "p".repeat(padding)),
                        )
                        .into(),
                    ]
                };
                chain.sign(&signer, instructions, time_ms)
            })
            .collect::<Vec<_>>();
        hashes.extend(
            transactions
                .iter()
                .map(|tx| tx.hash_as_entrypoint().to_string()),
        );
        assert!(
            chain
                .commit_at(time_ms, transactions)
                .into_iter()
                .all(|ok| ok)
        );
    }
    assert_eq!(hashes.len(), 202);
    let height = chain.height();
    let wire_bytes = chain.committed(height).block().encode_wire().unwrap().len();
    assert!(
        wire_bytes > 256 * 1024,
        "fixture really exceeds first source phase"
    );
    assert!(
        wire_bytes < 2 * 1024 * 1024,
        "fixture carrier stays below the configured source ceiling: {wire_bytes}"
    );
    let app = native_ingress_app_from_chain_for_test(&chain, validator);
    HistoryFixture {
        app,
        signer,
        recipient,
        multisig,
        first_height,
        height,
        transactions: hashes,
    }
}

#[derive(Debug)]
struct Attempt {
    bytes: usize,
    refused: bool,
    status: StatusCode,
}
type Attempts = Arc<Mutex<Vec<Attempt>>>;

async fn record_attempt(
    State((app, attempts)): State<(SharedAppState, Attempts)>,
    request: axum::http::Request<Body>,
    next: Next,
) -> Response {
    let owner = super::super::current_query_fanout_memory_for_state(&app)
        .expect("real route retains its admitted owner");
    let bytes = super::super::current_routed_read_memory_envelope(&app)
        .unwrap()
        .working_set_bytes;
    let response = next.run(request).await;
    attempts.lock().unwrap().push(Attempt {
        bytes,
        refused: owner.owner_limit_refused(),
        status: response.status(),
    });
    response
}

fn history_router(app: &SharedAppState, attempts: &Attempts) -> Router {
    use crate::router::builder::RouterBuilder;
    use iroha_torii_shared::route_catalog::RouteCatalog;
    use iroha_torii_shared::route_catalog::application_api::{
        ACCOUNTS_BY_ACCOUNT_ID_HISTORY_QUERY_POST, TRANSACTIONS_QUERY_POST,
    };
    let mut builder = RouterBuilder::new(
        Arc::clone(app),
        RouteCatalog::new(&[
            TRANSACTIONS_QUERY_POST,
            ACCOUNTS_BY_ACCOUNT_ID_HISTORY_QUERY_POST,
        ]),
        super::super::compiled_route_features(),
    )
    .unwrap();
    // These are the production handlers and the same canonical-body authentication
    // mounted by mount_transactions_query / mount_account_history_query.
    for (descriptor, method) in [
        (
            &TRANSACTIONS_QUERY_POST,
            super::super::catalog_post(super::super::handler_transactions_query),
        ),
        (
            &ACCOUNTS_BY_ACCOUNT_ID_HISTORY_QUERY_POST,
            super::super::catalog_post(super::super::handler_account_history_query),
        ),
    ] {
        let method = method
            .layer(from_fn_with_state(
                (Arc::clone(app), Arc::clone(attempts)),
                record_attempt,
            ))
            .optionally_authenticated_canonical_account_body(Arc::clone(app), 64 * 1024);
        builder.route(descriptor, method);
    }
    let (router, manifest) = builder.finish().unwrap();
    router
        .with_state(Arc::clone(app))
        .layer(from_fn_with_state(
            Arc::clone(app),
            super::super::enforce_app_routed_read_http_admission,
        ))
        .layer(from_fn_with_state(
            manifest.route_index(),
            super::super::attach_matched_route_metadata,
        ))
        .layer(Extension(crate::loopback_connect_info()))
        .layer(axum::middleware::from_fn(
            |request, next: Next| async move {
                crate::utils::with_current_response_format(ResponseFormat::Json, next.run(request))
                    .await
            },
        ))
}

async fn signed_page(
    fixture: &HistoryFixture,
    router: &Router,
    attempts: &Attempts,
    uri: &str,
    query: ListQuery,
) -> Vec<norito::json::Map> {
    attempts.lock().unwrap().clear();
    let uri: axum::http::Uri = uri.parse().unwrap();
    let body = norito::json::to_vec(&query).unwrap();
    let account = AccountId::new(fixture.signer.public_key().clone());
    let headers = signed_network_app_headers(
        fixture.app.state.network_id_ref(),
        &account,
        &fixture.signer,
        &Method::POST,
        &uri,
        &body,
    );
    let request = || {
        let mut request = axum::http::Request::builder()
            .method(Method::POST)
            .uri(uri.clone())
            .body(Body::from(body.clone()))
            .unwrap();
        *request.headers_mut() = headers.clone();
        request.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        request.headers_mut().insert(
            axum::http::header::CONTENT_LENGTH,
            HeaderValue::from_str(&body.len().to_string()).unwrap(),
        );
        request
    };
    let response = router.clone().oneshot(request()).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "attempts={:?}; body={}",
        attempts.lock().unwrap(),
        String::from_utf8_lossy(&bytes)
    );
    {
        let attempts = attempts.lock().unwrap();
        assert!(
            attempts.len() >= 2,
            "real source/decode pressure must grow the owner: {attempts:?}"
        );
        assert!(
            attempts
                .windows(2)
                .all(|pair| pair[0].bytes < pair[1].bytes)
        );
        assert!(
            attempts[..attempts.len() - 1]
                .iter()
                .all(|attempt| attempt.refused && !attempt.status.is_success())
        );
        assert_eq!(attempts.last().unwrap().status, StatusCode::OK);
    }
    let value: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let rows = value["items"]
        .as_array()
        .expect("collection page items")
        .iter()
        .map(|row| row.as_object().unwrap().clone())
        .collect();
    let replay = router.clone().oneshot(request()).await.unwrap();
    assert_eq!(
        replay.status(),
        StatusCode::UNAUTHORIZED,
        "external nonce replay remains forbidden"
    );
    let replay = axum::body::to_bytes(replay.into_body(), 64 * 1024)
        .await
        .unwrap();
    assert!(String::from_utf8_lossy(&replay).contains("request nonce already used"));
    rows
}

#[test]
fn signed_portal_history_pages_grow_owner_and_reject_replay() {
    run_executed_block_wire_handler_test("portal-history-http", || async {
        let fixture = populated_history();
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let router = history_router(&fixture.app, &attempts);
        let mut pages = Vec::new();
        for limit in [20, 200] {
            let rows = signed_page(
                &fixture,
                &router,
                &attempts,
                "/v1/transactions/query",
                ListQuery::new()
                    .limit(limit)
                    .filter(
                        field("block_height")
                            .gte(fixture.first_height)
                            .and(field("block_height").lte(fixture.height)),
                    )
                    .select(["entrypoint_hash"]),
            )
            .await;
            assert_eq!(rows.len(), limit as usize);
            let hashes = rows
                .iter()
                .map(|row| row["entrypoint_hash"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            assert_eq!(hashes.iter().collect::<BTreeSet<_>>().len(), hashes.len());
            assert!(
                hashes
                    .iter()
                    .all(|hash| fixture.transactions.contains(hash))
            );
            pages.push(hashes);
        }
        assert_eq!(pages[0], pages[1][..20]);
        let rows = signed_page(
            &fixture,
            &router,
            &attempts,
            &format!("/v1/accounts/{}/history/query", fixture.recipient),
            ListQuery::new().limit(20).filter(
                field("block_height")
                    .eq(fixture.height)
                    .and(field("direction").eq("incoming"))
                    .and(field("counterparty_account_id").eq(fixture.multisig.to_string())),
            ),
        )
        .await;
        assert_eq!(rows.len(), 1, "exact executed multisig movement");
        assert!(rows[0]["id"].as_str().unwrap().contains(":multisig:"));
        assert_eq!(rows[0]["amount"].as_str(), Some("7"));
    });
}
