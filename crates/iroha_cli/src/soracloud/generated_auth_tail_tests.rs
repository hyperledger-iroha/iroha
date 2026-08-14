#[test]
fn generated_pii_app_auth_smoke_supports_shared_sessions_and_cross_replica_logout_invalidation() {
    let dir = temp_dir("pii_auth_shared_session_smoke");
    InitArgs {
        output_dir: dir.clone(),
        service_name: "clinic_console".to_owned(),
        service_version: "1.0.0".to_owned(),
        template: InitTemplate::PiiApp,
        overwrite: false,
    }
    .run()
    .expect("pii-app init should succeed");
    let server_path = dir.join("pii-app/api/server.mjs");
    if !node_available() {
        eprintln!("node unavailable; validating static pii replay/session markers in scaffold");
        let api = fs::read_to_string(&server_path).expect("read pii api");
        assert!(api.contains("AUTH_CHALLENGE_REPLAYED"));
        assert!(api.contains("AUTH_CHALLENGE_EXPIRED"));
        assert!(api.contains("AUTH_CHALLENGE_NOT_FOUND"));
        assert!(api.contains("AUTH_CHALLENGE_PRINCIPAL_MISMATCH"));
        assert!(api.contains("AUTH_SIGNATURE_INVALID"));
        assert!(api.contains("AUTH_SESSION_PREFIX"));
        assert!(api.contains("SameSite=Strict"));
        assert!(api.contains("/pii/api/consent/state"));
        return;
    }
    let state_file = dir.join(".shared_auth_state.json");
    let harness_path = dir.join("pii_auth_shared_session_smoke.mjs");
    let mut script = include_str!("templates/v1/pii_auth_shared_session_smoke.mjs").to_owned();
    script = script.replace("__SERVER_PATH__", &js_string_literal(&server_path));
    script = script.replace("__STATE_FILE__", &js_string_literal(&state_file));
    fs::write(&harness_path, script).expect("write node harness");
    run_node_harness(&harness_path);
}
#[test]
fn legacy_health_app_template_selector_is_rejected() {
    use clap::ValueEnum;
    let parsed =
        <InitTemplate as ValueEnum>::from_str("health-app", true).expect_err("must reject");
    assert!(
        parsed.contains("health-app"),
        "error message should mention rejected selector: {parsed}"
    );
    let parsed_new =
        <InitTemplate as ValueEnum>::from_str("pii-app", true).expect("pii-app must parse");
    assert_eq!(parsed_new, InitTemplate::PiiApp);
    let parsed_hayahi =
        <InitTemplate as ValueEnum>::from_str("hayahi-app", true).expect("hayahi-app must parse");
    assert_eq!(parsed_hayahi, InitTemplate::HayahiApp);
    let parsed_http_service = <InitTemplate as ValueEnum>::from_str("http-service", true)
        .expect("http-service must parse");
    assert_eq!(parsed_http_service, InitTemplate::HttpService);
    let parsed_split_app =
        <AppInitTemplate as ValueEnum>::from_str("split-app", true).expect("split-app must parse");
    assert_eq!(parsed_split_app, AppInitTemplate::SplitApp);
}
