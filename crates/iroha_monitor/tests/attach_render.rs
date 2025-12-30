//! Regression tests: attached mode keeps rendering even with slow peers.

use std::{
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

const STUB_STATUS_BODY: &str = "{\"alias\":\"雅\",\"peers\":2,\"blocks\":5,\"blocks_non_empty\":4,\"commit_time_ms\":110,\"txs_approved\":20,\"txs_rejected\":1,\"queue_size\":1,\"uptime\":10,\"view_changes\":0,\"governance\":{\"proposals\":{\"proposed\":0,\"approved\":0,\"rejected\":0,\"enacted\":0},\"protected_namespace\":{\"total_checks\":0,\"allowed\":0,\"rejected\":0},\"manifest_quorum\":{\"total_checks\":0,\"satisfied\":0,\"rejected\":0},\"recent_manifest_activations\":[]}}";

fn monitor_bin() -> Option<PathBuf> {
    std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)
}

#[test]
fn attach_mode_with_slow_peer_renders_multiple_frames() {
    let Some(slow_addr) = spawn_status_metrics_stub(Duration::from_millis(600)) else {
        eprintln!("skipping attach_mode_with_slow_peer_renders_multiple_frames: no slow stub addr");
        return;
    };
    let Some(fast_addr) = spawn_status_metrics_stub(Duration::from_millis(50)) else {
        eprintln!("skipping attach_mode_with_slow_peer_renders_multiple_frames: no fast stub addr");
        return;
    };

    let Some(bin) = monitor_bin() else {
        eprintln!("skipping: monitor binary path not provided by cargo");
        return;
    };

    let mut child = Command::new(bin)
        .args([
            "--attach",
            &format!("http://{slow_addr}"),
            &format!("http://{fast_addr}"),
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
    let frames = stdout.matches("\u{1b}[2J").count();
    assert!(
        frames >= 2,
        "expected UI to render multiple frames; frames={frames}, stdout sample={}",
        stdout.chars().take(256).collect::<String>()
    );
}

fn spawn_status_metrics_stub(delay: Duration) -> Option<std::net::SocketAddr> {
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
        let app =
            Router::new()
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
                    get(|| async move {
                        "block_gas_used 200\nblock_fee_total_units 100\n".into_response()
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
