// Reputation route-catalog assertions kept separate to respect the source-file budget.
#[test]
fn reputation_surface_is_committed_projection_read_only() {
    let routes = [
        sorafs::REPUTATION_LATEST_GET,
        sorafs::REPUTATION_SNAPSHOT,
        sorafs::REPUTATION_PROVIDER,
        sorafs::REPUTATION_WEIGHTS,
        sorafs::REPUTATION_EVENTS,
        sorafs::REPUTATION_EVENTS_STREAM,
        sorafs::REPUTATION_EVENTS_WEBSOCKET,
    ];
    assert_eq!(
        routes.map(RouteDescriptor::stable_route_id),
        [
            "sorafs.reputation_snapshot.latest",
            "sorafs.reputation_snapshot.read",
            "sorafs.reputation_provider.read",
            "sorafs.reputation_weight.read",
            "sorafs.reputation_event.list",
            "protocol.sorafs.reputation_event_stream",
            "protocol.sorafs.reputation_event_websocket",
        ]
    );
    assert_eq!(RouteCatalog::new(&routes).validate(), Ok(()));
    for route in routes {
        assert_eq!(route.method(), HttpMethod::Get);
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature
        );
        assert!(
            route.implicit_head(),
            "Axum GET routing provides authenticated framework HEAD handling"
        );
        assert_eq!(
            CATALOGED_ROUTES
                .iter()
                .filter(|candidate| candidate.stable_route_id() == route.stable_route_id())
                .count(),
            1,
            "reputation route `{}` must appear exactly once",
            route.stable_route_id()
        );
    }
    assert_eq!(
        sorafs::REPUTATION_LATEST_GET.stable_route_id(),
        "sorafs.reputation_snapshot.latest"
    );
    assert!(
        !CATALOGED_ROUTES
            .iter()
            .any(|route| route.stable_route_id() == "sorafs.reputation_snapshot.publish")
    );
}
