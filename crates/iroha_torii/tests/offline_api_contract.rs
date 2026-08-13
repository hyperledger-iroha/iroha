//! Catalog guards for the first-release offline API.
#![cfg(feature = "app_api")]
use iroha_torii_shared::{
    route_catalog::{CatalogProjection, EnabledFeatures, HttpMethod, RouteCatalog, offline},
    uri,
};
#[test]
fn offline_catalog_exposes_only_the_first_release_routes() {
    assert_eq!(uri::OFFLINE_READINESS, "/v1/offline/readiness");
    assert_eq!(
        uri::OFFLINE_RECIPIENT_LINEAGE,
        "/v1/offline/receiver-lineage"
    );
    assert_eq!(uri::OFFLINE_TOP_UP, "/v1/offline/top-up");
    assert_eq!(uri::OFFLINE_REDEEM, "/v1/offline/redeem");
    assert_eq!(
        uri::OFFLINE_OPERATION,
        "/v1/offline/operations/{operation_id}"
    );
    let catalog = RouteCatalog::new(offline::ROUTES);
    catalog.validate().expect("offline route catalog is valid");
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
            (HttpMethod::Get, uri::OFFLINE_READINESS),
            (HttpMethod::Post, uri::OFFLINE_RECIPIENT_LINEAGE),
            (HttpMethod::Post, uri::OFFLINE_TOP_UP),
            (HttpMethod::Post, uri::OFFLINE_REDEEM),
            (HttpMethod::Get, uri::OFFLINE_OPERATION),
        ]
    );
}
#[test]
fn offline_catalog_projections_are_explicit() {
    let catalog = RouteCatalog::new(offline::ROUTES);
    let enabled = ["app_api"];
    let features = EnabledFeatures::new(&enabled);
    assert_eq!(
        catalog.project(CatalogProjection::OpenApi, features).len(),
        5
    );
    assert_eq!(
        catalog
            .project(CatalogProjection::Sdk, EnabledFeatures::none())
            .len(),
        5
    );
    assert_eq!(
        catalog.project(CatalogProjection::Mcp, features).len(),
        5,
        "the universal offline interface must not require a separate feature flag"
    );
}
