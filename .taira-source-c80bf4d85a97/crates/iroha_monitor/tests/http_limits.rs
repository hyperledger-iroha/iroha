//! Integration test: status payload limit warnings bubble into the TUI.

use std::{
    net::{Ipv4Addr, SocketAddr, TcpListener},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

fn monitor_bin() -> Option<PathBuf> {
    std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)
}

#[test]
fn status_limit_warning_is_rendered() {
    let Some(addr) = spawn_oversized_status_stub() else {
        eprintln!("skipping status_limit_warning_is_rendered: no stub addr");
        return;
    };

    let Some(bin) = monitor_bin() else {
        eprintln!("skipping: monitor binary path not provided by cargo");
        return;
    };

    let mut child = Command::new(bin)
        .args([
            "--attach",
            &format!("http://{addr}"),
            "--interval",
            "250",
            "--no-theme",
        ])
        .env("TERM", "dumb")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn iroha_monitor for status limit test");

    thread::sleep(Duration::from_millis(1600));

    let _ = child.kill();
    let output = child
        .wait_with_output()
        .expect("wait for iroha_monitor output");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("body exceeds"),
        "expected warning about status payload limit, stdout sample={}",
        stdout.chars().take(256).collect::<String>()
    );
}

fn spawn_oversized_status_stub() -> Option<SocketAddr> {
    let listener = match TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("stub bind failed: {err}");
            return None;
        }
    };
    listener
        .set_nonblocking(true)
        .expect("set stub listener nonblocking");
    let addr = match listener.local_addr() {
        Ok(addr) => addr,
        Err(err) => {
            eprintln!("stub local addr failed: {err}");
            return None;
        }
    };
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
            let app = axum::Router::new()
                .route(
                    "/status",
                    axum::routing::get({
                        move || async move {
                            let payload = format!(
                                "{{\"alias\":\"巨大\",\"pad\":\"{}\"}}",
                                "x".repeat(200_000)
                            );
                            axum::http::Response::builder()
                                .status(axum::http::StatusCode::OK)
                                .header(axum::http::header::CONTENT_TYPE, "application/json")
                                .body(axum::body::Body::from(payload))
                                .expect("stub response")
                        }
                    }),
                )
                .route(
                    "/metrics",
                    axum::routing::get(|| async move { "block_gas_used 1\n" }),
                );
            if let Err(err) = axum::serve(listener, app).await {
                eprintln!("stub server error: {err}");
            }
        });
    });
    Some(addr)
}
