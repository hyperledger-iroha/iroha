// Exact-network authentication contract for the tenant-scoped ZK attachment client.
#[test]
fn zk_attachment_calls_sign_the_exact_method_path_and_body_once() {
    use std::collections::HashSet;
    let client = client_with_base_url(base_url());
    let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&store);
    with_mock_http(
        move |snapshot| {
            let response = match (snapshot.method.as_str(), snapshot.url.path()) {
                ("POST", "/v1/zk/attachments") => json_response(StatusCode::CREATED, "{}"),
                ("GET", "/v1/zk/attachments") => json_response(StatusCode::OK, "[]"),
                ("GET", "/v1/zk/attachments/attachment-1") => HttpResponse::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "application/x-norito")
                    .body(vec![0x01, 0x02])
                    .expect("attachment response"),
                ("DELETE", "/v1/zk/attachments/attachment-1") => {
                    empty_response(StatusCode::NO_CONTENT)
                }
                (method, path) => panic!("unexpected attachment request {method:?} {path}"),
            };
            captured
                .lock()
                .expect("attachment snapshot store")
                .push(snapshot);
            Ok(response)
        },
        || {
            client
                .post_zk_attachment(br#"{"proof":1}"#, APPLICATION_JSON)
                .expect("signed attachment upload");
            client
                .get_zk_attachments_list()
                .expect("signed attachment list");
            client
                .get_zk_attachment_raw("attachment-1")
                .expect("signed attachment read");
            client
                .delete_zk_attachment("attachment-1")
                .expect("signed attachment delete");
        },
    );
    let snapshots = store.lock().expect("attachment snapshots");
    assert_eq!(snapshots.len(), 4);
    for snapshot in snapshots.iter() {
        assert_canonical_account_signed_request(&client, snapshot);
    }
    assert_eq!(snapshots[0].body, br#"{"proof":1}"#);
    assert!(
        snapshots[1..]
            .iter()
            .all(|snapshot| snapshot.body.is_empty())
    );
    let nonces: HashSet<_> = snapshots
        .iter()
        .map(|snapshot| {
            snapshot
                .headers
                .iter()
                .find(|(name, _)| name == HEADER_NONCE)
                .expect("attachment nonce")
                .1
                .clone()
        })
        .collect();
    assert_eq!(nonces.len(), snapshots.len(), "each call is one-shot");
}
#[test]
fn zk_compute_calls_sign_the_exact_network_method_path_and_body_once() {
    use std::collections::HashSet;
    let client = client_with_base_url(base_url());
    let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&store);
    with_mock_http(
        move |snapshot| {
            let response = match snapshot.url.path() {
                "/v1/zk/verify-batch" => {
                    json_response(StatusCode::OK, r#"{"ok":true,"statuses":[]}"#)
                }
                "/v1/zk/ivm/derive" => {
                    json_response(StatusCode::OK, r#"{"proved":{"placeholder":true}}"#)
                }
                path => panic!("unexpected ZK compute request {path}"),
            };
            captured
                .lock()
                .expect("ZK compute snapshot store")
                .push(snapshot);
            Ok(response)
        },
        || {
            client
                .post_zk_verify_batch_norito(&[0x01, 0x02])
                .expect("signed Norito verify batch");
            client
                .post_zk_verify_batch_json(&norito::json!([]))
                .expect("signed JSON verify batch");
            client
                .post_zk_ivm_derive_json(&norito::json!({
                    "vk_ref": { "backend": "halo2/ipa", "name": "vk_main" },
                    "authority": { "placeholder": true },
                    "fee_payment": { "placeholder": true },
                    "metadata": {},
                    "bytecode": { "placeholder": true },
                }))
                .expect("signed IVM derive");
        },
    );
    let snapshots = store.lock().expect("ZK compute snapshots");
    assert_eq!(snapshots.len(), 3);
    for snapshot in snapshots.iter() {
        assert_eq!(snapshot.method, HttpMethod::POST);
        assert_canonical_account_signed_request(&client, snapshot);
    }
    assert_eq!(snapshots[0].body, [0x01, 0x02]);
    let content_types: Vec<_> = snapshots
        .iter()
        .map(|snapshot| {
            snapshot
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
                .expect("ZK compute content type")
                .1
                .as_str()
        })
        .collect();
    assert_eq!(
        content_types,
        ["application/x-norito", APPLICATION_JSON, APPLICATION_JSON]
    );
    let nonces: HashSet<_> = snapshots
        .iter()
        .map(|snapshot| {
            snapshot
                .headers
                .iter()
                .find(|(name, _)| name == HEADER_NONCE)
                .expect("ZK compute nonce")
                .1
                .clone()
        })
        .collect();
    assert_eq!(nonces.len(), snapshots.len(), "each call is one-shot");
}
