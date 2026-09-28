#[test]
fn upgrade_returns_manifest_backed_service_projection() {
    let (dir, _) = service_fixture("upgrade_service_projection", InitTemplate::HttpService);
    let bundle_file = prepare_http_service_bundle(&dir, "upgrade");
    let key_pair = soracloud_fixture_key_pair(0x3F);
    let authority = AccountId::new(key_pair.public_key().clone());
    let upgrade_response = mock_soracloud_draft_response(&authority, &key_pair);
    let status_payload = mock_control_plane_status_payload(&["echo_console"]);
    let server = mock_bundle_mutation_server(
        &dir,
        "/v1/soracloud/upgrade",
        &upgrade_response,
        &status_payload,
        "encode public discovery pin register response",
        "encode upgrade response",
    );
    install_mock_submission_config(&authority, &key_pair);
    let inrou_preseed_receipt =
        qualify_test_inrou_service(&dir, &bundle_file, &key_pair, "upgrade");
    let output = UpgradeArgs {
        container: dir.join("container_manifest.json"),
        service: dir.join("service_manifest.json"),
        bundle_file,
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        initial_configs: None,
        initial_secrets: None,
        inrou_preseed_receipt: Some(inrou_preseed_receipt),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(MutationMode::Upgrade, &authority, &key_pair)
    .expect("upgrade should succeed");
    assert_eq!(output.service_name, "echo_console");
    assert_eq!(output.mode, "Upgrade");
    assert_eq!(output.execution_plane, "HttpService");
    assert_eq!(output.runtime, "Inrou");
    assert_eq!(output.route_path_prefix.as_deref(), Some("/api/v1"));
    assert_eq!(output.lease_volume_count, 2);
    assert_eq!(output.torii_url, server.base_url);
    assert!(!output.uses_api_token);
    assert_eq!(output.published_inrou_guest_images.len(), 2);
    assert!(output.workspace_dir.contains("upgrade_service_projection"));
    assert_optional_path_ends_with(output.workspace_scripts.upgrade.as_deref(), "upgrade.sh");
    assert!(
        output
            .routes
            .iter()
            .any(|route| route.route_kind == "hosted_http_prefix" && route.path == "/api/v1")
    );
    assert_eq!(
        output
            .response
            .get("service_name")
            .and_then(norito::json::Value::as_str),
        Some("echo_console")
    );
    assert_eq!(
        output
            .response
            .get("current_version")
            .and_then(norito::json::Value::as_str),
        Some("1.0.0")
    );
    let upgrade_request = server
        .requests()
        .into_iter()
        .find(|request| request.method == "POST" && request.path == "/v1/soracloud/upgrade")
        .expect("capture exact upgrade request");
    let request: SignedBundleRequest =
        json::from_slice(&upgrade_request.body).expect("decode exact signed upgrade");
    assert!(matches!(
        request.precondition,
        SoraServiceMutationPreconditionV1::ExactCurrentRevision(prior)
            if prior.service_version == "0.9.0" && prior.process_generation == 1
    ));
    assert_notes_contain(&output.notes, "live Torii status");
}
#[test]
fn init_http_service_template_scaffolds_inrou_service() {
    let (dir, output) = named_service_fixture(
        "http_service_template",
        "live_search",
        InitTemplate::HttpService,
    );
    assert_eq!(output.template, "http-service");
    assert_scaffold_file_contract(&dir, "init_http_service_template_scaffolds_inrou_service");
    let container: UnpublishedContainerManifestV1 =
        load_json(&dir.join("container_manifest.json")).expect("container manifest");
    assert_eq!(container.runtime, SoraContainerRuntimeV1::Inrou);
    assert_eq!(container.bundle_path, "/app/server.mjs");
    assert_eq!(
        container.capabilities.network,
        SoraNetworkPolicyV1::Isolated
    );
    assert!(
        container
            .inrou
            .as_ref()
            .expect("Inrou manifest")
            .guest_images
            .values()
            .all(|image| {
                norito::json::to_value(image)
                    .expect("serialize unpublished guest image")
                    .get("published_artifact")
                    .is_some_and(Value::is_null)
            })
    );
    let service: SoraServiceManifestV1 =
        load_json(&dir.join("service_manifest.json")).expect("service manifest");
    assert_eq!(
        service.execution_plane,
        SoraServiceExecutionPlaneV1::HttpService
    );
    assert_eq!(
        service
            .route
            .as_ref()
            .map(|route| route.path_prefix.as_str()),
        Some("/api/v1")
    );
    assert_eq!(service.replicas.get(), 1);
    assert!(service.handlers.is_empty());
    assert_eq!(service.lease_volumes.len(), 2);
    assert_eq!(
        service.lease_volumes[0].kind,
        SoraLeaseVolumeKindV1::PersistentRootLeaseVolume
    );
    assert_eq!(service.lease_volumes[1].volume_name.as_ref(), "app_data");
}
#[test]
fn generated_http_service_scaffold_smoke_serves_health_and_echo() {
    let (dir, _) = service_fixture("http_service_smoke", InitTemplate::HttpService);
    let server_path = dir.join("http-service/app/server.mjs");
    if !node_available() {
        let server = fs::read_to_string(&server_path).expect("read http-service server");
        assert!(server.contains("/echo"));
        assert!(server.contains("SORACLOUD_LEASE_VOLUME_APP_DATA_DIR"));
        return;
    }
    let harness_path = dir.join("http_service_smoke.mjs");
    let app_data_dir = dir.join("app-data");
    let mut script = TEST_HARNESSES_V1[2].to_owned();
    script = script.replace("__SERVER_PATH__", &js_string_literal(&server_path));
    script = script.replace("__APP_DATA_DIR__", &js_string_literal(&app_data_dir));
    fs::write(&harness_path, script).expect("write http-service smoke harness");
    run_node_harness(&harness_path);
}
#[test]
fn local_plan_http_service_reports_workspace_scripts_and_hosted_runtime() {
    let (dir, _) = service_fixture("http_service_local_plan", InitTemplate::HttpService);
    let output = LocalPlanArgs {
        container: dir.join("container_manifest.json"),
        service: dir.join("service_manifest.json"),
    }
    .run()
    .expect("plan should succeed");
    assert_eq!(output.service_name, "echo_console");
    assert_eq!(output.execution_plane, "HttpService");
    assert_eq!(output.runtime, "Inrou");
    assert_eq!(output.route_path_prefix.as_deref(), Some("/api/v1"));
    assert_eq!(output.replica_count, 1);
    assert_eq!(output.lease_volume_count, 2);
    assert_eq!(output.handler_count, 0);
    assert!(output.workspace_dir.contains("http_service_local_plan"));
    assert_service_workspace_scripts(&output.workspace_scripts);
    assert!(
        output
            .routes
            .iter()
            .any(|route| route.route_kind == "hosted_http_prefix" && route.path == "/api/v1")
    );
    assert_notes_contain(&output.notes, "hosted HttpService + Inrou");
}
#[test]
fn local_plan_single_api_service_reports_deterministic_handler_routes() {
    let (dir, _) = single_api_fixture("single_api_service_local_plan");
    let output = LocalPlanArgs {
        container: dir.join("services/api/container_manifest.json"),
        service: dir.join("services/api/service_manifest.json"),
    }
    .run()
    .expect("plan should succeed");
    assert_eq!(output.service_name, "travel-ops_api");
    assert_eq!(output.execution_plane, "DeterministicService");
    assert_eq!(output.runtime, "Ivm");
    assert_eq!(output.route_path_prefix.as_deref(), Some("/api"));
    assert_eq!(output.handler_count, 1);
    assert_eq!(output.state_binding_count, 0);
    assert_eq!(output.lease_volume_count, 0);
    assert!(output.workspace_dir.contains("services/api"));
    assert_optional_path_ends_with(output.workspace_scripts.local_dev.as_deref(), "dev.sh");
    assert_eq!(output.workspace_scripts.build_and_sync, None);
    assert_eq!(output.workspace_scripts.deploy, None);
    assert_eq!(output.workspace_scripts.upgrade, None);
    assert!(
        output
            .routes
            .iter()
            .any(|route| route.route_kind == "service_prefix" && route.path == "/api")
    );
    assert!(output.routes.iter().any(|route| {
        route.route_kind == "handler"
            && route.handler_name.as_deref() == Some("healthz")
            && route.path == "/api/healthz"
            && route.handler_class.as_deref() == Some("Query")
    }));
    assert_notes_contain(&output.notes, "deterministic IVM");
}
#[test]
fn local_dev_http_service_dry_run_reports_manifest_adjacent_script() {
    let (dir, _) = service_fixture("http_service_local_dev_dry_run", InitTemplate::HttpService);
    let output = LocalDevArgs {
        container: dir.join("container_manifest.json"),
        service: dir.join("service_manifest.json"),
        dry_run: true,
    }
    .run()
    .expect("dev dry-run should succeed");
    assert_eq!(output.mode, "dry_run");
    assert_eq!(output.execution_plane, "HttpService");
    assert_eq!(output.runtime, "Inrou");
    assert_eq!(output.command, vec!["./dev.sh".to_owned()]);
    assert!(output.script_path.ends_with("dev.sh"));
    assert!(
        output
            .working_dir
            .contains("http_service_local_dev_dry_run")
    );
    assert_eq!(output.route_path_prefix.as_deref(), Some("/api/v1"));
    assert_eq!(output.replica_count, 1);
    assert_eq!(output.lease_volume_count, 2);
    assert_optional_path_ends_with(output.workspace_scripts.local_dev.as_deref(), "dev.sh");
    assert!(
        output
            .routes
            .iter()
            .any(|route| route.route_kind == "hosted_http_prefix" && route.path == "/api/v1")
    );
    assert_notes_contain(&output.notes, "hosted HttpService + Inrou");
}
#[test]
fn local_dev_http_service_executes_manifest_adjacent_script() {
    if !bash_available() {
        return;
    }
    let (dir, _) = service_fixture("http_service_local_dev_run", InitTemplate::HttpService);
    let local_dev_script = dir.join("dev.sh");
    fs::write(&local_dev_script, STATIC_ASSETS_V1[0]).expect("write dev script");
    mark_template_file_executable(&local_dev_script).expect("mark dev executable");
    let output = LocalDevArgs {
        container: dir.join("container_manifest.json"),
        service: dir.join("service_manifest.json"),
        dry_run: false,
    }
    .run()
    .expect("dev execution should succeed");
    assert_eq!(output.mode, "completed");
    assert_eq!(output.execution_plane, "HttpService");
    assert_eq!(output.runtime, "Inrou");
    assert_eq!(output.route_path_prefix.as_deref(), Some("/api/v1"));
    assert_optional_path_ends_with(
        output.workspace_scripts.build_and_sync.as_deref(),
        "build-and-sync.sh",
    );
    assert_eq!(output.command, vec!["./dev.sh".to_owned()]);
    assert!(
        dir.join("http-service-dev-ran.txt").exists(),
        "dev command should run the manifest-adjacent script"
    );
}
#[test]
fn local_dev_http_service_treats_interrupt_exit_as_successful_session_end() {
    if !bash_available() {
        return;
    }
    let (dir, _) = service_fixture(
        "http_service_local_dev_interrupt",
        InitTemplate::HttpService,
    );
    let local_dev_script = dir.join("dev.sh");
    fs::write(&local_dev_script, STATIC_ASSETS_V1[1]).expect("write interrupting dev script");
    mark_template_file_executable(&local_dev_script).expect("mark dev executable");
    let output = LocalDevArgs {
        container: dir.join("container_manifest.json"),
        service: dir.join("service_manifest.json"),
        dry_run: false,
    }
    .run()
    .expect("interrupt status 130 should be treated as a successful dev stop");
    assert_eq!(output.mode, "interrupted");
    assert_eq!(output.exit_status, Some(130));
    assert_eq!(output.route_path_prefix.as_deref(), Some("/api/v1"));
    assert_optional_path_ends_with(output.workspace_scripts.upgrade.as_deref(), "upgrade.sh");
    assert_notes_contain(&output.notes, "interactive interrupt");
}
#[test]
fn build_and_sync_http_service_executes_manifest_adjacent_script() {
    if !bash_available() {
        return;
    }
    let (dir, _) = service_fixture("http_service_build_and_sync_run", InitTemplate::HttpService);
    let build_and_sync_script = dir.join("build-and-sync.sh");
    fs::write(&build_and_sync_script, STATIC_ASSETS_V1[2]).expect("write build-and-sync script");
    mark_template_file_executable(&build_and_sync_script).expect("mark build-and-sync executable");
    let output = BuildAndSyncArgs {
        container: dir.join("container_manifest.json"),
        service: dir.join("service_manifest.json"),
        dry_run: false,
    }
    .run()
    .expect("build-and-sync execution should succeed");
    assert_eq!(output.mode, "completed");
    assert_eq!(output.execution_plane, "HttpService");
    assert_eq!(output.runtime, "Inrou");
    assert_eq!(output.route_path_prefix.as_deref(), Some("/api/v1"));
    assert_eq!(output.lease_volume_count, 2);
    assert_optional_path_ends_with(output.workspace_scripts.deploy.as_deref(), "deploy.sh");
    assert_eq!(output.command, vec!["./build-and-sync.sh".to_owned()]);
    assert!(
        dir.join("http-service-build-and-sync-ran.txt").exists(),
        "build-and-sync command should run the manifest-adjacent script"
    );
    assert_notes_contain(&output.notes, "build-and-sync completed");
}
#[test]
fn deploy_workspace_http_service_dry_run_reports_manifest_adjacent_script() {
    let (dir, _) = service_fixture(
        "http_service_deploy_workspace_dry_run",
        InitTemplate::HttpService,
    );
    let output = WorkspaceMutationArgs {
        container: dir.join("container_manifest.json"),
        service: dir.join("service_manifest.json"),
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        initial_configs: None,
        initial_secrets: None,
        torii_url: Some("http://127.0.0.1:8080".to_owned()),
        api_token: Some("token".to_owned()),
        timeout_secs: 37,
        dry_run: true,
    }
    .run(MutationMode::Deploy)
    .expect("deploy dry-run should succeed");
    assert_eq!(output.mode, "dry_run");
    assert_eq!(output.script_name, "deploy.sh");
    assert_eq!(output.execution_plane, "HttpService");
    assert_eq!(output.runtime, "Inrou");
    assert_eq!(output.torii_url, "http://127.0.0.1:8080");
    assert!(output.uses_api_token);
    assert_eq!(output.route_path_prefix.as_deref(), Some("/api/v1"));
    assert_eq!(output.state_binding_count, 0);
    assert_eq!(output.lease_volume_count, 2);
    assert_optional_path_ends_with(output.workspace_scripts.deploy.as_deref(), "deploy.sh");
    assert_eq!(
        output.command,
        vec![
            "./deploy.sh".to_owned(),
            "--timeout-secs".to_owned(),
            "37".to_owned(),
        ]
    );
    assert!(output.script_path.ends_with("deploy.sh"));
    assert_notes_contain(&output.notes, "exact SoraFS retention epoch 2000000000");
}
#[test]
fn deploy_workspace_http_service_executes_manifest_adjacent_script() {
    if !bash_available() {
        return;
    }
    let (dir, _) = service_fixture(
        "http_service_deploy_workspace_run",
        InitTemplate::HttpService,
    );
    let configs_path = dir.join("materials").join("configs.json");
    let secrets_path = dir.join("materials").join("secrets.json");
    fs::create_dir_all(configs_path.parent().expect("materials parent"))
        .expect("create materials dir");
    fs::write(&configs_path, "{}").expect("write configs");
    fs::write(&secrets_path, "{}").expect("write secrets");
    let resolved_configs = fs::canonicalize(&configs_path).expect("canonicalize configs");
    let resolved_secrets = fs::canonicalize(&secrets_path).expect("canonicalize secrets");
    let deploy_script = dir.join("deploy.sh");
    fs::write(&deploy_script, STATIC_ASSETS_V1[3]).expect("write deploy script");
    mark_template_file_executable(&deploy_script).expect("mark deploy executable");
    let output = WorkspaceMutationArgs {
        container: dir.join("container_manifest.json"),
        service: dir.join("service_manifest.json"),
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        initial_configs: Some(configs_path),
        initial_secrets: Some(secrets_path),
        torii_url: Some("http://127.0.0.1:8080".to_owned()),
        api_token: Some("top-secret".to_owned()),
        timeout_secs: 27,
        dry_run: false,
    }
    .run(MutationMode::Deploy)
    .expect("deploy execution should succeed");
    assert_eq!(output.mode, "completed");
    assert_eq!(output.exit_status, Some(0));
    assert_eq!(output.script_name, "deploy.sh");
    assert_eq!(output.execution_plane, "HttpService");
    assert_eq!(output.runtime, "Inrou");
    assert_eq!(output.torii_url, "http://127.0.0.1:8080");
    assert!(output.uses_api_token);
    assert_eq!(output.route_path_prefix.as_deref(), Some("/api/v1"));
    assert_optional_path_ends_with(output.workspace_scripts.local_dev.as_deref(), "dev.sh");
    assert_eq!(
        fs::read_to_string(dir.join("deploy-torii.txt")).expect("read deploy torii"),
        "http://127.0.0.1:8080"
    );
    assert_eq!(
        fs::read_to_string(dir.join("deploy-token.txt")).expect("read deploy token"),
        "top-secret"
    );
    assert_eq!(
        fs::read_to_string(dir.join("deploy-retention-epoch.txt"))
            .expect("read deploy retention epoch"),
        test_sorafs_retention_epoch().to_string()
    );
    let args = fs::read_to_string(dir.join("deploy-args.txt")).expect("read deploy args");
    assert!(args.contains("--initial-configs"));
    assert!(args.contains(resolved_configs.to_string_lossy().as_ref()));
    assert!(args.contains("--initial-secrets"));
    assert!(args.contains(resolved_secrets.to_string_lossy().as_ref()));
    assert!(args.contains("--timeout-secs"));
    assert!(args.contains("27"));
    assert_notes_contain(&output.notes, "deploy completed");
}
#[test]
fn upgrade_workspace_http_service_executes_manifest_adjacent_script() {
    if !bash_available() {
        return;
    }
    let (dir, _) = service_fixture(
        "http_service_upgrade_workspace_run",
        InitTemplate::HttpService,
    );
    let upgrade_script = dir.join("upgrade.sh");
    fs::write(&upgrade_script, STATIC_ASSETS_V1[4]).expect("write upgrade script");
    mark_template_file_executable(&upgrade_script).expect("mark upgrade executable");
    let output = WorkspaceMutationArgs {
        container: dir.join("container_manifest.json"),
        service: dir.join("service_manifest.json"),
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        initial_configs: None,
        initial_secrets: None,
        torii_url: Some("http://127.0.0.1:8080".to_owned()),
        api_token: None,
        timeout_secs: 19,
        dry_run: false,
    }
    .run(MutationMode::Upgrade)
    .expect("upgrade execution should succeed");
    assert_eq!(output.mode, "completed");
    assert_eq!(output.exit_status, Some(0));
    assert_eq!(output.script_name, "upgrade.sh");
    assert_eq!(output.route_path_prefix.as_deref(), Some("/api/v1"));
    assert_optional_path_ends_with(output.workspace_scripts.upgrade.as_deref(), "upgrade.sh");
    assert_eq!(
        fs::read_to_string(dir.join("upgrade-torii.txt")).expect("read upgrade torii"),
        "http://127.0.0.1:8080"
    );
    assert_eq!(
        fs::read_to_string(dir.join("upgrade-retention-epoch.txt"))
            .expect("read upgrade retention epoch"),
        test_sorafs_retention_epoch().to_string()
    );
    let args = fs::read_to_string(dir.join("upgrade-args.txt")).expect("read upgrade args");
    assert!(args.contains("--timeout-secs"));
    assert!(args.contains("19"));
    assert_notes_contain(&output.notes, "upgrade completed");
}
#[test]
fn init_site_template_scaffolds_vue_and_sorafs_workflow() {
    let (dir, output) = service_fixture("site_template", InitTemplate::Site);
    assert_eq!(output.template, "site");
    assert!(!dir.join("registry.json").exists());
    assert!(dir.join("site/package.json").exists());
    assert!(dir.join("site/src/App.vue").exists());
    let readme = fs::read_to_string(dir.join("site/README.md")).expect("read site readme");
    assert!(readme.contains("iroha app sorafs toolkit pack"));
    assert!(readme.contains("alias-namespace soradns"));
    let container: SoraContainerManifestV1 =
        load_json(&dir.join("container_manifest.json")).expect("container manifest");
    assert_eq!(container.runtime, SoraContainerRuntimeV1::Ivm);
    let service: SoraServiceManifestV1 =
        load_json(&dir.join("service_manifest.json")).expect("service manifest");
    assert_eq!(
        service
            .route
            .as_ref()
            .map(|route| route.path_prefix.as_str()),
        Some("/")
    );
    assert_eq!(service.handlers.len(), 1);
    assert_eq!(service.handlers[0].handler_name.as_ref(), "assets");
    assert_eq!(service.handlers[0].class, SoraServiceHandlerClassV1::Asset);
    assert_eq!(service.artifacts.len(), 1);
    assert_eq!(service.artifacts[0].kind, SoraArtifactKindV1::StaticAsset);
    assert_eq!(
        service.artifacts[0].handler_name.as_ref().map(Name::as_ref),
        Some("assets")
    );
}
#[test]
fn init_webapp_template_scaffolds_frontend_and_api() {
    let (dir, output) = service_fixture("webapp_template", InitTemplate::Webapp);
    assert_eq!(output.template, "webapp");
    assert_scaffold_file_contract(&dir, "init_webapp_template_scaffolds_frontend_and_api");
    let service: SoraServiceManifestV1 =
        load_json(&dir.join("service_manifest.json")).expect("service manifest");
    assert_eq!(
        service
            .route
            .as_ref()
            .map(|route| route.path_prefix.as_str()),
        Some("/api")
    );
    assert!(
        service
            .state_bindings
            .iter()
            .any(|binding| binding.key_prefix == "/state/auth/challenges")
    );
    assert!(
        service
            .state_bindings
            .iter()
            .any(|binding| binding.key_prefix == "/state/auth/sessions")
    );
    assert_eq!(service.handlers.len(), 2);
    assert_eq!(service.handlers[0].class, SoraServiceHandlerClassV1::Query);
    assert_eq!(service.handlers[1].class, SoraServiceHandlerClassV1::Update);
    assert_eq!(
        service.handlers[1]
            .mailbox
            .as_ref()
            .map(|mailbox| mailbox.queue_name.as_ref()),
        Some("updates")
    );
    assert_eq!(service.artifacts.len(), 1);
    assert_eq!(service.artifacts[0].kind, SoraArtifactKindV1::Journal);
    assert_eq!(
        service.artifacts[0].handler_name.as_ref().map(Name::as_ref),
        Some("update")
    );
}
#[test]
fn init_pii_app_template_scaffolds_private_policy_workflows() {
    let (dir, output) = service_fixture("pii_app_template", InitTemplate::PiiApp);
    assert_eq!(output.template, "pii-app");
    assert_scaffold_file_contract(
        &dir,
        "init_pii_app_template_scaffolds_private_policy_workflows",
    );
    assert!(
        dir.join("pii-app/policy/consent_policy_template.json")
            .exists()
    );
    assert!(
        dir.join("pii-app/policy/retention_policy_template.json")
            .exists()
    );
    assert!(
        dir.join("pii-app/policy/deletion_workflow_template.json")
            .exists()
    );
    let service: SoraServiceManifestV1 =
        load_json(&dir.join("service_manifest.json")).expect("service manifest");
    assert_eq!(
        service
            .route
            .as_ref()
            .map(|route| route.path_prefix.as_str()),
        Some("/pii/api")
    );
    assert!(
        service.state_bindings.iter().any(|binding| {
            binding.binding_name.as_ref() == "pii_records"
                && binding.encryption == SoraStateEncryptionV1::FheCiphertext
                && binding.key_prefix == "/state/pii/records"
        }),
        "pii_records private binding missing from pii-app template"
    );
    assert!(
        service.state_bindings.iter().any(|binding| {
            binding.binding_name.as_ref() == "pii_consent_events"
                && binding.key_prefix == "/state/pii/consent"
        }),
        "pii_consent_events binding missing from pii-app template"
    );
    assert!(
        service
            .state_bindings
            .iter()
            .any(|binding| binding.key_prefix == "/state/auth/challenges"),
        "auth challenge shared binding missing from pii-app template"
    );
    assert!(
        service
            .state_bindings
            .iter()
            .any(|binding| binding.key_prefix == "/state/auth/sessions"),
        "auth session shared binding missing from pii-app template"
    );
    assert_eq!(service.handlers.len(), 3);
    assert_eq!(service.handlers[0].class, SoraServiceHandlerClassV1::Query);
    assert_eq!(service.handlers[1].class, SoraServiceHandlerClassV1::Update);
    assert_eq!(
        service.handlers[1]
            .mailbox
            .as_ref()
            .map(|mailbox| mailbox.queue_name.as_ref()),
        Some("updates")
    );
    assert_eq!(service.handlers[2].class, SoraServiceHandlerClassV1::Update);
    assert_eq!(
        service.handlers[2]
            .mailbox
            .as_ref()
            .map(|mailbox| mailbox.queue_name.as_ref()),
        Some("ciphertext_updates")
    );
    assert_eq!(service.artifacts.len(), 2);
    assert_eq!(service.artifacts[0].kind, SoraArtifactKindV1::Journal);
    assert_eq!(service.artifacts[1].kind, SoraArtifactKindV1::Checkpoint);
}
#[test]
fn app_init_single_api_template_scaffolds_admissible_root_binding_service() {
    let (dir, output) = single_api_fixture("single_api_template");
    assert_eq!(output.template, "single-api");
    assert_scaffold_file_contract(
        &dir,
        "app_init_single_api_template_scaffolds_admissible_root_binding_service",
    );
    assert!(
        output
            .template_artifacts
            .iter()
            .any(|path| path.ends_with("services/api/build.sh"))
    );
    let manifest: SoracloudAppManifestV1 =
        load_json(&dir.join("app_manifest.json")).expect("app manifest");
    assert_eq!(
        manifest
            .static_site
            .as_ref()
            .map(|site| site.publish_mode.as_str()),
        Some(APP_STATIC_SITE_PUBLISH_MODE_ROOT_BINDING)
    );
    assert_eq!(manifest.services.len(), 1);
    assert_eq!(
        manifest.services[0].bundle_file.as_deref(),
        Some("services/api/build/api-service.to")
    );
    let container: SoraContainerManifestV1 =
        load_json(&dir.join("services/api/container_manifest.json")).expect("container");
    let service: SoraServiceManifestV1 =
        load_json(&dir.join("services/api/service_manifest.json")).expect("service");
    assert_eq!(container.runtime, SoraContainerRuntimeV1::Ivm);
    assert_eq!(
        service.execution_plane,
        SoraServiceExecutionPlaneV1::DeterministicService
    );
    assert_eq!(
        service
            .route
            .as_ref()
            .map(|route| route.path_prefix.as_str()),
        Some("/api")
    );
    assert_eq!(service.handlers.len(), 1);
    assert_eq!(service.handlers[0].handler_name.as_ref(), "healthz");
    assert_eq!(service.handlers[0].class, SoraServiceHandlerClassV1::Query);
    assert_eq!(service.handlers[0].route_path.as_deref(), Some("/healthz"));
    assert_eq!(
        service.handlers[0].certified_response,
        SoraCertifiedResponsePolicyV1::AuditReceipt
    );
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    bundle
        .validate_for_admission()
        .expect("single-api scaffold should be admissible");
}
#[test]
fn generated_single_api_dev_server_smoke_serves_healthz() {
    let (dir, _) = single_api_fixture("single_api_dev_smoke");
    let server_path = dir.join("services/api/dev-server.mjs");
    if !node_available() {
        let server = fs::read_to_string(&server_path).expect("read single-api dev server");
        assert!(server.contains("/api/healthz"));
        assert!(server.contains("local_dev_shim"));
        return;
    }
    let harness_path = dir.join("single_api_dev_smoke.mjs");
    let mut script = TEST_HARNESSES_V1[3].to_owned();
    script = script.replace("__SERVER_PATH__", &js_string_literal(&server_path));
    fs::write(&harness_path, script).expect("write single-api dev smoke harness");
    run_node_harness(&harness_path);
}
#[test]
fn normalized_contract_identifier_rewrites_dns_labels_into_koto_safe_names() {
    assert_eq!(normalized_contract_identifier("travel-ops"), "travel_ops");
    assert_eq!(normalized_contract_identifier("travel ops"), "travel_ops");
    assert_eq!(
        normalized_contract_identifier("9lives-api"),
        "sora_9lives_api"
    );
    assert_eq!(normalized_contract_identifier("---"), "sora_contract");
}
#[test]
fn app_init_split_app_template_scaffolds_live_and_vault_services() {
    let (dir, output) = split_app_fixture("split_app_template");
    assert_eq!(output.template, "split-app");
    assert_scaffold_file_contract(
        &dir,
        "app_init_split_app_template_scaffolds_live_and_vault_services",
    );
    let manifest: SoracloudAppManifestV1 =
        load_json(&dir.join("app_manifest.json")).expect("app manifest");
    assert_eq!(
        manifest
            .static_site
            .as_ref()
            .map(|site| site.dist_dir.as_str()),
        Some("frontend/dist")
    );
    assert_eq!(
        manifest
            .static_site
            .as_ref()
            .map(|site| site.publish_mode.as_str()),
        Some(APP_STATIC_SITE_PUBLISH_MODE_CID_ONLY)
    );
    assert_eq!(manifest.services.len(), 2);
    assert!(
        manifest
            .services
            .iter()
            .any(|service| service.bundle_file.as_deref()
                == Some("services/live/build/live-api.tgz"))
    );
    assert!(
        manifest
            .services
            .iter()
            .any(|service| service.bundle_file.as_deref()
                == Some("services/vault/build/vault-api.to"))
    );
    let live_container: UnpublishedContainerManifestV1 =
        load_json(&dir.join("services/live/container_manifest.json")).expect("live container");
    let live_service: SoraServiceManifestV1 =
        load_json(&dir.join("services/live/service_manifest.json")).expect("live service");
    assert_eq!(live_container.runtime, SoraContainerRuntimeV1::Inrou);
    assert_eq!(
        live_container.capabilities.network,
        SoraNetworkPolicyV1::Isolated
    );
    assert_eq!(
        live_service.execution_plane,
        SoraServiceExecutionPlaneV1::HttpService
    );
    assert_eq!(
        live_service
            .route
            .as_ref()
            .map(|route| route.path_prefix.as_str()),
        Some("/api/v1")
    );
    assert_eq!(live_service.replicas.get(), 1);
    assert_eq!(live_service.lease_volumes.len(), 5);
    let inrou = live_container.inrou.expect("live inrou manifest");
    assert_eq!(
        inrou
            .guest_images
            .get("x86_64")
            .expect("x86_64 guest image")
            .kernel_image_path,
        "/inrou/x86_64/vmlinux"
    );
    assert_eq!(
        inrou
            .guest_images
            .get("x86_64")
            .expect("x86_64 guest image")
            .rootfs_image_path,
        "/inrou/x86_64/rootfs.ext4"
    );
    assert_eq!(
        inrou
            .guest_images
            .get("aarch64")
            .expect("aarch64 guest image")
            .kernel_image_path,
        "/inrou/aarch64/vmlinux"
    );
    assert_eq!(
        inrou
            .guest_images
            .get("aarch64")
            .expect("aarch64 guest image")
            .rootfs_image_path,
        "/inrou/aarch64/rootfs.ext4"
    );
    let vault_container: SoraContainerManifestV1 =
        load_json(&dir.join("services/vault/container_manifest.json")).expect("vault container");
    let vault_service: SoraServiceManifestV1 =
        load_json(&dir.join("services/vault/service_manifest.json")).expect("vault service");
    assert_eq!(vault_container.runtime, SoraContainerRuntimeV1::Ivm);
    assert_eq!(
        vault_service.execution_plane,
        SoraServiceExecutionPlaneV1::DeterministicService
    );
    assert!(
        vault_service
            .state_bindings
            .iter()
            .any(|binding| binding.key_prefix == "/state/users/preferences")
    );
    assert!(
        vault_service
            .state_bindings
            .iter()
            .any(|binding| binding.key_prefix == "/state/users/saved_searches")
    );
    assert!(
        vault_service
            .handlers
            .iter()
            .any(|handler| handler.route_path.as_deref() == Some("/auth/login"))
    );
    assert!(
        vault_service
            .handlers
            .iter()
            .any(|handler| handler.route_path.as_deref() == Some("/auth/me"))
    );
    assert!(
        vault_service
            .handlers
            .iter()
            .any(|handler| handler.route_path.as_deref() == Some("/v1/user/saved-searches"))
    );
    assert!(
        !vault_service
            .handlers
            .iter()
            .any(|handler| handler.route_path.as_deref() == Some("/auth/health"))
    );
    let frontend_app =
        fs::read_to_string(dir.join("frontend/src/App.vue")).expect("read frontend app");
    assert!(
        frontend_app.contains("const apiBase = import.meta.env.VITE_PUBLIC_API_BASE ?? \"/api\";")
    );
}
#[test]
fn app_init_split_app_existing_repo_template_omits_starter_sources() {
    let dir = temp_dir("split_app_existing_repo_template");
    let output = AppInitArgs {
        existing_repo: true,
        ..app_scaffold_args(dir.clone(), "travel_ops", AppInitTemplate::SplitApp)
    }
    .run()
    .expect("split-app existing-repo init should succeed");
    assert_eq!(output.template, "split-app");
    assert_scaffold_file_contract(
        &dir,
        "app_init_split_app_existing_repo_template_omits_starter_sources",
    );
    let manifest: SoracloudAppManifestV1 =
        load_json(&dir.join("app_manifest.json")).expect("app manifest");
    assert_eq!(
        manifest
            .static_site
            .as_ref()
            .map(|site| site.dist_dir.as_str()),
        Some("frontend/dist")
    );
    assert_eq!(
        manifest.services.len(),
        2,
        "existing-repo mode must still wire both services into the app manifest"
    );
}
#[test]
fn app_init_existing_repo_rejects_non_split_templates() {
    let dir = temp_dir("single_api_existing_repo_template");
    let err = AppInitArgs {
        existing_repo: true,
        ..app_scaffold_args(dir, "travel_ops", AppInitTemplate::SingleApi)
    }
    .run()
    .expect_err("existing-repo should be rejected for single-api");
    assert!(
        err.to_string()
            .contains("--existing-repo is only supported with --template split-app")
    );
}
#[test]
fn app_init_split_app_template_accepts_public_host_and_dist_dir_overrides() {
    let dir = temp_dir("split_app_template_overrides");
    let output = AppInitArgs {
        public_host: Some("taira.sora.org".to_owned()),
        static_site_dist_dir: Some("../../apps/web/dist".to_owned()),
        ..app_scaffold_args(dir.clone(), "hayahi", AppInitTemplate::SplitApp)
    }
    .run()
    .expect("split-app init with overrides should succeed");
    assert_eq!(output.public_url, "https://taira.sora.org");
    let manifest: SoracloudAppManifestV1 =
        load_json(&dir.join("app_manifest.json")).expect("app manifest");
    assert_eq!(manifest.public_url, "https://taira.sora.org");
    assert_eq!(
        manifest
            .static_site
            .as_ref()
            .map(|site| site.dist_dir.as_str()),
        Some("../../apps/web/dist")
    );
    let live_service: SoraServiceManifestV1 =
        load_json(&dir.join("services/live/service_manifest.json")).expect("live service");
    assert_eq!(
        live_service.route.as_ref().map(|route| route.host.as_str()),
        Some("taira.sora.org")
    );
    let vault_container: SoraContainerManifestV1 =
        load_json(&dir.join("services/vault/container_manifest.json")).expect("vault container");
    assert_eq!(
        vault_container
            .env
            .get("PUBLIC_BASE_URL")
            .map(String::as_str),
        Some("https://taira.sora.org")
    );
    let vault_service: SoraServiceManifestV1 =
        load_json(&dir.join("services/vault/service_manifest.json")).expect("vault service");
    assert_eq!(
        vault_service
            .route
            .as_ref()
            .map(|route| route.host.as_str()),
        Some("taira.sora.org")
    );
}
#[test]
fn app_local_plan_split_app_reports_mixed_routes_and_cid_gateway() {
    let (dir, _) = split_app_fixture("split_app_local_plan");
    let output = AppLocalPlanArgs {
        manifest: dir.join("app_manifest.json"),
    }
    .run()
    .expect("plan should succeed");
    assert_eq!(output.app_name, "travel_ops");
    assert!(output.manifest_path.ends_with("app_manifest.json"));
    assert_eq!(output.hostname, "travel-ops.sora");
    assert!(output.has_mixed_planes);
    assert_eq!(output.hosted_http_service_count, 1);
    assert_eq!(output.deterministic_service_count, 1);
    assert!(output.workspace_dir.contains("split_app_local_plan"));
    assert_app_workspace_scripts(&output.workspace_scripts);
    assert_eq!(
        output
            .frontend
            .as_ref()
            .and_then(|frontend| frontend.cid_gateway_url_template.as_deref()),
        Some("https://travel-ops.sora/sorafs/cid/<cid>")
    );
    assert!(output.services.iter().any(|service| {
        service.service_name == "travel-ops_live"
            && service
                .container_manifest_path
                .ends_with("services/live/container_manifest.json")
            && service
                .service_manifest_path
                .ends_with("services/live/service_manifest.json")
            && service.workspace_dir.ends_with("services/live")
            && service
                .workspace_scripts
                .dev
                .as_deref()
                .is_some_and(|path| path.ends_with("services/live/dev.sh"))
            && service
                .workspace_scripts
                .build
                .as_deref()
                .is_some_and(|path| path.ends_with("services/live/build.sh"))
            && service.workspace_scripts.verify_build.is_none()
            && service.execution_plane == "HttpService"
            && service.runtime == "Inrou"
            && service.route_path_prefix.as_deref() == Some("/api/v1")
    }));
    assert!(output.services.iter().any(|service| {
        service.service_name == "travel-ops_vault"
            && service
                .container_manifest_path
                .ends_with("services/vault/container_manifest.json")
            && service
                .service_manifest_path
                .ends_with("services/vault/service_manifest.json")
            && service.workspace_dir.ends_with("services/vault")
            && service
                .workspace_scripts
                .dev
                .as_deref()
                .is_some_and(|path| path.ends_with("services/vault/dev.sh"))
            && service
                .workspace_scripts
                .build
                .as_deref()
                .is_some_and(|path| path.ends_with("services/vault/build.sh"))
            && service
                .workspace_scripts
                .verify_build
                .as_deref()
                .is_some_and(|path| path.ends_with("services/vault/verify-build.sh"))
            && service.execution_plane == "DeterministicService"
            && service.runtime == "Ivm"
            && service.route_path_prefix.as_deref() == Some("/api")
    }));
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
    assert!(
        output
            .routes
            .iter()
            .any(|route| route.service_name == "travel-ops_vault"
                && route.route_kind == "handler"
                && route.handler_name.as_deref() == Some("user_preferences_put")
                && route.path == "/api/v1/user/preferences"
                && route.handler_class.as_deref() == Some("Update"))
    );
    assert!(output.notes.iter().any(|note| note.contains("CID-only")));
}
#[test]
fn app_local_plan_single_api_reports_child_service_workspace() {
    let (dir, _) = single_api_fixture("single_api_app_local_plan");
    let output = AppLocalPlanArgs {
        manifest: dir.join("app_manifest.json"),
    }
    .run()
    .expect("app plan should succeed");
    assert!(output.manifest_path.ends_with("app_manifest.json"));
    assert_eq!(output.hostname, "travel-ops.sora");
    assert!(!output.has_mixed_planes);
    assert_eq!(output.hosted_http_service_count, 0);
    assert_eq!(output.deterministic_service_count, 1);
    assert_eq!(output.services.len(), 1);
    let service = &output.services[0];
    assert_eq!(service.service_name, "travel-ops_api");
    assert!(
        service
            .container_manifest_path
            .ends_with("services/api/container_manifest.json")
    );
    assert!(
        service
            .service_manifest_path
            .ends_with("services/api/service_manifest.json")
    );
    assert!(service.workspace_dir.ends_with("services/api"));
    assert_optional_path_ends_with(
        service.workspace_scripts.dev.as_deref(),
        "services/api/dev.sh",
    );
    assert_optional_path_ends_with(
        service.workspace_scripts.build.as_deref(),
        "services/api/build.sh",
    );
    assert_optional_path_ends_with(
        service.workspace_scripts.verify_build.as_deref(),
        "services/api/verify-build.sh",
    );
    assert_eq!(service.execution_plane, "DeterministicService");
    assert_eq!(service.runtime, "Ivm");
    assert_eq!(service.route_path_prefix.as_deref(), Some("/api"));
}
#[test]
fn app_local_dev_split_app_dry_run_reports_manifest_adjacent_script() {
    let (dir, _) = split_app_fixture("split_app_local_dev_dry_run");
    let output = AppLocalDevArgs {
        manifest: dir.join("app_manifest.json"),
        dry_run: true,
    }
    .run()
    .expect("dev dry-run should succeed");
    assert_eq!(output.mode, "dry_run");
    assert_eq!(output.hostname, "travel-ops.sora");
    assert!(output.manifest_path.ends_with("app_manifest.json"));
    assert!(output.workspace_dir.contains("split_app_local_dev_dry_run"));
    assert_app_workspace_scripts(&output.workspace_scripts);
    assert!(output.has_mixed_planes);
    assert_eq!(output.hosted_http_service_count, 1);
    assert_eq!(output.deterministic_service_count, 1);
    assert_eq!(output.command, vec!["./dev.sh".to_owned()]);
    assert!(output.script_path.ends_with("dev.sh"));
    assert!(output.working_dir.contains("split_app_local_dev_dry_run"));
    assert_eq!(output.services.len(), 2);
    assert!(
        output
            .services
            .iter()
            .any(|service| service.service_name == "travel-ops_live")
    );
    assert!(
        output
            .routes
            .iter()
            .any(|route| route.service_name == "travel-ops_live" && route.path == "/api/v1")
    );
    assert!(
        output
            .notes
            .iter()
            .any(|note| note.contains("mixed app plan includes both hosted HttpService + Inrou"))
    );
}
#[test]
fn app_local_dev_executes_manifest_adjacent_script() {
    if !bash_available() {
        return;
    }
    let (dir, _) = single_api_fixture("single_api_local_dev_run");
    let local_dev_script = dir.join("dev.sh");
    fs::write(&local_dev_script, STATIC_ASSETS_V1[5]).expect("write test dev script");
    mark_template_file_executable(&local_dev_script).expect("mark dev executable");
    let output = AppLocalDevArgs {
        manifest: dir.join("app_manifest.json"),
        dry_run: false,
    }
    .run()
    .expect("dev execution should succeed");
    assert_eq!(output.mode, "completed");
    assert_eq!(output.hostname, "travel-ops.sora");
    assert!(output.workspace_dir.contains("single_api_local_dev_run"));
    assert_optional_path_ends_with(output.workspace_scripts.local_dev.as_deref(), "dev.sh");
    assert_eq!(output.exit_status, Some(0));
    assert_eq!(output.command, vec!["./dev.sh".to_owned()]);
    assert_eq!(output.services.len(), 1);
    assert!(
        output
            .routes
            .iter()
            .any(|route| route.service_name == "travel-ops_api" && route.path == "/api/healthz")
    );
    assert!(
        dir.join("dev-ran.txt").exists(),
        "dev command should run the manifest-adjacent script"
    );
}
#[test]
fn app_local_dev_treats_interrupt_exit_as_successful_session_end() {
    if !bash_available() {
        return;
    }
    let (dir, _) = single_api_fixture("single_api_local_dev_interrupt");
    let local_dev_script = dir.join("dev.sh");
    fs::write(&local_dev_script, STATIC_ASSETS_V1[6]).expect("write interrupting dev script");
    mark_template_file_executable(&local_dev_script).expect("mark dev executable");
    let output = AppLocalDevArgs {
        manifest: dir.join("app_manifest.json"),
        dry_run: false,
    }
    .run()
    .expect("interrupt status 130 should be treated as a successful dev stop");
    assert_eq!(output.mode, "interrupted");
    assert_eq!(output.hostname, "travel-ops.sora");
    assert!(
        output
            .workspace_dir
            .contains("single_api_local_dev_interrupt")
    );
    assert_eq!(output.exit_status, Some(130));
    assert_eq!(output.services.len(), 1);
    assert_notes_contain(&output.notes, "interactive interrupt");
}
#[test]
fn app_build_and_sync_split_app_dry_run_reports_manifest_adjacent_script() {
    let (dir, _) = split_app_fixture("split_app_build_and_sync_dry_run");
    let output = AppBuildAndSyncArgs {
        manifest: dir.join("app_manifest.json"),
        dry_run: true,
    }
    .run()
    .expect("build-and-sync dry-run should succeed");
    assert_eq!(output.mode, "dry_run");
    assert_eq!(output.hostname, "travel-ops.sora");
    assert!(output.manifest_path.ends_with("app_manifest.json"));
    assert!(
        output
            .workspace_dir
            .contains("split_app_build_and_sync_dry_run")
    );
    assert_app_workspace_scripts(&output.workspace_scripts);
    assert!(output.has_mixed_planes);
    assert_eq!(output.hosted_http_service_count, 1);
    assert_eq!(output.deterministic_service_count, 1);
    assert_eq!(output.command, vec!["./build-and-sync.sh".to_owned()]);
    assert!(output.script_path.ends_with("build-and-sync.sh"));
    assert!(
        output
            .working_dir
            .contains("split_app_build_and_sync_dry_run")
    );
    assert_eq!(output.services.len(), 2);
    assert!(output.routes.iter().any(
        |route| route.service_name == "travel-ops_vault" && route.path == "/api/auth/challenge"
    ));
}
#[test]
fn app_build_and_sync_executes_manifest_adjacent_script() {
    if !bash_available() {
        return;
    }
    let (dir, _) = single_api_fixture("single_api_build_and_sync_run");
    let build_and_sync_script = dir.join("build-and-sync.sh");
    fs::write(&build_and_sync_script, STATIC_ASSETS_V1[7])
        .expect("write test build-and-sync script");
    mark_template_file_executable(&build_and_sync_script).expect("mark build-and-sync executable");
    let output = AppBuildAndSyncArgs {
        manifest: dir.join("app_manifest.json"),
        dry_run: false,
    }
    .run()
    .expect("build-and-sync execution should succeed");
    assert_eq!(output.mode, "completed");
    assert_eq!(output.hostname, "travel-ops.sora");
    assert!(
        output
            .workspace_dir
            .contains("single_api_build_and_sync_run")
    );
    assert_optional_path_ends_with(
        output.workspace_scripts.build_and_sync.as_deref(),
        "build-and-sync.sh",
    );
    assert_eq!(output.exit_status, Some(0));
    assert_eq!(output.command, vec!["./build-and-sync.sh".to_owned()]);
    assert_eq!(output.services.len(), 1);
    assert!(
        dir.join("build-and-sync-ran.txt").exists(),
        "build-and-sync command should run the manifest-adjacent script"
    );
    assert_notes_contain(&output.notes, "build-and-sync completed");
}
#[test]
fn app_release_workspace_executes_manifest_adjacent_script() {
    if !bash_available() {
        return;
    }
    let (dir, _) = single_api_fixture("single_api_release_workspace_run");
    let release_script = dir.join("release.sh");
    fs::write(&release_script, STATIC_ASSETS_V1[8]).expect("write app release script");
    mark_template_file_executable(&release_script).expect("mark app release executable");
    let output = AppReleaseWorkspaceArgs {
        manifest: dir.join("app_manifest.json"),
        torii_url: Some("http://127.0.0.1:8080".to_owned()),
        api_token: Some("top-secret".to_owned()),
        timeout_secs: 29,
        dry_run: false,
    }
    .run()
    .expect("app release execution should succeed");
    assert_eq!(output.mode, "completed");
    assert_eq!(output.hostname, "travel-ops.sora");
    assert!(
        output
            .workspace_dir
            .contains("single_api_release_workspace_run")
    );
    assert_optional_path_ends_with(output.workspace_scripts.doctor.as_deref(), "doctor.sh");
    assert_optional_path_ends_with(output.workspace_scripts.release.as_deref(), "release.sh");
    assert_eq!(output.exit_status, Some(0));
    assert_eq!(output.script_name, "release.sh");
    assert_eq!(output.torii_url, "http://127.0.0.1:8080");
    assert!(output.uses_api_token);
    assert_eq!(output.services.len(), 1);
    assert_eq!(
        fs::read_to_string(dir.join("app-release-torii.txt")).expect("read app release torii"),
        "http://127.0.0.1:8080"
    );
    assert_eq!(
        fs::read_to_string(dir.join("app-release-token.txt")).expect("read app release token"),
        "top-secret"
    );
    let args = fs::read_to_string(dir.join("app-release-args.txt")).expect("read app release args");
    assert!(args.contains("--timeout-secs"));
    assert!(args.contains("29"));
    assert_notes_contain(&output.notes, "release completed");
}
#[test]
fn app_doctor_validates_split_app_release_contract() {
    let (dir, _) = split_app_fixture("split_app_doctor");
    fs::create_dir_all(dir.join("frontend/dist")).expect("create frontend dist");
    fs::write(
        dir.join("frontend/dist/index.html"),
        "<!doctype html><title>Travel Ops</title>",
    )
    .expect("write frontend index");
    fs::create_dir_all(dir.join("services/live/build")).expect("create live build dir");
    fs::create_dir_all(dir.join("services/vault/build")).expect("create vault build dir");
    fs::write(
        dir.join("services/live/build/live-api.tgz"),
        b"doctor-live-bundle",
    )
    .expect("write live bundle");
    fs::write(
        dir.join("services/vault/build/vault-api.to"),
        b"doctor-vault-bundle",
    )
    .expect("write vault bundle");
    let output = AppDoctorArgs {
        manifest: dir.join("app_manifest.json"),
    }
    .run()
    .expect("doctor should succeed");
    assert!(
        output.ok,
        "doctor should pass on the scaffolded split-app contract"
    );
    assert_eq!(output.hostname, "travel-ops.sora");
    assert!(output.has_mixed_planes);
    assert_eq!(output.hosted_http_service_count, 1);
    assert_eq!(output.deterministic_service_count, 1);
    assert!(
        output
            .checks
            .iter()
            .any(|check| check.name == "frontend_publish_mode" && check.status == "pass")
    );
    assert!(
        output
            .checks
            .iter()
            .any(|check| check.name == "live_storage_plane" && check.status == "pass")
    );
    assert!(
        output
            .checks
            .iter()
            .any(|check| check.name == "deterministic_service_surface" && check.status == "pass")
    );
    assert!(
        output
            .checks
            .iter()
            .all(|check| check.name != "app_infra_manifest"),
        "local doctor must not claim that the post-publication app-infra manifest was validated"
    );
    assert!(output.notes.iter().all(|note| !note.contains("deferred")));
    assert!(output.routes.iter().any(
        |route| route.service_name == "travel-ops_vault" && route.path == "/api/auth/challenge"
    ));
}
#[test]
fn app_doctor_accepts_single_api_root_binding_release_contract() {
    let (dir, _) = single_api_fixture("single_api_doctor");
    fs::create_dir_all(dir.join("web/dist")).expect("create web dist");
    fs::write(
        dir.join("web/dist/index.html"),
        "<!doctype html><title>Travel Ops</title>",
    )
    .expect("write web index");
    fs::create_dir_all(dir.join("services/api/build")).expect("create API build dir");
    fs::write(
        dir.join("services/api/build/api-service.to"),
        b"doctor-single-api-bundle",
    )
    .expect("write API bundle");
    let output = AppDoctorArgs {
        manifest: dir.join("app_manifest.json"),
    }
    .run()
    .expect("doctor should produce a report");
    assert!(output.ok, "single-api doctor checks: {:?}", output.checks);
    assert!(!output.has_mixed_planes);
    assert_eq!(output.hosted_http_service_count, 0);
    assert_eq!(output.deterministic_service_count, 1);
    assert!(
        output
            .checks
            .iter()
            .any(|check| { check.name == "frontend_publish_mode" && check.status == "pass" })
    );
    assert!(
        output.checks.iter().any(|check| {
            check.name == "deterministic_service_surface" && check.status == "pass"
        })
    );
}
#[test]
fn app_doctor_rejects_any_hosted_live_route_outside_api_v1() {
    let (dir, _) = split_app_fixture("split_app_doctor_rejects_hosted_prefix");
    fs::create_dir_all(dir.join("frontend/dist")).expect("create frontend dist");
    fs::write(
        dir.join("frontend/dist/index.html"),
        "<!doctype html><title>Travel Ops</title>",
    )
    .expect("write frontend index");
    fs::create_dir_all(dir.join("services/live/build")).expect("create live build dir");
    fs::create_dir_all(dir.join("services/vault/build")).expect("create vault build dir");
    fs::write(
        dir.join("services/live/build/live-api.tgz"),
        b"doctor-live-bundle",
    )
    .expect("write live bundle");
    fs::write(
        dir.join("services/vault/build/vault-api.to"),
        b"doctor-vault-bundle",
    )
    .expect("write vault bundle");
    let admin_dir = dir.join("services/admin");
    fs::create_dir_all(&admin_dir).expect("create admin service dir");
    let admin_container_path = admin_dir.join("container_manifest.json");
    let admin_service_path = admin_dir.join("service_manifest.json");
    let artifact_dir = dir.join("artifacts");
    let admin_bundle_path = artifact_dir.join("admin-api.tgz");
    fs::create_dir_all(&artifact_dir).expect("create artifact dir");
    fs::write(&admin_bundle_path, b"doctor-admin-bundle").expect("write admin bundle");
    let admin_container: UnpublishedContainerManifestV1 =
        load_json(&dir.join("services/live/container_manifest.json")).expect("live container");
    let mut admin_service: SoraServiceManifestV1 =
        load_json(&dir.join("services/live/service_manifest.json")).expect("live service");
    admin_service.service_name = "travel-ops_admin".parse().expect("admin service name");
    admin_service
        .route
        .as_mut()
        .expect("hosted service route")
        .path_prefix = "/admin".to_owned();
    write_json(&admin_container_path, &admin_container).expect("write admin container");
    write_json(&admin_service_path, &admin_service).expect("write admin service");
    let manifest_path = dir.join("app_manifest.json");
    let mut manifest: SoracloudAppManifestV1 = load_json(&manifest_path).expect("app manifest");
    manifest.services.push(SoracloudAppServiceRefV1 {
        service_name: "travel-ops_admin".to_owned(),
        container_manifest: relative_path_string(&manifest_path, &admin_container_path),
        service_manifest: relative_path_string(&manifest_path, &admin_service_path),
        bundle_file: Some(relative_path_string(&manifest_path, &admin_bundle_path)),
        initial_configs: None,
        initial_secrets: None,
    });
    write_json(&manifest_path, &manifest).expect("write app manifest");
    let output = AppDoctorArgs {
        manifest: manifest_path,
    }
    .run()
    .expect("doctor should produce a report");
    assert!(
        !output.ok,
        "doctor must fail when any hosted Inrou route leaves /api/v1"
    );
    let failing_checks = output
        .checks
        .iter()
        .filter(|check| check.status == "fail")
        .map(|check| check.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(failing_checks, vec!["live_route_prefix"]);
    let live_route_prefix = output
        .checks
        .iter()
        .find(|check| check.name == "live_route_prefix")
        .expect("live route prefix check");
    assert_eq!(live_route_prefix.status, "fail");
    assert!(live_route_prefix.detail.contains("travel-ops_live:/api/v1"));
    assert!(live_route_prefix.detail.contains("travel-ops_admin:/admin"));
}
#[test]
fn app_release_dry_run_reports_build_and_deploy_plan() {
    let (dir, _) = split_app_fixture("split_app_release_dry_run");
    let key_pair = soracloud_fixture_key_pair(0x40);
    let authority = AccountId::new(key_pair.public_key().clone());
    let output = AppReleaseArgs {
        manifest: dir.join("app_manifest.json"),
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        torii_url: Some("http://127.0.0.1:8080".to_owned()),
        api_token: Some("token".to_owned()),
        timeout_secs: 41,
        dry_run: true,
        inrou_preseed_receipt: None,
    }
    .run(&authority, &key_pair)
    .expect("release dry-run should succeed");
    assert_eq!(output.mode, "dry_run");
    assert_eq!(output.release_mode, "deploy");
    assert_eq!(output.torii_url, "http://127.0.0.1:8080");
    assert!(output.uses_api_token);
    assert!(output.release_response.is_none());
    assert_eq!(output.plan.hostname, "travel-ops.sora");
    assert!(output.plan.has_mixed_planes);
    assert_eq!(output.build_and_sync.mode, "dry_run");
    assert_notes_contain(&output.notes, "one explicit deploy");
}
#[test]
fn app_release_runs_build_and_then_deploys_split_app() {
    if !bash_available() {
        return;
    }
    let (dir, _) = split_app_fixture("split_app_release_run");
    let build_script = dir.join("build-and-sync.sh");
    fs::write(&build_script, STATIC_ASSETS_V1[9]).expect("write release build script");
    mark_template_file_executable(&build_script).expect("mark release build script executable");
    let status_payload =
        mock_control_plane_status_payload(&["travel-ops_live", "travel-ops_vault"]);
    let key_pair = soracloud_fixture_key_pair(0x41);
    let authority = AccountId::new(key_pair.public_key().clone());
    let draft_response = mock_soracloud_draft_response(&authority, &key_pair);
    let server = mock_bundle_mutation_server(
        &dir,
        "/v1/soracloud/apps/deploy",
        &draft_response,
        &status_payload,
        "encode pin register response",
        "encode deploy response",
    );
    point_split_app_live_route_at_mock_server(&dir, &server.base_url);
    install_mock_submission_config(&authority, &key_pair);
    let inrou_preseed_receipt = qualify_test_inrou_app(&dir, &key_pair, "release");
    let output = AppReleaseArgs {
        manifest: dir.join("app_manifest.json"),
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
        dry_run: false,
        inrou_preseed_receipt: Some(inrou_preseed_receipt),
    }
    .run(&authority, &key_pair)
    .expect("release should succeed");
    assert_eq!(output.mode, "completed");
    assert_eq!(output.release_mode, "Deploy");
    assert_eq!(output.plan.hostname, "127.0.0.1");
    assert_eq!(output.build_and_sync.mode, "completed");
    let release_response = output
        .release_response
        .as_ref()
        .expect("release must include the mutation response");
    assert!(release_response.has_mixed_planes);
    assert_eq!(release_response.services.len(), 2);
    let live_service = release_response
        .services
        .iter()
        .find(|service| service.service_name == "travel-ops_live")
        .expect("live service output");
    assert!(!live_service.published_bundle.content_cid.is_empty());
    assert_eq!(live_service.published_inrou_guest_images.len(), 2);
    let vault_service = release_response
        .services
        .iter()
        .find(|service| service.service_name == "travel-ops_vault")
        .expect("vault service output");
    assert!(!vault_service.published_bundle.content_cid.is_empty());
    assert!(release_response.published_static_site.is_some());
    assert_eq!(output.live_verifications.len(), 1);
    assert_eq!(output.live_verifications[0].status_code, 200);
    assert!(output.live_verifications[0].url.ends_with("/api/v1/health"));
    assert!(output.report.ok);
    assert!(
        output
            .report
            .phases
            .iter()
            .any(|phase| { phase.name == "verify" && phase.ok && !phase.skipped })
    );
    let deploy_requests = server
        .requests()
        .into_iter()
        .filter(|request| request.method == "POST" && request.path == "/v1/soracloud/apps/deploy")
        .count();
    assert_eq!(deploy_requests, 1);
}
#[test]
fn app_release_rejects_prepublished_inrou_guest_image_artifacts() {
    if !bash_available() {
        return;
    }
    let (dir, _) = split_app_fixture("split_app_release_reuse_guest_images");
    let build_script = dir.join("build-and-sync.sh");
    fs::write(&build_script, STATIC_ASSETS_V1[10]).expect("write release build script");
    mark_template_file_executable(&build_script).expect("mark release build script executable");
    let live_container_path = dir.join("services/live/container_manifest.json");
    let mut live_container: Value = load_json(&live_container_path).expect("live container");
    let guest_images = live_container
        .as_object_mut()
        .and_then(|container| container.get_mut("inrou"))
        .and_then(Value::as_object_mut)
        .and_then(|inrou| inrou.get_mut("guest_images"))
        .and_then(Value::as_object_mut)
        .expect("live guest-image map");
    for (guest_isa, seed) in [("x86_64", 0xAA), ("aarch64", 0xCC)] {
        guest_images
            .get_mut(guest_isa)
            .and_then(Value::as_object_mut)
            .expect("guest-image object")
            .insert(
                "published_artifact".to_owned(),
                norito::json::to_value(&sample_published_inrou_artifact(seed))
                    .expect("encode forbidden prepublished artifact"),
            );
    }
    write_json(&live_container_path, &live_container).expect("write live container manifest");
    fs::create_dir_all(dir.join("services/live/build")).expect("create live build dir");
    fs::create_dir_all(dir.join("services/vault/build")).expect("create vault build dir");
    fs::write(
        dir.join("services/live/build/live-api.tgz"),
        b"release-live-bundle",
    )
    .expect("write live bundle");
    fs::write(
        dir.join("services/vault/build/vault-api.to"),
        b"release-vault-bundle",
    )
    .expect("write vault bundle");
    fs::remove_dir_all(dir.join("services/live/inrou")).expect("remove local inrou staging");
    let status_payload =
        mock_control_plane_status_payload(&["travel-ops_live", "travel-ops_vault"]);
    let key_pair = soracloud_fixture_key_pair(0x42);
    let authority = AccountId::new(key_pair.public_key().clone());
    let draft_response = mock_soracloud_draft_response(&authority, &key_pair);
    let server = mock_bundle_mutation_server(
        &dir,
        "/v1/soracloud/apps/deploy",
        &draft_response,
        &status_payload,
        "encode pin register response",
        "encode deploy response",
    );
    install_mock_submission_config(&authority, &key_pair);
    let error = AppReleaseArgs {
        manifest: dir.join("app_manifest.json"),
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
        dry_run: false,
        inrou_preseed_receipt: None,
    }
    .run(&authority, &key_pair)
    .expect_err("V1 release must reject prepublished guest images");
    let rendered_error = format!("{error:#}");
    assert!(
        rendered_error.contains("published_artifact") || rendered_error.contains("expected null"),
        "unexpected release error: {error:#}"
    );
    assert!(
        server.requests().is_empty(),
        "draft decoding must fail before pin, register, or mutation requests"
    );
}
#[test]
fn app_local_plan_rejects_app_service_name_mismatch() {
    let (dir, _) = split_app_fixture("split_app_local_plan_service_name_mismatch");
    let manifest_path = dir.join("app_manifest.json");
    let mut manifest: SoracloudAppManifestV1 = load_json(&manifest_path).expect("app manifest");
    manifest.services[0].service_name = "wrong_live_name".to_owned();
    write_json(&manifest_path, &manifest).expect("write mismatched app manifest");
    let error = AppLocalPlanArgs {
        manifest: manifest_path,
    }
    .run()
    .expect_err("plan should fail on service name mismatch");
    assert!(error.to_string().contains("wrong_live_name"));
    assert!(
        error
            .to_string()
            .contains("referenced service manifest declares")
    );
}
#[test]
fn generated_split_app_live_server_smoke_serves_hayahi_routes() {
    let (dir, _) = split_app_fixture("split_app_live_smoke");
    let server_path = dir.join("services/live/app/server.mjs");
    if !node_available() {
        let server = fs::read_to_string(&server_path).expect("read split live server");
        assert!(server.contains("/search"));
        assert!(server.contains("/airports/search"));
        assert!(server.contains("/filters/metadata"));
        assert!(server.contains("/luxury/catalog"));
        assert!(server.contains("/links/resolve"));
        return;
    }
    let harness_path = dir.join("split_app_live_smoke.mjs");
    let shared_cache_dir = dir.join("lease/shared-cache");
    let search_sessions_dir = dir.join("lease/search-sessions");
    let collector_state_dir = dir.join("lease/collector-state");
    let runtime_cache_dir = dir.join("lease/runtime-cache");
    let mut script = TEST_HARNESSES_V1[4].to_owned();
    script = script.replace("__SERVER_PATH__", &js_string_literal(&server_path));
    script = script.replace(
        "__SHARED_CACHE_DIR__",
        &js_string_literal(&shared_cache_dir),
    );
    script = script.replace(
        "__SEARCH_SESSIONS_DIR__",
        &js_string_literal(&search_sessions_dir),
    );
    script = script.replace(
        "__COLLECTOR_STATE_DIR__",
        &js_string_literal(&collector_state_dir),
    );
    script = script.replace(
        "__RUNTIME_CACHE_DIR__",
        &js_string_literal(&runtime_cache_dir),
    );
    fs::write(&harness_path, script).expect("write split live smoke harness");
    run_node_harness(&harness_path);
}
#[test]
fn generated_split_app_vault_dev_server_smoke_serves_auth_and_user_state() {
    let (dir, _) = split_app_fixture("split_app_vault_dev_smoke");
    let server_path = dir.join("services/vault/dev-server.mjs");
    if !node_available() {
        let server = fs::read_to_string(&server_path).expect("read split vault dev server");
        assert!(server.contains("/auth/challenge"));
        assert!(server.contains("/auth/me"));
        assert!(server.contains("/v1/user/preferences"));
        assert!(server.contains("/v1/user/saved-searches"));
        return;
    }
    let harness_path = dir.join("split_app_vault_dev_smoke.mjs");
    let state_file = dir.join("services/vault/tmp/vault-dev-state.json");
    let mut script = TEST_HARNESSES_V1[5].to_owned();
    script = script.replace("__SERVER_PATH__", &js_string_literal(&server_path));
    script = script.replace("__STATE_FILE__", &js_string_literal(&state_file));
    fs::write(&harness_path, script).expect("write split vault dev smoke harness");
    run_node_harness(&harness_path);
}
#[test]
fn generated_split_app_frontend_build_guard_enforces_live_same_host_api() {
    let (dir, _) = split_app_fixture("split_app_frontend_guard");
    let guard_path = dir.join("frontend/scripts/validate-production-env.mjs");
    if !node_available() {
        let guard = fs::read_to_string(&guard_path).expect("read build guard");
        assert!(guard.contains("VITE_PUBLIC_API_BASE must be exactly '/api'"));
        assert!(guard.contains("VITE_DATA_MODE must be exactly 'live'"));
        return;
    }
    let success = Command::new("node")
        .arg(&guard_path)
        .env("VITE_PUBLIC_API_BASE", "/api")
        .env("VITE_DATA_MODE", "live")
        .output()
        .expect("run build guard success case");
    assert!(
        success.status.success(),
        "expected build guard success\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&success.stdout),
        String::from_utf8_lossy(&success.stderr)
    );
    let missing_api = Command::new("node")
        .arg(&guard_path)
        .env_remove("VITE_PUBLIC_API_BASE")
        .env("VITE_DATA_MODE", "live")
        .output()
        .expect("run missing api guard case");
    assert!(!missing_api.status.success());
    assert!(
        String::from_utf8_lossy(&missing_api.stderr)
            .contains("VITE_PUBLIC_API_BASE must be exactly '/api'")
    );
    let absolute_api = Command::new("node")
        .arg(&guard_path)
        .env("VITE_PUBLIC_API_BASE", "https://example.com/api")
        .env("VITE_DATA_MODE", "live")
        .output()
        .expect("run absolute api guard case");
    assert!(!absolute_api.status.success());
    assert!(
        String::from_utf8_lossy(&absolute_api.stderr)
            .contains("VITE_PUBLIC_API_BASE must be exactly '/api'")
    );
    let missing_mode = Command::new("node")
        .arg(&guard_path)
        .env("VITE_PUBLIC_API_BASE", "/api")
        .env_remove("VITE_DATA_MODE")
        .output()
        .expect("run missing mode guard case");
    assert!(!missing_mode.status.success());
    assert!(
        String::from_utf8_lossy(&missing_mode.stderr)
            .contains("VITE_DATA_MODE must be exactly 'live'")
    );
    let demo_mode = Command::new("node")
        .arg(&guard_path)
        .env("VITE_PUBLIC_API_BASE", "/api")
        .env("VITE_DATA_MODE", "demo")
        .output()
        .expect("run demo mode guard case");
    assert!(!demo_mode.status.success());
    assert!(
        String::from_utf8_lossy(&demo_mode.stderr)
            .contains("VITE_DATA_MODE must be exactly 'live'")
    );
}
#[test]
fn app_manifest_rejects_missing_static_site_publish_mode() {
    let error = json::from_str::<SoracloudAppManifestV1>(STATIC_ASSETS_V1[11])
        .expect_err("first-release app manifests must declare static-site publish_mode");
    assert!(
        error.to_string().contains("missing field `publish_mode`"),
        "unexpected missing publish_mode error: {error}"
    );
}
#[test]
fn app_manifest_json_is_closed_and_requires_explicit_nullable_and_vector_keys() {
    let canonical = json::to_value(&SoracloudAppManifestV1 {
        schema_version: SORACLOUD_APP_MANIFEST_VERSION_V1,
        app_name: "travel_ops".to_owned(),
        app_version: None,
        public_url: "https://travel-ops.sora".to_owned(),
        static_site: None,
        services: Vec::new(),
    })
    .expect("encode canonical app manifest");
    json::from_value::<SoracloudAppManifestV1>(canonical.clone())
        .expect("explicit null and empty app manifest keys must decode");
    for field in ["app_version", "static_site", "services"] {
        let mut omitted = canonical.clone();
        omitted
            .as_object_mut()
            .expect("app manifest object")
            .remove(field);
        json::from_value::<SoracloudAppManifestV1>(omitted)
            .expect_err("an omitted app manifest key must fail");
    }
    let mut unknown = canonical.clone();
    unknown
        .as_object_mut()
        .expect("app manifest object")
        .insert("legacy_app_version".to_owned(), json::Value::Null);
    json::from_value::<SoracloudAppManifestV1>(unknown)
        .expect_err("an unknown app manifest key must fail");

    let static_site = json::to_value(&SoracloudAppStaticSiteV1 {
        dist_dir: "frontend/dist".to_owned(),
        mount_path: "/".to_owned(),
        publish_mode: APP_STATIC_SITE_PUBLISH_MODE_ROOT_BINDING.to_owned(),
        api_base_path: None,
        publish_label: None,
    })
    .expect("encode canonical static-site entry");
    for field in ["api_base_path", "publish_label"] {
        let mut omitted = static_site.clone();
        omitted
            .as_object_mut()
            .expect("static-site object")
            .remove(field);
        json::from_value::<SoracloudAppStaticSiteV1>(omitted)
            .expect_err("an omitted static-site nullable key must fail");
    }
    let mut unknown_static_site = static_site;
    unknown_static_site
        .as_object_mut()
        .expect("static-site object")
        .insert("api_path".to_owned(), json::Value::Null);
    json::from_value::<SoracloudAppStaticSiteV1>(unknown_static_site)
        .expect_err("an unknown static-site key must fail");

    let service = json::to_value(&SoracloudAppServiceRefV1 {
        service_name: "travel_ops_live".to_owned(),
        container_manifest: "services/live/container_manifest.json".to_owned(),
        service_manifest: "services/live/service_manifest.json".to_owned(),
        bundle_file: None,
        initial_configs: None,
        initial_secrets: None,
    })
    .expect("encode canonical app service entry");
    for field in ["bundle_file", "initial_configs", "initial_secrets"] {
        let mut omitted = service.clone();
        omitted
            .as_object_mut()
            .expect("app service object")
            .remove(field);
        json::from_value::<SoracloudAppServiceRefV1>(omitted)
            .expect_err("an omitted app service nullable key must fail");
    }
    let mut unknown_service = service;
    unknown_service
        .as_object_mut()
        .expect("app service object")
        .insert("bundle_path".to_owned(), json::Value::Null);
    json::from_value::<SoracloudAppServiceRefV1>(unknown_service)
        .expect_err("an unknown app service key must fail");
}
#[test]
fn app_static_site_root_binding_plan_targets_public_host_for_root_binding() {
    let static_site = SoracloudAppStaticSiteV1 {
        dist_dir: "frontend/dist".to_owned(),
        mount_path: "/".to_owned(),
        publish_mode: APP_STATIC_SITE_PUBLISH_MODE_ROOT_BINDING.to_owned(),
        api_base_path: Some("/api".to_owned()),
        publish_label: None,
    };
    let publication = AppStaticSitePublishOutput {
        hostname: "travel-ops.sora".to_owned(),
        public_url: "https://travel-ops.sora".to_owned(),
        cid_gateway_url: "https://travel-ops.sora/sorafs/cid/bafytest".to_owned(),
        content_cid: "bafytest".to_owned(),
        manifest_digest_hex: "abcd".repeat(16),
    };
    let plan = plan_app_static_site_root_binding(
        "travel_ops",
        "https://travel-ops.sora",
        Some(&static_site),
        Some(&publication),
    )
    .expect("root binding plan should build")
    .expect("root binding plan should exist");
    assert_eq!(plan.target_host, "travel-ops.sora");
}
#[test]
fn app_static_site_root_binding_plan_skips_cid_only_sites() {
    let static_site = SoracloudAppStaticSiteV1 {
        dist_dir: "frontend/dist".to_owned(),
        mount_path: "/".to_owned(),
        publish_mode: APP_STATIC_SITE_PUBLISH_MODE_CID_ONLY.to_owned(),
        api_base_path: Some("/api".to_owned()),
        publish_label: None,
    };
    let publication = AppStaticSitePublishOutput {
        hostname: "travel-ops.sora".to_owned(),
        public_url: "https://travel-ops.sora".to_owned(),
        cid_gateway_url: "https://travel-ops.sora/sorafs/cid/bafytest".to_owned(),
        content_cid: "bafytest".to_owned(),
        manifest_digest_hex: "abcd".repeat(16),
    };
    let plan = plan_app_static_site_root_binding(
        "travel_ops",
        "https://travel-ops.sora",
        Some(&static_site),
        Some(&publication),
    )
    .expect("cid-only plan should build");
    assert!(
        plan.is_none(),
        "cid-only apps must not create root binding plans"
    );
}
#[test]
fn apply_app_static_site_root_binding_attaches_reserved_config_once() {
    let static_site = SoracloudAppStaticSiteV1 {
        dist_dir: "frontend/dist".to_owned(),
        mount_path: "/".to_owned(),
        publish_mode: APP_STATIC_SITE_PUBLISH_MODE_ROOT_BINDING.to_owned(),
        api_base_path: Some("/api".to_owned()),
        publish_label: None,
    };
    let publication = AppStaticSitePublishOutput {
        hostname: "travel-ops.sora".to_owned(),
        public_url: "https://travel-ops.sora".to_owned(),
        cid_gateway_url: "https://travel-ops.sora/sorafs/cid/bafytest".to_owned(),
        content_cid: "bafytest".to_owned(),
        manifest_digest_hex: "abcd".repeat(16),
    };
    let plan = plan_app_static_site_root_binding(
        "travel_ops",
        "https://travel-ops.sora",
        Some(&static_site),
        Some(&publication),
    )
    .expect("root binding plan should build")
    .expect("root binding plan should exist");
    let mut service = fixture_service();
    service.route = Some(SoraRouteTargetV1 {
        host: "travel-ops.sora".to_owned(),
        path_prefix: "/api".to_owned(),
        service_port: NonZeroU16::new(8787).expect("nonzero literal"),
        visibility: SoraRouteVisibilityV1::Public,
        tls_mode: SoraTlsModeV1::Required,
    });
    let mut initial_configs = BTreeMap::new();
    let attached = apply_app_static_site_root_binding(
        "travel_ops_live",
        &service,
        &mut initial_configs,
        Some(&plan),
        false,
    )
    .expect("binding attachment should succeed");
    assert!(
        attached,
        "matching public route should receive root binding"
    );
    assert!(initial_configs.contains_key(APP_STATIC_SITE_CONFIG_NAME));
    let mut second_service_configs = BTreeMap::new();
    let second_attach = apply_app_static_site_root_binding(
        "travel_ops_live",
        &service,
        &mut second_service_configs,
        Some(&plan),
        true,
    )
    .expect("second attachment should be skipped");
    assert!(!second_attach, "binding must only attach once");
}
#[test]
fn ensure_app_static_site_root_binding_attached_errors_when_required_route_is_missing() {
    let plan = AppStaticSiteRootBindingPlan {
        target_host: "travel-ops.sora".to_owned(),
        binding_value: Json::from(norito::json!({ "schema_version": 1 })),
    };
    let err = ensure_app_static_site_root_binding_attached(Some(&plan), false)
        .expect_err("missing matching route must fail");
    assert!(
        err.to_string().contains(
            "app static site host `travel-ops.sora` has no matching public service route"
        )
    );
}
#[test]
fn init_hayahi_app_template_scaffolds_real_ivm_api_project() {
    let (dir, output) = service_fixture("hayahi_app_template", InitTemplate::HayahiApp);
    assert_eq!(output.template, "hayahi-app");
    assert_scaffold_file_contract(
        &dir,
        "init_hayahi_app_template_scaffolds_real_ivm_api_project",
    );
    let service: SoraServiceManifestV1 =
        load_json(&dir.join("service_manifest.json")).expect("service manifest");
    assert_eq!(
        service
            .route
            .as_ref()
            .map(|route| route.path_prefix.as_str()),
        Some("/api")
    );
    assert!(
        service
            .state_bindings
            .iter()
            .any(|binding| binding.key_prefix == "/state/hayahi/search/sessions"),
        "search session shared binding missing from hayahi-app template"
    );
    assert!(
        service
            .state_bindings
            .iter()
            .any(|binding| binding.key_prefix == "/state/hayahi/search/cache"),
        "search cache shared binding missing from hayahi-app template"
    );
    assert!(
        service
            .state_bindings
            .iter()
            .any(|binding| binding.key_prefix == "/state/hayahi/collectors/jobs"),
        "collector jobs shared binding missing from hayahi-app template"
    );
    assert!(
        service.state_bindings.iter().any(|binding| {
            binding.binding_name.as_ref() == "user_saved_searches"
                && binding.encryption == SoraStateEncryptionV1::FheCiphertext
                && binding.key_prefix == "/state/hayahi/users/saved_searches"
        }),
        "user_saved_searches private binding missing from hayahi-app template"
    );
    assert!(
        service.state_bindings.iter().any(|binding| {
            binding.binding_name.as_ref() == "user_preferences"
                && binding.encryption == SoraStateEncryptionV1::FheCiphertext
                && binding.key_prefix == "/state/hayahi/users/preferences"
        }),
        "user_preferences private binding missing from hayahi-app template"
    );
    assert_eq!(service.handlers.len(), 12);
    assert_eq!(service.handlers[0].class, SoraServiceHandlerClassV1::Query);
    assert_eq!(
        service.handlers[0].route_path.as_deref(),
        Some("/v1/health")
    );
    assert_eq!(service.handlers[6].class, SoraServiceHandlerClassV1::Update);
    assert_eq!(
        service.handlers[6]
            .mailbox
            .as_ref()
            .map(|mailbox| mailbox.queue_name.as_ref()),
        Some("auth_updates")
    );
    assert_eq!(service.handlers[8].class, SoraServiceHandlerClassV1::Update);
    assert_eq!(
        service.handlers[8].route_path.as_deref(),
        Some("/auth/login")
    );
    assert_eq!(service.artifacts.len(), 2);
    assert_eq!(service.artifacts[0].kind, SoraArtifactKindV1::Journal);
    assert_eq!(service.artifacts[1].kind, SoraArtifactKindV1::Checkpoint);
}
#[test]
fn generated_webapp_auth_module_contains_replay_and_signature_guards() {
    let (dir, _) = service_fixture("webapp_auth_markers", InitTemplate::Webapp);
    let api = fs::read_to_string(dir.join("webapp/api/server.mjs")).expect("read api file");
    assert!(api.contains("AUTH_CHALLENGE_REPLAYED"));
    assert!(api.contains("AUTH_CHALLENGE_EXPIRED"));
    assert!(api.contains("AUTH_CHALLENGE_NOT_FOUND"));
    assert!(api.contains("AUTH_SIGNATURE_INVALID"));
    assert!(api.contains("AUTH_MESSAGE_VERSION"));
    assert!(api.contains("AUTH_REQUIRE_EXTERNAL_SHARED_STATE"));
    assert!(api.contains("__soracloudSharedStateAdapter"));
    assert!(api.contains("putIfAbsent"));
    assert!(api.contains("/state/auth/challenges"));
    assert!(api.contains("/state/auth/sessions"));
    assert!(api.contains("AUTH_CHALLENGE_EXPIRED_PREFIX"));
    assert!(api.contains("AUTH_CHALLENGE_CONSUME_LOCK_PREFIX"));
    assert!(api.contains("withAuthStateFileLock"));
    assert!(api.contains("sendInternalError"));
    assert!(api.contains("server.address()"));
}
#[test]
fn generated_pii_app_auth_module_contains_replay_and_signature_guards() {
    let (dir, _) = service_fixture("pii_auth_markers", InitTemplate::PiiApp);
    let api = fs::read_to_string(dir.join("pii-app/api/server.mjs")).expect("read pii api file");
    assert!(api.contains("AUTH_CHALLENGE_REPLAYED"));
    assert!(api.contains("AUTH_CHALLENGE_EXPIRED"));
    assert!(api.contains("AUTH_CHALLENGE_NOT_FOUND"));
    assert!(api.contains("AUTH_SIGNATURE_INVALID"));
    assert!(api.contains("AUTH_MESSAGE_VERSION"));
    assert!(api.contains("AUTH_REQUIRE_EXTERNAL_SHARED_STATE"));
    assert!(api.contains("__soracloudSharedStateAdapter"));
    assert!(api.contains("putIfAbsent"));
    assert!(api.contains("/state/auth/challenges"));
    assert!(api.contains("/state/auth/sessions"));
    assert!(api.contains("AUTH_CHALLENGE_EXPIRED_PREFIX"));
    assert!(api.contains("AUTH_CHALLENGE_CONSUME_LOCK_PREFIX"));
    assert!(api.contains("withAuthStateFileLock"));
    assert!(api.contains("sendInternalError"));
    assert!(api.contains("server.address()"));
}
#[test]
fn generated_hayahi_app_contract_contains_real_route_entrypoints() {
    let (dir, _) = service_fixture("hayahi_auth_markers", InitTemplate::HayahiApp);
    let contract = fs::read_to_string(dir.join("hayahi-app/contract/hayahi_api.ko"))
        .expect("read hayahi contract");
    assert!(contract.contains("route: \"/api/v1/health\""));
    assert!(contract.contains("route: \"/api/v1/state/overview\""));
    assert!(contract.contains("route: \"/api/v1/collector/status\""));
    assert!(contract.contains("route: \"/api/auth/challenge\""));
    assert!(contract.contains("route: \"/api/auth/login\""));
    assert!(contract.contains("route: \"/api/auth/logout\""));
    assert!(contract.contains("route: \"/api/v1/user/preferences\""));
    assert!(contract.contains("route: \"/api/v1/user/saved-searches\""));
}
#[test]
fn sync_manifests_updates_bundle_and_container_hashes() {
    let dir = temp_dir("sync_manifests");
    let mut container = fixture_container();
    container.env.insert(
        "PUBLIC_BASE_URL".to_owned(),
        "https://taira.sora.org".to_owned(),
    );
    let mut service = fixture_service();
    service.container.manifest_hash = Hash::prehashed([0x11; Hash::LENGTH]);
    let container_path = dir.join("container_manifest.json");
    let service_path = dir.join("service_manifest.json");
    let bundle_path = dir.join("hayahi-app-api.to");
    write_json(&container_path, &container).expect("write container");
    write_json(&service_path, &service).expect("write service");
    fs::write(&bundle_path, b"hayahi-ivm-bundle").expect("write bundle");
    let output = SyncManifestsArgs {
        app_manifest: None,
        container: container_path.clone(),
        service: service_path.clone(),
        bundle_file: Some(bundle_path.clone()),
    }
    .run()
    .expect("sync manifests should succeed");
    let synced_container: SoraContainerManifestV1 =
        load_json(&container_path).expect("synced container");
    let synced_service: SoraServiceManifestV1 = load_json(&service_path).expect("synced service");
    assert_eq!(
        synced_container.bundle_hash,
        Hash::new(b"hayahi-ivm-bundle")
    );
    assert_eq!(
        synced_service.container.manifest_hash,
        Hash::new(Encode::encode(&synced_container))
    );
    assert_eq!(
        synced_service.container.expected_schema_version,
        synced_container.schema_version
    );
    assert_eq!(output.bundle_hash, Some(synced_container.bundle_hash));
    assert_eq!(
        output.container_manifest_hash,
        Some(Hash::new(Encode::encode(&synced_container)))
    );
}
#[test]
fn sync_manifests_app_manifest_refreshes_every_service_pair() {
    let dir = temp_dir("sync_manifests_app");
    let manifest_path = dir.join("app_manifest.json");
    let live_bundle = build_split_app_live_service_bundle("travel_ops", "travel-ops.sora", "1.0.0")
        .expect("build live bundle");
    let vault_bundle =
        build_split_app_vault_service_bundle("travel_ops", "travel-ops.sora", "1.0.0")
            .expect("build vault bundle");
    let live_dir = dir.join("services/live");
    let vault_dir = dir.join("services/vault");
    fs::create_dir_all(live_dir.join("build")).expect("create live build dir");
    fs::create_dir_all(vault_dir.join("build")).expect("create vault build dir");
    write_json(
        &live_dir.join("container_manifest.json"),
        &live_bundle.container,
    )
    .expect("write live container");
    write_json(
        &live_dir.join("service_manifest.json"),
        &live_bundle.service,
    )
    .expect("write live service");
    write_json(
        &vault_dir.join("container_manifest.json"),
        &vault_bundle.container,
    )
    .expect("write vault container");
    write_json(
        &vault_dir.join("service_manifest.json"),
        &vault_bundle.service,
    )
    .expect("write vault service");
    fs::write(live_dir.join("build/live-api.tgz"), b"live-api-bundle").expect("write live bundle");
    fs::write(vault_dir.join("build/vault-api.to"), b"vault-api-bundle")
        .expect("write vault bundle");
    let manifest = SoracloudAppManifestV1 {
        schema_version: SORACLOUD_APP_MANIFEST_VERSION_V1,
        app_name: "travel_ops".to_owned(),
        app_version: Some("1.0.0".to_owned()),
        public_url: "https://travel-ops.sora".to_owned(),
        static_site: Some(SoracloudAppStaticSiteV1 {
            dist_dir: "frontend/dist".to_owned(),
            mount_path: "/".to_owned(),
            publish_mode: APP_STATIC_SITE_PUBLISH_MODE_CID_ONLY.to_owned(),
            api_base_path: Some("/api".to_owned()),
            publish_label: None,
        }),
        services: vec![
            SoracloudAppServiceRefV1 {
                service_name: live_bundle.service.service_name.to_string(),
                container_manifest: "services/live/container_manifest.json".to_owned(),
                service_manifest: "services/live/service_manifest.json".to_owned(),
                bundle_file: Some("services/live/build/live-api.tgz".to_owned()),
                initial_configs: None,
                initial_secrets: None,
            },
            SoracloudAppServiceRefV1 {
                service_name: vault_bundle.service.service_name.to_string(),
                container_manifest: "services/vault/container_manifest.json".to_owned(),
                service_manifest: "services/vault/service_manifest.json".to_owned(),
                bundle_file: Some("services/vault/build/vault-api.to".to_owned()),
                initial_configs: None,
                initial_secrets: None,
            },
        ],
    };
    write_json(&manifest_path, &manifest).expect("write app manifest");
    let output = SyncManifestsArgs {
        app_manifest: Some(manifest_path.clone()),
        container: dir.join("unused-container.json"),
        service: dir.join("unused-service.json"),
        bundle_file: None,
    }
    .run()
    .expect("app sync should succeed");
    assert_eq!(
        output.app_manifest_path.as_deref(),
        Some(manifest_path.to_string_lossy().as_ref())
    );
    assert_eq!(output.services.len(), 2);
    assert!(
        output
            .services
            .iter()
            .any(|service| service.bundle_hash == Hash::new(b"live-api-bundle"))
    );
    assert!(
        output
            .services
            .iter()
            .any(|service| service.bundle_hash == Hash::new(b"vault-api-bundle"))
    );
}
#[test]
fn sync_manifests_generated_split_app_scaffold_refreshes_service_refs() {
    let (dir, _) = split_app_fixture("sync_manifests_generated_split_app");
    fs::create_dir_all(dir.join("services/live/build")).expect("create live build dir");
    fs::create_dir_all(dir.join("services/vault/build")).expect("create vault build dir");
    fs::write(
        dir.join("services/live/build/live-api.tgz"),
        b"scaffold-live-bundle",
    )
    .expect("write live bundle");
    fs::write(
        dir.join("services/vault/build/vault-api.to"),
        b"scaffold-vault-bundle",
    )
    .expect("write vault bundle");
    let manifest_path = dir.join("app_manifest.json");
    let output = SyncManifestsArgs {
        app_manifest: Some(manifest_path.clone()),
        container: dir.join("unused-container.json"),
        service: dir.join("unused-service.json"),
        bundle_file: None,
    }
    .run()
    .expect("app sync should succeed");
    assert_eq!(
        output.app_manifest_path.as_deref(),
        Some(manifest_path.to_string_lossy().as_ref())
    );
    assert_eq!(output.services.len(), 2);
    assert!(
        output
            .services
            .iter()
            .any(|service| service.bundle_hash == Hash::new(b"scaffold-live-bundle"))
    );
    assert!(
        output
            .services
            .iter()
            .any(|service| service.bundle_hash == Hash::new(b"scaffold-vault-bundle"))
    );
    let live_container: UnpublishedContainerManifestV1 =
        load_json(&dir.join("services/live/container_manifest.json")).expect("live container");
    let live_service: SoraServiceManifestV1 =
        load_json(&dir.join("services/live/service_manifest.json")).expect("live service");
    assert_eq!(
        live_container.bundle_hash,
        Hash::new(b"scaffold-live-bundle")
    );
    assert_eq!(
        live_service.container.manifest_hash,
        live_container
            .workspace_hash()
            .expect("hash synced live source container")
    );
    let vault_container: SoraContainerManifestV1 =
        load_json(&dir.join("services/vault/container_manifest.json")).expect("vault container");
    let vault_service: SoraServiceManifestV1 =
        load_json(&dir.join("services/vault/service_manifest.json")).expect("vault service");
    assert_eq!(
        vault_container.bundle_hash,
        Hash::new(b"scaffold-vault-bundle")
    );
    assert_eq!(
        vault_service.container.manifest_hash,
        Hash::new(Encode::encode(&vault_container))
    );
}
#[test]
fn sync_manifests_generated_single_api_scaffold_refreshes_service_refs() {
    let (dir, _) = single_api_fixture("sync_manifests_generated_single_api");
    fs::create_dir_all(dir.join("services/api/build")).expect("create api build dir");
    fs::write(
        dir.join("services/api/build/api-service.to"),
        b"scaffold-api-bundle",
    )
    .expect("write api bundle");
    let manifest_path = dir.join("app_manifest.json");
    let output = SyncManifestsArgs {
        app_manifest: Some(manifest_path.clone()),
        container: dir.join("unused-container.json"),
        service: dir.join("unused-service.json"),
        bundle_file: None,
    }
    .run()
    .expect("app sync should succeed");
    assert_eq!(
        output.app_manifest_path.as_deref(),
        Some(manifest_path.to_string_lossy().as_ref())
    );
    assert_eq!(output.services.len(), 1);
    assert_eq!(
        output.services[0].bundle_hash,
        Hash::new(b"scaffold-api-bundle")
    );
    let container: SoraContainerManifestV1 =
        load_json(&dir.join("services/api/container_manifest.json")).expect("container");
    let service: SoraServiceManifestV1 =
        load_json(&dir.join("services/api/service_manifest.json")).expect("service");
    assert_eq!(container.bundle_hash, Hash::new(b"scaffold-api-bundle"));
    assert_eq!(
        service.container.manifest_hash,
        Hash::new(Encode::encode(&container))
    );
}
#[test]
fn app_status_filters_control_plane_status_to_split_app_services() {
    let (dir, _) = split_app_fixture("split_app_status");
    let payload = mock_control_plane_status_payload(&[
        "travel-ops_live",
        "travel-ops_vault",
        "unrelated_service",
    ]);
    let encoded = json::to_vec(&payload).expect("encode status payload");
    let app_encoded = json::to_vec(&app_infra_status_fixture_for_name("travel_ops"))
        .expect("encode exact app-infra status payload");
    let server = MockHttpServer::start(BTreeMap::from([
        (
            "/v1/soracloud/apps/travel_ops/status".to_owned(),
            MockHttpResponse::json(app_encoded),
        ),
        (
            "/v1/soracloud/status".to_owned(),
            MockHttpResponse::json(encoded),
        ),
    ]));
    install_mock_protected_read_signer();
    let output = AppStatusArgs {
        manifest: dir.join("app_manifest.json"),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run()
    .expect("app status should succeed");
    assert_eq!(output.app_name, "travel_ops");
    assert_eq!(output.hostname, "travel-ops.sora");
    assert_eq!(output.public_url, "https://travel-ops.sora");
    assert!(output.manifest_path.ends_with("app_manifest.json"));
    assert!(output.workspace_dir.contains("split_app_status"));
    assert_app_workspace_scripts(&output.workspace_scripts);
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
    assert_eq!(
        output
            .frontend
            .as_ref()
            .and_then(|frontend| frontend.root_binding_url.as_deref()),
        None
    );
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
    let live = output
        .services
        .iter()
        .find(|service| service.service_name == "travel-ops_live")
        .expect("app status should retain the live service entry");
    assert!(live.present_in_control_plane);
    assert_eq!(live.execution_plane, "HttpService");
    assert_eq!(live.runtime, "Inrou");
    assert_eq!(live.route_path_prefix.as_deref(), Some("/api/v1"));
    assert!(
        live.container_manifest_path
            .ends_with("services/live/container_manifest.json")
    );
    assert!(
        live.service_manifest_path
            .ends_with("services/live/service_manifest.json")
    );
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
    assert_eq!(
        live.status
            .as_ref()
            .and_then(|status| status.get("current_version"))
            .and_then(norito::json::Value::as_str),
        Some("1.0.0")
    );
    let vault = output
        .services
        .iter()
        .find(|service| service.service_name == "travel-ops_vault")
        .expect("app status should retain the vault service entry");
    assert!(vault.present_in_control_plane);
    assert_eq!(vault.execution_plane, "DeterministicService");
    assert_eq!(vault.runtime, "Ivm");
    assert_eq!(vault.route_path_prefix.as_deref(), Some("/api"));
    assert!(
        vault
            .container_manifest_path
            .ends_with("services/vault/container_manifest.json")
    );
    assert!(
        vault
            .service_manifest_path
            .ends_with("services/vault/service_manifest.json")
    );
    assert!(vault.workspace_dir.ends_with("services/vault"));
    assert_optional_path_ends_with(
        vault.workspace_scripts.dev.as_deref(),
        "services/vault/dev.sh",
    );
    assert_optional_path_ends_with(
        vault.workspace_scripts.build.as_deref(),
        "services/vault/build.sh",
    );
    assert_optional_path_ends_with(
        vault.workspace_scripts.verify_build.as_deref(),
        "services/vault/verify-build.sh",
    );
    assert_eq!(
        vault
            .status
            .as_ref()
            .and_then(|status| status.get("current_version"))
            .and_then(norito::json::Value::as_str),
        Some("1.0.0")
    );
}
#[test]
fn app_status_keeps_missing_manifest_services_visible() {
    let (dir, _) = split_app_fixture("split_app_status_missing_service");
    let payload = mock_control_plane_status_payload(&["travel-ops_live"]);
    let encoded = json::to_vec(&payload).expect("encode status payload");
    let app_encoded = json::to_vec(&app_infra_status_fixture_for_name("travel_ops"))
        .expect("encode exact app-infra status payload");
    let server = MockHttpServer::start(BTreeMap::from([
        (
            "/v1/soracloud/apps/travel_ops/status".to_owned(),
            MockHttpResponse::json(app_encoded),
        ),
        (
            "/v1/soracloud/status".to_owned(),
            MockHttpResponse::json(encoded),
        ),
    ]));
    install_mock_protected_read_signer();
    let output = AppStatusArgs {
        manifest: dir.join("app_manifest.json"),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run()
    .expect("app status should succeed");
    assert!(output.manifest_path.ends_with("app_manifest.json"));
    assert_eq!(output.hostname, "travel-ops.sora");
    assert!(
        output
            .workspace_dir
            .contains("split_app_status_missing_service")
    );
    assert_app_workspace_scripts(&output.workspace_scripts);
    assert_eq!(output.services.len(), 2);
    let live = output
        .services
        .iter()
        .find(|service| service.service_name == "travel-ops_live")
        .expect("live service should remain present");
    assert!(live.present_in_control_plane);
    assert!(live.status.is_some());
    let vault = output
        .services
        .iter()
        .find(|service| service.service_name == "travel-ops_vault")
        .expect("vault service should remain visible even when absent");
    assert!(!vault.present_in_control_plane);
    assert!(vault.status.is_none());
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
    assert!(
        vault
            .notes
            .iter()
            .any(|note| { note.contains("no matching Torii control-plane entry was returned") })
    );
}
#[test]
fn app_status_projects_single_api_frontend_root_binding_url() {
    let (dir, _) = single_api_fixture("single_api_status_root_binding");
    let payload = mock_control_plane_status_payload(&["travel-ops_api"]);
    let encoded = json::to_vec(&payload).expect("encode status payload");
    let app_encoded = json::to_vec(&app_infra_status_fixture_for_name("travel_ops"))
        .expect("encode exact app-infra status payload");
    let server = MockHttpServer::start(BTreeMap::from([
        (
            "/v1/soracloud/apps/travel_ops/status".to_owned(),
            MockHttpResponse::json(app_encoded),
        ),
        (
            "/v1/soracloud/status".to_owned(),
            MockHttpResponse::json(encoded),
        ),
    ]));
    install_mock_protected_read_signer();
    let output = AppStatusArgs {
        manifest: dir.join("app_manifest.json"),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run()
    .expect("app status should succeed");
    assert!(output.manifest_path.ends_with("app_manifest.json"));
    assert_eq!(output.hostname, "travel-ops.sora");
    assert!(
        output
            .workspace_dir
            .contains("single_api_status_root_binding")
    );
    assert_app_workspace_scripts(&output.workspace_scripts);
    assert!(!output.has_mixed_planes);
    assert_eq!(output.hosted_http_service_count, 0);
    assert_eq!(output.deterministic_service_count, 1);
    assert_eq!(
        output
            .frontend
            .as_ref()
            .map(|frontend| frontend.publish_mode.as_str()),
        Some(APP_STATIC_SITE_PUBLISH_MODE_ROOT_BINDING)
    );
    assert_eq!(
        output
            .frontend
            .as_ref()
            .and_then(|frontend| frontend.root_binding_url.as_deref()),
        Some("https://travel-ops.sora/")
    );
    assert_eq!(
        output
            .frontend
            .as_ref()
            .and_then(|frontend| frontend.cid_gateway_url_template.as_deref()),
        None
    );
    assert!(
        output
            .routes
            .iter()
            .any(|route| route.service_name == "travel-ops_api" && route.path == "/api/healthz")
    );
}
#[test]
fn app_deploy_single_api_root_binding_injects_reserved_static_site_config() {
    let (dir, _) = single_api_fixture("single_api_root_binding_deploy");
    fs::create_dir_all(dir.join("web/dist")).expect("create web dist dir");
    fs::write(
        dir.join("web/dist/index.html"),
        "<!doctype html><title>Travel Ops</title>",
    )
    .expect("write web index");
    fs::create_dir_all(dir.join("services/api/build")).expect("create api build dir");
    fs::write(
        dir.join("services/api/build/api-service.to"),
        b"deploy-single-api-bundle",
    )
    .expect("write api bundle");
    sync_test_app_manifests(&dir);
    let key_pair = soracloud_fixture_key_pair(0x43);
    let authority = AccountId::new(key_pair.public_key().clone());
    let status_payload = mock_control_plane_status_payload(&["travel-ops_api"]);
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
    let output = AppReleaseMutationArgs {
        manifest: dir.join("app_manifest.json"),
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        inrou_preseed_receipt: None,
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(MutationMode::Deploy, &authority, &key_pair)
    .expect("app deploy should succeed");
    assert_eq!(output.hostname, "travel-ops.sora");
    assert!(output.manifest_path.ends_with("app_manifest.json"));
    assert!(
        output
            .workspace_dir
            .contains("single_api_root_binding_deploy")
    );
    assert_app_workspace_scripts(&output.workspace_scripts);
    assert!(!output.has_mixed_planes);
    assert_eq!(output.hosted_http_service_count, 0);
    assert_eq!(output.deterministic_service_count, 1);
    assert_eq!(output.services.len(), 1);
    assert!(
        output
            .routes
            .iter()
            .any(|route| route.service_name == "travel-ops_api" && route.path == "/api/healthz")
    );
    assert_eq!(
        output
            .frontend
            .as_ref()
            .map(|frontend| frontend.publish_mode.as_str()),
        Some(APP_STATIC_SITE_PUBLISH_MODE_ROOT_BINDING)
    );
    assert_eq!(
        output
            .frontend
            .as_ref()
            .and_then(|frontend| frontend.root_binding_url.as_deref()),
        Some("https://travel-ops.sora/")
    );
    let api_service = output
        .services
        .iter()
        .find(|service| service.service_name == "travel-ops_api")
        .expect("single-api deploy should retain the API manifest entry");
    assert_eq!(api_service.execution_plane, "DeterministicService");
    assert_eq!(api_service.runtime, "Ivm");
    assert_eq!(api_service.route_path_prefix.as_deref(), Some("/api"));
    assert!(api_service.workspace_dir.ends_with("services/api"));
    assert_optional_path_ends_with(
        api_service.workspace_scripts.dev.as_deref(),
        "services/api/dev.sh",
    );
    assert_optional_path_ends_with(
        api_service.workspace_scripts.build.as_deref(),
        "services/api/build.sh",
    );
    assert_optional_path_ends_with(
        api_service.workspace_scripts.verify_build.as_deref(),
        "services/api/verify-build.sh",
    );
    assert_notes_contain(&api_service.notes, "deterministic IVM");
    let publication = output
        .published_static_site
        .as_ref()
        .expect("root-binding app should publish a static site");
    assert_eq!(publication.public_url, "https://travel-ops.sora");
    assert!(publication.content_cid.starts_with('b'));
    assert_eq!(
        publication.cid_gateway_url,
        format!(
            "https://travel-ops.sora/sorafs/cid/{}",
            publication.content_cid
        )
    );
    assert!(
        server.requests().iter().any(|request| {
            request.method == "POST" && request.path == "/v1/sorafs/pin/register"
        })
    );
    let deploy_request = server
        .requests()
        .into_iter()
        .find(|request| request.method == "POST" && request.path == "/v1/soracloud/apps/deploy")
        .expect("deploy request should be captured");
    let deploy_body: norito::json::Value =
        json::from_slice(&deploy_request.body).expect("decode deploy request");
    let initial_configs = deploy_body
        .get("deploy_services")
        .and_then(norito::json::Value::as_array)
        .and_then(|services| services.first())
        .and_then(|service| service.get("initial_service_configs"))
        .and_then(norito::json::Value::as_object)
        .expect("deploy request must include initial service configs");
    let binding = initial_configs
        .get(APP_STATIC_SITE_CONFIG_NAME)
        .expect("root-binding deploy must attach reserved static-site config");
    assert_eq!(
        binding
            .get("hostname")
            .and_then(norito::json::Value::as_str),
        Some("travel-ops.sora")
    );
    assert_eq!(
        binding
            .get("mount_path")
            .and_then(norito::json::Value::as_str),
        Some("/")
    );
    assert_eq!(
        binding
            .get("api_base_path")
            .and_then(norito::json::Value::as_str),
        Some("/api")
    );
}
#[test]
fn app_deploy_rejects_app_service_name_mismatch_before_network_mutation() {
    let (dir, _) = split_app_fixture("app_deploy_service_name_mismatch");
    let manifest_path = dir.join("app_manifest.json");
    let mut manifest: SoracloudAppManifestV1 = load_json(&manifest_path).expect("app manifest");
    manifest.services[0].service_name = "wrong_live_name".to_owned();
    write_json(&manifest_path, &manifest).expect("write mismatched app manifest");
    let server = MockHttpServer::start(BTreeMap::new());
    let key_pair = soracloud_fixture_key_pair(0x44);
    let authority = AccountId::new(key_pair.public_key().clone());
    install_mock_submission_config(&authority, &key_pair);
    let error = AppReleaseMutationArgs {
        manifest: manifest_path,
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        inrou_preseed_receipt: None,
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(MutationMode::Deploy, &authority, &key_pair)
    .expect_err("app deploy should fail on mismatched service name");
    assert!(error.to_string().contains("wrong_live_name"));
    assert!(
        server.requests().is_empty(),
        "preflight mismatch should fail before any Soracloud or Sorafs network mutation"
    );
}
