#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii handler test for instances listing filters and pagination.
#![allow(clippy::too_many_lines)]

use axum::response::IntoResponse;
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_torii::NoritoQuery;

#[tokio::test]
async fn instances_list_filters_and_pagination() {
    let mut state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    // Seed several instances under the same namespace
    let _header = iroha_data_model::block::BlockHeader::new(
        nonzero_ext::nonzero!(1_u64),
        None,
        None,
        None,
        0,
        0,
    );
    let h1 = iroha_crypto::Hash::prehashed([0xAB; 32]);
    let h2 = iroha_crypto::Hash::prehashed([0xAC; 32]);
    let h3 = iroha_crypto::Hash::prehashed([0xBB; 32]);
    iroha_core::query::insert_contract_instance_for_test(&mut state, "apps", "calc.v1", h1);
    iroha_core::query::insert_contract_instance_for_test(&mut state, "apps", "calc.v2", h2);
    iroha_core::query::insert_contract_instance_for_test(&mut state, "apps", "dex.v1", h3);

    let state = std::sync::Arc::new(state);

    // Query: contains=calc, order=cid_asc, paginate limit=1
    let q = iroha_torii::InstancesQuery {
        contains: Some("calc".to_string()),
        hash_prefix: None,
        offset: Some(0),
        limit: Some(1),
        order: Some("cid_asc".to_string()),
    };
    let resp = iroha_torii::handle_gov_instances_by_ns(
        state.clone(),
        axum::extract::Path("apps".to_string()),
        NoritoQuery(q),
    )
    .await
    .expect("ok")
    .into_response();
    let body = resp
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).expect("json parse");
    assert_eq!(
        v.get("total").and_then(norito::json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        v.get("limit").and_then(norito::json::Value::as_u64),
        Some(1)
    );
    let arr = v
        .get("instances")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(
        arr[0].get("contract_id").and_then(|x| x.as_str()),
        Some("calc.v1")
    );

    // Next page: offset=1
    let q2 = iroha_torii::InstancesQuery {
        contains: Some("calc".to_string()),
        hash_prefix: None,
        offset: Some(1),
        limit: Some(1),
        order: Some("cid_asc".to_string()),
    };
    let resp2 = iroha_torii::handle_gov_instances_by_ns(
        state.clone(),
        axum::extract::Path("apps".to_string()),
        NoritoQuery(q2),
    )
    .await
    .expect("ok")
    .into_response();
    let body2 = resp2
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    let v2: norito::json::Value = norito::json::from_slice(&body2).expect("json parse");
    let arr2 = v2
        .get("instances")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap();
    assert_eq!(arr2.len(), 1);
    assert_eq!(
        arr2[0].get("contract_id").and_then(|x| x.as_str()),
        Some("calc.v2")
    );

    // hash_prefix filter
    let pref = hex::encode(<[u8; 32]>::from(h3))[..4].to_string();
    let q3 = iroha_torii::InstancesQuery {
        contains: None,
        hash_prefix: Some(pref.to_ascii_lowercase()),
        offset: None,
        limit: None,
        order: Some("hash_desc".to_string()),
    };
    let resp3 = iroha_torii::handle_gov_instances_by_ns(
        state,
        axum::extract::Path("apps".to_string()),
        NoritoQuery(q3),
    )
    .await
    .expect("ok")
    .into_response();
    let body3 = resp3
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    let v3: norito::json::Value = norito::json::from_slice(&body3).expect("json parse");
    let arr3 = v3
        .get("instances")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap();
    assert_eq!(arr3.len(), 1);
    assert_eq!(
        arr3[0].get("contract_id").and_then(|x| x.as_str()),
        Some("dex.v1")
    );
}
