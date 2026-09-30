//! Smoke tests for the refactored `iroha_monitor` CLI.
#[path = "attach_render.rs"]
mod attach_render;
#[path = "http_limits.rs"]
mod http_limits;
#[path = "invalid_credentials.rs"]
mod invalid_credentials;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};
fn serial_guard() -> std::sync::MutexGuard<'static, ()> {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
fn monitor_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_iroha_monitor"))
}
struct StatusStub {
    addr: std::net::SocketAddr,
    // The listener and HTTP task stay live until the process assertion finishes.
    _runtime: tokio::runtime::Runtime,
}
fn serve_stub(app: axum::Router) -> Option<StatusStub> {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let addr = runtime.block_on(async move {
        let listener = match tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await
        {
            Ok(listener) => listener,
            Err(err) => {
                eprintln!("stub bind failed: {err}");
                return None;
            }
        };
        let addr = match listener.local_addr() {
            Ok(addr) => addr,
            Err(err) => {
                eprintln!("stub local addr failed: {err}");
                return None;
            }
        };
        tokio::spawn(async move {
            if let Err(err) = axum::serve(listener, app).await {
                eprintln!("stub server error: {err}");
            }
        });
        Some(addr)
    })?;
    Some(StatusStub {
        addr,
        _runtime: runtime,
    })
}
#[test]
fn status_stub_retains_listener_until_its_owner_drops() {
    let _serial = serial_guard();
    let stub = spawn_status_metrics_stub().expect("local status stub");
    let response = attohttpc::get(format!("http://{}/status", stub.addr))
        .send()
        .expect("retained runtime serves status");
    assert!(response.status().is_success());
    let addr = stub.addr;
    drop(stub);
    assert!(std::net::TcpStream::connect(addr).is_err());
}
#[test]
fn spawn_lite_smoke_renders_frames() {
    let _serial = crate::serial_guard();
    let bin = monitor_bin();
    let mut child = Command::new(bin)
        .args([
            "--spawn-lite",
            "--peers",
            "2",
            "--interval",
            "200",
            "--no-theme",
        ])
        .env("TERM", "dumb")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn iroha_monitor --spawn-lite");
    thread::sleep(Duration::from_millis(1200));
    let _ = child.kill();
    let output = child
        .wait_with_output()
        .expect("wait for iroha_monitor output");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[headless]"),
        "expected headless summary in stdout, got: {}",
        stdout.chars().take(200).collect::<String>()
    );
    assert!(
        stdout.contains("UPLINK ESTABLISHED") || stdout.contains("telemetry online"),
        "expected startup or recovery activity in stdout, got: {}",
        stdout.chars().take(300).collect::<String>()
    );
}
#[test]
fn headless_max_frames_triggers_auto_exit() {
    let _serial = crate::serial_guard();
    let bin = monitor_bin();
    let status = Command::new(bin)
        .args([
            "--spawn-lite",
            "--peers",
            "2",
            "--interval",
            "120",
            "--no-theme",
            "--no-audio",
            "--headless-max-frames",
            "3",
        ])
        .env("TERM", "dumb")
        .status()
        .expect("spawn iroha_monitor --spawn-lite with auto-exit");
    assert!(
        status.success(),
        "monitor should exit cleanly with capped frames"
    );
}
#[test]
fn attach_mode_with_stubs_runs_cleanly() {
    let _serial = crate::serial_guard();
    let Some(stub1) = spawn_status_metrics_stub() else {
        eprintln!("skipping attach_mode_with_stubs_runs_cleanly: no stub addr");
        return;
    };
    let Some(stub2) = spawn_status_metrics_stub() else {
        eprintln!("skipping attach_mode_with_stubs_runs_cleanly: no stub addr");
        return;
    };
    let bin = monitor_bin();
    let mut child = Command::new(bin)
        .args([
            "--attach",
            &format!("http://{}", stub1.addr),
            &format!("http://{}", stub2.addr),
            "--interval",
            "250",
            "--no-theme",
        ])
        .env("TERM", "dumb")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn iroha_monitor --attach <stubs>");
    thread::sleep(Duration::from_millis(1500));
    let _ = child.kill();
    let output = child
        .wait_with_output()
        .expect("wait for iroha_monitor output");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("[headless]"));
    assert!(stdout.contains("telemetry online"));
    assert!(
        stderr.contains("falling back to headless output"),
        "expected headless fallback notice in stderr: {stderr}"
    );
}
fn spawn_status_metrics_stub() -> Option<StatusStub> {
    use axum::{Router, response::IntoResponse, routing::get};
    let app = Router::new()
            .route(
                "/status",
                get(|| async move {
                    const BODY: &str =
                        "{\"alias\":\"祭りノード\",\"peers\":2,\"blocks\":4,\"blocks_non_empty\":3,\"commit_time_ms\":90,\"txs_approved\":12,\"txs_rejected\":0,\"queue_size\":0,\"uptime\":1,\"view_changes\":0,\"governance\":{\"proposals\":{\"proposed\":0,\"rejected\":0,\"enacted\":0,\"superseded\":0,\"execution_failed\":0},\"protected_namespace\":{\"total_checks\":0,\"allowed\":0,\"rejected\":0},\"manifest_quorum\":{\"total_checks\":0,\"satisfied\":0,\"rejected\":0},\"recent_manifest_activations\":[]}}";
                    (
                        axum::http::StatusCode::OK,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        BODY,
                    )
                        .into_response()
                }),
            )
            .route(
                "/metrics",
                get(|| async move {
                    let body = "block_gas_used 111\nblock_fee_total_units 222\n";
                    body.into_response()
                }),
            );
    serve_stub(app)
}
