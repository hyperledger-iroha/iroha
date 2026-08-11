// SoraFS inventory request-signing contract tests.

#[test]
fn sorafs_legacy_inventory_methods_sign_the_exact_filtered_get() {
    let client = client_with_base_url(base_url());
    let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let response = json_response(StatusCode::OK, "{}");

    with_mock_http(respond_with(&store, response), || {
        client
            .get_sorafs_aliases(&SorafsAliasListFilter {
                limit: Some(10),
                offset: Some(3),
                namespace: Some("docs"),
                manifest_digest: Some("deadbeef"),
            })
            .expect("signed alias projection request");
        client
            .get_sorafs_replication_orders(&SorafsReplicationListFilter {
                limit: Some(50),
                offset: Some(2),
                status: Some("completed"),
                manifest_digest: Some("abc123"),
            })
            .expect("signed replication projection request");
    });

    let snapshots = store.lock().expect("snapshot store");
    assert_eq!(snapshots.len(), 2);
    for snapshot in snapshots.iter() {
        assert_canonical_account_signed_request(&client, snapshot);
        assert_eq!(snapshot.method, HttpMethod::GET);
        assert!(snapshot.body.is_empty());
    }
    assert_eq!(
        snapshots[0].url.query(),
        Some("limit=10&offset=3&namespace=docs&manifest_digest=deadbeef")
    );
    assert_eq!(
        snapshots[1].url.query(),
        Some("limit=50&offset=2&status=completed&manifest_digest=abc123")
    );
}
