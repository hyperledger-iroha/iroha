fn por_weekly_report_fixture(cycle: PorReportIsoWeek) -> PorWeeklyReportV1 {
    let provider_summary = PorProviderSummaryV1 {
        provider_id: [0x88; 32],
        manifest_count: 12,
        challenges: 96,
        successes: 94,
        failures: 2,
        forced: 0,
        success_rate_bps: 9_791,
        first_failure_at: Some(1_700_000_300),
        last_success_latency_ms_p95: Some(1_850),
        repair_dispatched: true,
        pending_repairs: 1,
        ticket_id: Some("REP-123".to_string()),
    };
    let slashing_event = PorSlashingEventV1 {
        provider_id: [0x90; 32],
        manifest_digest: [0x91; 32],
        penalty_xor: XorQuantity::try_from_micro(250_000_000)
            .expect("legacy micro-XOR value is representable"),
        verdict_cid: "ipfs://verdict".to_string(),
        decided_at: 1_700_000_200,
    };
    let report = PorWeeklyReportV1 {
        version: POR_WEEKLY_REPORT_VERSION_V1,
        cycle,
        generated_at: 1_700_000_400,
        challenges_total: 128,
        challenges_verified: 120,
        challenges_failed: 8,
        forced_challenges: 2,
        repairs_enqueued: 4,
        repairs_completed: 3,
        mean_latency_ms: Some(820),
        p95_latency_ms: Some(1_980),
        slashing_events: vec![slashing_event],
        providers_missing_vrf: vec![[0x77; 32]],
        top_offenders: vec![provider_summary],
        notes: Some("All forced challenges recovered within SLA.".to_string()),
    };
    report.validate().expect("report validates");
    report
}

#[test]
fn por_report_rejects_response_for_a_different_week_before_output() {
    let server = MockServer::start();
    let report = por_weekly_report_fixture(PorReportIsoWeek {
        year: 2025,
        week: 13,
    });
    let body = to_bytes(&report).expect("encode substituted-cycle report");
    server.mock(|when, then| {
        when.method(GET).path("/v1/sorafs/por/report/2025-W12");
        then.status(200)
            .header("content-type", "application/x-norito")
            .body(body);
    });

    let output = sorafs_cli_cmd()
        .arg("por")
        .arg("report")
        .arg(format!("--torii-url={}", server.base_url()))
        .arg("--week=2025-W12")
        .output()
        .expect("command executes");

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "a report for a different week must not be rendered"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
    assert!(
        stderr.contains("cycle does not match the requested week"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn por_export_rejects_cross_origin_redirect_before_writing() {
    let origin = MockServer::start();
    let substituted_origin = MockServer::start();
    let payload = to_bytes(&TestPorStatusExportPageV1 {
        version: 1,
        start_epoch: None,
        end_epoch: None,
        page: test_por_status_page(Vec::new(), None),
    })
    .expect("encode substituted PoR export page");
    let substituted_response = substituted_origin.mock(|when, then| {
        when.method(GET).path("/substituted-export");
        then.status(200)
            .header("content-type", "application/octet-stream")
            .body(payload);
    });
    let location = substituted_origin.url("/substituted-export");
    let redirect = origin.mock(|when, then| {
        when.method(GET).path("/v1/sorafs/por/export");
        then.status(302).header("location", location.as_str());
    });

    let tempdir = tempdir().expect("tempdir");
    let out_path = tempdir.path().join("redirected-export.norito");
    let output = sorafs_cli_cmd()
        .arg("por")
        .arg("export")
        .arg(format!("--torii-url={}", origin.base_url()))
        .arg(format!("--out={}", out_path.display()))
        .output()
        .expect("command executes");

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "a redirected export must not report success"
    );
    assert!(
        !out_path.exists(),
        "a redirected export must not be written"
    );
    redirect.assert_calls(1);
    substituted_response.assert_calls(0);
}

#[test]
fn por_report_rejects_cross_origin_redirect_before_output() {
    let origin = MockServer::start();
    let substituted_origin = MockServer::start();
    let report = por_weekly_report_fixture(PorReportIsoWeek {
        year: 2025,
        week: 12,
    });
    let body = to_bytes(&report).expect("encode substituted PoR report");
    let substituted_response = substituted_origin.mock(|when, then| {
        when.method(GET).path("/substituted-report");
        then.status(200)
            .header("content-type", "application/x-norito")
            .body(body);
    });
    let location = substituted_origin.url("/substituted-report");
    let redirect = origin.mock(|when, then| {
        when.method(GET).path("/v1/sorafs/por/report/2025-W12");
        then.status(307).header("location", location.as_str());
    });

    let output = sorafs_cli_cmd()
        .arg("por")
        .arg("report")
        .arg(format!("--torii-url={}", origin.base_url()))
        .arg("--week=2025-W12")
        .output()
        .expect("command executes");

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "a redirected report must not be rendered"
    );
    redirect.assert_calls(1);
    substituted_response.assert_calls(0);
}

#[test]
fn manifest_submit_rejects_cross_origin_redirect_before_response_output() {
    let tempdir = tempdir().expect("tempdir");
    let (authority, private_key) = deterministic_ed25519_authority_and_private_key();
    let (manifest_path, plan_path) = prepare_manifest_artifacts(tempdir.path());
    let origin = MockServer::start();
    let substituted_origin = MockServer::start();
    let substituted_response = substituted_origin.mock(|when, then| {
        when.method(POST).path("/substituted-pin-register");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"status":"ok"}"#);
    });
    let location = substituted_origin.url("/substituted-pin-register");
    let redirect = origin.mock(|when, then| {
        when.method(POST).path("/v1/sorafs/pin/register");
        then.status(307)
            .header("location", location.as_str())
            .body("redirect forbidden");
    });

    let summary_path = tempdir.path().join("redirected-submit-summary.json");
    let response_path = tempdir.path().join("redirected-submit-response.json");
    let output = sorafs_cli_cmd()
        .arg("manifest")
        .arg("submit")
        .arg(format!("--manifest={}", manifest_path.display()))
        .arg(format!("--chunk-plan={}", plan_path.display()))
        .arg(format!("--torii-url={}", origin.base_url()))
        .arg(format!("--network-id={TEST_NETWORK_ID_LITERAL}"))
        .arg(format!("--authority={authority}"))
        .arg(format!("--private-key={private_key}"))
        .arg(format!("--summary-out={}", summary_path.display()))
        .arg(format!("--response-out={}", response_path.display()))
        .output()
        .expect("command executes");

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "a redirected submit must emit no summary"
    );
    assert!(
        !summary_path.exists(),
        "a redirected submit must write no summary"
    );
    assert!(
        !response_path.exists(),
        "a redirected submit must write no response"
    );
    redirect.assert_calls(1);
    substituted_response.assert_calls(0);
}

include!("moderation_redirect.rs");
