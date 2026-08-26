// URL-mutating SoraFS filters use a separate test adapter from request-builder filters.
macro_rules! sorafs_url_filter_query_test {
    ($name:ident, $path:literal, $filter:expr, $expected:expr $(,)?) => {
        #[test]
        fn $name() {
            let client = Client::new(config_factory());
            let mut url = join_torii_url(&client.torii_url, $path);
            $filter
                .apply_to_url(&mut url)
                .expect("filter must be canonical");
            let request = client
                .default_request(HttpMethod::GET, url)
                .build()
                .expect("build request");
            assert_eq!(request.uri().query(), Some($expected));
        }
    };
}
sorafs_url_filter_query_test!(
    sorafs_alias_filter_sets_query_params,
    "v1/sorafs/aliases",
    SorafsAliasListFilter {
        limit: Some(10),
        offset: Some(3),
        namespace: Some("docs"),
        manifest_digest: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    },
    "limit=10&offset=3&namespace=docs&manifest_digest=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
);
sorafs_url_filter_query_test!(
    sorafs_replication_filter_sets_query_params,
    "v1/sorafs/replication",
    SorafsReplicationListFilter {
        limit: Some(50),
        offset: Some(2),
        status: Some(SorafsReplicationStatus::Completed),
        manifest_digest: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
    },
    "limit=50&offset=2&status=completed&manifest_digest=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
);

#[test]
fn sorafs_replication_status_labels_are_closed_and_canonical() {
    assert_eq!(SorafsReplicationStatus::Pending.as_str(), "pending");
    assert_eq!(SorafsReplicationStatus::Completed.as_str(), "completed");
    assert_eq!(SorafsReplicationStatus::Cancelled.as_str(), "cancelled");
    assert_eq!(SorafsReplicationStatus::Expired.as_str(), "expired");
}
