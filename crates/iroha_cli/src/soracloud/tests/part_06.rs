#[test]
fn app_deploy_uses_app_level_infra_endpoint_when_available() {
    let (dir, _) = single_api_fixture("app_deploy_app_infra_endpoint");
    let manifest_path = dir.join("app_manifest.json");
    let mut manifest: SoracloudAppManifestV1 = load_json(&manifest_path).expect("app manifest");
    manifest.static_site = None;
    write_json(&manifest_path, &manifest).expect("write app manifest without static site");
    fs::create_dir_all(dir.join("services/api/build")).expect("create api build dir");
    fs::write(
        dir.join("services/api/build/api-service.to"),
        b"app-infra-deploy-bundle",
    )
    .expect("write api bundle");
    sync_test_app_manifests(&dir);
    let key_pair = soracloud_fixture_key_pair(0x45);
    let authority = AccountId::new(key_pair.public_key().clone());
    let draft_response = mock_soracloud_draft_response(&authority, &key_pair);
    let status_payload = mock_control_plane_status_payload(&["travel-ops_api"]);
    let server = mock_bundle_mutation_server(
        &dir,
        "/v1/soracloud/apps/deploy",
        &draft_response,
        &status_payload,
        "encode pin registration response",
        "encode app deploy draft response",
    );
    install_mock_submission_config(&authority, &key_pair);
    let output = AppReleaseMutationArgs {
        manifest: manifest_path,
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        inrou_preseed_receipt: None,
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(MutationMode::Deploy, &authority, &key_pair)
    .expect("app deploy should use app infra endpoint");
    assert!(output.app_infra_manifest_hash.is_some());
    assert!(output.app_infra_response.is_some());
    assert!(
        output
            .notes
            .iter()
            .any(|note| { note.contains("canonical app-level Soracloud infra request") })
    );
    let requests = server.requests();
    assert!(requests.iter().any(|request| {
        request.method == "POST" && request.path == "/v1/soracloud/apps/deploy"
    }));
    assert!(
        !requests
            .iter()
            .any(|request| { request.method == "POST" && request.path == "/v1/soracloud/deploy" })
    );
    let app_request = requests
        .iter()
        .find(|request| request.path == "/v1/soracloud/apps/deploy")
        .expect("app infra deploy request should be captured");
    let body: norito::json::Value =
        json::from_slice(&app_request.body).expect("decode app deploy request");
    assert_eq!(
        body.get("deploy_services")
            .and_then(norito::json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        body.get("manifest")
            .and_then(|manifest| manifest.get("app_version"))
            .and_then(norito::json::Value::as_str),
        Some("1.0.0")
    );
}
#[test]
fn app_deploy_split_app_cid_only_skips_reserved_static_site_config() {
    let (dir, _) = split_app_fixture("split_app_cid_only_deploy");
    fs::create_dir_all(dir.join("frontend/dist")).expect("create frontend dist dir");
    fs::write(
        dir.join("frontend/dist/index.html"),
        "<!doctype html><title>Travel Ops Split</title>",
    )
    .expect("write frontend index");
    fs::create_dir_all(dir.join("services/live/build")).expect("create live build dir");
    fs::create_dir_all(dir.join("services/vault/build")).expect("create vault build dir");
    write_test_inrou_guest_images(&dir.join("services/live/inrou"), "deploy");
    fs::write(
        dir.join("services/live/build/live-api.tgz"),
        b"deploy-live-bundle",
    )
    .expect("write live bundle");
    fs::write(
        dir.join("services/vault/build/vault-api.to"),
        b"deploy-vault-bundle",
    )
    .expect("write vault bundle");
    fs::write(dir.join("build-and-sync.sh"), "#!/bin/sh\nexit 0\n")
        .expect("retain prebuilt exact app fixture artifacts");
    let status_payload =
        mock_control_plane_status_payload(&["travel-ops_live", "travel-ops_vault"]);
    let key_pair = soracloud_fixture_key_pair(0x46);
    let authority = AccountId::new(key_pair.public_key().clone());
    let draft_response = mock_soracloud_draft_response(&authority, &key_pair);
    let server = mock_bundle_mutation_server(
        &dir,
        "/v1/soracloud/apps/deploy",
        &draft_response,
        &status_payload,
        "encode pin register response",
        "encode deploy draft response",
    );
    install_mock_submission_config(&authority, &key_pair);
    let inrou_preseed_receipt = qualify_test_inrou_app(&dir, &key_pair, "deploy");
    let output = AppReleaseMutationArgs {
        manifest: dir.join("app_manifest.json"),
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        inrou_preseed_receipt: Some(inrou_preseed_receipt),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(MutationMode::Deploy, &authority, &key_pair)
    .expect("split-app deploy should succeed");
    assert_eq!(output.hostname, "travel-ops.sora");
    assert!(output.manifest_path.ends_with("app_manifest.json"));
    assert!(output.workspace_dir.contains("split_app_cid_only_deploy"));
    assert_app_workspace_scripts(&output.workspace_scripts);
    assert!(output.has_mixed_planes);
    assert_eq!(output.hosted_http_service_count, 1);
    assert_eq!(output.deterministic_service_count, 1);
    assert_eq!(output.services.len(), 2);
    assert!(
        output
            .routes
            .iter()
            .any(|route| route.service_name == "travel-ops_live"
                && route.route_kind == "hosted_http_prefix"
                && route.path == "/api/v1")
    );
    assert!(
        output
            .routes
            .iter()
            .any(|route| route.service_name == "travel-ops_vault"
                && route.route_kind == "handler"
                && route.handler_name.as_deref() == Some("auth_me")
                && route.path == "/api/auth/me")
    );
    assert_eq!(
        output
            .static_site
            .as_ref()
            .map(|site| site.publish_mode.as_str()),
        Some(APP_STATIC_SITE_PUBLISH_MODE_CID_ONLY)
    );
    assert_eq!(
        output
            .frontend
            .as_ref()
            .map(|frontend| frontend.publish_mode.as_str()),
        Some(APP_STATIC_SITE_PUBLISH_MODE_CID_ONLY)
    );
    assert_eq!(
        output
            .frontend
            .as_ref()
            .and_then(|frontend| frontend.cid_gateway_url_template.as_deref()),
        Some("https://travel-ops.sora/sorafs/cid/<cid>")
    );
    let live = output
        .services
        .iter()
        .find(|service| service.service_name == "travel-ops_live")
        .expect("split-app deploy should retain the live service entry");
    assert_eq!(live.execution_plane, "HttpService");
    assert_eq!(live.runtime, "Inrou");
    assert_eq!(live.route_path_prefix.as_deref(), Some("/api/v1"));
    assert!(live.workspace_dir.ends_with("services/live"));
    assert!(
        live.workspace_scripts
            .dev
            .as_deref()
            .is_some_and(|path| path.ends_with("services/live/dev.sh"))
    );
    assert!(
        live.workspace_scripts
            .build
            .as_deref()
            .is_some_and(|path| path.ends_with("services/live/build.sh"))
    );
    assert!(live.workspace_scripts.verify_build.is_none());
    assert!(
        live.notes
            .iter()
            .any(|note| note.contains("hosted HttpService + Inrou"))
    );
    let vault = output
        .services
        .iter()
        .find(|service| service.service_name == "travel-ops_vault")
        .expect("split-app deploy should retain the vault service entry");
    assert_eq!(vault.execution_plane, "DeterministicService");
    assert_eq!(vault.runtime, "Ivm");
    assert_eq!(vault.route_path_prefix.as_deref(), Some("/api"));
    assert!(vault.workspace_dir.ends_with("services/vault"));
    assert_optional_path_ends_with(
        vault.workspace_scripts.verify_build.as_deref(),
        "services/vault/verify-build.sh",
    );
    let publication = output
        .published_static_site
        .as_ref()
        .expect("cid-only app should publish a static site");
    assert_eq!(publication.public_url, "https://travel-ops.sora");
    assert!(publication.content_cid.starts_with('b'));
    assert_eq!(
        publication.cid_gateway_url,
        format!(
            "https://travel-ops.sora/sorafs/cid/{}",
            publication.content_cid
        )
    );
    assert_ne!(
        publication.cid_gateway_url, publication.public_url,
        "cid-only apps must publish the frontend under the CID gateway path instead of the host root"
    );
    let deploy_request = server
        .requests()
        .into_iter()
        .find(|request| request.method == "POST" && request.path == "/v1/soracloud/apps/deploy")
        .expect("canonical app deploy request");
    let deploy_body: norito::json::Value =
        json::from_slice(&deploy_request.body).expect("decode deploy request");
    let deploy_services = deploy_body
        .get("deploy_services")
        .and_then(norito::json::Value::as_array)
        .expect("app deploy request must include deploy services");
    assert_eq!(deploy_services.len(), 2);
    for service in deploy_services {
        let initial_configs = service
            .get("initial_service_configs")
            .and_then(norito::json::Value::as_object)
            .expect("deploy request must include initial service configs");
        assert!(
            !initial_configs.contains_key(APP_STATIC_SITE_CONFIG_NAME),
            "cid-only app deploy must not inject the reserved static-site config"
        );
    }
}
#[test]
fn app_upgrade_split_app_cid_only_keeps_route_projection() {
    let (dir, _) = split_app_fixture("split_app_cid_only_upgrade");
    fs::create_dir_all(dir.join("frontend/dist")).expect("create frontend dist dir");
    fs::write(
        dir.join("frontend/dist/index.html"),
        "<!doctype html><title>Travel Ops Split</title>",
    )
    .expect("write frontend index");
    fs::create_dir_all(dir.join("services/live/build")).expect("create live build dir");
    fs::create_dir_all(dir.join("services/vault/build")).expect("create vault build dir");
    write_test_inrou_guest_images(&dir.join("services/live/inrou"), "upgrade");
    fs::write(
        dir.join("services/live/build/live-api.tgz"),
        b"upgrade-live-bundle",
    )
    .expect("write live bundle");
    fs::write(
        dir.join("services/vault/build/vault-api.to"),
        b"upgrade-vault-bundle",
    )
    .expect("write vault bundle");
    fs::write(dir.join("build-and-sync.sh"), "#!/bin/sh\nexit 0\n")
        .expect("retain prebuilt exact app fixture artifacts");
    let status_payload =
        mock_control_plane_status_payload(&["travel-ops_live", "travel-ops_vault"]);
    let key_pair = soracloud_fixture_key_pair(0x47);
    let authority = AccountId::new(key_pair.public_key().clone());
    let draft_response = mock_soracloud_draft_response(&authority, &key_pair);
    let server = mock_bundle_mutation_server(
        &dir,
        "/v1/soracloud/apps/upgrade",
        &draft_response,
        &status_payload,
        "encode pin register response",
        "encode upgrade draft response",
    );
    install_mock_submission_config(&authority, &key_pair);
    let inrou_preseed_receipt = qualify_test_inrou_app(&dir, &key_pair, "upgrade");
    let output = AppReleaseMutationArgs {
        manifest: dir.join("app_manifest.json"),
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        inrou_preseed_receipt: Some(inrou_preseed_receipt),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(MutationMode::Upgrade, &authority, &key_pair)
    .expect("split-app upgrade should succeed");
    assert_eq!(output.hostname, "travel-ops.sora");
    assert!(output.manifest_path.ends_with("app_manifest.json"));
    assert!(output.workspace_dir.contains("split_app_cid_only_upgrade"));
    assert_app_workspace_scripts(&output.workspace_scripts);
    assert_eq!(output.mode, "upgrade");
    assert!(output.has_mixed_planes);
    assert_eq!(output.hosted_http_service_count, 1);
    assert_eq!(output.deterministic_service_count, 1);
    assert_eq!(output.services.len(), 2);
    assert!(output.routes.iter().any(|route| {
        route.service_name == "travel-ops_live"
            && route.route_kind == "hosted_http_prefix"
            && route.path == "/api/v1"
    }));
    assert!(output.routes.iter().any(|route| {
        route.service_name == "travel-ops_vault"
            && route.route_kind == "handler"
            && route.handler_name.as_deref() == Some("auth_me")
            && route.path == "/api/auth/me"
    }));
    assert_eq!(
        output
            .frontend
            .as_ref()
            .map(|frontend| frontend.publish_mode.as_str()),
        Some(APP_STATIC_SITE_PUBLISH_MODE_CID_ONLY)
    );
    assert_eq!(
        output
            .frontend
            .as_ref()
            .and_then(|frontend| frontend.cid_gateway_url_template.as_deref()),
        Some("https://travel-ops.sora/sorafs/cid/<cid>")
    );
    let publication = output
        .published_static_site
        .as_ref()
        .expect("cid-only app should publish a static site");
    assert!(publication.content_cid.starts_with('b'));
    let upgrade_request = server
        .requests()
        .into_iter()
        .find(|request| request.method == "POST" && request.path == "/v1/soracloud/apps/upgrade")
        .expect("canonical app upgrade request");
    let request: SignedAppInfraRequest =
        json::from_slice(&upgrade_request.body).expect("decode exact signed app upgrade");
    assert!(matches!(
        request.precondition,
        SoraAppInfraMutationPreconditionV1::ExactCurrentRevision(prior)
            if prior.app_version == "0.9.0" && prior.revision_count == 1
    ));
    assert!(request.upgrade_services.iter().all(|service| matches!(
        &service.precondition,
        SoraServiceMutationPreconditionV1::ExactCurrentRevision(prior)
            if prior.service_version == "0.9.0" && prior.process_generation == 1
    )));
    let upgrade_body: norito::json::Value =
        json::from_slice(&upgrade_request.body).expect("decode upgrade request");
    let upgrade_services = upgrade_body
        .get("upgrade_services")
        .and_then(norito::json::Value::as_array)
        .expect("app upgrade request must include upgrade services");
    assert_eq!(upgrade_services.len(), 2);
    for service in upgrade_services {
        let initial_configs = service
            .get("initial_service_configs")
            .and_then(norito::json::Value::as_object)
            .expect("upgrade request must include initial service configs");
        assert!(
            !initial_configs.contains_key(APP_STATIC_SITE_CONFIG_NAME),
            "cid-only app upgrade must not inject the reserved static-site config"
        );
    }
}
generated_auth_harness_test!(
    generated_webapp_auth_startup_fails_on_weak_session_key_in_strict_mode,
    6
);
generated_auth_harness_test!(generated_webapp_auth_startup_fails_on_invalid_auth_mode, 7);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_weak_session_key_in_strict_mode,
    "strict_key"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_invalid_auth_mode,
    "invalid_mode"
);
generated_auth_harness_test!(
    generated_webapp_auth_startup_fails_when_external_state_is_required_without_adapter,
    8
);
generated_auth_harness_test!(
    generated_webapp_auth_startup_fails_when_external_state_is_defaulted_without_adapter,
    9
);
generated_auth_harness_test!(
    generated_webapp_auth_startup_fails_when_production_disables_external_state_requirement,
    10
);
generated_auth_harness_test!(
    generated_webapp_auth_startup_fails_with_invalid_external_state_adapter_shape,
    11
);
generated_auth_harness_test!(
    generated_webapp_auth_external_state_adapter_path_mints_sessions_without_file_fallback,
    12
);
generated_auth_harness_test!(
    generated_pii_app_auth_startup_fails_when_external_state_is_required_without_adapter,
    13
);
generated_auth_harness_test!(
    generated_pii_app_auth_startup_fails_when_external_state_is_defaulted_without_adapter,
    14
);
generated_auth_harness_test!(
    generated_pii_app_auth_startup_fails_when_production_disables_external_state_requirement,
    15
);
generated_auth_harness_test!(
    generated_pii_app_auth_startup_fails_with_invalid_external_state_adapter_shape,
    16
);
generated_auth_harness_test!(
    generated_pii_app_auth_external_state_adapter_path_mints_sessions_without_file_fallback,
    17
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_without_capability_map,
    "missing_capability_map"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_invalid_capability_map_json,
    "invalid_capability_map"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_empty_capability_map_object,
    "empty_capability_map_object"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_non_object_capability_map,
    "non_object_capability_map"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_invalid_capability_map_principal,
    "invalid_capability_map_principal"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_empty_capability_array,
    "empty_capability_array"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_non_array_capability_value,
    "non_array_capability_value"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_non_string_capability,
    "non_string_capability"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_blank_capability,
    "blank_capability"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_invalid_public_base_url,
    "invalid_public_base_url"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_invalid_session_ttl,
    "invalid_session_ttl"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_non_numeric_session_ttl,
    "non_numeric_session_ttl"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_invalid_challenge_ttl,
    "invalid_challenge_ttl"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_challenge_ttl_above_max,
    "challenge_ttl_above_max"
);
pii_startup_failure_test!(
    generated_pii_app_auth_startup_fails_on_invalid_external_state_boolean,
    "invalid_external_state_boolean"
);
generated_auth_harness_test!(
    generated_webapp_private_route_requires_non_empty_capability_map,
    18
);
generated_auth_harness_test!(
    generated_webapp_auth_smoke_rejects_replay_and_supports_shared_sessions,
    19
);
generated_auth_harness_test!(
    generated_webapp_auth_replay_lock_contention_is_fail_closed,
    20
);
generated_auth_harness_test!(
    generated_pii_app_auth_replay_lock_contention_is_fail_closed,
    21
);
generated_auth_harness_test!(generated_webapp_auth_smoke_rejects_origin_mismatch, 22);
generated_auth_harness_test!(generated_pii_app_auth_smoke_rejects_origin_mismatch, 23);
generated_auth_harness_test!(
    generated_pii_app_auth_smoke_enforces_capability_authorization,
    24
);
#[test]
fn generated_pii_app_auth_core_normalizes_capabilities_and_parses_success_values() {
    run_generated_pii_app_auth_core_harness(
        "pii_auth_core_success_values",
        "pii_auth_core_success_values.mjs",
        &[
            ("AUTH_CHALLENGE_TTL_SECS", "5"),
            ("AUTH_REQUIRE_EXTERNAL_SHARED_STATE", "off"),
            ("AUTH_SESSION_TTL_SECS", "86400"),
            ("PUBLIC_BASE_URL", ""),
        ],
        &[
            "normalizedCapabilities.sort()",
            "Array.from(new Set(normalizedCapabilities))",
            "parseBooleanEnv",
            "parsePositiveIntEnv",
            "parsePublicOrigin",
        ],
        STATIC_ASSETS_V1[12],
    );
}
#[test]
fn generated_pii_app_auth_core_persists_file_state_canonically() {
    run_generated_pii_app_auth_core_harness(
        "pii_auth_core_file_state",
        "pii_auth_core_file_state.mjs",
        &[],
        &[
            "stableJsonStringify",
            "readAuthStateSnapshot",
            "statePutIfAbsent",
            "stateEntries",
        ],
        STATIC_ASSETS_V1[13],
    );
}
#[test]
fn generated_pii_app_auth_core_uses_and_validates_shared_state_adapter() {
    run_generated_pii_app_auth_core_harness_with_setup(
        "pii_auth_core_shared_adapter",
        "pii_auth_core_shared_adapter.mjs",
        &[("AUTH_REQUIRE_EXTERNAL_SHARED_STATE", "1")],
        &[
            "SHARED_STATE_ADAPTER",
            "shared state adapter putIfAbsent(key, value) must return boolean",
            "shared state adapter entries(prefix) must return [key, value][]",
        ],
        STATIC_ASSETS_V1[14],
        STATIC_ASSETS_V1[15],
    );
}
#[test]
fn generated_pii_app_auth_core_handles_session_tokens_cookies_and_origins() {
    run_generated_pii_app_auth_core_harness(
        "pii_auth_core_session_cookie",
        "pii_auth_core_session_cookie.mjs",
        &[("PUBLIC_BASE_URL", "")],
        &[
            "parseCookies",
            "requestOrigin",
            "buildSetCookieHeader",
            "getSessionFromRequest",
        ],
        STATIC_ASSETS_V1[16],
    );
}
#[test]
fn generated_pii_app_auth_core_cleans_expired_records_and_manages_consume_locks() {
    run_generated_pii_app_auth_core_harness(
        "pii_auth_core_cleanup_locks",
        "pii_auth_core_cleanup_locks.mjs",
        &[],
        &[
            "cleanupExpiredAuthRecords",
            "acquireChallengeConsumeLock",
            "releaseChallengeConsumeLock",
            "challengeExpiredStateKey",
        ],
        STATIC_ASSETS_V1[17],
    );
}
#[test]
fn generated_pii_app_auth_core_handlers_reject_login_auth_failures() {
    run_generated_pii_app_auth_core_harness(
        "pii_auth_core_login_failures",
        "pii_auth_core_login_failures.mjs",
        &[("PUBLIC_BASE_URL", "")],
        &[
            "AUTH_CHALLENGE_EXPIRED",
            "AUTH_CHALLENGE_NOT_FOUND",
            "AUTH_CHALLENGE_PRINCIPAL_MISMATCH",
            "AUTH_ORIGIN_MISMATCH",
            "AUTH_SIGNATURE_INVALID",
        ],
        STATIC_ASSETS_V1[18],
    );
}
#[test]
fn generated_pii_app_auth_core_handlers_complete_login_me_and_logout() {
    run_generated_pii_app_auth_core_harness(
        "pii_auth_core_handlers_success",
        "pii_auth_core_handlers_success.mjs",
        &[("PUBLIC_BASE_URL", "")],
        &[
            "handleAuthChallenge",
            "handleAuthLogin",
            "handleAuthMe",
            "handleAuthLogout",
        ],
        STATIC_ASSETS_V1[19],
    );
}
#[test]
fn generated_pii_app_auth_core_handlers_reject_bad_request_bodies() {
    run_generated_pii_app_auth_core_harness(
        "pii_auth_core_handlers_bad_requests",
        "pii_auth_core_handlers_bad_requests.mjs",
        &[],
        &["readJson", "sendAuthError", "INVALID_REQUEST"],
        STATIC_ASSETS_V1[20],
    );
}
#[test]
fn static_asset_bytes_order_and_reconstruction_are_stable() {
    use sha2::{Digest as _, Sha256};
    let mut digest = Sha256::new();
    for asset in [WEBAPP_API_TAIL_V1, PII_API_TAIL_V1] {
        digest.update(Sha256::digest(asset.as_bytes()));
        digest.update([u8::from(asset.ends_with('\n'))]);
    }
    for asset in STATIC_ASSETS_V1.into_iter().chain(TEST_HARNESSES_V1) {
        digest.update(Sha256::digest(asset.as_bytes()));
        digest.update([u8::from(asset.ends_with('\n'))]);
    }
    assert_eq!(
        hex::encode(digest.finalize()),
        "3f7bc8344d679c13a5138ed6149c9f86cf2ac486b1c2f16cb160900f82ae9774"
    );
    assert_eq!(
        webapp_api_server_mjs().strip_prefix(soracloud_auth_core_mjs()),
        Some(WEBAPP_API_TAIL_V1)
    );
    assert_eq!(
        pii_app_api_server_mjs().strip_prefix(soracloud_auth_core_mjs()),
        Some(PII_API_TAIL_V1)
    );
}
include!("../generated_auth_tail_tests.rs");
