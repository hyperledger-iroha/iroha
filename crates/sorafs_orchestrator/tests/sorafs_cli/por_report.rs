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
