//! Ask the selected daemon whether it accepts a generation before anything starts.
//!
//! A frontend and daemon built from different sources otherwise fail much later, with the cause
//! buried in a validator log. `--check-config` is read-only and never binds a port.

use super::{Error, Result, RootKind, runtime};
use std::{
    io::Read,
    path::Path,
    process::Stdio,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const POLL: Duration = Duration::from_millis(50);
// A reason is one line of a larger message; the daemon's report stays available by rerunning it.
const MAX_REASON_CHARS: usize = 300;

/// Run `iroha3d --check-config --json` on one exact peer configuration.
///
/// Returns why the daemon refused it, or `None` when it reported the configuration ready.
///
/// # Errors
/// The daemon could not be spawned or observed, or `deadline` passed. Running out of time says
/// nothing about the configuration, so it is reported as `Error::Timeout(timeout)`.
pub(super) fn daemon_refusal(
    daemon: &Path,
    config: &Path,
    root_kind: &RootKind,
    deadline: Instant,
    timeout: Duration,
) -> Result<Option<String>> {
    let mut command = runtime::daemon_command(daemon, config, root_kind)?;
    let mut child = command
        .arg("--check-config")
        .arg("--json")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = read_in_background(child.stdout.take());
    let stderr = read_in_background(child.stderr.take());
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        thread::sleep(POLL);
    };
    let (stdout, stderr) = (finish(stdout), finish(stderr));
    match status {
        Some(status) => Ok(interpret(
            status.success(),
            &status.to_string(),
            &stdout,
            &stderr,
        )),
        None => Err(Error::Timeout(timeout)),
    }
}

fn read_in_background(stream: Option<impl Read + Send + 'static>) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stream {
            // A truncated diagnostic is still a refusal; the exit status decides the outcome.
            let _ = stream.read_to_end(&mut bytes);
        }
        bytes
    })
}

fn finish(reader: JoinHandle<Vec<u8>>) -> Vec<u8> {
    reader.join().unwrap_or_default()
}

fn interpret(succeeded: bool, exit: &str, stdout: &[u8], stderr: &[u8]) -> Option<String> {
    if !succeeded {
        return Some(
            last_diagnostic(stderr)
                .or_else(|| last_diagnostic(stdout))
                .unwrap_or_else(|| format!("iroha3d exited without a diagnostic ({exit})")),
        );
    }
    let status = norito::json::from_slice::<norito::json::Value>(stdout)
        .ok()
        .and_then(|report| {
            report
                .get("status")
                .and_then(norito::json::Value::as_str)
                .map(str::to_owned)
        });
    match status.as_deref() {
        Some("ready") => None,
        Some(other) => Some(format!("iroha3d reported the configuration as `{other}`")),
        None => Some("iroha3d did not print a readable --check-config report".into()),
    }
}

/// The deepest line of a report tree is its cause. Advice after the first `; ` is written for
/// operators copying configuration between nodes, which never applies to a generated network.
fn last_diagnostic(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let line = text
        .lines()
        .rev()
        .map(|line| line.trim_start_matches(|c: char| c.is_whitespace() || "│├╰╴─▶".contains(c)))
        .find(|line| !line.is_empty())?;
    let cause = line.split("; ").next().unwrap_or(line).trim_end();
    Some(cause.chars().take(MAX_REASON_CHARS).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_report_accepts_and_other_statuses_refuse() {
        assert_eq!(interpret(true, "0", br#"{"status":"ready"}"#, b""), None);
        assert_eq!(
            interpret(true, "0", br#"{"status":"pending"}"#, b""),
            Some("iroha3d reported the configuration as `pending`".into())
        );
        assert_eq!(
            interpret(true, "0", b"not json", b""),
            Some("iroha3d did not print a readable --check-config report".into())
        );
    }

    #[test]
    fn failure_reports_the_deepest_cause_without_operator_advice() {
        let report = "Configuration error\n\
            ├╴native genesis execution failed\n\
            │\n\
            ╰─▶ Signed genesis policy mismatch: execution a != b; copy them from the network\n";
        assert_eq!(
            interpret(false, "1", b"", report.as_bytes()),
            Some("Signed genesis policy mismatch: execution a != b".into())
        );
        assert_eq!(
            interpret(
                false,
                "2",
                b"",
                b"error: unexpected argument '--check-config' found\n\n"
            ),
            Some("error: unexpected argument '--check-config' found".into())
        );
    }

    #[test]
    fn failure_without_stderr_falls_back_to_stdout_then_a_fixed_message() {
        assert_eq!(
            interpret(false, "1", b"from stdout\n", b""),
            Some("from stdout".into())
        );
        assert_eq!(
            interpret(false, "signal: 9 (SIGKILL)", b"", b" \n"),
            Some("iroha3d exited without a diagnostic (signal: 9 (SIGKILL))".into())
        );
    }

    #[test]
    fn long_causes_are_bounded() {
        let long = "x".repeat(MAX_REASON_CHARS * 2);
        let reason = interpret(false, "1", b"", long.as_bytes()).unwrap();
        assert_eq!(reason.chars().count(), MAX_REASON_CHARS);
    }

    #[cfg(unix)]
    #[test]
    fn a_daemon_that_cannot_be_run_is_an_error_and_a_hung_one_is_bounded() {
        let temporary = tempfile::tempdir().unwrap();
        let config = temporary.path().join("peer0.toml");
        std::fs::write(&config, "").unwrap();
        let missing = temporary.path().join("missing");
        let far = Instant::now() + Duration::from_secs(30);
        let budget = Duration::from_secs(30);
        assert!(daemon_refusal(&missing, &config, &RootKind::Global, far, budget).is_err());

        let sleeper = temporary.path().join("sleeper");
        std::fs::write(&sleeper, "#!/bin/sh\nexec sleep 30\n").unwrap();
        std::fs::set_permissions(
            &sleeper,
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .unwrap();
        let started = Instant::now();
        let error = daemon_refusal(
            &sleeper,
            &config,
            &RootKind::Global,
            started + Duration::from_millis(200),
            budget,
        )
        .unwrap_err();
        assert!(matches!(error, Error::Timeout(value) if value == budget));
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
