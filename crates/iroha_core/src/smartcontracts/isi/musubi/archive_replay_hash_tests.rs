#[test]
fn query_hash_is_domain_separated() {
    assert_ne!(
        query_hash(b"versions", b"same"),
        query_hash(b"maintainers", b"same")
    );
    assert_ne!(
        query_hash(b"versions", b"same"),
        query_hash(b"versions", b"different")
    );
}
#[test]
fn borrowed_query_hashes_match_the_legacy_clone_encoding() {
    let page = MusubiPageRequestV1 {
        limit: 17,
        cursor: None,
    };
    let resolver = MusubiResolverIndexQueryV1 {
        package: package("borrowed-hash"),
        requirement: Some("~1.2.3".parse().expect("version requirement")),
        page: page.clone(),
    };
    let mut legacy_resolver = resolver.clone();
    legacy_resolver.page.cursor = None;
    assert_eq!(
        resolver_query_hash(&resolver).expect("borrowed resolver hash"),
        query_hash(b"resolver-index", &legacy_resolver.encode())
    );
    let package_page = MusubiPackagePageQueryV1 {
        package: package("borrowed-page-hash"),
        page: page.clone(),
    };
    let mut legacy_package_page = package_page.clone();
    legacy_package_page.page.cursor = None;
    assert_eq!(
        package_page_query_hash(b"versions", &package_page).expect("borrowed package-page hash"),
        query_hash(b"versions", &legacy_package_page.encode())
    );
    let archive = MusubiArchiveLocationQueryV1 {
        archive_id: ArchiveId::new([0xA5; 32]),
        page: page.clone(),
    };
    let mut legacy_archive = archive.clone();
    legacy_archive.page.cursor = None;
    assert_eq!(
        archive_location_query_hash(&archive).expect("borrowed archive hash"),
        query_hash(b"archive-locations", &legacy_archive.encode())
    );
    let alias = MusubiAliasQueryV1 {
        alias: "borrowed-hash".parse().expect("alias"),
        page: page.clone(),
    };
    let mut legacy_alias = alias.clone();
    legacy_alias.page.cursor = None;
    assert_eq!(
        alias_history_query_hash(&alias).expect("borrowed alias hash"),
        query_hash(b"alias-history", &legacy_alias.encode())
    );
    let prefix = MusubiOrderedPrefixQueryV1 {
        prefix: MusubiOrderedPrefixV1::new("sora/borrowed-").expect("ordered prefix"),
        page,
    };
    let mut legacy_prefix = prefix.clone();
    legacy_prefix.page.cursor = None;
    assert_eq!(
        ordered_prefix_query_hash(&prefix).expect("borrowed prefix hash"),
        query_hash(b"ordered-prefix", &legacy_prefix.encode())
    );
}
