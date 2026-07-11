//! Catalog guards for the first-release offline API.
#![cfg(feature = "app_api")]

use iroha_torii_shared::{
    route_catalog::{CatalogProjection, EnabledFeatures, HttpMethod, RouteCatalog, offline},
    uri,
};

#[test]
fn offline_catalog_exposes_only_the_first_release_routes() {
    assert_eq!(uri::OFFLINE_READINESS, "/v1/offline/readiness");
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
    assert_eq!(mounted.len(), 4);
    assert_eq!(mounted[0].method(), HttpMethod::Get);
    assert_eq!(mounted[1].method(), HttpMethod::Post);
    assert_eq!(mounted[2].method(), HttpMethod::Post);
    assert_eq!(mounted[3].method(), HttpMethod::Get);

    for route in mounted {
        assert!(route.path().starts_with("/v1/offline/"));
        assert!(
            !route.path().contains("/v2/"),
            "the first release must not expose nested route versions: {}",
            route.path()
        );
    }
}

#[test]
fn offline_catalog_projections_are_explicit() {
    let catalog = RouteCatalog::new(offline::ROUTES);
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
    assert!(
        catalog.project(CatalogProjection::Mcp, features).is_empty(),
        "offline value-moving commands are not implicitly MCP tools"
    );
}
