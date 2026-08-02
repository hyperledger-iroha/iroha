//! Rust-owner conformance guard for the shared Musubi SDK V1 JSON fixture.

use std::collections::BTreeSet;

use iroha_data_model::musubi::{
    MusubiAliasHistoryPageV1, MusubiAliasPricingPolicyV1, MusubiAliasQueryV1, MusubiAliasRecordV1,
    MusubiArchiveLocationPageV1, MusubiArchiveLocationQueryV1, MusubiArchiveRetentionPageV1,
    MusubiArchiveRetentionQueryV1, MusubiExactPackageQueryV1, MusubiExactReleaseQueryV1,
    MusubiExactReleaseSnapshotV1, MusubiMaintainerPageV1, MusubiNamespaceV1,
    MusubiOrderedPackagePageV1, MusubiOrderedPrefixQueryV1, MusubiPackageIdV1, MusubiPackageNameV1,
    MusubiPackagePageQueryV1, MusubiPackageRecordV1, MusubiPackageSelectorV1,
    MusubiProviderBundleAttestationKeyV1, MusubiProviderBundleAttestationRecordV1,
    MusubiResolverIndexPageV1, MusubiResolverIndexQueryV1, MusubiSearchPageV1, MusubiSearchQueryV1,
    MusubiVersionPageV1, MusubiVersionReqV1, MusubiVersionV1,
};
use norito::json::{self, JsonDeserialize, JsonSerialize, Value};

const FIXTURE: &str = include_str!("../../../fixtures/musubi/sdk_v1.json");

fn object(value: &Value) -> &norito::json::Map {
    value.as_object().expect("fixture value is an object")
}

fn keys(value: &Value) -> BTreeSet<&str> {
    object(value).keys().map(String::as_str).collect()
}

fn canonical_roundtrip<T>(value: &Value) -> T
where
    T: JsonDeserialize + JsonSerialize,
{
    let decoded = json::from_value(value.clone()).expect("decode typed Musubi fixture value");
    let encoded = json::to_value(&decoded).expect("encode typed Musubi fixture value");
    assert_eq!(
        encoded,
        value.clone(),
        "fixture value is not canonical Norito JSON"
    );
    decoded
}

#[test]
fn shared_musubi_sdk_fixture_is_owned_and_canonical() {
    let root: Value = json::from_str(FIXTURE).expect("parse Musubi SDK fixture with Norito JSON");
    assert_eq!(
        keys(&root),
        BTreeSet::from([
            "canonical",
            "fixture_version",
            "format",
            "reject",
            "routes",
            "rust_owner",
        ])
    );
    assert_eq!(
        root.get("format").and_then(Value::as_str),
        Some("iroha-musubi-sdk-v1")
    );
    assert_eq!(root.get("fixture_version").and_then(Value::as_u64), Some(1));
    assert_eq!(
        root.get("rust_owner").and_then(Value::as_str),
        Some("iroha_data_model::musubi")
    );

    let canonical = root.get("canonical").expect("canonical vectors");
    assert_eq!(
        keys(canonical),
        BTreeSet::from([
            "namespace",
            "package",
            "package_name",
            "requirement_aliases",
            "requirement_matches",
            "requirements",
            "selector",
            "version",
        ])
    );
    let namespace: MusubiNamespaceV1 =
        canonical_roundtrip(canonical.get("namespace").expect("namespace vector"));
    assert_eq!(namespace.as_str(), "sora");
    let package_name: MusubiPackageNameV1 =
        canonical_roundtrip(canonical.get("package_name").expect("package-name vector"));
    assert_eq!(package_name.as_str(), "math-utils");
    let selector: MusubiPackageSelectorV1 =
        canonical_roundtrip(canonical.get("selector").expect("selector vector"));
    assert_eq!(selector.namespace, namespace);
    let package: MusubiPackageIdV1 =
        canonical_roundtrip(canonical.get("package").expect("package vector"));
    assert_eq!(package.name.as_str(), "math-utils");
    let version: MusubiVersionV1 =
        canonical_roundtrip(canonical.get("version").expect("version vector"));
    assert_eq!(version.to_string(), "1.2.3-rc.1");

    let requirements = canonical
        .get("requirements")
        .and_then(Value::as_array)
        .expect("requirement vectors");
    assert_eq!(requirements.len(), 5);
    for requirement in requirements {
        assert_eq!(keys(requirement), BTreeSet::from(["text", "wire"]));
        let text = requirement
            .get("text")
            .and_then(Value::as_str)
            .expect("requirement text");
        let wire: MusubiVersionReqV1 =
            canonical_roundtrip(requirement.get("wire").expect("requirement wire"));
        assert_eq!(
            text.parse::<MusubiVersionReqV1>().expect("parse text"),
            wire
        );
        assert_eq!(wire.to_string(), text);
    }

    let aliases = canonical
        .get("requirement_aliases")
        .and_then(Value::as_array)
        .expect("requirement alias vectors");
    assert_eq!(aliases.len(), 2);
    for alias in aliases {
        assert_eq!(keys(alias), BTreeSet::from(["canonical", "input", "wire"]));
        let input = alias
            .get("input")
            .and_then(Value::as_str)
            .expect("requirement alias input");
        let canonical_text = alias
            .get("canonical")
            .and_then(Value::as_str)
            .expect("canonical requirement text");
        let wire: MusubiVersionReqV1 =
            canonical_roundtrip(alias.get("wire").expect("requirement alias wire"));
        let parsed = input
            .parse::<MusubiVersionReqV1>()
            .expect("parse requirement alias");
        assert_eq!(parsed, wire);
        assert_eq!(parsed.to_string(), canonical_text);
    }

    let match_cases = canonical
        .get("requirement_matches")
        .and_then(Value::as_array)
        .expect("requirement match vectors");
    assert_eq!(match_cases.len(), 6);
    for match_case in match_cases {
        assert_eq!(
            keys(match_case),
            BTreeSet::from(["candidate", "matches", "requirement"])
        );
        let requirement = match_case
            .get("requirement")
            .and_then(Value::as_str)
            .expect("match requirement")
            .parse::<MusubiVersionReqV1>()
            .expect("parse match requirement");
        let candidate = match_case
            .get("candidate")
            .and_then(Value::as_str)
            .expect("match candidate")
            .parse::<MusubiVersionV1>()
            .expect("parse match candidate");
        assert_eq!(
            requirement.matches(&candidate),
            match_case
                .get("matches")
                .and_then(Value::as_bool)
                .expect("match expectation"),
        );
    }

    let routes = root
        .get("routes")
        .and_then(Value::as_array)
        .expect("route fixtures");
    let route_ids = routes
        .iter()
        .map(|route| {
            assert_eq!(
                keys(route),
                BTreeSet::from(["id", "path", "request", "response"])
            );
            route.get("id").and_then(Value::as_str).expect("route id")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        route_ids,
        BTreeSet::from([
            "alias",
            "alias-history",
            "archive-locations",
            "archive-retention",
            "exact-package",
            "exact-release",
            "maintainers",
            "ordered-prefix",
            "provider-bundle-attestation",
            "resolver-index",
            "search",
            "versions",
        ])
    );

    for route in routes {
        let id = route.get("id").and_then(Value::as_str).expect("route id");
        let path = route
            .get("path")
            .and_then(Value::as_str)
            .expect("route path");
        assert_eq!(path, format!("/v1/musubi/queries/{id}"));
        let request = route.get("request").expect("route request");
        let response = route.get("response").expect("route response");
        match id {
            "exact-package" => {
                let _: MusubiExactPackageQueryV1 = canonical_roundtrip(request);
                let record: MusubiPackageRecordV1 = canonical_roundtrip(response);
                record.validate().expect("valid package fixture");
            }
            "exact-release" => {
                let request: MusubiExactReleaseQueryV1 = canonical_roundtrip(request);
                let snapshot: MusubiExactReleaseSnapshotV1 = canonical_roundtrip(response);
                snapshot
                    .validate_for(&request)
                    .expect("valid exact-release snapshot fixture");
            }
            "provider-bundle-attestation" => {
                let key: MusubiProviderBundleAttestationKeyV1 = canonical_roundtrip(request);
                let record: MusubiProviderBundleAttestationRecordV1 = canonical_roundtrip(response);
                record
                    .validate()
                    .expect("valid provider bundle attestation fixture");
                assert_eq!(record.key, key);
                record
                    .attestation
                    .verify(&record.attestation.payload.binding)
                    .expect("provider bundle attestation fixture is genuinely signed");
            }
            "resolver-index" => {
                let request: MusubiResolverIndexQueryV1 = canonical_roundtrip(request);
                let page: MusubiResolverIndexPageV1 = canonical_roundtrip(response);
                page.validate_for(&request)
                    .expect("valid resolver page fixture");
                assert_eq!(page.chain_id.as_str(), "musubi-fixture-chain");
                assert_eq!(page.genesis_hash, [8; 32]);
            }
            "versions" => {
                let request: MusubiPackagePageQueryV1 = canonical_roundtrip(request);
                let page: MusubiVersionPageV1 = canonical_roundtrip(response);
                page.validate_for(&request)
                    .expect("valid version page fixture");
            }
            "maintainers" => {
                let request: MusubiPackagePageQueryV1 = canonical_roundtrip(request);
                let page: MusubiMaintainerPageV1 = canonical_roundtrip(response);
                page.validate_for(&request)
                    .expect("valid maintainer page fixture");
            }
            "archive-locations" => {
                let request: MusubiArchiveLocationQueryV1 = canonical_roundtrip(request);
                let page: MusubiArchiveLocationPageV1 = canonical_roundtrip(response);
                page.validate()
                    .expect("valid archive-location page fixture");
                assert_eq!(page.archive.archive_id, request.archive_id);
                assert_eq!(page.chain_id.as_str(), "musubi-fixture-chain");
                assert_eq!(page.genesis_hash, [8; 32]);
            }
            "archive-retention" => {
                let request: MusubiArchiveRetentionQueryV1 = canonical_roundtrip(request);
                request
                    .validate()
                    .expect("valid archive-retention request fixture");
                let page: MusubiArchiveRetentionPageV1 = canonical_roundtrip(response);
                page.validate()
                    .expect("valid archive-retention page fixture");
                assert_eq!(page.chain_id.as_str(), "musubi-fixture-chain");
                assert_eq!(page.genesis_hash, [8; 32]);
                assert_eq!(page.finalized_time_ms, 1_700_000_000_000);
                assert_eq!(page.items.len(), request.archive_ids.len());
                assert!(
                    page.items
                        .iter()
                        .map(|decision| decision.archive_id)
                        .eq(request.archive_ids.iter().copied())
                );
                assert_eq!(request.expected_snapshot, Some(page.snapshot));
                assert_eq!(
                    page.items
                        .iter()
                        .map(|decision| decision.must_retain())
                        .collect::<Vec<_>>(),
                    vec![true, true, false, false]
                );
            }
            "alias" => {
                let _: MusubiAliasQueryV1 = canonical_roundtrip(request);
                let record: MusubiAliasRecordV1 = canonical_roundtrip(response);
                record
                    .validate(&MusubiAliasPricingPolicyV1::GENESIS)
                    .expect("valid alias fixture");
            }
            "alias-history" => {
                let request: MusubiAliasQueryV1 = canonical_roundtrip(request);
                let page: MusubiAliasHistoryPageV1 = canonical_roundtrip(response);
                page.validate_for(&request)
                    .expect("valid alias-history page fixture");
            }
            "ordered-prefix" => {
                let request: MusubiOrderedPrefixQueryV1 = canonical_roundtrip(request);
                let page: MusubiOrderedPackagePageV1 = canonical_roundtrip(response);
                page.validate_for(&request)
                    .expect("valid ordered-prefix page fixture");
                assert_eq!(page.chain_id.as_str(), "musubi-fixture-chain");
                assert_eq!(page.genesis_hash, [8; 32]);
            }
            "search" => {
                let request: MusubiSearchQueryV1 = canonical_roundtrip(request);
                request.validate().expect("valid search request fixture");
                let page: MusubiSearchPageV1 = canonical_roundtrip(response);
                page.validate_for(&request)
                    .expect("valid search page fixture");
            }
            _ => panic!("unexpected Musubi route fixture `{id}`"),
        }
    }

    let reject = root.get("reject").expect("negative fixture vectors");
    assert_eq!(
        keys(reject),
        BTreeSet::from(["fixture_versions", "names", "requirements", "versions"])
    );
    assert_eq!(
        reject
            .get("fixture_versions")
            .and_then(Value::as_array)
            .expect("rejected fixture versions")
            .iter()
            .map(|value| value.as_u64().expect("fixture version"))
            .collect::<Vec<_>>(),
        vec![0, 2]
    );
    for version in reject
        .get("versions")
        .and_then(Value::as_array)
        .expect("rejected versions")
    {
        assert!(
            version
                .as_str()
                .expect("version text")
                .parse::<MusubiVersionV1>()
                .is_err()
        );
    }
    for name in reject
        .get("names")
        .and_then(Value::as_array)
        .expect("rejected names")
    {
        assert!(
            name.as_str()
                .expect("name text")
                .parse::<MusubiPackageNameV1>()
                .is_err()
        );
    }
    for requirement in reject
        .get("requirements")
        .and_then(Value::as_array)
        .expect("rejected requirements")
    {
        assert!(
            requirement
                .as_str()
                .expect("requirement text")
                .parse::<MusubiVersionReqV1>()
                .is_err()
        );
    }
}

#[test]
fn shared_musubi_search_fixture_is_canonical() {
    let root: Value = json::from_str(FIXTURE).expect("parse Musubi SDK fixture");
    let route = root
        .get("routes")
        .and_then(Value::as_array)
        .expect("route fixtures")
        .iter()
        .find(|route| route.get("id").and_then(Value::as_str) == Some("search"))
        .expect("search fixture route");
    let request: MusubiSearchQueryV1 = canonical_roundtrip(
        route
            .get("request")
            .expect("search fixture request is present"),
    );
    request.validate().expect("valid search request fixture");
    let page: MusubiSearchPageV1 = canonical_roundtrip(
        route
            .get("response")
            .expect("search fixture response is present"),
    );
    page.validate_for(&request)
        .expect("valid search page fixture");
}
