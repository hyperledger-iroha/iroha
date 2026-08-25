// Route-catalog validation and hard-cut surface regressions.
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mcp_json_rpc_is_a_sealed_nested_route_gateway() {
        let route = mcp_transport::JSON_RPC;
        assert_eq!(route.effect(), RouteEffect::Mutation);
        assert_eq!(route.admission(), AdmissionPolicy::TargetRoute);
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::NestedRouteAuthentication
        );
        assert_eq!(RouteCatalog::new(mcp_transport::ROUTES).validate(), Ok(()));
    }
    #[test]
    fn nested_route_authentication_rejects_mismatched_admission_or_surface() {
        let missing_auth = RouteDescriptor::new(
            "test.target_route_without_nested_auth",
            HttpMethod::Post,
            "/v1/tests/target-route-without-nested-auth",
            ApiSurface::Protocol,
            Listener::Torii,
            RouteEffect::Mutation,
            AdmissionPolicy::TargetRoute,
        );
        let wrong_surface = RouteDescriptor::new(
            "test.nested_auth_on_public_surface",
            HttpMethod::Post,
            "/v1/tests/nested-auth-on-public-surface",
            ApiSurface::Public,
            Listener::Torii,
            RouteEffect::Mutation,
            AdmissionPolicy::TargetRoute,
        )
        .with_authentication(AuthenticationPolicy::NestedRouteAuthentication);
        let errors = validate_catalog(&[missing_auth, wrong_surface]).expect_err("invalid pairs");
        assert!(errors.iter().any(|error| {
            error.kind
                == CatalogValidationErrorKind::TargetRouteAdmissionRequiresNestedAuthentication
        }));
        assert!(errors.iter().any(|error| {
            error.kind
                == CatalogValidationErrorKind::NestedAuthenticationRequiresProtocolTargetRoute
        }));
    }
    const FEATURED_ROUTES: &[RouteDescriptor] = &[
        RouteDescriptor::new(
            "test.always",
            HttpMethod::Get,
            "/v1/tests/always",
            ApiSurface::Public,
            Listener::Torii,
            RouteEffect::ReadOnly,
            AdmissionPolicy::Public,
        )
        .with_projections(RouteProjections::ALL),
        RouteDescriptor::new(
            "test.featured",
            HttpMethod::Get,
            "/v1/tests/featured",
            ApiSurface::Public,
            Listener::Torii,
            RouteEffect::ReadOnly,
            AdmissionPolicy::Public,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_projections(RouteProjections::OPENAPI_AND_SDK),
        RouteDescriptor::new(
            "test.diagnostic",
            HttpMethod::Get,
            "/v1/tests/diagnostic",
            ApiSurface::Diagnostic,
            Listener::Torii,
            RouteEffect::ReadOnly,
            AdmissionPolicy::Public,
        ),
    ];
    #[test]
    fn final_offline_catalog_is_valid_and_unique() {
        let catalog = RouteCatalog::new(offline::ROUTES);
        assert_eq!(catalog.validate(), Ok(()));
        let ids: BTreeSet<_> = catalog
            .routes()
            .iter()
            .map(|route| route.stable_route_id())
            .collect();
        let method_paths: BTreeSet<_> = catalog
            .routes()
            .iter()
            .map(|route| (route.method(), route.path()))
            .collect();
        assert_eq!(ids.len(), offline::ROUTES.len());
        assert_eq!(method_paths.len(), offline::ROUTES.len());
    }
    #[test]
    fn canonical_catalog_satisfies_closed_security_axes() {
        assert_eq!(RouteCatalog::new(CATALOGED_ROUTES).validate(), Ok(()));
    }
    #[test]
    fn offline_routes_are_universal_for_app_api_and_project_to_mcp() {
        let catalog = RouteCatalog::new(offline::ROUTES);
        assert_eq!(
            catalog
                .project(
                    CatalogProjection::Mounted,
                    EnabledFeatures::new(&["app_api"]),
                )
                .len(),
            offline::ROUTES.len(),
            "every app-api node must expose the complete offline route family"
        );
        assert_eq!(
            catalog
                .project(CatalogProjection::Mcp, EnabledFeatures::new(&["app_api"]))
                .len(),
            offline::ROUTES.len(),
            "the offline route family must be available to MCP clients"
        );
    }
    #[test]
    fn ordinary_kagemusha_lifecycle_has_one_dedicated_canonical_signed_route() {
        let route = offline::KAGEMUSHA_LIFECYCLE_TRANSACTION;
        assert_eq!(
            route.path(),
            "/v1/offline/kagemusha/lifecycle-v4/transactions"
        );
        assert_eq!(crate::uri::KAGEMUSHA_LIFECYCLE_TRANSACTION, route.path());
        assert_eq!(route.method(), HttpMethod::Post);
        assert_eq!(route.surface(), ApiSurface::Public);
        assert_eq!(route.effect(), RouteEffect::Mutation);
        assert_eq!(route.admission(), AdmissionPolicy::AuthenticatedAccount);
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalSignedBody
        );
        assert_eq!(route.feature_gate(), FeatureGate::Feature("app_api"));
        assert_eq!(route.route_match(), RouteMatch::Exact);
        assert_eq!(route.path_normalization(), PathNormalization::Strict);
        assert_ne!(route.path(), pipeline::TRANSACTION.path());
        assert_eq!(
            offline::ROUTES
                .iter()
                .filter(|candidate| candidate.path() == route.path())
                .count(),
            1
        );
    }
    #[test]
    fn canonical_catalog_retires_global_sumeragi_rbc_and_collectors() {
        assert!(
            CATALOGED_ROUTES
                .iter()
                .any(|route| route.path() == "/v1/sumeragi/status")
        );
        for retired in [
            "/v1/sumeragi/rbc",
            "/v1/sumeragi/rbc/delivered/{height}/{view}",
            "/v1/sumeragi/rbc/sessions",
            "/v1/sumeragi/rbc/sample",
            "/v1/sumeragi/collectors",
            "/v1/sumeragi/telemetry",
        ] {
            assert!(
                CATALOGED_ROUTES.iter().all(|route| route.path() != retired),
                "retired route {retired} leaked into the canonical catalog"
            );
        }
    }
    #[test]
    fn canonical_catalog_retires_direct_sumeragi_mutation_and_vrf_snapshot_routes() {
        assert_eq!(sumeragi::EVIDENCE_LIST.method(), HttpMethod::Get);
        assert_eq!(sumeragi::EVIDENCE_LIST.path(), "/v1/sumeragi/evidence");
        for (stable_route_id, path) in [
            ("operator.sumeragi.evidence.submit", "/v1/sumeragi/evidence"),
            ("operator.sumeragi.vrf.commit", "/v1/sumeragi/vrf/commit"),
            ("operator.sumeragi.vrf.reveal", "/v1/sumeragi/vrf/reveal"),
        ] {
            assert!(
                CATALOGED_ROUTES
                    .iter()
                    .all(|route| route.stable_route_id() != stable_route_id),
                "retired Sumeragi route id remains cataloged: {stable_route_id}"
            );
            assert!(
                CATALOGED_ROUTES
                    .iter()
                    .all(|route| route.method() != HttpMethod::Post || route.path() != path),
                "retired Sumeragi mutation route remains cataloged: POST {path}"
            );
        }
        for path in [
            "/v1/sumeragi/vrf/commit",
            "/v1/sumeragi/vrf/epoch/{epoch}",
            "/v1/sumeragi/vrf/penalties/{epoch}",
            "/v1/sumeragi/vrf/reveal",
        ] {
            assert!(
                CATALOGED_ROUTES.iter().all(|route| route.path() != path),
                "retired Sumeragi VRF path remains cataloged: {path}"
            );
        }
    }
    #[test]
    fn canonical_catalog_exposes_only_the_authoritative_privacy_routes() {
        let privacy_routes = CATALOGED_ROUTES
            .iter()
            .filter(|route| route.path().starts_with("/v1/privacy/"))
            .collect::<Vec<_>>();
        assert_eq!(
            privacy_routes,
            vec![
                &runtime_governance::PRIVACY_CAPABILITIES,
                &runtime_governance::PRIVACY_BOOTLE_LANTERN_ISSUANCE_AUTHORIZE,
                &runtime_governance::PRIVACY_BOOTLE_LANTERN_ISSUANCE_ISSUE,
            ]
        );
        assert_eq!(
            runtime_governance::PRIVACY_CAPABILITIES.path(),
            "/v1/privacy/capabilities"
        );
        assert_eq!(
            runtime_governance::PRIVACY_CAPABILITIES.method(),
            HttpMethod::Get
        );
        assert_eq!(
            runtime_governance::PRIVACY_CAPABILITIES.stable_route_id(),
            "privacy.capabilities"
        );
        for (route, stable_route_id, path) in [
            (
                runtime_governance::PRIVACY_BOOTLE_LANTERN_ISSUANCE_AUTHORIZE,
                "privacy.bootle_lantern.issuance.authorize",
                "/v1/privacy/bootle-lantern/issuance/authorize",
            ),
            (
                runtime_governance::PRIVACY_BOOTLE_LANTERN_ISSUANCE_ISSUE,
                "privacy.bootle_lantern.issuance.issue",
                "/v1/privacy/bootle-lantern/issuance/issue",
            ),
        ] {
            assert_eq!(route.stable_route_id(), stable_route_id);
            assert_eq!(route.path(), path);
            assert_eq!(route.method(), HttpMethod::Post);
            assert_eq!(route.surface(), ApiSurface::Public);
            assert_eq!(route.listener(), Listener::Torii);
            assert_eq!(route.effect(), RouteEffect::Mutation);
            assert_eq!(
                route.admission(),
                AdmissionPolicy::AuthenticatedProtocolPrincipal
            );
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::ProtocolHandshake
            );
            assert_eq!(route.feature_gate(), FeatureGate::Always);
            assert!(route.projections().openapi());
            assert!(route.projections().sdk());
            assert!(!route.projections().mcp());
            assert_eq!(route.route_match(), RouteMatch::Exact);
            assert_eq!(route.path_normalization(), PathNormalization::Strict);
            assert!(route.cors_options());
            assert!(!route.implicit_head());
        }
    }
    #[test]
    fn canonical_catalog_exposes_the_complete_regulated_account_recovery_family() {
        let expected = [
            (
                contracts_and_verification_keys::ACCOUNT_RECOVERY_POLICY_SET_POST,
                "/v1/accounts/recovery/policy/set",
            ),
            (
                contracts_and_verification_keys::ACCOUNT_RECOVERY_PROPOSE_POST,
                "/v1/accounts/recovery/propose",
            ),
            (
                contracts_and_verification_keys::ACCOUNT_RECOVERY_APPROVE_POST,
                "/v1/accounts/recovery/approve",
            ),
            (
                contracts_and_verification_keys::ACCOUNT_RECOVERY_FINALIZE_POST,
                "/v1/accounts/recovery/finalize",
            ),
            (
                contracts_and_verification_keys::ACCOUNT_RECOVERY_STATUS_POST,
                "/v1/accounts/recovery/status",
            ),
        ];
        for (route, path) in expected {
            assert_eq!(route.method(), HttpMethod::Post);
            assert_eq!(route.path(), path);
            assert_eq!(route.feature_gate(), FeatureGate::Feature("app_api"));
            assert!(CATALOGED_ROUTES.contains(&route));
        }
    }
    #[test]
    fn canonical_catalog_has_no_direct_storage_ingest_route() {
        assert!(
            CATALOGED_ROUTES
                .iter()
                .all(|route| route.path() != "/v1/sorafs/storage/pin"),
            "storage ingest must remain provider-internal and finalized-ledger driven"
        );
        assert!(
            CATALOGED_ROUTES
                .iter()
                .all(|route| route.stable_route_id() != "sorafs.storage.pin"),
            "the retired direct storage-ingest route id must not be reusable"
        );
    }
    #[test]
    fn internal_torii_proxy_is_the_only_identity_bound_operator_route() {
        assert_eq!(core::INTERNAL_PROXY.surface(), ApiSurface::Operator);
        assert_eq!(
            core::INTERNAL_PROXY.authentication(),
            AuthenticationPolicy::IdentityBoundSignature
        );
        assert_eq!(validate_catalog(&[core::INTERNAL_PROXY]), Ok(()));
        let generic_identity_bound_operator = RouteDescriptor::new(
            "test.identity_bound_operator",
            HttpMethod::Post,
            "/v1/tests/identity-bound-operator",
            ApiSurface::Operator,
            Listener::Torii,
            RouteEffect::ReadOnly,
            AdmissionPolicy::Operator,
        )
        .with_authentication(AuthenticationPolicy::IdentityBoundSignature);
        let errors = validate_catalog(&[generic_identity_bound_operator])
            .expect_err("generic identity-bound keys must not receive operator privileges");
        assert!(errors.iter().any(|error| {
            error.kind == CatalogValidationErrorKind::OperatorSurfaceRequiresAuthentication
        }));
    }
    #[test]
    fn sccp_governance_descriptor_uses_the_canonical_uri() {
        assert_eq!(
            runtime_governance::GOV_PROPOSE_SCCP.path(),
            crate::uri::GOV_PROPOSE_SCCP_ROUTE_GOVERNANCE
        );
    }
    #[test]
    fn parliament_cutover_excludes_legacy_governance_mutation_routes() {
        for retired_path in [
            "/v1/gov/parliament/ballots",
            "/v1/gov/finalize",
            "/v1/gov/enact",
        ] {
            assert!(
                runtime_governance::ROUTES
                    .iter()
                    .all(|route| route.path() != retired_path),
                "retired governance mutation route remains cataloged: {retired_path}"
            );
        }
        for active_path in [
            "/v1/gov/parliament/attempts/draft",
            "/v1/gov/parliament/attempts/{governance_attempt_id}",
            "/v1/gov/parliament/transitions/draft",
            "/v1/gov/ballots/zk-v1",
            "/v1/gov/ballots/zk-v1/ballot-proof",
            "/v1/gov/ballots/plain",
        ] {
            assert!(
                runtime_governance::ROUTES
                    .iter()
                    .any(|route| route.path() == active_path),
                "Parliament or explicitly standalone referendum route is missing: {active_path}"
            );
        }
    }
    #[test]
    fn protected_namespace_update_is_an_explicit_operator_mcp_route() {
        let route = runtime_governance::GOV_PROTECTED_POST;
        assert_eq!(route.surface(), ApiSurface::Operator);
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::OperatorSignature
        );
        assert!(route.projections().openapi());
        assert!(!route.projections().sdk());
        assert!(route.projections().mcp());
        let routes = [route];
        let projected = RouteCatalog::new(&routes)
            .project(CatalogProjection::Mcp, EnabledFeatures::new(&["app_api"]));
        assert_eq!(projected, vec![&routes[0]]);
    }
    #[test]
    fn bridge_finality_routes_are_not_telemetry_gated() {
        for route in [
            sumeragi::BRIDGE_FINALITY,
            sumeragi::BRIDGE_FINALITY_ATTESTATION,
            sumeragi::BRIDGE_FINALITY_BUNDLE,
        ] {
            assert_eq!(route.feature_gate(), FeatureGate::Always);
        }
    }
    #[test]
    fn canonical_websocket_streams_are_openapi_projected() {
        let enabled = EnabledFeatures::new(&["app_api"]);
        let projected =
            RouteCatalog::new(streaming::APP_ROUTES).project(CatalogProjection::OpenApi, enabled);
        for route in [streaming::SUBSCRIPTION_WS, streaming::BLOCKS_WS] {
            assert!(route.projections().openapi());
            assert!(!route.projections().sdk());
            assert!(!route.projections().mcp());
            assert!(projected.iter().any(|projected| **projected == route));
        }
    }
    #[test]
    fn documented_system_and_telemetry_routes_are_openapi_projected() {
        let enabled = EnabledFeatures::new(&["app_api", "telemetry", "profiling"]);
        let projected =
            RouteCatalog::new(CATALOGED_ROUTES).project(CatalogProjection::OpenApi, enabled);
        for route in [
            diagnostic::STATUS,
            diagnostic::STATUS_TAIL,
            diagnostic::METRICS,
            diagnostic::PROFILE,
            diagnostic::OPENAPI_JSON,
            diagnostic::OPENAPI,
            streaming::P2P,
            application_api::EXPLORER_METRICS_GET,
            application_api::TELEMETRY_PEERS_INFO_GET,
            application_api::TELEMETRY_LIVE_GET,
        ] {
            assert!(route.projections().openapi());
            assert!(!route.projections().sdk());
            assert!(!route.projections().mcp());
            assert!(projected.iter().any(|projected| **projected == route));
        }
        assert!(
            !application_api::TELEMETRY_PROPAGATION_GET
                .projections()
                .openapi()
        );
        assert!(
            !projected
                .iter()
                .any(|route| **route == application_api::TELEMETRY_PROPAGATION_GET)
        );
    }
    #[test]
    fn first_release_catalog_excludes_unsupported_method_paths() {
        for (method, path) in [
            (HttpMethod::Post, "/v1/nexus/lifecycle"),
            (HttpMethod::Post, "/v1/sorafs/capacity/por-challenge"),
            (HttpMethod::Post, "/v1/sorafs/capacity/por"),
            (HttpMethod::Post, "/v1/sorafs/por/trigger"),
            (HttpMethod::Post, "/v1/sorafs/storage/por-sample"),
            (HttpMethod::Post, "/v1/sorafs/storage/por-challenge"),
            (HttpMethod::Post, "/v1/sorafs/storage/por-proof"),
            (HttpMethod::Post, "/v1/sorafs/storage/por-verdict"),
        ] {
            assert!(
                CATALOGED_ROUTES
                    .iter()
                    .all(|route| route.method() != method || route.path() != path),
                "unsupported route leaked into the first-release catalog: {method:?} {path}"
            );
        }
        assert!(CATALOGED_ROUTES.contains(&core::NEXUS_LIFECYCLE_GET));
        assert!(
            CATALOGED_ROUTES
                .contains(&contracts_and_verification_keys::SORAFS_CAPACITY_POR_PROOF_POST)
        );
        assert!(
            CATALOGED_ROUTES
                .contains(&contracts_and_verification_keys::SORAFS_CAPACITY_POR_VERDICT_POST)
        );
    }
    #[test]
    fn canonical_catalog_includes_host_gateway_and_directory_routes() {
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
                .all(|route| !route.path().starts_with("/soradns/")),
            "the first-release gateway must not expose a path-encoded alias"
        );
    }
    #[test]
    fn public_runtime_gateway_authentication_is_exactly_scoped() {
        let catalog_routes = CATALOGED_ROUTES
            .iter()
            .filter(|route| route.stable_route_id().starts_with("protocol.soracloud."))
            .collect::<Vec<_>>();
        assert_eq!(catalog_routes.len(), soracloud_gateway::ROUTES.len());
        assert_eq!(soracloud_gateway::ROUTES.len(), 2);
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
    fn formerly_bearer_only_routes_require_exact_signatures() {
        assert_eq!(
            sorafs::STORAGE_TOKEN.authentication(),
            AuthenticationPolicy::OperatorSignature
        );
        for route in [
            application_api::WEBHOOKS_GET,
            application_api::WEBHOOKS_POST,
            application_api::WEBHOOKS_BY_ID_DELETE,
        ] {
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::OperatorSignature,
                "{} authentication",
                route.stable_route_id()
            );
        }
    }
    #[test]
    fn iso20022_routes_require_fresh_operator_signatures() {
        for route in iso20022::ROUTES {
            assert_eq!(route.admission(), AdmissionPolicy::Operator);
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::OperatorSignature
            );
        }
    }
    #[test]
    fn vpn_and_push_device_routes_declare_canonical_account_authentication() {
        for route in [
            core::VPN_QUOTE_CREATE,
            core::VPN_SESSION_CREATE,
            core::VPN_RECEIPTS,
            core::VPN_RECEIPT_SUBMIT,
            core::VPN_SESSION,
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
    fn sorafs_catalog_has_one_strict_first_release_path_per_operation() {
        let catalog = RouteCatalog::new(sorafs::ROUTES);
        assert_eq!(catalog.validate(), Ok(()));
        for expected in sorafs::ROUTES {
            assert!(
                CATALOGED_ROUTES.iter().any(|route| route == expected),
                "missing canonical SoraFS route {}",
                expected.stable_route_id()
            );
            assert_eq!(
                expected.path_normalization(),
                PathNormalization::Strict,
                "SoraFS route must reject normalization aliases: {}",
                expected.stable_route_id()
            );
        }
        for unsupported_path in [
            "/ws/reputation",
            "/sorafs/cid/{cid}/",
            "/v1/sorafs/storage/por-sample",
            "/v1/sorafs/storage/por-challenge",
            "/v1/sorafs/storage/por-proof",
            "/v1/sorafs/storage/por-verdict",
            "/v1/sorafs/deal/fund-provider",
            "/v1/sorafs/deal/fund-client",
            "/v1/sorafs/deal/open",
            "/v1/sorafs/deal/cancel",
            "/v1/sorafs/deal/usage",
            "/v1/sorafs/deal/settle",
            "/v1/sorafs/economics/pricing/manifests",
            "/v1/sorafs/economics/hedging/feeds",
            "/v1/sorafs/economics/status",
            "/v1/sorafs/economics/pricing/active",
            "/v1/sorafs/economics/hedging/reference",
        ] {
            assert!(
                sorafs::ROUTES
                    .iter()
                    .all(|route| route.path() != unsupported_path),
                "unsupported SoraFS path leaked into the catalog: {unsupported_path}"
            );
        }
        assert_eq!(
            sorafs::REPUTATION_EVENTS_WEBSOCKET.path(),
            "/v1/sorafs/reputation/events/ws"
        );
        assert_eq!(sorafs::CID_ROOT.path(), "/sorafs/cid/{cid}");
        assert_eq!(sorafs::CID_ROOT.route_match(), RouteMatch::Exact);
        assert_eq!(sorafs::CID_PATH.route_match(), RouteMatch::Wildcard);
        for invalid_path in [
            "/v1/sorafs//providers",
            "/v1/sorafs/providers/%2fadmin",
            "/v1/sorafs/providers/%5Cadmin",
            "/v1/SoraFs/providers",
        ] {
            let descriptor = RouteDescriptor::new(
                "test.sorafs_invalid_path",
                HttpMethod::Get,
                invalid_path,
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            )
            .with_implicit_head(true);
            assert!(
                validate_catalog(&[descriptor]).is_err(),
                "normalization alias must be rejected: {invalid_path}"
            );
        }
        let trailing_root = RouteDescriptor::new(
            "protocol.sorafs_invalid_root",
            HttpMethod::Get,
            "/sorafs/cid/{cid}/",
            ApiSurface::Protocol,
            Listener::Torii,
            RouteEffect::ReadOnly,
            AdmissionPolicy::Public,
        )
        .with_path_policy(PathPolicy::ProtocolException {
            reason: "adversarial trailing-slash test",
        })
        .with_implicit_head(true);
        assert!(validate_catalog(&[trailing_root]).is_err());
    }
    #[test]
    fn sorafs_hedging_billing_routes_require_canonical_auth_and_tooling_projection() {
        for route in [
            sorafs::BILLING_STATUS,
            sorafs::BILLING_STATEMENTS,
            sorafs::BILLING_STATEMENT,
            sorafs::BILLING_STATEMENT_ACKNOWLEDGEMENTS,
            sorafs::BILLING_RECONCILIATION,
            sorafs::HEDGING_EXPOSURE,
            sorafs::HEDGING_INTENTS,
        ] {
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::CanonicalAccountSignature,
                "{} must require canonical account authentication",
                route.stable_route_id()
            );
            assert_eq!(
                route.projections(),
                RouteProjections::OPENAPI_AND_SDK,
                "{} must remain projected to the OpenAPI and SDK inventories",
                route.stable_route_id()
            );
            assert!(
                route.cors_options(),
                "{} must expose the cataloged CORS preflight",
                route.stable_route_id()
            );
        }
    }
    #[test]
    fn sorafs_pop_routes_declare_their_authenticated_protocol_effects() {
        let routes = [
            (sorafs::POP_ENROLLMENT, RouteEffect::Mutation),
            (sorafs::POP_ENROLLMENT_STATUS, RouteEffect::ReadOnly),
            (sorafs::POP_APPROVAL, RouteEffect::Mutation),
            (sorafs::POP_ISSUE, RouteEffect::Mutation),
            (sorafs::POP_REVOCATION, RouteEffect::Mutation),
            (sorafs::POP_REGISTRY_SUBMIT, RouteEffect::Mutation),
            (sorafs::POP_REGISTRY_RECONCILE, RouteEffect::Mutation),
            (sorafs::POP_REGISTRY_PROJECTION, RouteEffect::ReadOnly),
            (sorafs::POP_WALLET_DELIVERY, RouteEffect::ReadOnly),
            (sorafs::POP_WALLET_IMPORT, RouteEffect::Mutation),
            (sorafs::POP_WALLET_ACKNOWLEDGE, RouteEffect::Mutation),
            (sorafs::POP_WALLET_SYNCHRONIZE, RouteEffect::Mutation),
            (sorafs::POP_WALLET_PROVE, RouteEffect::ExpensiveCompute),
            (sorafs::POP_VERIFY, RouteEffect::Mutation),
        ];
        for (route, effect) in routes {
            assert_eq!(route.effect(), effect, "{} effect", route.stable_route_id());
            assert_eq!(
                route.admission(),
                AdmissionPolicy::AuthenticatedProtocolPrincipal,
                "{} admission",
                route.stable_route_id()
            );
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::ProtocolHandshake,
                "{} authentication",
                route.stable_route_id()
            );
        }
    }
    #[test]
    fn provider_advert_is_an_authenticated_protocol_mutation() {
        let route = sorafs::PROVIDER_ADVERT;
        assert_eq!(route.effect(), RouteEffect::Mutation);
        assert_eq!(
            route.admission(),
            AdmissionPolicy::AuthenticatedProtocolPrincipal
        );
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::ProtocolHandshake
        );
    }
    #[test]
    fn account_faucet_claim_is_an_authenticated_protocol_mutation() {
        let route = application_api::ACCOUNTS_FAUCET_POST;
        assert_eq!(route.effect(), RouteEffect::Mutation);
        assert_eq!(
            route.admission(),
            AdmissionPolicy::AuthenticatedProtocolPrincipal
        );
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::ProtocolHandshake
        );
    }
    #[test]
    fn converted_route_families_are_valid_and_exclude_retired_spellings() {
        let routes = aliases::ROUTES
            .iter()
            .chain(fees::ROUTES)
            .chain(operator_authentication::ROUTES)
            .chain(iso20022::ROUTES)
            .chain(data_availability::ROUTES)
            .chain(musubi::ROUTES)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(validate_catalog(&routes), Ok(()));
        for unsupported_path in [
            "/v1/aliases/resolve_index",
            "/v1/aliases/by_account",
            "/v1/fee-sponsor-policies/by-id",
            "/v1/da/proof_policies",
            "/v1/da/proof_policy_snapshot",
            "/v1/da/pin_intents",
            "/v1/iso20022/status/{msg_id}",
        ] {
            assert!(
                routes.iter().all(|route| route.path() != unsupported_path),
                "unsupported route must not enter the first-release catalog: {unsupported_path}"
            );
        }
        assert!(operator_authentication::ROUTES.iter().all(|route| {
            route.surface() == ApiSurface::Operator
                && route.authentication() == AuthenticationPolicy::OperatorCredentialExchange
                && !route.projections().sdk()
                && !route.projections().mcp()
        }));
    }
    fn contract_and_application_routes() -> Vec<RouteDescriptor> {
        contracts_and_verification_keys::ROUTES
            .iter()
            .chain(application_api::ROUTES)
            .copied()
            .collect()
    }
    #[test]
    fn contract_and_application_routes_are_canonical() {
        let routes = contract_and_application_routes();
        assert_eq!(validate_catalog(&routes), Ok(()));
        for expected in &routes {
            assert!(
                CATALOGED_ROUTES.iter().any(|route| route == expected),
                "missing canonical route {}",
                expected.stable_route_id()
            );
            assert_eq!(expected.path_normalization(), PathNormalization::Strict);
        }
    }
    #[test]
    fn contract_and_application_routes_exclude_retired_spellings() {
        let routes = contract_and_application_routes();
        let openapi = RouteCatalog::new(CATALOGED_ROUTES).project(
            CatalogProjection::OpenApi,
            EnabledFeatures::new(&["app_api"]),
        );
        for unsupported_path in [
            "/v1/multisig/proposals/lookup",
            "/v1/multisig/proposals/list",
            "/v1/multisig/proposals/get",
            "/v1/multisig/proposals/search",
            "/v1/multisig/approvals/list",
            "/v1/multisig/approvals/get",
            "/v1/multisig/approvals/list_for_authority",
            "/v1/multisig/approvals/get_for_authority",
            "/v1/multisig/approvals/query",
            "/v1/multisig/approvals/lookup",
            "/v1/multisig/approvals/query-for-authority",
            "/v1/multisig/approvals/lookup-for-authority",
            "/v1/controls/asset-transfer/get",
            "/v1/nexus/public_lanes/{lane_id}/validators",
            "/v1/sorafs/capacity/por-challenge",
            "/v1/sorafs/capacity/por",
            "/v1/sorafs/por/trigger",
            "/v1/gov/ballots/zk",
        ] {
            assert!(
                routes.iter().all(|route| route.path() != unsupported_path),
                "unsupported first-release spelling leaked into the catalog: {unsupported_path}"
            );
            assert!(
                openapi.iter().all(|route| route.path() != unsupported_path),
                "unsupported first-release spelling leaked into OpenAPI projection: {unsupported_path}"
            );
        }
    }
    #[test]
    fn contract_and_application_routes_include_first_release_spellings() {
        let routes = contract_and_application_routes();
        for canonical_path in [
            "/v1/assets/transfer",
            "/v1/multisig/proposals/query",
            "/v1/multisig/proposals/resolve",
            "/v1/controls/asset-transfer/query",
            "/v1/nexus/public-lanes/{lane_id}/validators",
        ] {
            assert!(
                routes.iter().any(|route| route.path() == canonical_path),
                "missing canonical first-release route: {canonical_path}"
            );
        }
    }
    #[test]
    fn contract_and_application_route_policies_are_projection_safe() {
        for route in [
            contracts_and_verification_keys::CONTRACTS_CODE_BYTES_BY_CODE_HASH_GET,
            contracts_and_verification_keys::MULTISIG_SPEC_POST,
            contracts_and_verification_keys::MULTISIG_PROPOSALS_QUERY_POST,
            contracts_and_verification_keys::MULTISIG_PROPOSALS_RESOLVE_POST,
        ] {
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::CanonicalAccountSignature,
                "{}",
                route.stable_route_id()
            );
        }
        for route in [
            contracts_and_verification_keys::EVIDENCE_VIEWER_GET,
            contracts_and_verification_keys::EVIDENCE_VIEWER_CSS_GET,
            contracts_and_verification_keys::EVIDENCE_VIEWER_JS_GET,
        ] {
            assert_eq!(route.surface(), ApiSurface::Public);
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::Unauthenticated
            );
            assert_eq!(route.projections(), RouteProjections::NONE);
            assert_eq!(route.feature_gate(), FeatureGate::Feature("app_api"));
        }
        for route in [
            contracts_and_verification_keys::SORAFS_CAPACITY_POR_PROOF_POST,
            contracts_and_verification_keys::SORAFS_CAPACITY_POR_VERDICT_POST,
        ] {
            assert_eq!(route.surface(), ApiSurface::Operator);
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::OperatorSignature
            );
            assert!(route.projections().openapi());
            assert!(!route.projections().sdk());
            assert!(!route.projections().mcp());
        }
        assert_eq!(
            application_api::NOTIFY_DEVICES_POST.feature_gate(),
            FeatureGate::All(&["app_api", "push"])
        );
        assert_eq!(
            application_api::TELEMETRY_LIVE_GET.surface(),
            ApiSurface::Diagnostic
        );
        assert_eq!(
            application_api::APP_API_CID_BY_CID_BY_PATH_GET.route_match(),
            RouteMatch::Wildcard
        );
    }
    #[test]
    fn contract_post_routes_close_effect_admission_and_authentication_axes() {
        for route in contracts_and_verification_keys::ROUTES {
            if matches!(
                route.effect(),
                RouteEffect::Mutation | RouteEffect::ExpensiveCompute
            ) {
                assert_ne!(
                    route.admission(),
                    AdmissionPolicy::Public,
                    "{} exposes protected work through public admission",
                    route.stable_route_id()
                );
                assert!(
                    !matches!(
                        route.authentication(),
                        AuthenticationPolicy::ToriiDefault | AuthenticationPolicy::Unauthenticated
                    ),
                    "{} relies on an open authentication policy",
                    route.stable_route_id()
                );
            }
        }
        for route in [
            contracts_and_verification_keys::CONTRACTS_ALIASES_POST,
            contracts_and_verification_keys::BRIDGE_PROOFS_SUBMIT_POST,
        ] {
            assert_eq!(route.effect(), RouteEffect::Mutation);
            assert_eq!(route.admission(), AdmissionPolicy::AuthenticatedAccount);
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::CanonicalAccountSignature
            );
        }
        for route in [
            contracts_and_verification_keys::SORAFS_CAPACITY_DECLARE_POST,
            contracts_and_verification_keys::SORAFS_ORDERBOOK_ORDERS_POST,
        ] {
            assert_eq!(route.effect(), RouteEffect::Mutation);
            assert_eq!(route.admission(), AdmissionPolicy::AuthenticatedAccount);
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::CanonicalSignedBody
            );
        }
        for route in [
            contracts_and_verification_keys::CONTRACTS_CALL_SIMULATE_POST,
            contracts_and_verification_keys::ZK_VK_REGISTER_POST,
        ] {
            assert_eq!(route.effect(), RouteEffect::ExpensiveCompute);
            assert_eq!(route.admission(), AdmissionPolicy::AuthenticatedAccount);
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::CanonicalAccountSignature
            );
        }
        let public_posts = contracts_and_verification_keys::ROUTES
            .iter()
            .filter(|route| {
                route.method() == HttpMethod::Post && route.admission() == AdmissionPolicy::Public
            })
            .map(|route| route.stable_route_id())
            .collect::<Vec<_>>();
        assert_eq!(
            public_posts,
            vec![
                "contracts.sorafs_appeals_pricing_quote_post",
                "contracts.sorafs_appeals_finance_settle_post",
                "contracts.sorafs_appeals_finance_disburse_post",
            ]
        );
        for route in [
            contracts_and_verification_keys::SORAFS_APPEALS_PRICING_QUOTE_POST,
            contracts_and_verification_keys::SORAFS_APPEALS_FINANCE_SETTLE_POST,
            contracts_and_verification_keys::SORAFS_APPEALS_FINANCE_DISBURSE_POST,
        ] {
            assert_eq!(route.effect(), RouteEffect::ReadOnly);
            assert_eq!(route.authentication(), AuthenticationPolicy::ToriiDefault);
        }
    }
    #[test]
    fn pin_registration_is_a_closed_signed_body_mutation() {
        assert_eq!(sorafs::PIN_REGISTER.effect(), RouteEffect::Mutation);
        assert_eq!(
            sorafs::PIN_REGISTER.admission(),
            AdmissionPolicy::AuthenticatedAccount
        );
        assert_eq!(
            sorafs::PIN_REGISTER.authentication(),
            AuthenticationPolicy::CanonicalSignedBody
        );
    }
    #[test]
    fn app_api_post_dispatch_is_an_authenticated_compute_boundary() {
        for route in [
            application_api::APP_API_CID_BY_CID_BY_PATH_POST,
            application_api::APP_API_ACTIVE_BY_PATH_POST,
            application_api::API_CID_BY_CID_BY_PATH_POST,
        ] {
            assert_eq!(route.effect(), RouteEffect::ExpensiveCompute);
            assert_eq!(route.admission(), AdmissionPolicy::AuthenticatedAccount);
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::CanonicalAccountSignature
            );
            assert_eq!(route.route_match(), RouteMatch::Wildcard);
        }
    }
    #[test]
    fn contract_and_application_route_projections_are_explicit() {
        for route in [
            contracts_and_verification_keys::BRIDGE_PROOFS_SUBMIT_POST,
            contracts_and_verification_keys::BRIDGE_MESSAGES_POST,
            contracts_and_verification_keys::MULTISIG_PROPOSALS_QUERY_POST,
            contracts_and_verification_keys::MULTISIG_PROPOSALS_RESOLVE_POST,
        ] {
            assert!(route.projections().openapi(), "{}", route.stable_route_id());
        }
        for route in [
            contracts_and_verification_keys::BRIDGE_PROOFS_SUBMIT_POST,
            contracts_and_verification_keys::BRIDGE_MESSAGES_POST,
        ] {
            assert!(route.projections().sdk(), "{}", route.stable_route_id());
        }
        assert!(application_api::SORACLOUD_DEPLOY_POST.projections().sdk());
        assert!(
            application_api::SORACLOUD_DEPLOY_POST
                .projections()
                .openapi()
        );
        assert!(
            application_api::APP_API_CID_BY_CID_BY_PATH_GET
                .projections()
                .sdk()
        );
        assert!(
            !application_api::APP_API_CID_BY_CID_BY_PATH_GET
                .projections()
                .openapi()
        );
    }
    #[test]
    fn soracloud_release_surface_is_exactly_sixty_openapi_and_sdk_routes() {
        let soracloud_routes = application_api::ROUTES
            .iter()
            .filter(|route| route.path().starts_with("/v1/soracloud/"))
            .collect::<Vec<_>>();
        assert_eq!(soracloud_routes.len(), 60);

        let catalog_routes = CATALOGED_ROUTES
            .iter()
            .filter(|route| {
                route.surface() == ApiSurface::Public && route.path().starts_with("/v1/soracloud/")
            })
            .map(|route| (route.method(), route.path()))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            catalog_routes,
            soracloud_routes
                .iter()
                .map(|route| (route.method(), route.path()))
                .collect(),
            "the canonical catalog must contain the exact application Soracloud inventory"
        );

        let public_reads = BTreeSet::from([
            "/v1/soracloud/model/upload/encryption-recipient",
            "/v1/soracloud/services/{service_name}/public-discovery",
            "/v1/soracloud/services/{service_name}/revisions/{service_version}/public-discovery",
        ]);
        for route in soracloud_routes {
            assert_eq!(route.surface(), ApiSurface::Public, "{}", route.path());
            assert_eq!(route.listener(), Listener::Torii, "{}", route.path());
            assert_eq!(
                route.feature_gate(),
                FeatureGate::Feature("app_api"),
                "{}",
                route.path()
            );
            assert!(route.projections().openapi(), "{}", route.path());
            assert!(route.projections().sdk(), "{}", route.path());
            assert!(!route.projections().mcp(), "{}", route.path());
            assert_eq!(route.route_match(), RouteMatch::Exact, "{}", route.path());
            assert_eq!(
                route.path_normalization(),
                PathNormalization::Strict,
                "{}",
                route.path()
            );
            assert!(route.cors_options(), "{}", route.path());
            assert_eq!(
                route.implicit_head(),
                route.method() == HttpMethod::Get,
                "{}",
                route.path()
            );

            if public_reads.contains(route.path()) {
                assert_eq!(route.method(), HttpMethod::Get, "{}", route.path());
                assert_eq!(
                    route.authentication(),
                    AuthenticationPolicy::ToriiDefault,
                    "{}",
                    route.path()
                );
                assert_eq!(
                    route.admission(),
                    AdmissionPolicy::Public,
                    "{}",
                    route.path()
                );
                assert_eq!(route.effect(), RouteEffect::ReadOnly, "{}", route.path());
                continue;
            }

            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::CanonicalAccountSignature,
                "{}",
                route.path()
            );
            assert_eq!(
                route.admission(),
                AdmissionPolicy::AuthenticatedAccount,
                "{}",
                route.path()
            );
            let expected_effect = match (route.method(), route.path()) {
                (HttpMethod::Get, _) | (HttpMethod::Post, "/v1/soracloud/ciphertext/query") => {
                    RouteEffect::ReadOnly
                }
                (HttpMethod::Post, "/v1/soracloud/model/upload/private/execute") => {
                    RouteEffect::ExpensiveCompute
                }
                (HttpMethod::Post, _) => RouteEffect::Mutation,
                (method, path) => panic!("unsupported Soracloud release route {method:?} {path}"),
            };
            assert_eq!(route.effect(), expected_effect, "{}", route.path());
        }
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "cohesive first-release telemetry and Sumeragi route policy matrix"
    )]
    fn telemetry_and_sumeragi_routes_are_valid_sharp_first_release_surfaces() {
        let routes = telemetry::ROUTES
            .iter()
            .chain(sumeragi::ROUTES)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(validate_catalog(&routes), Ok(()));
        for unsupported_path in [
            "/v1/sumeragi/phases",
            "/v1/sumeragi/telemetry",
            "/v1/sumeragi/new_view/json",
            "/v1/sumeragi/new_view/sse",
            "/v1/sumeragi/bls_keys",
            "/v1/sumeragi/commit_qc/{hash}",
            "/v1/sumeragi/commit-certificates",
            "/v1/sumeragi/commit-qcs/{block_hash}",
            "/v1/sumeragi/checkpoints",
            "/v1/sumeragi/validator-sets",
            "/v1/sumeragi/validator-sets/{height}",
            "/v1/sumeragi/key-lifecycle",
            "/v1/sumeragi/vrf/penalties/{epoch}",
            "/v1/sumeragi/vrf/epoch/{epoch}",
        ] {
            assert!(
                routes.iter().all(|route| route.path() != unsupported_path),
                "unsupported route must not enter the first-release catalog: {unsupported_path}"
            );
        }
        for canonical_path in [
            "/v1/sumeragi/bls-keys",
            "/v1/sumeragi/consensus-keys",
            "/v1/sumeragi/diagnostics",
        ] {
            assert!(
                routes.iter().any(|route| route.path() == canonical_path),
                "missing canonical first-release route: {canonical_path}"
            );
        }
        assert!(
            telemetry::ROUTES
                .iter()
                .filter(|route| route.surface() == ApiSurface::Operator)
                .all(|route| route.authentication() == AuthenticationPolicy::OperatorSignature)
        );
        for route in [
            sumeragi::STATUS,
            sumeragi::DIAGNOSTICS,
            sumeragi::LEADER,
            sumeragi::BLS_KEYS,
            sumeragi::QC,
            sumeragi::CONSENSUS_KEYS,
            sumeragi::PARAMETERS,
            sumeragi::EVIDENCE_COUNT,
            sumeragi::EVIDENCE_LIST,
        ] {
            assert_eq!(route.surface(), ApiSurface::Operator, "{}", route.path());
            assert_eq!(
                route.admission(),
                AdmissionPolicy::Operator,
                "{}",
                route.path()
            );
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::OperatorSignature,
                "{}",
                route.path()
            );
            assert_eq!(route.effect(), RouteEffect::ReadOnly, "{}", route.path());
            assert!(route.projections().openapi(), "{}", route.path());
            assert!(route.projections().sdk(), "{}", route.path());
            assert!(!route.projections().mcp(), "{}", route.path());
        }
        assert!(
            [sumeragi::STATUS_SSE]
                .into_iter()
                .all(|route| route.surface() == ApiSurface::Protocol
                    && route.authentication() == AuthenticationPolicy::ProtocolHandshake
                    && route.projections().openapi()
                    && !route.projections().sdk()
                    && !route.projections().mcp())
        );
        assert!(!sumeragi::SCCP_CAPABILITIES.projections().mcp());
        assert!(!telemetry::DEBUG_WITNESS.projections().openapi());
        for route in [
            telemetry::SORANET_PRIVACY_EVENT,
            telemetry::SORANET_PRIVACY_SHARE,
        ] {
            assert_eq!(route.effect(), RouteEffect::Mutation);
            assert_eq!(route.admission(), AdmissionPolicy::Operator);
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::OperatorSignature
            );
        }
        let catalog = RouteCatalog::new(&routes);
        let without_features = catalog.project(CatalogProjection::Mounted, EnabledFeatures::none());
        assert!(
            without_features
                .iter()
                .any(|route| route.stable_route_id() == sumeragi::EVIDENCE_LIST.stable_route_id())
        );
        assert!(without_features.iter().all(|route| {
            route.stable_route_id() != sumeragi::STATUS.stable_route_id()
                && route.stable_route_id() != telemetry::ASSET_HOLDERS.stable_route_id()
        }));
        let all_features = catalog.project(
            CatalogProjection::Mounted,
            EnabledFeatures::new(&["telemetry", "app_api"]),
        );
        assert_eq!(all_features.len(), routes.len());
    }
    #[test]
    fn projections_are_explicit_and_sdk_is_a_canonical_superset() {
        let catalog = RouteCatalog::new(FEATURED_ROUTES);
        let no_features = EnabledFeatures::none();
        let app_api = EnabledFeatures::new(&["app_api"]);
        assert_eq!(
            catalog
                .project(CatalogProjection::Mounted, no_features)
                .len(),
            2
        );
        assert_eq!(
            catalog.project(CatalogProjection::Mounted, app_api).len(),
            3
        );
        assert_eq!(
            catalog
                .project(CatalogProjection::OpenApi, no_features)
                .len(),
            1
        );
        assert_eq!(
            catalog.project(CatalogProjection::OpenApi, app_api).len(),
            2
        );
        assert_eq!(
            catalog.project(CatalogProjection::Sdk, no_features).len(),
            2
        );
        assert_eq!(
            catalog.project(CatalogProjection::Mcp, no_features).len(),
            1
        );
        assert_eq!(catalog.project(CatalogProjection::Mcp, app_api).len(), 1);
        let route_ids = |projection| {
            catalog
                .project(projection, app_api)
                .into_iter()
                .map(|route| route.stable_route_id())
                .collect::<BTreeSet<_>>()
        };
        let mounted = route_ids(CatalogProjection::Mounted);
        let openapi = route_ids(CatalogProjection::OpenApi);
        let sdk = route_ids(CatalogProjection::Sdk);
        let mcp = route_ids(CatalogProjection::Mcp);
        assert_ne!(mounted, openapi);
        assert_ne!(mounted, sdk);
        assert_ne!(mounted, mcp);
        assert_ne!(openapi, mcp);
    }
    #[test]
    fn feature_expressions_have_deterministic_semantics() {
        let enabled = EnabledFeatures::new(&["app_api", "telemetry"]);
        assert!(FeatureGate::Always.is_enabled(enabled));
        assert!(FeatureGate::Feature("app_api").is_enabled(enabled));
        assert!(FeatureGate::All(&["app_api", "telemetry"]).is_enabled(enabled));
        assert!(!FeatureGate::All(&["app_api", "profiling"]).is_enabled(enabled));
        assert!(FeatureGate::Any(&["profiling", "telemetry"]).is_enabled(enabled));
        assert!(!FeatureGate::Any(&["profiling", "schema"]).is_enabled(enabled));
    }
    #[test]
    fn descriptor_builders_and_accessors_preserve_metadata() {
        let projections = RouteProjections::OPENAPI;
        let descriptor = RouteDescriptor::new(
            "protocol.content",
            HttpMethod::Get,
            "/content/{*tail}",
            ApiSurface::Protocol,
            Listener::Torii,
            RouteEffect::ReadOnly,
            AdmissionPolicy::Public,
        )
        .with_feature_gate(FeatureGate::Feature("app_api"))
        .with_authentication(AuthenticationPolicy::ProtocolHandshake)
        .with_projections(projections)
        .with_route_match(RouteMatch::Wildcard)
        .with_path_policy(PathPolicy::ProtocolException {
            reason: "content-addressed protocol namespace",
        })
        .with_implicit_head(true)
        .with_cors_options(true);
        assert_eq!(descriptor.stable_route_id(), "protocol.content");
        assert_eq!(descriptor.method(), HttpMethod::Get);
        assert_eq!(descriptor.method().as_str(), "GET");
        assert_eq!(descriptor.path(), "/content/{*tail}");
        assert_eq!(descriptor.surface(), ApiSurface::Protocol);
        assert_eq!(descriptor.listener(), Listener::Torii);
        assert_eq!(
            descriptor.authentication(),
            AuthenticationPolicy::ProtocolHandshake
        );
        assert_eq!(descriptor.feature_gate(), FeatureGate::Feature("app_api"));
        assert_eq!(descriptor.projections(), projections);
        assert!(descriptor.projections().openapi());
        assert!(!descriptor.projections().sdk());
        assert!(!descriptor.projections().mcp());
        assert_eq!(descriptor.route_match(), RouteMatch::Wildcard);
        assert!(matches!(
            descriptor.path_policy(),
            PathPolicy::ProtocolException { .. }
        ));
        assert_eq!(descriptor.path_normalization(), PathNormalization::Strict);
        assert!(descriptor.implicit_head());
        assert!(descriptor.cors_options());
        assert_eq!(validate_catalog(&[descriptor]), Ok(()));
    }
    #[test]
    fn content_wildcard_declares_manifest_conditional_authentication() {
        let descriptor = content_directory::CONTENT;
        assert_eq!(
            descriptor.authentication(),
            AuthenticationPolicy::ManifestConditionalContent
        );
        assert_eq!(descriptor.route_match(), RouteMatch::Wildcard);
        assert!(descriptor.projections().openapi());
    }
    #[test]
    fn validation_reports_duplicate_ids_and_method_paths() {
        let routes = [
            RouteDescriptor::new(
                "test.duplicate",
                HttpMethod::Get,
                "/v1/tests/one",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            ),
            RouteDescriptor::new(
                "test.duplicate",
                HttpMethod::Get,
                "/v1/tests/two",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            ),
            RouteDescriptor::new(
                "test.same_path",
                HttpMethod::Get,
                "/v1/tests/one",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            ),
            RouteDescriptor::new(
                "test.same_shape_one",
                HttpMethod::Get,
                "/v1/tests/shapes/{first_id}",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            ),
            RouteDescriptor::new(
                "test.same_shape_two",
                HttpMethod::Get,
                "/v1/tests/shapes/{second_id}",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            ),
        ];
        let errors = validate_catalog(&routes).expect_err("duplicates must fail validation");
        assert!(
            errors
                .iter()
                .any(|error| { error.kind == CatalogValidationErrorKind::DuplicateStableRouteId })
        );
        assert!(errors.iter().any(|error| {
            matches!(
                error.kind,
                CatalogValidationErrorKind::DuplicateMethodAndPath {
                    existing_route_id: "test.duplicate"
                }
            )
        }));
        assert!(errors.iter().any(|error| {
            matches!(
                error.kind,
                CatalogValidationErrorKind::DuplicateMethodAndShape {
                    existing_route_id: "test.same_shape_one"
                }
            )
        }));
    }
    #[test]
    fn canonical_path_grammar_rejects_ambiguous_shapes() {
        let invalid_paths = [
            "/tests/readiness",
            "/v1/tests/snake_case",
            "/v1/tests/{itemId}",
            "/v1/tests/{item_id}/{item_id}",
            "/v1/tests//readiness",
            "/v1/tests/readiness/",
            "/v1/tests/%72eadiness",
        ];
        for path in invalid_paths {
            let descriptor = RouteDescriptor::new(
                "test.invalid_path",
                HttpMethod::Get,
                path,
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            );
            assert!(
                validate_catalog(&[descriptor]).is_err(),
                "path should be rejected: {path}"
            );
        }
    }
    #[test]
    fn canonical_path_grammar_rejects_crud_read_operation_segments() {
        for descriptor in [
            RouteDescriptor::new(
                "test.resources_list_post",
                HttpMethod::Post,
                "/v1/tests/resources/list",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            ),
            RouteDescriptor::new(
                "test.resources_get_post",
                HttpMethod::Post,
                "/v1/tests/resources/get",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            ),
            RouteDescriptor::new(
                "test.resources_list_get",
                HttpMethod::Get,
                "/v1/tests/resources/list",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            ),
            RouteDescriptor::new(
                "test.resources_list_post",
                HttpMethod::Post,
                "/v1/tests/resources/list/details",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            ),
            RouteDescriptor::new(
                "test.resources_query_post",
                HttpMethod::Post,
                "/v1/tests/resources/list",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            ),
        ] {
            assert_eq!(
                validate_path(&descriptor),
                Err("static path segment uses a forbidden transport or CRUD word")
            );
        }
        for descriptor in [
            RouteDescriptor::new(
                "test.resources_json_post",
                HttpMethod::Post,
                "/v1/tests/resources/json",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            ),
            RouteDescriptor::new(
                "test.resources_sse_post",
                HttpMethod::Post,
                "/v1/tests/resources/sse",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            ),
        ] {
            assert_eq!(
                validate_path(&descriptor),
                Err("static path segment uses a forbidden transport or CRUD word")
            );
        }
    }
    #[test]
    fn wildcard_and_protocol_exceptions_must_be_explicit() {
        let wildcard = RouteDescriptor::new(
            "test.wildcard",
            HttpMethod::Get,
            "/v1/content/{*tail}",
            ApiSurface::Protocol,
            Listener::Torii,
            RouteEffect::ReadOnly,
            AdmissionPolicy::Public,
        )
        .with_route_match(RouteMatch::Wildcard)
        .with_implicit_head(true);
        let health = RouteDescriptor::new(
            "protocol.health",
            HttpMethod::Get,
            "/health",
            ApiSurface::Protocol,
            Listener::Torii,
            RouteEffect::ReadOnly,
            AdmissionPolicy::Public,
        )
        .with_path_policy(PathPolicy::ProtocolException {
            reason: "orchestrator health-probe convention",
        })
        .with_implicit_head(true);
        assert_eq!(validate_catalog(&[wildcard, health]), Ok(()));
        let implicit_wildcard = RouteDescriptor::new(
            "test.implicit_wildcard",
            HttpMethod::Get,
            "/v1/content/{*tail}",
            ApiSurface::Protocol,
            Listener::Torii,
            RouteEffect::ReadOnly,
            AdmissionPolicy::Public,
        );
        assert!(validate_catalog(&[implicit_wildcard]).is_err());
    }
    #[test]
    fn validation_enforces_projection_and_implicit_method_boundaries() {
        let routes = [
            RouteDescriptor::new(
                "test.diagnostic_sdk",
                HttpMethod::Get,
                "/v1/tests/diagnostic-sdk",
                ApiSurface::Diagnostic,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            )
            .with_projections(RouteProjections::SDK),
            RouteDescriptor::new(
                "test.protocol_handshake_mcp",
                HttpMethod::Get,
                "/v1/tests/protocol-handshake",
                ApiSurface::Protocol,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            )
            .with_authentication(AuthenticationPolicy::ProtocolHandshake)
            .with_projections(RouteProjections::MCP),
            RouteDescriptor::new(
                "test.operator_without_signature",
                HttpMethod::Post,
                "/v1/tests/operator-without-signature",
                ApiSurface::Operator,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Operator,
            )
            .with_projections(RouteProjections::MCP),
            RouteDescriptor::new(
                "test.head_on_post",
                HttpMethod::Post,
                "/v1/tests/head-on-post",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            )
            .with_implicit_head(true),
            RouteDescriptor::new(
                "test.public_credential_exchange",
                HttpMethod::Post,
                "/v1/tests/public-credential-exchange",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            )
            .with_authentication(AuthenticationPolicy::OperatorCredentialExchange),
        ];
        let errors = validate_catalog(&routes).expect_err("invalid boundaries must be rejected");
        assert!(errors.iter().any(|error| {
            error.kind == CatalogValidationErrorKind::DiagnosticToolingProjection
        }));
        assert!(errors.iter().any(|error| {
            error.kind == CatalogValidationErrorKind::ProtocolHandshakeMcpProjection
        }));
        assert!(errors.iter().any(|error| {
            error.kind == CatalogValidationErrorKind::OperatorSurfaceRequiresAuthentication
        }));
        assert!(errors.iter().any(|error| {
            error.kind
                == CatalogValidationErrorKind::OperatorCredentialExchangeRequiresOperatorSurface
        }));
        assert!(
            errors
                .iter()
                .any(|error| { error.kind == CatalogValidationErrorKind::ImplicitHeadRequiresGet })
        );
    }
    #[test]
    fn validation_rejects_unsafe_effect_and_principal_combinations() {
        let routes = [
            RouteDescriptor::new(
                "test.public_mutation",
                HttpMethod::Post,
                "/v1/tests/public-mutation",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::Mutation,
                AdmissionPolicy::Public,
            ),
            RouteDescriptor::new(
                "test.public_expensive_compute",
                HttpMethod::Post,
                "/v1/tests/public-expensive-compute",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ExpensiveCompute,
                AdmissionPolicy::Public,
            ),
            RouteDescriptor::new(
                "test.account_without_account_auth",
                HttpMethod::Post,
                "/v1/tests/account-without-account-auth",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::Mutation,
                AdmissionPolicy::AuthenticatedAccount,
            )
            .with_authentication(AuthenticationPolicy::IdentityBoundSignature),
            RouteDescriptor::new(
                "test.validator_without_roster_auth",
                HttpMethod::Post,
                "/v1/tests/validator-without-roster-auth",
                ApiSurface::Protocol,
                Listener::Torii,
                RouteEffect::Mutation,
                AdmissionPolicy::ValidatorRosterMember,
            )
            .with_authentication(AuthenticationPolicy::CanonicalAccountSignature),
            RouteDescriptor::new(
                "test.protocol_principal_without_handshake",
                HttpMethod::Post,
                "/v1/tests/protocol-principal-without-handshake",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::Mutation,
                AdmissionPolicy::AuthenticatedProtocolPrincipal,
            )
            .with_authentication(AuthenticationPolicy::CanonicalAccountSignature),
            RouteDescriptor::new(
                "test.post_stream",
                HttpMethod::Post,
                "/v1/tests/post-stream",
                ApiSurface::Protocol,
                Listener::Torii,
                RouteEffect::LongLivedStream,
                AdmissionPolicy::ValidatorRosterMember,
            )
            .with_authentication(AuthenticationPolicy::ProtocolHandshake),
        ];
        let errors = validate_catalog(&routes).expect_err("unsafe admission metadata must fail");
        for expected in [
            CatalogValidationErrorKind::PublicMutation,
            CatalogValidationErrorKind::PublicExpensiveCompute,
            CatalogValidationErrorKind::AuthenticatedAccountRequiresAuthentication,
            CatalogValidationErrorKind::AuthenticatedProtocolPrincipalRequiresHandshake,
            CatalogValidationErrorKind::ValidatorAdmissionRequiresAuthentication,
            CatalogValidationErrorKind::LongLivedStreamRequiresGetOrAny,
        ] {
            assert!(
                errors.iter().any(|error| error.kind == expected),
                "missing catalog validation error: {expected:?}"
            );
        }
    }
    #[test]
    fn critical_routes_expose_closed_effect_and_admission_axes() {
        for route in [
            pipeline::TRANSACTION,
            pipeline::TRANSACTION_ENTRYPOINT,
            pipeline::TRANSACTIONS_BATCH,
        ] {
            assert_eq!(route.effect(), RouteEffect::Mutation);
            assert_eq!(route.admission(), AdmissionPolicy::AuthenticatedAccount);
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::CanonicalSignedBody
            );
        }
        assert_eq!(pipeline::QUERY.effect(), RouteEffect::ExpensiveCompute);
        assert_eq!(
            pipeline::QUERY.admission(),
            AdmissionPolicy::AuthenticatedAccount
        );
        assert_eq!(
            pipeline::QUERY.authentication(),
            AuthenticationPolicy::CanonicalSignedBody
        );
        assert_eq!(pipeline::TRANSACTION_STATUS.effect(), RouteEffect::ReadOnly);
        assert_eq!(
            pipeline::TRANSACTION_STATUS.admission(),
            AdmissionPolicy::Public
        );
        assert_eq!(
            pipeline::TRANSACTION_DETAILS.effect(),
            RouteEffect::ExpensiveCompute
        );
        assert_eq!(
            pipeline::TRANSACTION_DETAILS.admission(),
            AdmissionPolicy::AuthenticatedAccount
        );
        assert_eq!(
            pipeline::TRANSACTION_DETAILS.authentication(),
            AuthenticationPolicy::CanonicalSignedBody
        );
        assert_eq!(core::HEALTH.effect(), RouteEffect::ReadOnly);
        assert_eq!(core::HEALTH.admission(), AdmissionPolicy::Public);
        assert_eq!(
            runtime_governance::ZK_IVM_PROVE.effect(),
            RouteEffect::ExpensiveCompute
        );
        assert_eq!(
            runtime_governance::ZK_IVM_PROVE.admission(),
            AdmissionPolicy::AuthenticatedAccount
        );
        assert_eq!(
            streaming::SUBSCRIPTION_WS.effect(),
            RouteEffect::LongLivedStream
        );
        assert_eq!(
            streaming::P2P.admission(),
            AdmissionPolicy::ValidatorRosterMember
        );
    }
    #[test]
    fn implicit_head_and_cors_routes_are_separate_from_explicit_operations() {
        let routes = [
            RouteDescriptor::new(
                "test.read",
                HttpMethod::Get,
                "/v1/tests/resource",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            )
            .with_implicit_head(true)
            .with_cors_options(true),
            RouteDescriptor::new(
                "test.write",
                HttpMethod::Post,
                "/v1/tests/resource",
                ApiSurface::Public,
                Listener::Torii,
                RouteEffect::ReadOnly,
                AdmissionPolicy::Public,
            )
            .with_cors_options(true),
        ];
        let catalog = RouteCatalog::new(&routes);
        let implicit = catalog.implicit_routes(EnabledFeatures::none());
        assert_eq!(implicit.len(), 2, "OPTIONS is emitted once per path");
        assert!(implicit.iter().any(|route| {
            route.kind() == ImplicitRouteKind::Head
                && route.parent_route_id() == "test.read"
                && route.path() == "/v1/tests/resource"
        }));
        assert!(implicit.iter().any(|route| {
            route.kind() == ImplicitRouteKind::CorsOptions && route.path() == "/v1/tests/resource"
        }));
        assert_eq!(
            catalog
                .project(CatalogProjection::Mounted, EnabledFeatures::none())
                .len(),
            2,
            "framework routes do not enter the application projection"
        );
    }
    #[test]
    fn any_method_is_protocol_only_and_never_generated() {
        let valid = RouteDescriptor::new(
            "protocol.gateway",
            HttpMethod::Any,
            "/gateway/{*tail}",
            ApiSurface::Protocol,
            Listener::Torii,
            RouteEffect::ReadOnly,
            AdmissionPolicy::Public,
        )
        .with_route_match(RouteMatch::Wildcard)
        .with_path_policy(PathPolicy::ProtocolException {
            reason: "protocol-native HTTP gateway",
        });
        assert_eq!(validate_catalog(&[valid]), Ok(()));
        let invalid = RouteDescriptor::new(
            "test.gateway",
            HttpMethod::Any,
            "/v1/tests/{*tail}",
            ApiSurface::Public,
            Listener::Torii,
            RouteEffect::ReadOnly,
            AdmissionPolicy::Public,
        )
        .with_route_match(RouteMatch::Wildcard)
        .with_projections(RouteProjections::OPENAPI);
        let errors = validate_catalog(&[invalid]).expect_err("unsafe ANY route must fail");
        assert!(errors.iter().any(|error| {
            error.kind == CatalogValidationErrorKind::AnyMethodRequiresProtocolSurface
        }));
        assert!(
            errors.iter().any(|error| {
                error.kind == CatalogValidationErrorKind::AnyMethodToolingProjection
            })
        );
    }
    #[test]
    fn musubi_v1_catalog_is_post_only_and_has_no_legacy_routes() {
        assert_eq!(musubi::ROUTES.len(), 31);
        assert_eq!(RouteCatalog::new(musubi::ROUTES).validate(), Ok(()));
        assert!(musubi::ROUTES.iter().all(|route| {
            route.method() == HttpMethod::Post
                && (route.path().starts_with("/v1/musubi/queries/")
                    || route.path().starts_with("/v1/musubi/instructions/"))
        }));
        for route in musubi::ROUTES {
            assert_eq!(
                route.admission(),
                AdmissionPolicy::AuthenticatedAccount,
                "{} admission",
                route.stable_route_id()
            );
            assert_eq!(
                route.authentication(),
                AuthenticationPolicy::CanonicalAccountSignature,
                "{} authentication",
                route.stable_route_id()
            );
            assert_eq!(
                route.effect(),
                if route.path().starts_with("/v1/musubi/queries/") {
                    RouteEffect::ReadOnly
                } else {
                    RouteEffect::ExpensiveCompute
                },
                "{} effect",
                route.stable_route_id()
            );
        }
        assert!(musubi::ARCHIVE_RETENTION.projections().openapi());
        assert!(musubi::ARCHIVE_RETENTION.projections().sdk());
        assert!(musubi::ARCHIVE_RETENTION.projections().mcp());
        for expected in musubi::ROUTES {
            assert!(
                CATALOGED_ROUTES.iter().any(|route| route == expected),
                "missing canonical Musubi route {}",
                expected.stable_route_id()
            );
        }
        for legacy_path in [
            "/v1/musubi/packages",
            "/v1/musubi/release",
            "/v1/musubi/releases",
            "/v1/musubi/versions",
            "/v1/musubi/aliases/{alias}",
            "/v1/musubi/instructions/publish-release",
            "/v1/musubi/instructions/yank-release",
            "/v1/musubi/instructions/set-alias",
            "/v1/musubi/instructions/assert-release-exists",
        ] {
            assert!(
                !CATALOGED_ROUTES
                    .iter()
                    .any(|route| route.path() == legacy_path),
                "retired Musubi route remains cataloged: {legacy_path}"
            );
        }
    }
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
                    .filter(|candidate| { candidate.stable_route_id() == route.stable_route_id() })
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
    include!("authentication_routes_test.rs");
}
