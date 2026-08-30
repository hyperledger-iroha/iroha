use super::*;
use axum::http::StatusCode;
use http_body_util::BodyExt as _;
use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute as _,
    state::{State, World},
    sumeragi::network_topology::Topology,
    tx::AcceptedTransaction,
};
use iroha_crypto::Algorithm;
use iroha_data_model::prelude as dm;
use iroha_primitives::const_vec::ConstVec;
use std::{borrow::Cow, sync::Arc};
// use tower::ServiceExt; // not needed in this module
const TEST_ACCOUNT: &str = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";
fn checked_smoke_keypair(
    seed: u8,
    algorithm: iroha_crypto::Algorithm,
    context: &'static str,
) -> KeyPair {
    checked_routing_fixture_keypair(seed, algorithm, context)
}
fn checked_smoke_account(seed: u8, context: &'static str) -> (dm::AccountId, KeyPair) {
    let kp = checked_smoke_keypair(seed, iroha_crypto::Algorithm::Ed25519, context);
    let account = dm::AccountId::new(kp.public_key().clone());
    (account, kp)
}
fn checked_smoke_account_id(seed: u8, context: &'static str) -> AccountId {
    AccountId::new(
        checked_smoke_keypair(seed, iroha_crypto::Algorithm::Ed25519, context)
            .public_key()
            .clone(),
    )
}
fn account_with_key() -> (dm::AccountId, KeyPair) {
    checked_smoke_account(0x40, "derive transaction query smoke fixture account key")
}
#[must_use]
struct DebugEnvGuard {
    prev_torii_debug_match: bool,
    prev_iroha_debug_tx_eval: bool,
}
impl DebugEnvGuard {
    fn enable() -> Self {
        let prev_torii_debug_match = super::debug_toggle_override::set_torii_override(true);
        let prev_iroha_debug_tx_eval = super::debug_toggle_override::set_iroha_override(true);
        Self {
            prev_torii_debug_match,
            prev_iroha_debug_tx_eval,
        }
    }
}
impl Drop for DebugEnvGuard {
    fn drop(&mut self) {
        super::debug_toggle_override::set_torii_override(self.prev_torii_debug_match);
        super::debug_toggle_override::set_iroha_override(self.prev_iroha_debug_tx_eval);
    }
}
fn obj(pairs: Vec<(&'static str, Value)>) -> Value {
    crate::json_object(pairs)
}
fn arr(values: Vec<Value>) -> Value {
    crate::json_array(values)
}
fn val<T: json::JsonSerialize + ?Sized>(value: &T) -> Value {
    crate::json_value(value)
}
fn decode_latin1_utf8(input: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(input.len());
    for ch in input.chars() {
        let code = ch as u32;
        if code > 0xFF {
            return None;
        }
        bytes.push(code as u8);
    }
    String::from_utf8(bytes).ok()
}
fn log_instruction() -> dm::InstructionBox {
    dm::Log::new(dm::Level::INFO, "test".to_string()).into()
}
#[tokio::test]
async fn handle_v1_account_transactions_returns_empty_on_blank_state() {
    // Minimal in-memory state: no blocks yet
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = iroha_core::state::State::new_for_testing(World::default(), kura, query);
    // Request with a simple filter (authority == alice) and default pagination
    let env = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::Eq(
            crate::filter::FieldPath("authority".into()),
            norito::json::Value::String(
                "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE".into(),
            ),
        )),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: Some(50),
            offset: 0,
        },
        fetch_size: None,
        count_mode: Some("exact".to_owned()),
    };
    let resp = handle_v1_account_transactions(
        Arc::new(state),
        axum::extract::Path("sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE".into()),
        crate::utils::extractors::NoritoJson(env),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let s = String::from_utf8(body.to_vec()).unwrap();
    let v: norito::json::Value = norito::json::from_str(&s).unwrap();
    let items_len = match &v {
        norito::json::Value::Object(m) => m
            .get("items")
            .and_then(|v| match v {
                norito::json::Value::Array(a) => Some(a.len()),
                _ => None,
            })
            .unwrap_or(0),
        _ => 0,
    };
    let total = match &v {
        norito::json::Value::Object(m) => m
            .get("total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0),
        _ => 0,
    };
    assert_eq!(items_len, 0);
    assert_eq!(total, 0);
}
#[tokio::test]
async fn handle_v1_transactions_query_returns_empty_on_blank_state() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = iroha_core::state::State::new_for_testing(World::default(), kura, query);
    let env = crate::filter::QueryEnvelope {
        query: None,
        filter: None,
        select: None,
        aggregate: None,
        sort: vec![crate::filter::SortKey {
            key: crate::filter::FieldPath("timestamp_ms".into()),
            order: crate::filter::Order::Desc,
        }],
        pagination: crate::filter::Pagination {
            limit: Some(50),
            offset: 0,
        },
        fetch_size: None,
        count_mode: Some("exact".to_owned()),
    };
    let resp = handle_v1_transactions_query(
        Arc::new(state),
        crate::utils::extractors::NoritoJson(env),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(v["items"].as_array().unwrap().len(), 0);
    assert_eq!(v["total"].as_u64(), Some(0));
}
#[tokio::test]
async fn transactions_query_aggregate_uses_sparse_index_for_an_exact_miss() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = iroha_core::state::State::new_for_testing(World::default(), kura, query);
    let missing_entrypoint = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"missing-aggregate-entrypoint",
    ))
    .to_string();
    let env = crate::filter::QueryEnvelope {
        query: Some("transaction-aggregate-sparse-miss".to_owned()),
        filter: Some(crate::filter::FilterExpr::Eq(
            crate::filter::FieldPath("entrypoint_hash".into()),
            norito::json::Value::String(missing_entrypoint),
        )),
        select: None,
        aggregate: Some(crate::filter::AggregateSpec {
            group_by: vec![crate::filter::FieldPath("metadata.readiness_probe".into())],
            metrics: vec![crate::filter::AggregateMetric {
                alias: "occurrences".to_owned(),
                r#fn: crate::filter::AggregateFn::Count,
                field: None,
            }],
            having: None,
        }),
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: Some(1),
            offset: 0,
        },
        fetch_size: None,
        count_mode: Some("exact".to_owned()),
    };
    let response = handle_v1_transactions_query(
        Arc::new(state),
        crate::utils::extractors::NoritoJson(env),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("aggregate sparse miss")
    .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let value: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert!(value["items"].as_array().unwrap().is_empty());
    assert_eq!(value["total"].as_u64(), Some(0));
    assert_eq!(value["has_more"].as_bool(), Some(false));
    assert_eq!(value["count_mode"].as_str(), Some("exact"));
    assert_eq!(value["indexed_height"].as_u64(), Some(0));
    assert!(value["indexed_block_hash"].is_null());
    assert_eq!(value["query_source"].as_str(), Some("live"));
}
#[tokio::test]
async fn handle_v1_transactions_visible_query_returns_empty_on_blank_state() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = iroha_core::state::State::new_for_testing(World::default(), kura, query);
    let viewer = checked_smoke_account_id(0x41, "derive visible-query viewer fixture key");
    let env = crate::filter::QueryEnvelope {
        query: Some("VisibleTransactions".to_owned()),
        filter: None,
        select: None,
        aggregate: None,
        sort: vec![crate::filter::SortKey {
            key: crate::filter::FieldPath("timestamp_ms".into()),
            order: crate::filter::Order::Desc,
        }],
        pagination: crate::filter::Pagination {
            limit: Some(50),
            offset: 0,
        },
        fetch_size: None,
        count_mode: Some("exact".to_owned()),
    };
    let resp = handle_v1_transactions_visible_query_with_policy(
        Arc::new(state),
        crate::utils::extractors::NoritoJson(env),
        crate::routing::MaybeTelemetry::for_tests(),
        TxHistoryVisibilityScope {
            viewer_account_ids: vec![viewer],
            viewer_dataspace_id: "wonderland".to_owned(),
            allow_dataspace_wide: false,
            asset_definition_domains: std::collections::BTreeMap::new(),
        },
        None,
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(v["items"].as_array().unwrap().len(), 0);
    assert_eq!(v["total"].as_u64(), Some(0));
}
#[tokio::test]
async fn account_transactions_query_rejects_limit_above_cap() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = iroha_core::state::State::new_for_testing(World::default(), kura, query);
    let cap = app_query_limits().max_page_limit;
    let env = crate::filter::QueryEnvelope {
        query: None,
        filter: None,
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: Some(cap + 1),
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let err = handle_v1_account_transactions(
        Arc::new(state),
        axum::extract::Path(TEST_ACCOUNT.to_string()),
        crate::utils::extractors::NoritoJson(env),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await;
    match err {
        Err(Error::AppQueryValidation { code, .. }) => assert_eq!(code, "invalid_pagination"),
        Err(other) => panic!("unexpected error: {other:?}"),
        Ok(_) => panic!("expected error for limit above cap"),
    }
}
#[tokio::test]
async fn account_transactions_query_rejects_invalid_field_path() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = iroha_core::state::State::new_for_testing(World::default(), kura, query);
    let env = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::Eq(
            crate::filter::FieldPath("unsupported.field".into()),
            norito::json::Value::from(1u64),
        )),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: Some(1),
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let err = handle_v1_account_transactions(
        Arc::new(state),
        axum::extract::Path(TEST_ACCOUNT.to_string()),
        crate::utils::extractors::NoritoJson(env),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await;
    match err {
        Err(Error::AppQueryValidation { code, .. }) => assert_eq!(code, "invalid_field_path"),
        Err(other) => panic!("unexpected error: {other:?}"),
        Ok(_) => panic!("expected error for invalid field"),
    }
}
#[tokio::test]
async fn account_transactions_get_rejects_limit_above_cap() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = iroha_core::state::State::new_for_testing(World::default(), kura, query);
    let cap = app_query_limits().max_page_limit;
    let params = AccountTransactionsGetParams {
        limit: Some(cap + 1),
        offset: 0,
        asset_id: None,
        count_mode: None,
    };
    let err = handle_v1_account_transactions_get(
        Arc::new(state),
        axum::extract::Path(TEST_ACCOUNT.to_string()),
        crate::NoritoQuery(params),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await;
    match err {
        Err(Error::AppQueryValidation { code, .. }) => assert_eq!(code, "invalid_pagination"),
        Err(other) => panic!("unexpected error: {other:?}"),
        Ok(_) => panic!("expected error for limit above cap"),
    }
}
#[tokio::test]
async fn account_transactions_get_filters_by_asset_id() {
    use iroha_crypto::Algorithm;
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    // Prepare world: domain + account
    let leader0 = checked_smoke_keypair(
        0x42,
        Algorithm::BlsNormal,
        "derive asset-filter setup block leader fixture key",
    );
    let _topo0 = Topology::new(vec![dm::PeerId::new(leader0.public_key().clone())]);
    let unverified0 = BlockBuilder::new(vec![dummy_accepted_transaction()])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader0.private_key())
        .unpack(|_| {});
    let mut st_block0 = state.block(unverified0.header());
    let mut stx0 = st_block0.transaction();
    let domain_id: dm::DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let kp_exec = checked_smoke_keypair(
        0x43,
        Algorithm::Ed25519,
        "derive asset-filter executor fixture key",
    );
    let exec_id = dm::AccountId::new(kp_exec.public_key().clone());
    dm::Register::domain(dm::Domain::new(domain_id.clone()))
        .execute(exec_id.account(), &mut stx0)
        .ok();
    let kp_actor = checked_smoke_keypair(
        0x44,
        Algorithm::Ed25519,
        "derive asset-filter actor fixture key",
    );
    let actor_id = dm::AccountId::new(kp_actor.public_key().clone());
    dm::Register::account(dm::Account::new(actor_id.account().clone()))
        .execute(exec_id.account(), &mut stx0)
        .ok();
    stx0.apply();
    let valid0 = unverified0
        .clone()
        .validate_and_record_transactions(&mut st_block0)
        .unpack(|_| {});
    let committed0 = valid0.commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block0, committed0);
    let network_id = *state.network_id_ref();
    let asset_def: dm::AssetDefinitionId =
        test_asset_definition_id_from_hex("550e8400e29b41d4a7164466554400dd");
    let asset_id = dm::AssetId::new(asset_def, actor_id.clone().into());
    let mint = dm::Mint::asset_quantity(1_u32, asset_id.clone());
    let mut bldr_asset = dm::TransactionBuilder::new(
        network_id,
        actor_id.clone().into(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    bldr_asset.set_creation_time(core::time::Duration::from_millis(1000));
    let signed_asset = bldr_asset
        .with_instructions::<dm::InstructionBox>([mint.into()])
        .sign(kp_actor.private_key());
    let entry_hash_asset = format!("{}", signed_asset.hash_as_entrypoint());
    let tx_asset = AcceptedTransaction::new_unchecked(Cow::Owned(signed_asset));
    let mut bldr_log = dm::TransactionBuilder::new(
        network_id,
        actor_id.clone().into(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    bldr_log.set_creation_time(core::time::Duration::from_millis(1100));
    let signed_log = bldr_log
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_actor.private_key());
    let tx_log = AcceptedTransaction::new_unchecked(Cow::Owned(signed_log));
    let leader = checked_smoke_keypair(
        0x45,
        Algorithm::BlsNormal,
        "derive asset-filter transaction block leader fixture key",
    );
    let _topo = Topology::new(vec![dm::PeerId::new(leader.public_key().clone())]);
    let unverified = BlockBuilder::new(vec![tx_asset, tx_log])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut st_block = state.block(unverified.header());
    let valid: ValidBlock = unverified
        .validate_and_record_transactions(&mut st_block)
        .unpack(|_| {});
    let committed = valid.clone().commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block, committed);
    let params = AccountTransactionsGetParams {
        limit: Some(10),
        offset: 0,
        asset_id: Some(asset_id.to_string()),
        count_mode: Some("exact".to_owned()),
    };
    let actor_literal = actor_id
        .account()
        .to_account_address()
        .and_then(|address| address.to_i105())
        .expect("actor i105 literal");
    let resp = handle_v1_account_transactions_get(
        state,
        axum::extract::Path(actor_literal),
        crate::NoritoQuery(params),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let parsed: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(parsed["total"].as_u64(), Some(1));
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0]["entrypoint_hash"].as_str(),
        Some(entry_hash_asset.as_str())
    );
}
#[tokio::test]
async fn account_transactions_get_includes_recipient_transfer_asset_filters() {
    use iroha_crypto::Algorithm;
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let leader0 = checked_smoke_keypair(
        0x46,
        Algorithm::BlsNormal,
        "derive recipient-filter setup block leader fixture key",
    );
    let _topo0 = Topology::new(vec![dm::PeerId::new(leader0.public_key().clone())]);
    let unverified0 = BlockBuilder::new(vec![dummy_accepted_transaction()])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader0.private_key())
        .unpack(|_| {});
    let mut st_block0 = state.block(unverified0.header());
    let mut stx0 = st_block0.transaction();
    let domain_id: dm::DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let kp_exec = checked_smoke_keypair(
        0x47,
        Algorithm::Ed25519,
        "derive recipient-filter executor fixture key",
    );
    let exec_id = dm::AccountId::new(kp_exec.public_key().clone());
    let kp_alice = checked_smoke_keypair(
        0x48,
        Algorithm::Ed25519,
        "derive recipient-filter alice fixture key",
    );
    let alice_id = dm::AccountId::new(kp_alice.public_key().clone());
    let kp_bob = checked_smoke_keypair(
        0x49,
        Algorithm::Ed25519,
        "derive recipient-filter bob fixture key",
    );
    let bob_id = dm::AccountId::new(kp_bob.public_key().clone());
    let def_id: dm::AssetDefinitionId =
        test_asset_definition_id_from_hex("550e8400e29b41d4a7164466554400dd");
    dm::Register::domain(dm::Domain::new(domain_id.clone()))
        .execute(exec_id.account(), &mut stx0)
        .ok();
    dm::Register::account(dm::Account::new(exec_id.account().clone()))
        .execute(exec_id.account(), &mut stx0)
        .ok();
    dm::Register::account(dm::Account::new(alice_id.account().clone()))
        .execute(exec_id.account(), &mut stx0)
        .ok();
    dm::Register::account(dm::Account::new(bob_id.account().clone()))
        .execute(exec_id.account(), &mut stx0)
        .ok();
    dm::Register::asset_definition({
        let __asset_definition_id = def_id.clone();
        dm::AssetDefinition::numeric(
            __asset_definition_id.clone(),
            asset_definition_display_name(&__asset_definition_id),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
    })
    .execute(exec_id.account(), &mut stx0)
    .ok();
    dm::Mint::asset_quantity(
        100_u32,
        dm::AssetId::new(def_id.clone(), alice_id.account().clone()),
    )
    .execute(exec_id.account(), &mut stx0)
    .ok();
    stx0.apply();
    let valid0 = unverified0
        .clone()
        .validate_and_record_transactions(&mut st_block0)
        .unpack(|_| {});
    let committed0 = valid0.commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block0, committed0);
    let network_id = *state.network_id_ref();
    let source_asset_id = dm::AssetId::new(def_id.clone(), alice_id.account().clone());
    let recipient_asset_id = dm::AssetId::new(def_id.clone(), bob_id.account().clone());
    let unrelated_asset_id = dm::AssetId::new(def_id.clone(), exec_id.account().clone());
    let mut tx_builder = dm::TransactionBuilder::new(
        network_id,
        alice_id.account().clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    tx_builder.set_creation_time(core::time::Duration::from_millis(1_000));
    let signed_transfer = tx_builder
        .with_instructions::<dm::InstructionBox>([dm::Transfer::asset_quantity(
            source_asset_id,
            7_u32,
            bob_id.account().clone(),
        )
        .into()])
        .sign(kp_alice.private_key());
    let entry_hash = format!("{}", signed_transfer.hash_as_entrypoint());
    let transfer_tx = AcceptedTransaction::new_unchecked(Cow::Owned(signed_transfer));
    let leader = checked_smoke_keypair(
        0x4A,
        Algorithm::BlsNormal,
        "derive recipient-filter transaction block leader fixture key",
    );
    let _topo = Topology::new(vec![dm::PeerId::new(leader.public_key().clone())]);
    let unverified = BlockBuilder::new(vec![transfer_tx])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut st_block = state.block(unverified.header());
    let valid: ValidBlock = unverified
        .validate_and_record_transactions(&mut st_block)
        .unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block, committed);
    let bob_literal = bob_id
        .account()
        .to_account_address()
        .and_then(|address| address.to_i105())
        .expect("recipient i105 literal");
    let resp = handle_v1_account_transactions_get(
        state.clone(),
        axum::extract::Path(bob_literal.clone()),
        crate::NoritoQuery(AccountTransactionsGetParams {
            limit: Some(10),
            offset: 0,
            asset_id: Some(def_id.to_string()),
            count_mode: Some("exact".to_owned()),
        }),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let parsed: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(parsed["total"].as_u64(), Some(1));
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0]["entrypoint_hash"].as_str(),
        Some(entry_hash.as_str())
    );
    let resp = handle_v1_account_transactions_get(
        state.clone(),
        axum::extract::Path(bob_literal.clone()),
        crate::NoritoQuery(AccountTransactionsGetParams {
            limit: Some(10),
            offset: 0,
            asset_id: Some(recipient_asset_id.to_string()),
            count_mode: Some("exact".to_owned()),
        }),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("recipient bucket handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let parsed: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(parsed["total"].as_u64(), Some(1));
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0]["entrypoint_hash"].as_str(),
        Some(entry_hash.as_str())
    );
    let resp = handle_v1_account_transactions_get(
        state.clone(),
        axum::extract::Path(bob_literal),
        crate::NoritoQuery(AccountTransactionsGetParams {
            limit: Some(10),
            offset: 0,
            asset_id: Some(unrelated_asset_id.to_string()),
            count_mode: Some("exact".to_owned()),
        }),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("unrelated bucket handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let parsed: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(parsed["total"].as_u64(), Some(0));
    assert!(items.is_empty());
    let resp = handle_v1_transactions_history_get(
        state,
        crate::NoritoQuery(AccountTransactionsGetParams {
            limit: Some(10),
            offset: 0,
            asset_id: Some(def_id.to_string()),
            count_mode: Some("exact".to_owned()),
        }),
        crate::routing::MaybeTelemetry::for_tests(),
        TxHistoryVisibilityScope {
            viewer_account_ids: vec![bob_id.account().clone()],
            viewer_dataspace_id: domain_id.to_string(),
            allow_dataspace_wide: false,
            asset_definition_domains: std::collections::BTreeMap::new(),
        },
        None,
    )
    .await
    .expect("history handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let parsed: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(parsed["total"].as_u64(), Some(1));
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0]["entrypoint_hash"].as_str(),
        Some(entry_hash.as_str())
    );
}
#[tokio::test]
async fn handle_v1_contracts_activity_returns_contract_call_metadata() {
    use iroha_crypto::Algorithm;
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let leader0 = checked_smoke_keypair(
        0x4B,
        Algorithm::BlsNormal,
        "derive contract-activity setup block leader fixture key",
    );
    let _topo0 = Topology::new(vec![dm::PeerId::new(leader0.public_key().clone())]);
    let unverified0 = BlockBuilder::new(vec![dummy_accepted_transaction()])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader0.private_key())
        .unpack(|_| {});
    let mut st_block0 = state.block(unverified0.header());
    let valid0 = unverified0
        .clone()
        .validate_and_record_transactions(&mut st_block0)
        .unpack(|_| {});
    let committed0 = valid0.commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block0, committed0);
    let (authority, keypair) = account_with_key();
    let network_id = *state.network_id_ref();
    let mut metadata = dm::Metadata::default();
    metadata.insert(
        "contract_address".parse().unwrap(),
        dm::Json::new("irohac1fixturedlmmrouter"),
    );
    metadata.insert(
        "contract_alias".parse().unwrap(),
        dm::Json::new("dlmm_router"),
    );
    metadata.insert(
        "contract_entrypoint".parse().unwrap(),
        dm::Json::new("route_swap"),
    );
    metadata.insert(
        "contract_payload".parse().unwrap(),
        dm::Json::new(norito::json!({
            "amount_in": 100,
            "min_out": 95
        })),
    );
    let gas_asset_id = test_asset_definition_id_from_hex("550e8400e29b41d4a7164466554400aa");
    let fee_payment = dm::FeePaymentIntent::sponsor(
        dm::FeeSponsorProgramId::new(
            authority.clone(),
            "contract-activity".parse().expect("program name"),
        ),
        1,
        vec![dm::FeeChargeLimit::new(
            dm::FeeChargeKind::PipelineGas,
            gas_asset_id.clone(),
            dm::Quantity::from(1_000_u32),
        )],
        std::num::NonZeroU64::new(100_000),
    );
    let mut tx_builder = dm::TransactionBuilder::new(network_id, authority.clone(), fee_payment);
    tx_builder.set_creation_time(core::time::Duration::from_millis(1_710_000_000_000));
    let signed = tx_builder
        .with_metadata(metadata)
        .with_executable(dm::Executable::Instructions(ConstVec::from(Vec::<
            dm::InstructionBox,
        >::new(
        ))))
        .sign(keypair.private_key());
    let entry_hash = format!("{}", signed.hash_as_entrypoint());
    let tx = AcceptedTransaction::new_unchecked(Cow::Owned(signed));
    let leader = checked_smoke_keypair(
        0x4C,
        Algorithm::BlsNormal,
        "derive contract-activity transaction block leader fixture key",
    );
    let _topo = Topology::new(vec![dm::PeerId::new(leader.public_key().clone())]);
    let unverified = BlockBuilder::new(vec![tx])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut st_block = state.block(unverified.header());
    let valid = unverified
        .validate_and_record_transactions(&mut st_block)
        .unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block, committed);
    let resp = handle_v1_contracts_activity_get(
        state,
        DataspaceReadVisibility::all_for_tests(),
        crate::NoritoQuery(ContractActivityGetParams {
            limit: Some(10),
            offset: 0,
            authority: Some(authority.to_string()),
            contract_alias: Some("dlmm_router".into()),
            contract_entrypoint: Some("route_swap".into()),
            result_ok: Some(true),
            ..Default::default()
        }),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let parsed: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(parsed["total"].as_u64(), Some(1));
    assert_eq!(
        items[0]["entrypoint_hash"].as_str(),
        Some(entry_hash.as_str())
    );
    assert_eq!(items[0]["contract_alias"].as_str(), Some("dlmm_router"));
    assert_eq!(items[0]["contract_entrypoint"].as_str(), Some("route_swap"));
    assert_eq!(
        items[0]["contract_payload"]["amount_in"].as_u64(),
        Some(100)
    );
    assert_eq!(items[0]["fee_payment"]["payer"].as_str(), Some("sponsor"));
    assert_eq!(
        items[0]["fee_payment"]["value"]["gas_limit"].as_u64(),
        Some(100_000)
    );
    assert_eq!(
        items[0]["fee_payment"]["value"]["charge_limits"][0]["asset_definition_id"]
            .as_str()
            .expect("projected fee asset"),
        gas_asset_id.to_string()
    );
}
#[tokio::test]
async fn handle_v1_account_transactions_returns_and_sorts() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(iroha_core::state::State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    // Prepare world: domain + two accounts
    // Apply domain + accounts in a state transaction, then insert an empty
    // transactions block before committing to satisfy state invariants.
    let leader0 = checked_smoke_keypair(
        0x4D,
        iroha_crypto::Algorithm::BlsNormal,
        "derive sorted account-query setup block leader fixture key",
    );
    let _topo0 = Topology::new(vec![dm::PeerId::new(leader0.public_key().clone())]);
    let unverified0 = BlockBuilder::new(vec![dummy_accepted_transaction()])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader0.private_key())
        .unpack(|_| {});
    let mut st_block0 = state.block(unverified0.header());
    let mut stx = st_block0.transaction();
    let domain_id: dm::DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    // Execute with a placeholder authority; in this test we don't enforce on-chain permissions
    let kp_exec = checked_smoke_keypair(
        0x4E,
        iroha_crypto::Algorithm::Ed25519,
        "derive sorted account-query executor fixture key",
    );
    let exec_id = dm::AccountId::new(kp_exec.public_key().clone());
    dm::Register::domain(dm::Domain::new(domain_id.clone()))
        .execute(exec_id.account(), &mut stx)
        .ok();
    let kp_a = checked_smoke_keypair(
        0x4F,
        iroha_crypto::Algorithm::Ed25519,
        "derive sorted account-query authority fixture key",
    );
    let acc_a = dm::AccountId::new(kp_a.public_key().clone());
    let account_literal = acc_a.account().to_string();
    dm::Register::account(dm::Account::new(acc_a.account().clone()))
        .execute(exec_id.account(), &mut stx)
        .ok();
    stx.apply();
    // Validate and persist a minimal block record to initialize transactions state
    let valid0 = unverified0
        .clone()
        .validate_and_record_transactions(&mut st_block0)
        .unpack(|_| {});
    let committed0 = valid0.commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block0, committed0);
    // Build three transactions for the same authority (two share timestamp for tie-breaking)
    let network_id = *state.network_id_ref();
    let (_max_clock_drift, _tx_limits) = {
        let v = state.view();
        let p = v.world().parameters();
        (p.sumeragi().max_clock_drift(), p.transaction())
    };
    // tx_a: authority acc_a at t=1000ms
    let mut bldr_a = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone().into(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    bldr_a.set_creation_time(core::time::Duration::from_millis(1000));
    let tx_a = bldr_a
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_a.private_key());
    let tx_a = AcceptedTransaction::new_unchecked(Cow::Owned(tx_a));
    // tx_b: authority acc_a at t=2000ms
    let mut bldr_b = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone().into(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    bldr_b.set_creation_time(core::time::Duration::from_millis(2000));
    let signed_b = bldr_b
        .with_instructions::<dm::InstructionBox>([dm::Log::new(
            dm::Level::INFO,
            "test-b".to_string(),
        )
        .into()])
        .sign(kp_a.private_key());
    let _entry_b_str = format!("{}", signed_b.hash_as_entrypoint());
    let tx_b = AcceptedTransaction::new_unchecked(Cow::Owned(signed_b));
    // tx_c: authority acc_a at t=2000ms (different entrypoint hash)
    let mut bldr_c = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone().into(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    bldr_c.set_creation_time(core::time::Duration::from_millis(2000));
    let signed_c = bldr_c
        .with_instructions::<dm::InstructionBox>([dm::Log::new(
            dm::Level::INFO,
            "test-c".to_string(),
        )
        .into()])
        .sign(kp_a.private_key());
    let _entry_c_str = format!("{}", signed_c.hash_as_entrypoint());
    let tx_c = AcceptedTransaction::new_unchecked(Cow::Owned(signed_c));
    // Build one block containing both transactions and commit
    let leader = checked_smoke_keypair(
        0x50,
        iroha_crypto::Algorithm::BlsNormal,
        "derive sorted account-query transaction block leader fixture key",
    );
    let _topo = Topology::new(vec![dm::PeerId::new(leader.public_key().clone())]);
    let unverified = BlockBuilder::new(vec![tx_a, tx_b, tx_c])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut st_block1 = state.block(unverified.header());
    let valid: ValidBlock = unverified
        .validate_and_record_transactions(&mut st_block1)
        .unpack(|_| {});
    // Persist by committing to topology (produces CommittedBlock) and applying state bookkeeping
    let committed = valid.clone().commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block1, committed);
    // Now query via handler with sorting by timestamp_ms ascending, tie-break by entrypoint_hash asc
    let timestamp_filter = crate::filter::FilterExpr::Gte(
        crate::filter::FieldPath("timestamp_ms".into()),
        norito::json::Value::from(1000u64),
    );
    let env = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(timestamp_filter.clone()),
        select: None,
        aggregate: None,
        sort: vec![
            crate::filter::SortKey {
                key: crate::filter::FieldPath("timestamp_ms".into()),
                order: crate::filter::Order::Asc,
            },
            crate::filter::SortKey {
                key: crate::filter::FieldPath("entrypoint_hash".into()),
                order: crate::filter::Order::Asc,
            },
        ],
        pagination: crate::filter::Pagination {
            limit: Some(2),
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(account_literal.clone()),
        crate::utils::extractors::NoritoJson(env),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    #[cfg(test)]
    eprintln!("[debug multi_sort] body={}", String::from_utf8_lossy(&body));
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    // First item should be the earlier timestamp (1000)
    assert_eq!(items[0]["timestamp_ms"].as_u64(), Some(1000));
    // Second page first element (offset=1) should be the lexicographically smaller of the two 2000ms entrypoint hashes
    // Pagination: fetch only the second item
    let env2 = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(timestamp_filter),
        select: None,
        aggregate: None,
        sort: vec![
            crate::filter::SortKey {
                key: crate::filter::FieldPath("timestamp_ms".into()),
                order: crate::filter::Order::Asc,
            },
            crate::filter::SortKey {
                key: crate::filter::FieldPath("entrypoint_hash".into()),
                order: crate::filter::Order::Asc,
            },
        ],
        pagination: crate::filter::Pagination {
            limit: Some(1),
            offset: 1,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp2 = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(account_literal),
        crate::utils::extractors::NoritoJson(env2),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp2.status(), StatusCode::OK);
    let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let v2: norito::json::Value = norito::json::from_slice(&body2).unwrap();
    let items2 = v2["items"].as_array().unwrap();
    assert_eq!(items2.len(), 1);
    // With offset=1, we should get the first 2000ms item in the global
    // ascending order, which is exactly the second element of the first page.
    assert_eq!(items2[0]["timestamp_ms"].as_u64(), Some(2000));
    assert_eq!(
        items2[0]["entrypoint_hash"].as_str(),
        items[1]["entrypoint_hash"].as_str()
    );
}
#[tokio::test]
async fn handle_v1_account_transactions_caps_total_with_fetch_size() {
    use iroha_crypto::Algorithm;
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    // Register domain + operator + target account
    let leader0 = checked_smoke_keypair(
        0x51,
        Algorithm::BlsNormal,
        "derive fetch-size setup block leader fixture key",
    );
    let _topo0 = Topology::new(vec![dm::PeerId::new(leader0.public_key().clone())]);
    let unverified0 = BlockBuilder::new(vec![dummy_accepted_transaction()])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader0.private_key())
        .unpack(|_| {});
    let mut st_block0 = state.block(unverified0.header());
    let mut stx0 = st_block0.transaction();
    let kp_exec = checked_smoke_keypair(
        0x52,
        Algorithm::Ed25519,
        "derive fetch-size executor fixture key",
    );
    let exec_id = dm::AccountId::new(kp_exec.public_key().clone());
    dm::Register::account(dm::Account::new(exec_id.account().clone()))
        .execute(exec_id.account(), &mut stx0)
        .unwrap();
    let kp_actor = checked_smoke_keypair(
        0x53,
        Algorithm::Ed25519,
        "derive fetch-size actor fixture key",
    );
    let actor_id = dm::AccountId::new(kp_actor.public_key().clone());
    dm::Register::account(dm::Account::new(actor_id.account().clone()))
        .execute(exec_id.account(), &mut stx0)
        .unwrap();
    stx0.apply();
    let valid0 = unverified0
        .clone()
        .validate_and_record_transactions(&mut st_block0)
        .unpack(|_| {});
    let committed0 = valid0.commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block0, committed0);
    // Create five transactions for the same authority
    let network_id = *state.network_id_ref();
    let mut accepted = Vec::new();
    for i in 0..5u64 {
        let mut builder = dm::TransactionBuilder::new(
            network_id,
            actor_id.clone().into(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(core::time::Duration::from_millis(1_000 + i * 250));
        let signed = builder
            .with_instructions::<dm::InstructionBox>([log_instruction()])
            .sign(kp_actor.private_key());
        accepted.push(AcceptedTransaction::new_unchecked(Cow::Owned(signed)));
    }
    let leader = checked_smoke_keypair(
        0x54,
        Algorithm::BlsNormal,
        "derive fetch-size transaction block leader fixture key",
    );
    let _topo = Topology::new(vec![dm::PeerId::new(leader.public_key().clone())]);
    let unverified = BlockBuilder::new(accepted)
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut st_block = state.block(unverified.header());
    let valid: ValidBlock = unverified
        .validate_and_record_transactions(&mut st_block)
        .unpack(|_| {});
    let committed = valid.clone().commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block, committed);
    // Query with fetch_size smaller than total to ensure streaming totals kick in.
    let authority_literal = actor_id.account().to_string();
    let env = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::Eq(
            crate::filter::FieldPath("authority".into()),
            norito::json::Value::String(authority_literal.clone()),
        )),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: Some(2),
            offset: 0,
        },
        fetch_size: Some(2),
        count_mode: Some("exact".to_owned()),
    };
    let resp = handle_v1_account_transactions(
        state,
        axum::extract::Path(authority_literal),
        crate::utils::extractors::NoritoJson(env),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let doc: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(doc["items"].as_array().unwrap().len(), 2);
    assert_eq!(doc["total"].as_u64(), Some(4));
}
#[tokio::test]
async fn multi_sort_and_mixed_eq_ne_filter() {
    // Enable detailed filter debug for this test only
    let _debug_env = DebugEnvGuard::enable();
    use iroha_data_model::prelude as dm;
    // State and topology
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(iroha_core::state::State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    // Ensure domain and accounts exist before committing txs
    let kp_a = checked_smoke_keypair(
        0x55,
        iroha_crypto::Algorithm::Ed25519,
        "derive mixed-filter account A fixture key",
    );
    let kp_b = checked_smoke_keypair(
        0x56,
        iroha_crypto::Algorithm::Ed25519,
        "derive mixed-filter account B fixture key",
    );
    let acc_a = dm::AccountId::new(kp_a.public_key().clone());
    let acc_b = dm::AccountId::new(kp_b.public_key().clone());
    let leader0 = checked_smoke_keypair(
        0x57,
        iroha_crypto::Algorithm::BlsNormal,
        "derive mixed-filter setup block leader fixture key",
    );
    let _topo0 = Topology::new(vec![dm::PeerId::new(leader0.public_key().clone())]);
    let unverified0 = BlockBuilder::new(vec![dummy_accepted_transaction()])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader0.private_key())
        .unpack(|_| {});
    let mut st_block0 = state.block(unverified0.header());
    let mut stx0 = st_block0.transaction();
    let kp_seed = checked_smoke_keypair(
        0x58,
        iroha_crypto::Algorithm::Ed25519,
        "derive mixed-filter executor fixture key",
    );
    let exec_id = dm::AccountId::new(kp_seed.public_key().clone());
    dm::Register::account(dm::Account::new(exec_id.account().clone()))
        .execute(exec_id.account(), &mut stx0)
        .unwrap();
    dm::Register::account(dm::Account::new(acc_a.account().clone()))
        .execute(exec_id.account(), &mut stx0)
        .unwrap();
    dm::Register::account(dm::Account::new(acc_b.account().clone()))
        .execute(exec_id.account(), &mut stx0)
        .unwrap();
    stx0.apply();
    let valid0 = unverified0
        .clone()
        .validate_and_record_transactions(&mut st_block0)
        .unpack(|_| {});
    let committed0 = valid0.commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block0, committed0);
    let network_id = *state.network_id_ref();
    let (_max_clock_drift, _tx_limits) = {
        let v = state.view();
        let p = v.world().parameters();
        (p.sumeragi().max_clock_drift(), p.transaction())
    };
    // tx1: t=1000, result_ok=true, capture entrypoint hash string
    let mut b1 = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone().into(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b1.set_creation_time(core::time::Duration::from_millis(1000));
    let signed1 = b1
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_a.private_key());
    let entry_hash1_str = format!("{}", signed1.hash_as_entrypoint());
    let tx1 = AcceptedTransaction::new_unchecked(Cow::Owned(signed1));
    // tx2: t=2000, result_ok=true
    let mut b2 = dm::TransactionBuilder::new(
        network_id,
        acc_b.clone().into(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b2.set_creation_time(core::time::Duration::from_millis(2000));
    let signed2 = b2
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_b.private_key());
    let tx2 = AcceptedTransaction::new_unchecked(Cow::Owned(signed2));
    // Commit the block with both transactions
    let leader = checked_smoke_keypair(
        0x59,
        iroha_crypto::Algorithm::BlsNormal,
        "derive mixed-filter transaction block leader fixture key",
    );
    let _topo = Topology::new(vec![dm::PeerId::new(leader.public_key().clone())]);
    let unverified = BlockBuilder::new(vec![tx1, tx2])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut st_block = state.block(unverified.header());
    let valid: ValidBlock = unverified
        .validate_and_record_transactions(&mut st_block)
        .unpack(|_| {});
    let committed = valid.clone().commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block, committed);
    // Filter: result_ok == true AND entrypoint_hash != entry_hash1_str AND timestamp_ms >= 1500
    // Sort: result_ok desc, timestamp_ms asc, entrypoint_hash asc
    let expr = crate::filter::FilterExpr::And(vec![
        crate::filter::FilterExpr::Eq(
            crate::filter::FieldPath("result_ok".into()),
            norito::json::Value::Bool(true),
        ),
        crate::filter::FilterExpr::Ne(
            crate::filter::FieldPath("entrypoint_hash".into()),
            norito::json::Value::String(entry_hash1_str),
        ),
        crate::filter::FilterExpr::Gte(
            crate::filter::FieldPath("timestamp_ms".into()),
            norito::json::Value::from(1500u64),
        ),
    ]);
    let env = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(expr),
        select: None,
        aggregate: None,
        sort: vec![
            crate::filter::SortKey {
                key: crate::filter::FieldPath("result_ok".into()),
                order: crate::filter::Order::Desc,
            },
            crate::filter::SortKey {
                key: crate::filter::FieldPath("timestamp_ms".into()),
                order: crate::filter::Order::Asc,
            },
            crate::filter::SortKey {
                key: crate::filter::FieldPath("entrypoint_hash".into()),
                order: crate::filter::Order::Asc,
            },
        ],
        pagination: crate::filter::Pagination {
            limit: Some(10),
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(acc_b.account().to_string()),
        crate::utils::extractors::NoritoJson(env),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let items = v["items"].as_array().unwrap();
    if torii_debug_match_enabled() {
        eprintln!("[torii-debug-response] {:?}", v);
    }
    // Only tx2 should pass (timestamp 2000)
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["timestamp_ms"].as_u64(), Some(2000));
}
#[tokio::test]
async fn handle_v1_account_transactions_emits_requested_format() {
    use iroha_core::{
        block::{BlockBuilder, ValidBlock},
        tx::AcceptedTransaction,
    };
    use iroha_data_model::prelude as dm;
    use std::borrow::Cow;
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(iroha_core::state::State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let network_id = *state.network_id_ref();
    let kp = checked_smoke_keypair(
        0x5A,
        iroha_crypto::Algorithm::Ed25519,
        "derive requested-format account fixture key",
    );
    let account = dm::AccountId::new(kp.public_key().clone());
    let mut builder = dm::TransactionBuilder::new(
        network_id,
        account.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(core::time::Duration::from_millis(1000));
    let signed = builder
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp.private_key());
    let tx = AcceptedTransaction::new_unchecked(Cow::Owned(signed));
    let leader = checked_smoke_keypair(
        0x5B,
        iroha_crypto::Algorithm::BlsNormal,
        "derive requested-format block leader fixture key",
    );
    let unverified = BlockBuilder::new(vec![tx])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut st_block = state.block(unverified.header());
    let valid: ValidBlock = unverified
        .validate_and_record_transactions(&mut st_block)
        .unpack(|_| {});
    let committed = valid.clone().commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block, committed);
    let i105_literal = account
        .to_account_address()
        .and_then(|address| address.to_i105())
        .expect("account i105 literal");
    let env = crate::filter::QueryEnvelope {
        query: None,
        filter: None,
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: Some(10),
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(i105_literal.clone()),
        crate::utils::extractors::NoritoJson(env),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    let raw = items[0]["authority"].as_str().unwrap();
    let normalized = decode_latin1_utf8(raw).unwrap_or_else(|| raw.to_string());
    assert_eq!(normalized, i105_literal);
}
#[tokio::test]
async fn authority_and_timestamp_bounds_filter_local_and_handler() {
    use iroha_data_model::prelude as dm;
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(iroha_core::state::State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let network_id = *state.network_id_ref();
    let kp_a = checked_smoke_keypair(
        0x5C,
        iroha_crypto::Algorithm::Ed25519,
        "derive authority-bounds account A fixture key",
    );
    let kp_b = checked_smoke_keypair(
        0x5D,
        iroha_crypto::Algorithm::Ed25519,
        "derive authority-bounds account B fixture key",
    );
    let acc_a = dm::AccountId::new(kp_a.public_key().clone());
    let acc_b = dm::AccountId::new(kp_b.public_key().clone());
    let acc_b_str = acc_b
        .to_account_address()
        .and_then(|address| address.to_i105())
        .expect("account i105 literal");
    let (_max_clock_drift, _tx_limits) = {
        let v = state.view();
        let p = v.world().parameters();
        (p.sumeragi().max_clock_drift(), p.transaction())
    };
    // tx for A at 1000, tx for B at 2000
    let mut b1 = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b1.set_creation_time(core::time::Duration::from_millis(1000));
    let signed_a = b1
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_a.private_key());
    let _entry_hash_a = format!("{}", signed_a.hash_as_entrypoint());
    let tx1 = AcceptedTransaction::new_unchecked(Cow::Owned(signed_a));
    let mut b2 = dm::TransactionBuilder::new(
        network_id,
        acc_b.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b2.set_creation_time(core::time::Duration::from_millis(2000));
    let signed2 = b2
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_b.private_key());
    let tx2 = AcceptedTransaction::new_unchecked(Cow::Owned(signed2.clone()));
    // Commit block
    let leader = checked_smoke_keypair(
        0x5E,
        iroha_crypto::Algorithm::BlsNormal,
        "derive authority-bounds block leader fixture key",
    );
    let _topo = Topology::new(vec![dm::PeerId::new(leader.public_key().clone())]);
    let unverified = BlockBuilder::new(vec![tx1, tx2])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut st_block = state.block(unverified.header());
    let valid: ValidBlock = unverified
        .validate_and_record_transactions(&mut st_block)
        .unpack(|_| {});
    let committed = valid.clone().commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block, committed);
    // Build filter: authority == acc_b AND 1500 <= timestamp_ms <= 2500
    let expr = crate::filter::FilterExpr::And(vec![
        crate::filter::FilterExpr::Eq(
            crate::filter::FieldPath("authority".into()),
            norito::json::Value::String(acc_b_str.clone()),
        ),
        crate::filter::FilterExpr::Gte(
            crate::filter::FieldPath("timestamp_ms".into()),
            norito::json::Value::from(1500u64),
        ),
        crate::filter::FilterExpr::Lte(
            crate::filter::FieldPath("timestamp_ms".into()),
            norito::json::Value::from(2500u64),
        ),
    ]);
    // Local path: synthesize a CommittedTransaction-like struct and check filter_tx
    // Reuse the unit-test helper approach minimally by reconstructing a tx projection check via tx_field_value
    // We directly assert the handler path instead (primary), since local path is covered by unit tests.
    // Handler path
    let env = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(expr),
        select: None,
        aggregate: None,
        sort: vec![crate::filter::SortKey {
            key: crate::filter::FieldPath("timestamp_ms".into()),
            order: crate::filter::Order::Asc,
        }],
        pagination: crate::filter::Pagination {
            limit: Some(10),
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(acc_b_str.clone()),
        crate::utils::extractors::NoritoJson(env),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["timestamp_ms"].as_u64(), Some(2000));
}
#[tokio::test]
async fn or_union_matches_both_authority_or_timestamp() {
    use iroha_data_model::prelude as dm;
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(iroha_core::state::State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    // Build two transactions: A at 1500ms, A at 900ms
    let network_id = *state.network_id_ref();
    let kp_a = checked_smoke_keypair(
        0x5F,
        iroha_crypto::Algorithm::Ed25519,
        "derive OR-union account fixture key",
    );
    let acc_a = dm::AccountId::new(kp_a.public_key().clone());
    let acc_a_str = acc_a
        .to_account_address()
        .and_then(|address| address.to_i105())
        .expect("account i105 literal");
    let (_max_clock_drift, _tx_limits) = {
        let v = state.view();
        let p = v.world().parameters();
        (p.sumeragi().max_clock_drift(), p.transaction())
    };
    let mut b1 = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b1.set_creation_time(core::time::Duration::from_millis(1500));
    let tx1 = b1
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_a.private_key());
    let tx1 = AcceptedTransaction::new_unchecked(Cow::Owned(tx1));
    let mut b2 = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b2.set_creation_time(core::time::Duration::from_millis(900));
    let tx2 = b2
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_a.private_key());
    let tx2 = AcceptedTransaction::new_unchecked(Cow::Owned(tx2));
    // Commit block
    let leader = checked_smoke_keypair(
        0x60,
        iroha_crypto::Algorithm::BlsNormal,
        "derive OR-union block leader fixture key",
    );
    let _topo = Topology::new(vec![dm::PeerId::new(leader.public_key().clone())]);
    let unverified = BlockBuilder::new(vec![tx1, tx2])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut st_block = state.block(unverified.header());
    let valid: ValidBlock = unverified
        .validate_and_record_transactions(&mut st_block)
        .unpack(|_| {});
    let committed = valid.clone().commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block, committed);
    // Filter: authority == acc_a_str OR timestamp_ms < 1000
    let expr = crate::filter::FilterExpr::Or(vec![
        crate::filter::FilterExpr::Eq(
            crate::filter::FieldPath("authority".into()),
            norito::json::Value::String(acc_a_str.clone()),
        ),
        crate::filter::FilterExpr::Lt(
            crate::filter::FieldPath("timestamp_ms".into()),
            norito::json::Value::from(1000u64),
        ),
    ]);
    let env = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(expr),
        select: None,
        aggregate: None,
        sort: vec![crate::filter::SortKey {
            key: crate::filter::FieldPath("timestamp_ms".into()),
            order: crate::filter::Order::Asc,
        }],
        pagination: crate::filter::Pagination {
            limit: Some(10),
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(acc_a_str.clone()),
        crate::utils::extractors::NoritoJson(env),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let items = v["items"].as_array().unwrap();
    // Both transactions should match the OR condition
    assert_eq!(items.len(), 2);
    // Verify timestamps present: 900 and 1500
    let mut stamps: Vec<u64> = items
        .iter()
        .filter_map(|it| it["timestamp_ms"].as_u64())
        .collect();
    stamps.sort_unstable();
    assert_eq!(stamps, vec![900, 1500]);
}
// The production app path always uses the typed server-side predicate and
// then applies the authoritative endpoint filter to returned candidates.
#[tokio::test]
async fn typed_tx_predicate_matches_all_filter() {
    use iroha_data_model::prelude as dm;
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(iroha_core::state::State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let network_id = *state.network_id_ref();
    let kp_a = checked_smoke_keypair(
        0x61,
        iroha_crypto::Algorithm::Ed25519,
        "derive tx-predicate all-filter account fixture key",
    );
    let _dom: dm::DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let acc_a = dm::AccountId::new(kp_a.public_key().clone());
    let (_max_clock_drift, _tx_limits) = {
        let v = state.view();
        let p = v.world().parameters();
        (p.sumeragi().max_clock_drift(), p.transaction())
    };
    let mut b1 = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b1.set_creation_time(core::time::Duration::from_millis(1000));
    let signed_a = b1
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_a.private_key());
    let _entry_hash_a = format!("{}", signed_a.hash_as_entrypoint());
    let tx1 = AcceptedTransaction::new_unchecked(Cow::Owned(signed_a));
    let mut b2 = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b2.set_creation_time(core::time::Duration::from_millis(2000));
    let tx2 = b2
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_a.private_key());
    let tx2 = AcceptedTransaction::new_unchecked(Cow::Owned(tx2));
    let leader = checked_smoke_keypair(
        0x62,
        iroha_crypto::Algorithm::BlsNormal,
        "derive tx-predicate all-filter block leader fixture key",
    );
    let _topo = Topology::new(vec![dm::PeerId::new(leader.public_key().clone())]);
    let unverified = BlockBuilder::new(vec![tx1, tx2])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut st_block = state.block(unverified.header());
    let valid: ValidBlock = unverified
        .validate_and_record_transactions(&mut st_block)
        .unpack(|_| {});
    let committed = valid.clone().commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block, committed);
    // Filter that should match all: Exists(authority) OR Lt(timestamp_ms, very large)
    let expr = crate::filter::FilterExpr::Or(vec![
        crate::filter::FilterExpr::Exists(crate::filter::FieldPath("authority".into())),
        crate::filter::FilterExpr::Lt(
            crate::filter::FieldPath("timestamp_ms".into()),
            norito::json::Value::from(9_223_372_036_854_775_807u64),
        ),
    ]);
    let env = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(expr),
        select: None,
        aggregate: None,
        sort: vec![crate::filter::SortKey {
            key: crate::filter::FieldPath("timestamp_ms".into()),
            order: crate::filter::Order::Asc,
        }],
        pagination: crate::filter::Pagination {
            limit: Some(10),
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(
            acc_a
                .account()
                .to_account_address()
                .and_then(|address| address.to_i105())
                .expect("account i105 literal"),
        ),
        crate::utils::extractors::NoritoJson(env),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let items = v["items"].as_array().unwrap();
    // Both transactions should be present under predicate-based filtering as well
    assert_eq!(items.len(), 2);
}
#[tokio::test]
async fn typed_tx_predicate_handles_deep_boolean_and_large_sets() {
    use iroha_data_model::prelude as dm;
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(iroha_core::state::State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let network_id = *state.network_id_ref();
    let kp_a = checked_smoke_keypair(
        0x63,
        iroha_crypto::Algorithm::Ed25519,
        "derive tx-predicate deep account A fixture key",
    );
    let kp_b = checked_smoke_keypair(
        0x64,
        iroha_crypto::Algorithm::Ed25519,
        "derive tx-predicate deep account B fixture key",
    );
    let kp_c = checked_smoke_keypair(
        0x65,
        iroha_crypto::Algorithm::Ed25519,
        "derive tx-predicate deep account C fixture key",
    );
    let _dom: dm::DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let acc_a = dm::AccountId::new(kp_a.public_key().clone());
    let acc_b = dm::AccountId::new(kp_b.public_key().clone());
    let acc_c = dm::AccountId::new(kp_c.public_key().clone());
    let acc_a_str = acc_a.account().to_string();
    let acc_b_str = acc_b.account().to_string();
    let (_max_clock_drift, _tx_limits) = {
        let v = state.view();
        let p = v.world().parameters();
        (p.sumeragi().max_clock_drift(), p.transaction())
    };
    // Build four transactions across three authorities and different timestamps
    let mut b1 = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b1.set_creation_time(core::time::Duration::from_millis(1000));
    let tx1 = b1
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_a.private_key());
    let tx1 = AcceptedTransaction::new_unchecked(Cow::Owned(tx1));
    let mut b2 = dm::TransactionBuilder::new(
        network_id,
        acc_b.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b2.set_creation_time(core::time::Duration::from_millis(2000));
    let tx2 = b2
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_b.private_key());
    let tx2 = AcceptedTransaction::new_unchecked(Cow::Owned(tx2));
    let mut b3 = dm::TransactionBuilder::new(
        network_id,
        acc_c.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b3.set_creation_time(core::time::Duration::from_millis(3000));
    let signed3 = b3
        .with_instructions::<dm::InstructionBox>([dm::Unregister::domain(
            DomainId::try_new("bad", "universal").unwrap(),
        )
        .into()])
        .sign(kp_c.private_key());
    let tx3 = AcceptedTransaction::new_unchecked(Cow::Owned(signed3));
    let mut b4 = dm::TransactionBuilder::new(
        network_id,
        acc_b.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b4.set_creation_time(core::time::Duration::from_millis(2500));
    let signed4 = b4
        .with_instructions::<dm::InstructionBox>([dm::Unregister::domain(
            DomainId::try_new("nope", "universal").unwrap(),
        )
        .into()])
        .sign(kp_b.private_key());
    let tx4 = AcceptedTransaction::new_unchecked(Cow::Owned(signed4));
    // Commit block
    let leader = checked_smoke_keypair(
        0x66,
        iroha_crypto::Algorithm::BlsNormal,
        "derive tx-predicate deep block leader fixture key",
    );
    let _topo = Topology::new(vec![dm::PeerId::new(leader.public_key().clone())]);
    let unverified = BlockBuilder::new(vec![tx1, tx2, tx3, tx4])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut st_block = state.block(unverified.header());
    let valid: ValidBlock = unverified
        .validate_and_record_transactions(&mut st_block)
        .unpack(|_| {});
    let committed = valid.clone().commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block, committed);
    // Large, duplicate-free IN set below the deterministic membership cap.
    let mut big_set = vec![
        norito::json::Value::String(acc_a_str.clone()),
        norito::json::Value::String(acc_b_str.clone()),
    ];
    for seed in 0_u8..=u8::MAX {
        if seed == 0x63 || seed == 0x64 {
            continue;
        }
        let keypair = checked_smoke_keypair(
            seed,
            iroha_crypto::Algorithm::Ed25519,
            "derive unique tx-predicate membership fixture key",
        );
        let account = dm::AccountId::new(keypair.public_key().clone());
        big_set.push(norito::json::Value::String(account.account().to_string()));
        if big_set.len() == 250 {
            break;
        }
    }
    assert_eq!(big_set.len(), 250);
    // Deep boolean: NOT(NOT(IN(authority, big_set))) AND (timestamp_ms >= 1500 OR result_ok == false)
    let expr = crate::filter::FilterExpr::And(vec![
        crate::filter::FilterExpr::Not(Box::new(crate::filter::FilterExpr::Not(Box::new(
            crate::filter::FilterExpr::In(crate::filter::FieldPath("authority".into()), big_set),
        )))),
        crate::filter::FilterExpr::Or(vec![
            crate::filter::FilterExpr::Gte(
                crate::filter::FieldPath("timestamp_ms".into()),
                json_value(&1500u64),
            ),
            crate::filter::FilterExpr::Eq(
                crate::filter::FieldPath("result_ok".into()),
                json_value(&false),
            ),
        ]),
    ]);
    let env = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(expr),
        select: None,
        aggregate: None,
        sort: vec![crate::filter::SortKey {
            key: crate::filter::FieldPath("timestamp_ms".into()),
            order: crate::filter::Order::Asc,
        }],
        pagination: crate::filter::Pagination {
            limit: Some(10),
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(acc_b_str.clone()),
        crate::utils::extractors::NoritoJson(env),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    let items = v["items"].as_array().unwrap();
    // Expect B@2000 and B@2500 and exclude A@1000 (below 1500)
    assert_eq!(items.len(), 2);
    let ts: Vec<u64> = items
        .iter()
        .map(|i| i["timestamp_ms"].as_u64().unwrap())
        .collect();
    assert_eq!(ts, vec![2000, 2500]);
}
// Typed server predicates mirror the authoritative endpoint semantics for
// authority and entrypoint-hash equality and membership operators.
#[tokio::test]
async fn typed_tx_predicate_handles_authority_equality_sets() {
    use iroha_data_model::prelude as dm;
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(iroha_core::state::State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let network_id = *state.network_id_ref();
    let kp_a = checked_smoke_keypair(
        0x67,
        iroha_crypto::Algorithm::Ed25519,
        "derive tx-predicate authority-set account A fixture key",
    );
    let kp_b = checked_smoke_keypair(
        0x68,
        iroha_crypto::Algorithm::Ed25519,
        "derive tx-predicate authority-set account B fixture key",
    );
    let kp_c = checked_smoke_keypair(
        0x69,
        iroha_crypto::Algorithm::Ed25519,
        "derive tx-predicate authority-set account C fixture key",
    );
    let _dom: dm::DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let acc_a = dm::AccountId::new(kp_a.public_key().clone());
    let acc_b = dm::AccountId::new(kp_b.public_key().clone());
    let acc_c = dm::AccountId::new(kp_c.public_key().clone());
    let acc_a_str = acc_a.account().to_string();
    let acc_b_str = acc_b.account().to_string();
    let acc_c_str = acc_c.account().to_string();
    let (_max_clock_drift, _tx_limits) = {
        let v = state.view();
        let p = v.world().parameters();
        (p.sumeragi().max_clock_drift(), p.transaction())
    };
    // Three external transactions with the same authority
    let mut b1 = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b1.set_creation_time(core::time::Duration::from_millis(1));
    let tx1 = b1
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_a.private_key());
    let tx1 = AcceptedTransaction::new_unchecked(Cow::Owned(tx1));
    let mut b2 = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b2.set_creation_time(core::time::Duration::from_millis(2));
    let tx2 = b2
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_a.private_key());
    let tx2 = AcceptedTransaction::new_unchecked(Cow::Owned(tx2));
    let mut b3 = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b3.set_creation_time(core::time::Duration::from_millis(3));
    let tx3 = b3
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_a.private_key());
    let tx3 = AcceptedTransaction::new_unchecked(Cow::Owned(tx3));
    // Commit block with all three
    let leader = checked_smoke_keypair(
        0x6A,
        iroha_crypto::Algorithm::BlsNormal,
        "derive tx-predicate authority-set block leader fixture key",
    );
    let _topo = Topology::new(vec![dm::PeerId::new(leader.public_key().clone())]);
    let unverified = iroha_core::block::BlockBuilder::new(vec![tx1, tx2, tx3])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut st_block = state.block(unverified.header());
    let valid: iroha_core::block::ValidBlock = unverified
        .validate_and_record_transactions(&mut st_block)
        .unpack(|_| {});
    let committed = valid.clone().commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block, committed);
    // 1) Eq(authority == A) => all
    let env_eq = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::Eq(
            crate::filter::FieldPath("authority".into()),
            norito::json::Value::String(acc_a_str.clone()),
        )),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: None,
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp_eq = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(acc_a_str.clone()),
        crate::utils::extractors::NoritoJson(env_eq),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp_eq.status(), StatusCode::OK);
    let v_eq: norito::json::Value =
        norito::json::from_slice(&resp_eq.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(v_eq["items"].as_array().unwrap().len(), 3);
    // 2) Ne(authority != B) => all
    let env_ne = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::Ne(
            crate::filter::FieldPath("authority".into()),
            norito::json::Value::String(acc_b_str.clone()),
        )),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: None,
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp_ne = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(acc_a_str.clone()),
        crate::utils::extractors::NoritoJson(env_ne),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp_ne.status(), StatusCode::OK);
    let v_ne: norito::json::Value =
        norito::json::from_slice(&resp_ne.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(v_ne["items"].as_array().unwrap().len(), 3);
    // 3) In(authority IN {A,C}) => all
    let env_in = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::In(
            crate::filter::FieldPath("authority".into()),
            vec![
                norito::json::Value::String(acc_a_str.clone()),
                norito::json::Value::String(acc_c_str),
            ],
        )),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: None,
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp_in = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(acc_a_str.clone()),
        crate::utils::extractors::NoritoJson(env_in),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp_in.status(), StatusCode::OK);
    let v_in: norito::json::Value =
        norito::json::from_slice(&resp_in.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(v_in["items"].as_array().unwrap().len(), 3);
    // 4) Nin(authority NIN {A}) => 0
    let env_nin = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::Nin(
            crate::filter::FieldPath("authority".into()),
            vec![norito::json::Value::String(acc_a_str.clone())],
        )),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: None,
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp_nin = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(acc_a_str.clone()),
        crate::utils::extractors::NoritoJson(env_nin),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp_nin.status(), StatusCode::OK);
    let v_nin: norito::json::Value =
        norito::json::from_slice(&resp_nin.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    assert_eq!(v_nin["items"].as_array().unwrap().len(), 0);
}
#[tokio::test]
async fn typed_tx_predicate_handles_entrypoint_hash_sets() {
    use iroha_data_model::prelude as dm;
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(iroha_core::state::State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let network_id = *state.network_id_ref();
    let kp_a = checked_smoke_keypair(
        0x6B,
        iroha_crypto::Algorithm::Ed25519,
        "derive tx-predicate entrypoint-hash account fixture key",
    );
    let _dom: dm::DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let acc_a = dm::AccountId::new(kp_a.public_key().clone());
    let account_literal = acc_a.account().to_string();
    let (_max_clock_drift, _tx_limits) = {
        let v = state.view();
        let p = v.world().parameters();
        (p.sumeragi().max_clock_drift(), p.transaction())
    };
    // Two transactions with distinct entrypoint hashes
    let mut b1 = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b1.set_creation_time(core::time::Duration::from_millis(10));
    let signed1 = b1
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_a.private_key());
    let entry1 = format!("{}", signed1.hash_as_entrypoint());
    let tx1 = AcceptedTransaction::new_unchecked(Cow::Owned(signed1));
    let mut b2 = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b2.set_creation_time(core::time::Duration::from_millis(20));
    let signed2 = b2
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_a.private_key());
    let entry2 = format!("{}", signed2.hash_as_entrypoint());
    let tx2 = AcceptedTransaction::new_unchecked(Cow::Owned(signed2));
    // Commit
    let leader = checked_smoke_keypair(
        0x6C,
        iroha_crypto::Algorithm::BlsNormal,
        "derive tx-predicate entrypoint-hash block leader fixture key",
    );
    let _topo = Topology::new(vec![dm::PeerId::new(leader.public_key().clone())]);
    let unverified = BlockBuilder::new(vec![tx1, tx2])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut st_block = state.block(unverified.header());
    let valid: ValidBlock = unverified
        .validate_and_record_transactions(&mut st_block)
        .unpack(|_| {});
    let committed = valid.clone().commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block, committed);
    // Eq(entrypoint_hash == entry1) => 1
    let env_eq = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::Eq(
            crate::filter::FieldPath("entrypoint_hash".into()),
            norito::json::Value::String(entry1.clone()),
        )),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: None,
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp_eq = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(account_literal.clone()),
        crate::utils::extractors::NoritoJson(env_eq),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    let v_eq: norito::json::Value =
        norito::json::from_slice(&resp_eq.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(v_eq["items"].as_array().unwrap().len(), 1);
    // Ne(entrypoint_hash != entry1) => 1 (the other)
    let env_ne = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::Ne(
            crate::filter::FieldPath("entrypoint_hash".into()),
            norito::json::Value::String(entry1.clone()),
        )),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: None,
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp_ne = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(account_literal.clone()),
        crate::utils::extractors::NoritoJson(env_ne),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    let v_ne: norito::json::Value =
        norito::json::from_slice(&resp_ne.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(v_ne["items"].as_array().unwrap().len(), 1);
    // In(entrypoint_hash IN {entry1}) => 1
    let env_in_one = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::In(
            crate::filter::FieldPath("entrypoint_hash".into()),
            vec![norito::json::Value::String(entry1.clone())],
        )),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: None,
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp_in_one = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(account_literal.clone()),
        crate::utils::extractors::NoritoJson(env_in_one),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    let v_in_one: norito::json::Value =
        norito::json::from_slice(&resp_in_one.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    assert_eq!(v_in_one["items"].as_array().unwrap().len(), 1);
    // In(entrypoint_hash IN {entry1, entry2}) => 2
    let env_in_two = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::In(
            crate::filter::FieldPath("entrypoint_hash".into()),
            vec![
                norito::json::Value::String(entry1.clone()),
                norito::json::Value::String(entry2),
            ],
        )),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: None,
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp_in_two = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(account_literal.clone()),
        crate::utils::extractors::NoritoJson(env_in_two),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    let v_in_two: norito::json::Value =
        norito::json::from_slice(&resp_in_two.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    assert_eq!(v_in_two["items"].as_array().unwrap().len(), 2);
    // Nin(entrypoint_hash NIN {entry1}) => 1
    let env_nin = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::Nin(
            crate::filter::FieldPath("entrypoint_hash".into()),
            vec![norito::json::Value::String(entry1.clone())],
        )),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: None,
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp_nin = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(account_literal),
        crate::utils::extractors::NoritoJson(env_nin),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    let v_nin: norito::json::Value =
        norito::json::from_slice(&resp_nin.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    assert_eq!(v_nin["items"].as_array().unwrap().len(), 1);
}
#[tokio::test]
async fn typed_tx_predicate_handles_exists_is_null_entrypoint_and_result() {
    use iroha_data_model::prelude as dm;
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(iroha_core::state::State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let network_id = *state.network_id_ref();
    let kp_a = checked_smoke_keypair(
        0x6D,
        iroha_crypto::Algorithm::Ed25519,
        "derive tx-predicate nullability account fixture key",
    );
    let _dom: dm::DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let acc_a = dm::AccountId::new(kp_a.public_key().clone());
    let account_literal = acc_a.account().to_string();
    let (_max_clock_drift, _tx_limits) = {
        let v = state.view();
        let p = v.world().parameters();
        (p.sumeragi().max_clock_drift(), p.transaction())
    };
    // A: success, A: failure
    let mut b1 = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b1.set_creation_time(core::time::Duration::from_millis(100));
    let tx1 = b1
        .with_instructions::<dm::InstructionBox>([log_instruction()])
        .sign(kp_a.private_key());
    let tx1 = AcceptedTransaction::new_unchecked(Cow::Owned(tx1));
    let mut b2 = dm::TransactionBuilder::new(
        network_id,
        acc_a.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    b2.set_creation_time(core::time::Duration::from_millis(200));
    let signed_b = b2
        .with_instructions::<dm::InstructionBox>([dm::Unregister::domain(
            DomainId::try_new("nope", "universal").unwrap(),
        )
        .into()])
        .sign(kp_a.private_key());
    let tx2 = AcceptedTransaction::new_unchecked(Cow::Owned(signed_b));
    // Commit
    let leader = checked_smoke_keypair(
        0x6E,
        iroha_crypto::Algorithm::BlsNormal,
        "derive tx-predicate nullability block leader fixture key",
    );
    let unverified = BlockBuilder::new(vec![tx1, tx2])
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut st_block = state.block(unverified.header());
    let valid: ValidBlock = unverified
        .validate_and_record_transactions(&mut st_block)
        .unpack(|_| {});
    let committed = valid.clone().commit_unchecked().unpack(|_| {});
    crate::test_utils::finalize_committed_block(&state, st_block, committed);
    // Exists(entrypoint_hash) => 2 (always present)
    let env_exists_entry = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::Exists(crate::filter::FieldPath(
            "entrypoint_hash".into(),
        ))),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: None,
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp_exists_entry = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(account_literal.clone()),
        crate::utils::extractors::NoritoJson(env_exists_entry),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    let v_exists_entry: norito::json::Value = norito::json::from_slice(
        &resp_exists_entry
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(v_exists_entry["items"].as_array().unwrap().len(), 2);
    // IsNull(entrypoint_hash) => 0
    let env_null_entry = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::IsNull(crate::filter::FieldPath(
            "entrypoint_hash".into(),
        ))),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: None,
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp_null_entry = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(account_literal.clone()),
        crate::utils::extractors::NoritoJson(env_null_entry),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    let v_null_entry: norito::json::Value = norito::json::from_slice(
        &resp_null_entry
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(v_null_entry["items"].as_array().unwrap().len(), 0);
    // Exists(result_ok) => 2 (always present as boolean)
    let env_exists_result = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::Exists(crate::filter::FieldPath(
            "result_ok".into(),
        ))),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: None,
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp_exists_result = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(account_literal.clone()),
        crate::utils::extractors::NoritoJson(env_exists_result),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    let v_exists_result: norito::json::Value = norito::json::from_slice(
        &resp_exists_result
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(v_exists_result["items"].as_array().unwrap().len(), 2);
    // IsNull(result_ok) => 0
    let env_null_result = crate::filter::QueryEnvelope {
        query: None,
        filter: Some(crate::filter::FilterExpr::IsNull(crate::filter::FieldPath(
            "result_ok".into(),
        ))),
        select: None,
        aggregate: None,
        sort: Vec::new(),
        pagination: crate::filter::Pagination {
            limit: None,
            offset: 0,
        },
        fetch_size: None,
        count_mode: None,
    };
    let resp_null_result = handle_v1_account_transactions(
        state.clone(),
        axum::extract::Path(account_literal),
        crate::utils::extractors::NoritoJson(env_null_result),
        crate::routing::MaybeTelemetry::for_tests(),
    )
    .await
    .expect("handler ok")
    .into_response();
    let v_null_result: norito::json::Value = norito::json::from_slice(
        &resp_null_result
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(v_null_result["items"].as_array().unwrap().len(), 0);
}
