// SoraFS inventory request-signing contract tests.
#[test]
fn sorafs_inventory_methods_sign_the_exact_filtered_get() {
    let client = client_with_base_url(base_url());
    let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let response = json_response(StatusCode::OK, "{}");
    let alias_digest = "aa".repeat(32);
    let replication_digest = "bb".repeat(32);
    with_mock_http(respond_with(&store, response), || {
        client
            .get_sorafs_aliases(&SorafsAliasListFilter {
                limit: Some(10),
                offset: Some(3),
                namespace: Some("docs"),
                manifest_digest: Some(alias_digest.as_str()),
            })
            .expect("signed alias projection request");
        client
            .get_sorafs_replication_orders(&SorafsReplicationListFilter {
                limit: Some(50),
                offset: Some(2),
                status: Some(SorafsReplicationStatus::Completed),
                manifest_digest: Some(replication_digest.as_str()),
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
    let alias_query = format!("limit=10&offset=3&namespace=docs&manifest_digest={alias_digest}");
    assert_eq!(snapshots[0].url.query(), Some(alias_query.as_str()));
    let replication_query =
        format!("limit=50&offset=2&status=completed&manifest_digest={replication_digest}");
    assert_eq!(
        snapshots[1].url.query(),
        Some(replication_query.as_str())
    );
}

#[test]
fn sorafs_manifest_digest_inputs_fail_before_http_io() {
    let client = client_with_base_url(base_url());
    let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let response = json_response(StatusCode::OK, "{}");
    with_mock_http(respond_with(&store, response), || {
        let alias_error = client
            .get_sorafs_aliases(&SorafsAliasListFilter {
                manifest_digest: Some("deadbeef"),
                ..SorafsAliasListFilter::default()
            })
            .expect_err("short alias manifest digest must fail");
        assert!(alias_error.to_string().contains("64 lowercase"));

        let uppercase_digest = "AA".repeat(32);
        let replication_error = client
            .get_sorafs_replication_orders(&SorafsReplicationListFilter {
                manifest_digest: Some(uppercase_digest.as_str()),
                ..SorafsReplicationListFilter::default()
            })
            .expect_err("uppercase replication manifest digest must fail");
        assert!(replication_error.to_string().contains("64 lowercase"));

        let pin_error = client
            .get_sorafs_pin_manifest(&"00".repeat(32))
            .expect_err("zero pin manifest digest must fail");
        assert!(pin_error.to_string().contains("non-zero"));

        let pin_list_error = client
            .get_sorafs_pin_registry(&SorafsPinListFilter {
                after_digest_hex: Some("abc123"),
                ..SorafsPinListFilter::default()
            })
            .expect_err("short pin-list cursor must fail");
        assert!(pin_list_error.to_string().contains("64 lowercase"));
    });
    assert!(
        store.lock().expect("snapshot store").is_empty(),
        "invalid digests must fail before HTTP I/O"
    );
}
