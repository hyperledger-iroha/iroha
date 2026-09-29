//! Catalog guards for the first-release KAGEMUSHA API.
#![cfg(feature = "app_api")]
use iroha_torii_shared::route_catalog::{
    CatalogProjection, EnabledFeatures, HttpMethod, RouteCatalog, kagemusha,
};
#[test]
fn kagemusha_catalog_exposes_only_the_first_release_routes() {
    assert_eq!(kagemusha::READINESS_PATH, "/v1/kagemusha/readiness");
    assert_eq!(kagemusha::TOP_UP_PATH, "/v1/kagemusha/top-up");
    assert_eq!(kagemusha::REDEEM_PATH, "/v1/kagemusha/redeem");
    assert_eq!(
        kagemusha::OPERATION_PATH,
        "/v1/kagemusha/operations/{operation_id}"
    );
    let catalog = RouteCatalog::new(kagemusha::ROUTES);
    catalog
        .validate()
        .expect("KAGEMUSHA route catalog is valid");
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
            (HttpMethod::Get, kagemusha::READINESS_PATH),
            (HttpMethod::Post, kagemusha::TOP_UP_PATH),
            (HttpMethod::Post, kagemusha::REDEEM_PATH),
            (HttpMethod::Get, kagemusha::OPERATION_PATH),
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
        "the universal KAGEMUSHA interface must not require a separate feature flag"
    );
}
