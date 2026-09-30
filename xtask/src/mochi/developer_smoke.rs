//! Installed-runtime checks with no supplied configuration or build tools on PATH.

use norito::json::{self, Value};
use std::{
    error::Error,
    fs::{self, File},
    io::Read as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(180);
const MAX_OUTPUT: u64 = 1024 * 1024;
const SOURCE: &str = "seiyaku BundleSmoke { view fn quote(int cups) -> int { return cups * 10; } }";

pub(super) fn run(kagami: &Path) -> Result<(), Box<dyn Error>> {
    let mut harness = Harness::new(kagami)?;
    let result = harness.exercise();
    // Cleanup is itself a required observation. On uncertainty retain custody and logs instead
    // of deleting a directory which an owned worker or validator could still be using.
    let stopped = harness
        .command(&["localnet", "down"])
        .and_then(|value| require_phase(&value, "stopped", 0));
    match (result, stopped) {
        (Ok(()), Ok(())) => Ok(()),
        (result, stopped) => {
            let root = harness.root.keep();
            Err(format!("installed developer smoke failed; retained diagnostics at {}: flow={result:?}; cleanup={stopped:?}", root.display()).into())
        }
    }
}

struct Harness {
    root: tempfile::TempDir,
    kagami: PathBuf,
    workspace: PathBuf,
    state: PathBuf,
    empty_path: PathBuf,
    sequence: usize,
}

impl Harness {
    fn new(kagami: &Path) -> Result<Self, Box<dyn Error>> {
        let root = tempfile::Builder::new()
            .prefix("iroha-bundle-smoke-")
            .tempdir()?;
        let workspace = root.path().join("workspace");
        let empty_path = root.path().join("empty-path");
        fs::create_dir(&workspace)?;
        fs::create_dir(&empty_path)?;
        fs::write(workspace.join("hello.ko"), SOURCE)?;
        Ok(Self {
            kagami: kagami.canonicalize()?,
            state: root.path().join("state"),
            root,
            workspace,
            empty_path,
            sequence: 0,
        })
    }

    fn command(&mut self, args: &[&str]) -> Result<Value, Box<dyn Error>> {
        self.sequence += 1;
        let stdout = self.root.path().join(format!("{}.stdout", self.sequence));
        let stderr = self.root.path().join(format!("{}.stderr", self.sequence));
        let started = Instant::now();
        let mut child = Command::new(&self.kagami)
            .args(args)
            .arg("--state")
            .arg(&self.state)
            .arg("--json")
            .current_dir(&self.workspace)
            .env("PATH", &self.empty_path)
            .stdin(Stdio::null())
            .stdout(File::create(&stdout)?)
            .stderr(File::create(&stderr)?)
            .spawn()?;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if started.elapsed() >= COMMAND_TIMEOUT {
                // This unreaped handle belongs to the smoke invocation. The managed worker is
                // stopped separately through its authenticated API, never through a stored PID.
                child.kill()?;
                child.wait()?;
                return Err(format!("kagami {args:?} exceeded {COMMAND_TIMEOUT:?}").into());
            }
            thread::sleep(Duration::from_millis(50));
        };
        if !status.success() {
            return Err(format!(
                "kagami {args:?} failed with {status}; inspect {}",
                stderr.display()
            )
            .into());
        }
        let mut bytes = Vec::new();
        File::open(stdout)?
            .take(MAX_OUTPUT + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_OUTPUT {
            return Err("managed command output exceeds its bound".into());
        }
        let value = json::from_slice(&bytes)?;
        eprintln!(
            "[developer-smoke] kagami {}: {:.2}s",
            args.join(" "),
            started.elapsed().as_secs_f64()
        );
        Ok(value)
    }

    fn exercise(&mut self) -> Result<(), Box<dyn Error>> {
        // The first call must both provision the network and deploy from raw source.
        let first = self.command(&["contract", "deploy", "hello.ko"])?;
        require_deployment(&first)?;
        let initial = self.command(&["localnet", "status"])?;
        require_phase(&initial, "ready", 4)?;
        let repeated_up = self.command(&["localnet", "up"])?;
        require_same_context(&initial, &repeated_up)?;
        require_phase(&repeated_up, "ready", 4)?;
        let repeated_deploy = self.command(&["contract", "deploy", "hello.ko"])?;
        require_same_deployment(&first, &repeated_deploy)?;
        let stopped = self.command(&["localnet", "down"])?;
        require_phase(&stopped, "stopped", 0)?;
        let restarted = self.command(&["localnet", "up"])?;
        require_phase(&restarted, "ready", 4)?;
        require_same_context(&initial, &restarted)?;
        let retained_deploy = self.command(&["contract", "deploy", "hello.ko"])?;
        require_same_deployment(&first, &retained_deploy)?;
        let files = fs::read_dir(&self.workspace)?.collect::<Result<Vec<_>, _>>()?;
        if files.len() != 1 || files[0].file_name() != "hello.ko" {
            return Err("config-free deployment wrote additional files into the project".into());
        }
        Ok(())
    }
}

fn require_phase(value: &Value, phase: &str, peers: u64) -> Result<(), Box<dyn Error>> {
    if value.get("phase").and_then(Value::as_str) != Some(phase)
        || value.get("running_peers").and_then(Value::as_u64) != Some(peers)
    {
        return Err(format!("expected {phase} with {peers} validators").into());
    }
    Ok(())
}

fn require_same_context(before: &Value, after: &Value) -> Result<(), Box<dyn Error>> {
    let Some(expected) = before.get("context").filter(|context| context.is_object()) else {
        return Err("missing initial managed context".into());
    };
    if after.get("context") != Some(expected) {
        return Err("idempotent start or restart changed retained client identity".into());
    }
    Ok(())
}

fn require_deployment(value: &Value) -> Result<(), Box<dyn Error>> {
    if value.get("status").and_then(Value::as_str) != Some("applied")
        || !value.get("receipt").is_some_and(Value::is_object)
        || value
            .get("journal")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err("deployment lacks Applied evidence or exact recovery journal".into());
    }
    Ok(())
}

fn require_same_deployment(before: &Value, after: &Value) -> Result<(), Box<dyn Error>> {
    require_deployment(before)?;
    require_deployment(after)?;
    if before.get("receipt") != after.get("receipt")
        || before.get("journal") != after.get("journal")
    {
        return Err("unchanged deployment created a new receipt, transaction or journal".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercise an already-built matching runtime without invoking Cargo from the bundle.
    #[test]
    #[ignore = "requires IROHA_DEVEX_BUNDLE_BIN pointing to matching native runtime binaries"]
    fn installed_runtime_without_configuration() {
        let directory = std::env::var_os("IROHA_DEVEX_BUNDLE_BIN")
            .map(PathBuf::from)
            .expect("set IROHA_DEVEX_BUNDLE_BIN to the installed runtime bin directory");
        let kagami = directory.join(if cfg!(windows) {
            "kagami.exe"
        } else {
            "kagami"
        });
        run(&kagami)
            .expect("installed developer workflow must retain Applied evidence across restart");
    }

    #[test]
    fn readiness_requires_every_validator_and_exact_phase() {
        let ready = norito::json!({"phase": "ready", "running_peers": 4});
        require_phase(&ready, "ready", 4).unwrap();
        assert!(require_phase(&ready, "stopped", 0).is_err());
        assert!(
            require_phase(
                &norito::json!({"phase": "ready", "running_peers": 3}),
                "ready",
                4
            )
            .is_err()
        );
        assert!(require_phase(&Value::Null, "ready", 4).is_err());
    }

    #[test]
    fn repeat_checks_reject_missing_evidence_and_changed_receipts_or_identities() {
        let context = norito::json!({"context": {"network_id": "retained", "account_id": "owner"}});
        require_same_context(&context, &context).unwrap();
        assert!(
            require_same_context(
                &context,
                &norito::json!({"context": {"network_id": "changed"}})
            )
            .is_err()
        );
        assert!(require_same_context(&Value::Null, &Value::Null).is_err());
        let receipt = norito::json!({"status": "applied", "receipt": {"commit": "original"}, "journal": "retained"});
        require_same_deployment(&receipt, &receipt).unwrap();
        let changed = norito::json!({"status": "applied", "receipt": {"commit": "new"}, "journal": "retained"});
        assert!(require_same_deployment(&receipt, &changed).is_err());
        assert!(require_same_deployment(&Value::Null, &Value::Null).is_err());
        assert!(
            require_deployment(
                &norito::json!({"status": "pending", "receipt": {}, "journal": "retained"})
            )
            .is_err()
        );
    }
}
