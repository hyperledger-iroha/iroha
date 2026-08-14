#[test]
fn offline_receiver_lineage_requires_account_authentication_before_expensive_proof_work() {
    let route = offline::RECIPIENT_LINEAGE;
    assert_eq!(route.effect(), RouteEffect::ExpensiveCompute);
    assert_eq!(route.admission(), AdmissionPolicy::AuthenticatedAccount);
    assert_eq!(
        route.authentication(),
        AuthenticationPolicy::CanonicalAccountSignature
    );
}
#[test]
fn application_query_posts_authenticate_before_expensive_compute() {
    for route in [
        application_api::ACCOUNTS_BY_ACCOUNT_ID_TRANSACTIONS_QUERY_POST,
        application_api::ACCOUNTS_BY_ACCOUNT_ID_ASSETS_QUERY_POST,
        application_api::DOMAINS_QUERY_POST,
        application_api::ACCOUNTS_QUERY_POST,
        application_api::TRANSACTIONS_QUERY_POST,
        application_api::TRANSACTIONS_VISIBLE_QUERY_POST,
        application_api::REPO_AGREEMENTS_QUERY_POST,
        telemetry::ASSET_HOLDERS_QUERY,
        application_api::ASSETS_DEFINITIONS_QUERY_POST,
        application_api::NFTS_QUERY_POST,
        application_api::RWAS_QUERY_POST,
    ] {
        assert_eq!(
            route.effect(),
            RouteEffect::ExpensiveCompute,
            "{}",
            route.path()
        );
        assert_eq!(
            route.admission(),
            AdmissionPolicy::AuthenticatedAccount,
            "{}",
            route.path()
        );
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature,
            "{}",
            route.path()
        );
    }
    let proof_query = application_api::PROOFS_QUERY_POST;
    assert_eq!(proof_query.effect(), RouteEffect::ExpensiveCompute);
    assert_eq!(
        proof_query.admission(),
        AdmissionPolicy::AuthenticatedAccount
    );
    assert_eq!(
        proof_query.authentication(),
        AuthenticationPolicy::CanonicalSignedBody
    );
}
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
fn local_sorafs_governance_state_is_operator_signed() {
    for route in [
        sorafs::GOVERNANCE_DAG_DASHBOARD,
        sorafs::GOVERNANCE_DAG_HEAD,
        sorafs::GOVERNANCE_DAG_BLOCK,
        sorafs::GOVERNANCE_DAG_NODE,
        sorafs::GOVERNANCE_DAG_PUBLISH_INDEX,
        sorafs::GOVERNANCE_DAG_PUBLISH_DIGEST,
        sorafs::GOVERNANCE_DAG_PUBLISH_KIND,
        sorafs::GOVERNANCE_DAG_CAR_QUEUE,
        sorafs::GOVERNANCE_DAG_CAR_QUEUE_DIGEST,
        sorafs::GOVERNANCE_DAG_CAR_QUEUE_KIND,
        sorafs::GOVERNANCE_DAG_CAR_QUEUE_ARCHIVE,
        sorafs::GOVERNANCE_DAG_RUNTIME,
        sorafs::GOVERNANCE_DAG_RUNTIME_HEAD,
        sorafs::GOVERNANCE_DAG_RUNTIME_BLOCK,
        sorafs::GOVERNANCE_DAG_RUNTIME_NODE,
        sorafs::GOVERNANCE_DAG_RUNTIME_DIGEST,
        sorafs::GOVERNANCE_DAG_RUNTIME_KIND,
    ] {
        assert_eq!(route.method(), HttpMethod::Get);
        assert_eq!(route.surface(), ApiSurface::Operator);
        assert_eq!(route.effect(), RouteEffect::ReadOnly);
        assert_eq!(route.admission(), AdmissionPolicy::Operator);
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::OperatorSignature,
            "{} must not expose node-local inventory anonymously",
            route.stable_route_id()
        );
        assert_eq!(route.projections(), RouteProjections::NONE);
    }
}
#[test]
fn node_local_core_and_pipeline_reads_require_exact_operator_signatures() {
    for route in [
        core::PEERS,
        core::TIME_STATUS,
        pipeline::PREFLIGHT,
        pipeline::POLICY,
        pipeline::PROOF_RETENTION,
        pipeline::RECOVERY,
    ] {
        assert_eq!(route.method(), HttpMethod::Get);
        assert_eq!(route.surface(), ApiSurface::Operator);
        assert_eq!(route.effect(), RouteEffect::ReadOnly);
        assert_eq!(route.admission(), AdmissionPolicy::Operator);
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::OperatorSignature,
            "{} must authenticate before reading node-local state",
            route.stable_route_id()
        );
        assert!(route.projections().openapi());
        assert!(route.projections().sdk());
        assert!(
            !route.projections().mcp(),
            "{} must not retain a public MCP projection",
            route.stable_route_id()
        );
        assert!(!route.cors_options());
    }
}
#[test]
fn sorafs_inventory_and_storage_reads_declare_fail_closed_admission() {
    for route in [sorafs::ALIASES, sorafs::REPLICATION] {
        assert_eq!(route.method(), HttpMethod::Get);
        assert_eq!(route.surface(), ApiSurface::Public);
        assert_eq!(route.effect(), RouteEffect::ExpensiveCompute);
        assert_eq!(route.admission(), AdmissionPolicy::AuthenticatedAccount);
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature
        );
        assert_eq!(route.projections(), RouteProjections::OPENAPI_AND_SDK);
    }
    let storage_state = sorafs::STORAGE_STATE;
    assert_eq!(storage_state.method(), HttpMethod::Get);
    assert_eq!(storage_state.surface(), ApiSurface::Operator);
    assert_eq!(storage_state.effect(), RouteEffect::ReadOnly);
    assert_eq!(storage_state.admission(), AdmissionPolicy::Operator);
    assert_eq!(
        storage_state.authentication(),
        AuthenticationPolicy::OperatorSignature
    );
    assert_eq!(storage_state.projections(), RouteProjections::NONE);
    let storage_fetch = sorafs::STORAGE_FETCH;
    assert_eq!(storage_fetch.method(), HttpMethod::Post);
    assert_eq!(storage_fetch.surface(), ApiSurface::Operator);
    assert_eq!(storage_fetch.effect(), RouteEffect::ExpensiveCompute);
    assert_eq!(storage_fetch.admission(), AdmissionPolicy::Operator);
    assert_eq!(
        storage_fetch.authentication(),
        AuthenticationPolicy::OperatorSignature
    );
    assert_eq!(storage_fetch.projections(), RouteProjections::NONE);
    for route in [sorafs::STORAGE_CAR, sorafs::STORAGE_CHUNK] {
        assert_eq!(route.method(), HttpMethod::Get);
        assert_eq!(route.effect(), RouteEffect::ReadOnly);
        assert_eq!(
            route.admission(),
            AdmissionPolicy::AuthenticatedProtocolPrincipal
        );
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::ProtocolHandshake
        );
        assert_eq!(route.projections(), RouteProjections::OPENAPI_AND_SDK);
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
fn soracloud_commands_require_exact_account_authentication_and_honest_effects() {
    let commands = application_api::ROUTES
        .iter()
        .filter(|route| {
            route.method() == HttpMethod::Post
                && route
                    .stable_route_id()
                    .starts_with("application.soracloud_")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        commands.len(),
        41,
        "every SoraCloud POST must be classified"
    );
    for route in commands {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature,
            "{} must authenticate the exact body before decoding it",
            route.stable_route_id()
        );
        assert_eq!(
            route.admission(),
            AdmissionPolicy::AuthenticatedAccount,
            "{} must admit only an on-ledger account",
            route.stable_route_id()
        );
        let expected_effect = match route.stable_route_id() {
            "application.soracloud_ciphertext_query_post" => RouteEffect::ReadOnly,
            "application.soracloud_model_upload_private_execute_post" => {
                RouteEffect::ExpensiveCompute
            }
            _ => RouteEffect::Mutation,
        };
        assert_eq!(
            route.effect(),
            expected_effect,
            "{} advertises the wrong strongest effect",
            route.stable_route_id()
        );
    }
}
#[test]
fn soracloud_sensitive_reads_require_exact_account_authentication() {
    let protected = [
        application_api::SORACLOUD_STATUS_GET,
        application_api::SORACLOUD_APPS_STATUS_GET,
        application_api::SORACLOUD_APPS_BY_APP_NAME_STATUS_GET,
        application_api::SORACLOUD_SERVICE_CONFIG_STATUS_GET,
        application_api::SORACLOUD_SERVICE_SECRET_STATUS_GET,
        application_api::SORACLOUD_HEALTH_COMPLIANCE_REPORT_GET,
        application_api::SORACLOUD_TRAINING_JOB_STATUS_GET,
        application_api::SORACLOUD_MODEL_WEIGHT_STATUS_GET,
        application_api::SORACLOUD_MODEL_ARTIFACT_STATUS_GET,
        application_api::SORACLOUD_MODEL_UPLOAD_STATUS_GET,
        application_api::SORACLOUD_MODEL_UPLOAD_PRIVATE_RECEIPTS_GET,
        application_api::SORACLOUD_HF_STATUS_GET,
        application_api::SORACLOUD_MODEL_HOST_STATUS_GET,
        application_api::SORACLOUD_AGENT_STATUS_GET,
        application_api::SORACLOUD_AGENT_MAILBOX_STATUS_GET,
        application_api::SORACLOUD_AGENT_AUTONOMY_STATUS_GET,
    ];
    assert_eq!(
        protected.len(),
        16,
        "every sensitive Soracloud GET must be classified"
    );
    for route in protected {
        assert_eq!(route.method(), HttpMethod::Get);
        assert_eq!(route.effect(), RouteEffect::ReadOnly);
        assert_eq!(route.admission(), AdmissionPolicy::AuthenticatedAccount);
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature,
            "{} must authenticate its exact path and query",
            route.stable_route_id()
        );
    }
}
#[test]
fn soracloud_public_reads_are_bounded_single_object_discovery() {
    for route in [
        application_api::SORACLOUD_SERVICES_BY_SERVICE_NAME_PUBLIC_DISCOVERY_GET,
        application_api::SORACLOUD_SERVICES_BY_SERVICE_NAME_REVISIONS_BY_SERVICE_VERSION_PUBLIC_DISCOVERY_GET,
        application_api::SORACLOUD_MODEL_UPLOAD_ENCRYPTION_RECIPIENT_GET,
    ] {
        assert_eq!(route.method(), HttpMethod::Get);
        assert_eq!(route.effect(), RouteEffect::ReadOnly);
        assert_eq!(route.admission(), AdmissionPolicy::Public);
        assert_eq!(route.authentication(), AuthenticationPolicy::ToriiDefault);
    }
}
#[test]
fn subscription_commands_require_exact_account_authentication_and_mutation_admission() {
    for route in [
        application_api::SUBSCRIPTIONS_PLANS_POST,
        application_api::SUBSCRIPTIONS_POST,
        application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_PAUSE_POST,
        application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_RESUME_POST,
        application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_CANCEL_POST,
        application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_KEEP_POST,
        application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_USAGE_POST,
        application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_CHARGE_NOW_POST,
    ] {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature,
            "{} must authenticate its exact path and body before decoding",
            route.stable_route_id()
        );
        assert_eq!(route.admission(), AdmissionPolicy::AuthenticatedAccount);
        assert_eq!(route.effect(), RouteEffect::Mutation);
    }
}
#[test]
fn application_drafts_and_cryptographic_services_require_exact_account_authentication() {
    for route in [
        application_api::SPACE_DIRECTORY_MANIFESTS_POST,
        application_api::SPACE_DIRECTORY_MANIFESTS_REVOKE_POST,
        application_api::RAM_LFE_PROGRAMS_BY_PROGRAM_ID_EXECUTE_POST,
        application_api::RAM_LFE_RECEIPTS_VERIFY_POST,
        application_api::ACCOUNTS_BY_ACCOUNT_ID_IDENTIFIERS_CLAIM_RECEIPT_POST,
        application_api::IDENTIFIERS_RESOLVE_POST,
    ] {
        assert_eq!(route.method(), HttpMethod::Post);
        assert_eq!(route.effect(), RouteEffect::ExpensiveCompute);
        assert_eq!(route.admission(), AdmissionPolicy::AuthenticatedAccount);
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature,
            "{} must authenticate its exact path and body before decoding",
            route.stable_route_id()
        );
    }
}
#[test]
fn webhook_registry_is_operator_signed_and_effects_are_exact() {
    assert_eq!(
        application_api::WEBHOOKS_GET.effect(),
        RouteEffect::ReadOnly
    );
    for route in [
        application_api::WEBHOOKS_POST,
        application_api::WEBHOOKS_BY_ID_DELETE,
    ] {
        assert_eq!(route.effect(), RouteEffect::Mutation);
    }
    for route in [
        application_api::WEBHOOKS_GET,
        application_api::WEBHOOKS_POST,
        application_api::WEBHOOKS_BY_ID_DELETE,
    ] {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::OperatorSignature
        );
        assert_eq!(route.admission(), AdmissionPolicy::Operator);
    }
}
#[test]
fn zk_attachment_tenant_routes_are_account_authenticated_before_storage_access() {
    for route in [
        runtime_governance::ZK_ATTACHMENTS_GET,
        runtime_governance::ZK_ATTACHMENTS_POST,
        runtime_governance::ZK_ATTACHMENT_GET,
        runtime_governance::ZK_ATTACHMENT_DELETE,
        runtime_governance::ZK_ATTACHMENTS_COUNT,
    ] {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature,
            "{} must authenticate the tenant account",
            route.stable_route_id()
        );
        assert_eq!(
            route.admission(),
            AdmissionPolicy::AuthenticatedAccount,
            "{} must reject anonymous storage access",
            route.stable_route_id()
        );
    }
    assert_eq!(
        runtime_governance::ZK_ATTACHMENTS_POST.effect(),
        RouteEffect::Mutation
    );
    assert_eq!(
        runtime_governance::ZK_ATTACHMENT_DELETE.effect(),
        RouteEffect::Mutation
    );
}
#[test]
fn zk_compute_routes_require_exact_account_authentication() {
    for route in [
        runtime_governance::ZK_IVM_DERIVE,
        runtime_governance::ZK_IVM_PROVE,
        runtime_governance::ZK_VERIFY_BATCH,
    ] {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature,
            "{} must authenticate before bounded compute",
            route.stable_route_id()
        );
        assert_eq!(route.admission(), AdmissionPolicy::AuthenticatedAccount);
        assert_eq!(route.effect(), RouteEffect::ExpensiveCompute);
    }
}
#[test]
fn state_backed_runtime_and_governance_routes_require_exact_account_authentication() {
    let routes = [
        runtime_governance::ZK_ROOTS,
        runtime_governance::ZK_MERKLE_PATH,
        runtime_governance::ZK_VOTE_TALLY,
        runtime_governance::RUNTIME_ABI_ACTIVE,
        runtime_governance::RUNTIME_METRICS,
        runtime_governance::NODE_CAPABILITIES,
        runtime_governance::PRIVACY_CAPABILITIES,
        runtime_governance::NODE_PROJECTION_CHECKPOINT,
        runtime_governance::MINISTRY_AGENDA_DRAFT,
        runtime_governance::MINISTRY_AGENDA_GET,
        runtime_governance::GOV_PROPOSE_DEPLOY,
        runtime_governance::GOV_PROPOSE_SCCP,
        runtime_governance::GOV_CAPABILITIES,
        runtime_governance::GOV_CITIZEN_DRAFT,
        runtime_governance::VALIDATION_FEE_CURRENT_POLICY_PROOF,
        runtime_governance::VALIDATION_FEE_PROPOSALS,
        runtime_governance::VALIDATION_FEE_PROPOSAL_DETAIL,
        runtime_governance::VALIDATION_FEE_PROPOSAL_DRAFT,
        runtime_governance::VALIDATION_FEE_PLAIN_BALLOT_DRAFT,
        runtime_governance::GOV_PROPOSAL_GET,
        runtime_governance::GOV_LOCKS_GET,
        runtime_governance::GOV_REFERENDUM_GET,
        runtime_governance::GOV_TALLY_GET,
        runtime_governance::GOV_PROTECTED_GET,
        runtime_governance::GOV_UNLOCK_STATS,
        runtime_governance::GOV_CONTRACT_GET,
        runtime_governance::GOV_ENACT,
        runtime_governance::GOV_COUNCIL_CURRENT,
        runtime_governance::GOV_CITIZENS_COUNT,
        runtime_governance::GOV_CITIZEN_STATUS,
    ];
    for route in routes {
        assert_eq!(
            route.authentication(),
            AuthenticationPolicy::CanonicalAccountSignature,
            "{} must authenticate the exact network request before state access",
            route.stable_route_id()
        );
        assert_eq!(
            route.admission(),
            AdmissionPolicy::AuthenticatedAccount,
            "{} must admit only a verified on-ledger account",
            route.stable_route_id()
        );
        assert_eq!(route.path_normalization(), PathNormalization::Strict);
        assert!(
            !route.path().ends_with('/'),
            "{} must not expose a redirectable trailing-slash alias",
            route.stable_route_id()
        );
    }
    for route in [
        runtime_governance::ZK_ROOTS,
        runtime_governance::ZK_MERKLE_PATH,
        runtime_governance::RUNTIME_METRICS,
        runtime_governance::VALIDATION_FEE_CURRENT_POLICY_PROOF,
        runtime_governance::VALIDATION_FEE_PROPOSAL_DETAIL,
        runtime_governance::GOV_LOCKS_GET,
        runtime_governance::GOV_TALLY_GET,
    ] {
        assert_eq!(route.effect(), RouteEffect::ExpensiveCompute);
    }
    for route in [
        runtime_governance::RUNTIME_ABI_HASH,
        runtime_governance::GOV_FINALIZE,
    ] {
        assert_eq!(route.authentication(), AuthenticationPolicy::ToriiDefault);
        assert_eq!(route.admission(), AdmissionPolicy::Public);
        assert_eq!(route.effect(), RouteEffect::ReadOnly);
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
