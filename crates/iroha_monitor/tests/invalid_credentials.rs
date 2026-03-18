//! CLI regression tests for error surfaces.

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
fn invalid_endpoint_surfaces_warning() {
    let Some(bin) = monitor_bin() else {
        eprintln!("skipping: monitor binary path not provided by cargo");
        return;
    };

    let mut child = Command::new(bin)
        .args([
            "--attach",
            "http://127.0.0.1:65535", // unreachable
            "--interval",
            "200",
            "--no-theme",
        ])
        .env("TERM", "dumb")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn iroha_monitor with unreachable endpoint");

    thread::sleep(Duration::from_millis(900));

    let _ = child.kill();
    let output = child
        .wait_with_output()
        .expect("wait for iroha_monitor output");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[headless]") && stdout.to_ascii_lowercase().contains("/status failed"),
        "expected status fetch failure in stdout, sample={}",
        stdout.chars().take(256).collect::<String>()
    );
}
