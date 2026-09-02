//! Catalog guards for the first-release Kagemusha API.
#![cfg(feature = "app_api")]
use iroha_torii_shared::{
    route_catalog::{CatalogProjection, EnabledFeatures, HttpMethod, RouteCatalog, kagemusha},
    uri,
};
#[test]
fn kagemusha_catalog_exposes_only_the_first_release_routes() {
    assert_eq!(uri::KAGEMUSHA_READINESS, "/v1/kagemusha/readiness");
    assert_eq!(uri::KAGEMUSHA_TOP_UP, "/v1/kagemusha/top-up");
    assert_eq!(uri::KAGEMUSHA_REDEEM, "/v1/kagemusha/redeem");
    assert_eq!(
        uri::KAGEMUSHA_OPERATION,
        "/v1/kagemusha/operations/{operation_id}"
    );
    let catalog = RouteCatalog::new(kagemusha::ROUTES);
    catalog.validate().expect("Kagemusha route catalog is valid");
    let mounted = catalog.project(
        CatalogProjection::Mounted,
        EnabledFeatures::new(&["app_api"]),
    );
    let actual = mounted
        .iter()
        .map(|route| (route.method(), route.path()))
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            (HttpMethod::Get, uri::KAGEMUSHA_READINESS),
            (HttpMethod::Post, uri::KAGEMUSHA_TOP_UP),
            (HttpMethod::Post, uri::KAGEMUSHA_REDEEM),
            (HttpMethod::Get, uri::KAGEMUSHA_OPERATION),
        ]
    );
}
#[test]
fn kagemusha_catalog_projections_are_explicit() {
    let catalog = RouteCatalog::new(kagemusha::ROUTES);
    let enabled = ["app_api"];
    let features = EnabledFeatures::new(&enabled);
    assert_eq!(
        catalog.project(CatalogProjection::OpenApi, features).len(),
        4
    );
    assert_eq!(
        catalog
            .project(CatalogProjection::Sdk, EnabledFeatures::none())
            .len(),
        4
    );
    assert_eq!(
        catalog.project(CatalogProjection::Mcp, features).len(),
        4,
        "the universal Kagemusha interface must not require a separate feature flag"
    );
}
