//! Tests for the `SoraFS` chaos drill logging helpers.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .expect("crate directory has parent")
        .parent()
        .expect("workspace root is the parent of crates/")
        .to_path_buf()
}

fn telemetry_script(name: &str) -> PathBuf {
    workspace_root()
        .join("scripts")
        .join("telemetry")
        .join(name)
}

fn run_command(mut command: Command) {
    let display = format!("{command:?}");
    let output = command
        .output()
        .expect("failed to spawn drill logging command");
    assert!(
        output.status.success(),
        "command {display} failed: stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn log_entry(log_path: &Path, scenario: &str, status: &str, date: &str) {
    let log_script = telemetry_script("log_sorafs_drill.sh");
    let mut command = Command::new(log_script);
    command
        .arg("--scenario")
        .arg(scenario)
        .arg("--status")
        .arg(status)
        .arg("--date")
        .arg(date)
        .arg("--start")
        .arg("10:00Z")
        .arg("--end")
        .arg("10:30Z")
        .arg("--ic")
        .arg("Automation Bot")
        .arg("--scribe")
        .arg("Observer Bot")
        .arg("--notes")
        .arg("Dry-run executed for automation verification")
        .arg("--link")
        .arg("ops://dry-run")
        .arg("--log")
        .arg(log_path);
    run_command(command);
}

#[test]
fn log_and_validate_sorafs_drill_entries() {
    let repo_root = workspace_root();
    let log_dir = repo_root
        .join("target")
        .join("test-artifacts")
        .join("sorafs-drill-log");
    fs::create_dir_all(&log_dir).expect("failed to create test artifact directory");

    let log_path = log_dir.join("drill-log.md");
    if log_path.exists() {
        fs::remove_file(&log_path).expect("failed to remove stale test drill log");
    }

    log_entry(
        &log_path,
        "Gateway outage chaos drill (dry run)",
        "pass",
        "2025-03-01",
    );
    log_entry(
        &log_path,
        "Proof failure surge drill (dry run)",
        "scheduled",
        "2025-03-02",
    );

    let validate_script = telemetry_script("validate_drill_log.sh");
    let mut validate_command = Command::new(validate_script);
    validate_command.arg(&log_path);
    run_command(validate_command);

    let log_contents = fs::read_to_string(&log_path).expect("failed to read generated drill log");
    assert!(
        log_contents.contains("| 2025-03-01 | Gateway outage chaos drill (dry run) | pass |"),
        "expected gateway outage entry to be logged: {log_contents}"
    );
    assert!(
        log_contents.contains("| 2025-03-02 | Proof failure surge drill (dry run) | scheduled |"),
        "expected proof failure entry to be logged: {log_contents}"
    );
}
