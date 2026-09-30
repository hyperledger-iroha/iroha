//! Regression tests: attached mode keeps rendering even with slow peers.
use std::{
    process::{Command, Stdio},
    thread,
    time::Duration,
};
const STUB_STATUS_BODY: &str = "{\"alias\":\"雅\",\"peers\":2,\"blocks\":5,\"blocks_non_empty\":4,\"commit_time_ms\":110,\"txs_approved\":20,\"txs_rejected\":1,\"queue_size\":1,\"uptime\":10,\"view_changes\":0,\"governance\":{\"proposals\":{\"proposed\":0,\"rejected\":0,\"enacted\":0,\"superseded\":0,\"execution_failed\":0},\"protected_namespace\":{\"total_checks\":0,\"allowed\":0,\"rejected\":0},\"manifest_quorum\":{\"total_checks\":0,\"satisfied\":0,\"rejected\":0},\"recent_manifest_activations\":[]}}";
#[test]
fn attach_mode_with_slow_peer_renders_multiple_frames() {
    let _serial = crate::serial_guard();
    let Some(slow_stub) = spawn_status_metrics_stub(Duration::from_millis(600)) else {
        eprintln!("skipping attach_mode_with_slow_peer_renders_multiple_frames: no slow stub addr");
        return;
    };
    let Some(fast_stub) = spawn_status_metrics_stub(Duration::from_millis(50)) else {
        eprintln!("skipping attach_mode_with_slow_peer_renders_multiple_frames: no fast stub addr");
        return;
    };
    let bin = crate::monitor_bin();
    let mut child = Command::new(bin)
        .args([
            "--attach",
            &format!("http://{}", slow_stub.addr),
            &format!("http://{}", fast_stub.addr),
            "--interval",
            "250",
            "--no-theme",
        ])
        .env("TERM", "dumb")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn iroha_monitor --attach with slow peer");
    thread::sleep(Duration::from_millis(2200));
    let _ = child.kill();
    let output = child
        .wait_with_output()
        .expect("wait for iroha_monitor output");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let frames = stdout.matches("[headless]").count();
    assert!(
        frames >= 2,
        "expected monitor to render multiple headless frames; frames={frames}, stdout sample={}",
        stdout.chars().take(256).collect::<String>()
    );
    assert!(
        stdout.contains("telemetry online"),
        "expected the retained peer stubs to provide telemetry: {stdout}"
    );
}
fn spawn_status_metrics_stub(delay: Duration) -> Option<crate::StatusStub> {
    use axum::{Router, response::IntoResponse, routing::get};
    let app = Router::new()
        .route(
            "/status",
            get(move || {
                let delay = delay;
                async move {
                    tokio::time::sleep(delay).await;
                    (
                        axum::http::StatusCode::OK,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        STUB_STATUS_BODY,
                    )
                        .into_response()
                }
            }),
        )
        .route(
            "/metrics",
            get(
                || async move { "block_gas_used 200\nblock_fee_total_units 100\n".into_response() },
            ),
        );
    crate::serve_stub(app)
}
