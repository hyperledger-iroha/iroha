#[test]
fn canonical_catalog_includes_exact_gateway_and_directory_routes() {
    let catalog = RouteCatalog::new(CATALOGED_ROUTES);
    assert_eq!(catalog.validate(), Ok(()));

    for expected in soracloud_gateway::ROUTES
        .iter()
        .chain(content_directory::ROUTES)
    {
        assert!(
            catalog.routes().iter().any(|route| route == expected),
            "missing canonical route {}",
            expected.stable_route_id()
        );
    }
    assert!(
        catalog
            .routes()
            .iter()
            .all(|route| route.path() != "/soradns/{fqdn}/"),
        "the first-release gateway must not expose a trailing-slash alias"
    );
}

#[test]
fn public_runtime_gateway_authentication_is_exactly_scoped() {
    let catalog_routes = CATALOGED_ROUTES
        .iter()
        .filter(|route| route.stable_route_id().starts_with("protocol.soracloud."))
        .collect::<Vec<_>>();
    assert_eq!(catalog_routes.len(), soracloud_gateway::ROUTES.len());
    assert_eq!(soracloud_gateway::ROUTES.len(), 4);

    for route in soracloud_gateway::ROUTES {
        assert!(catalog_routes.iter().any(|catalog| **catalog == *route));
        assert_eq!(route.surface(), ApiSurface::Protocol);
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::Unauthenticated
        );
        assert_eq!(route.projections(), RouteProjections::NONE);
    }
}

#[test]
fn dedicated_onboarding_authentication_is_exactly_scoped() {
    for route in [
        application_api::ACCOUNTS_ONBOARD_PLAN_POST,
        application_api::ACCOUNTS_ONBOARD_POST,
        application_api::ACCOUNTS_ONBOARDING_READINESS_GET,
    ] {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::OnboardingToken,
            "{} must advertise its dedicated credential",
            route.stable_route_id()
        );
    }
    assert_eq!(
        CATALOGED_ROUTES
            .iter()
            .filter(|route| { route.authentication() == AuthenticationPolicy::OnboardingToken })
            .count(),
        3,
        "no unrelated route may inherit the onboarding credential policy"
    );
}

#[test]
fn required_api_token_authentication_is_exactly_scoped() {
    let required_routes = iso20022::ROUTES
        .iter()
        .copied()
        .chain([
            sorafs::STORAGE_TOKEN,
            application_api::WEBHOOKS_GET,
            application_api::WEBHOOKS_POST,
            application_api::WEBHOOKS_BY_ID_DELETE,
        ])
        .collect::<Vec<_>>();
    for route in &required_routes {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::RequiredApiToken,
            "{} must advertise its unconditional API credential",
            route.stable_route_id()
        );
    }
    assert_eq!(
        CATALOGED_ROUTES
            .iter()
            .filter(|route| { route.authentication() == AuthenticationPolicy::RequiredApiToken })
            .count(),
        required_routes.len(),
        "no unrelated route may inherit the unconditional API-token policy"
    );
}

#[test]
fn vpn_and_push_device_routes_declare_canonical_account_authentication() {
    for route in [
        core::VPN_QUOTE_CREATE,
        core::VPN_SESSION_CREATE,
        core::VPN_RECEIPTS,
        core::VPN_RECEIPT_SUBMIT,
        core::VPN_SESSION,
        core::VPN_SESSION_DELETE,
        application_api::NOTIFY_DEVICES_POST,
        application_api::NOTIFY_DEVICES_DELETE,
    ] {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature,
            "{} must advertise its body-bound account signature",
            route.stable_route_id()
        );
    }
}

#[test]
fn trusted_internal_account_reads_are_not_projected_to_public_tooling() {
    let routes = [
        application_api::INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_GET,
        application_api::INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_TRANSACTIONS_BY_ENTRYPOINT_HASH_GET,
        application_api::INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_ASSETS_BY_ASSET_DEFINITION_ID_GET,
    ];
    let catalog = RouteCatalog::new(&routes);
    assert_eq!(catalog.validate(), Ok(()));
    for route in routes {
        assert_eq!(route.projections(), RouteProjections::NONE);
        assert!(!route.cors_options());
    }
    let enabled = EnabledFeatures::new(&["app_api"]);
    assert!(
        catalog
            .project(CatalogProjection::OpenApi, enabled)
            .is_empty()
    );
    assert!(catalog.project(CatalogProjection::Sdk, enabled).is_empty());
    assert!(catalog.project(CatalogProjection::Mcp, enabled).is_empty());
}

#[test]
fn account_alias_visibility_and_signed_operator_routes_declare_exact_authentication() {
    for route in [
        aliases::RESOLVE,
        aliases::RESOLVE_INDEX,
        aliases::BY_ACCOUNT,
    ] {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::ToriiDefault,
            "{} conditionally authenticates restricted dataspace reads in its handler",
            route.stable_route_id()
        );
    }

    for route in [
        aliases::SETUP_PLAN,
        aliases::LEASE_RENEW_PLAN,
        aliases::AUTO_RENEW_PLAN,
        aliases::RETAIL_RECIPIENT_LOOKUP,
        aliases::RETAIL_RECIPIENT_ROUTE,
        fees::QUOTE,
        fees::SPONSOR_PROGRAM_BY_ID,
    ] {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature,
            "{} must require canonical account authentication",
            route.stable_route_id()
        );
    }

    assert_eq!(
        aliases::ASSET_RESOLVE.authentication(),
        AuthenticationPolicy::ToriiDefault,
        "public asset aliases do not expose an account binding"
    );

    for route in [
        contracts_and_verification_keys::CONTRACTS_ALIASES_RESOLVE_POST,
        contracts_and_verification_keys::CONTRACTS_DEPLOYMENT_STATE_POST,
        runtime_governance::GOV_CONTRACT_GET,
    ] {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature,
            "{} exposes contract identity and must require a canonical account signature",
            route.stable_route_id()
        );
    }
}

#[test]
fn moderation_dead_letter_routes_are_account_signed_operator_role_posts() {
    let routes = [
        (
            contracts_and_verification_keys::SORAFS_MODERATION_DEAD_LETTERS_PREPARE_POST,
            "contracts.sorafs_moderation_dead_letters_prepare_post",
            "/v1/sorafs/moderation/dead-letters/prepare",
        ),
        (
            contracts_and_verification_keys::SORAFS_MODERATION_DEAD_LETTERS_APPLY_POST,
            "contracts.sorafs_moderation_dead_letters_apply_post",
            "/v1/sorafs/moderation/dead-letters/apply",
        ),
    ];

    for (route, stable_route_id, path) in routes {
        assert_eq!(route.stable_route_id(), stable_route_id);
        assert_eq!(route.method(), HttpMethod::Post);
        assert_eq!(route.path(), path);
        assert_eq!(route.surface(), ApiSurface::Public);
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature
        );
        assert_eq!(route.projections(), RouteProjections::OPENAPI_AND_SDK);
        assert!(route.cors_options());
        assert!(
            route
                .feature_gate()
                .is_enabled(EnabledFeatures::new(&["app_api"]))
        );
        assert!(CATALOGED_ROUTES.contains(&route));
    }

    assert_eq!(validate_catalog(&routes.map(|(route, _, _)| route)), Ok(()));
}
