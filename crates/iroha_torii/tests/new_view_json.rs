#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Basic shape test for `NEW_VIEW` JSON endpoint

#[tokio::test]
async fn new_view_json_shape() {
    use axum::{Router, routing::get};
    use tower::ServiceExt;

    // Router with JSON endpoint
    let app = Router::new().route(
        "/v2/sumeragi/new_view/json",
        get(|| async move { iroha_torii::handle_v1_new_view_json().await }),
    );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v2/sumeragi/new_view/json")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let s = String::from_utf8(body.to_vec()).unwrap();
    let v: norito::json::Value = norito::json::from_str(&s).unwrap();
    assert!(v.get("ts_ms").is_some());
    let items = v.get("items").and_then(|x| x.as_array()).unwrap();
    // items is an array of objects with (height,view,count); possibly empty
    for item in items {
        let obj = item.as_object().unwrap();
        assert!(obj.get("height").is_some());
        assert!(obj.get("view").is_some());
        assert!(obj.get("count").is_some());
    }
}
