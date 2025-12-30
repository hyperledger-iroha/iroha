//! Smoke tests for the refactored `iroha_monitor` CLI.

use std::{
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

fn monitor_bin() -> Option<PathBuf> {
    std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)
}

#[test]
fn spawn_lite_smoke_renders_frames() {
    let Some(bin) = monitor_bin() else {
        eprintln!("skipping: monitor binary path not provided by cargo");
        return;
    };

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
        stdout.contains("祭 Matsuri Vision"),
        "expected Matsuri header in stdout, got: {}",
        stdout.chars().take(200).collect::<String>()
    );
}

#[test]
fn headless_max_frames_triggers_auto_exit() {
    let Some(bin) = monitor_bin() else {
        eprintln!("skipping: monitor binary path not provided by cargo");
        return;
    };

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
    let Some(addr1) = spawn_status_metrics_stub() else {
        eprintln!("skipping attach_mode_with_stubs_runs_cleanly: no stub addr");
        return;
    };
    let Some(addr2) = spawn_status_metrics_stub() else {
        eprintln!("skipping attach_mode_with_stubs_runs_cleanly: no stub addr");
        return;
    };

    let Some(bin) = monitor_bin() else {
        eprintln!("skipping: monitor binary path not provided by cargo");
        return;
    };

    let mut child = Command::new(bin)
        .args([
            "--attach",
            &format!("http://{addr1}"),
            &format!("http://{addr2}"),
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

    assert!(stdout.contains("Festival whispers"));
    assert!(stderr.trim().is_empty(), "stderr should be empty: {stderr}");
}

fn spawn_status_metrics_stub() -> Option<std::net::SocketAddr> {
    use axum::{Router, response::IntoResponse, routing::get};

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async move {
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
        let app = Router::new()
            .route(
                "/status",
                get(|| async move {
                    const BODY: &str =
                        "{\"alias\":\"祭りノード\",\"peers\":2,\"blocks\":4,\"blocks_non_empty\":3,\"commit_time_ms\":90,\"txs_approved\":12,\"txs_rejected\":0,\"queue_size\":0,\"uptime\":1,\"view_changes\":0,\"governance\":{\"proposals\":{\"proposed\":0,\"approved\":0,\"rejected\":0,\"enacted\":0},\"protected_namespace\":{\"total_checks\":0,\"allowed\":0,\"rejected\":0},\"manifest_quorum\":{\"total_checks\":0,\"satisfied\":0,\"rejected\":0},\"recent_manifest_activations\":[]}}";
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
        tokio::spawn(async move {
            if let Err(err) = axum::serve(listener, app).await {
                eprintln!("stub server error: {err}");
            }
        });
        Some(addr)
    })
}
