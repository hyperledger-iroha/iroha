//! Retained validator supervision and the native graceful EOF lifecycle fixture.

use super::profile_files::private_log;
#[cfg(windows)]
use color_eyre::eyre::ensure;
use color_eyre::eyre::{Result, WrapErr as _};
#[cfg(all(windows, test))]
use iroha_fs::ProcessCommand as Command;
use iroha_fs::{PrivateDirectory, ProcessStdio as Stdio, RetainedFile};
use iroha_test_network::{ResolvedAsyncChild as Child, ResolvedProgram};
use std::path::{Path, PathBuf};
use tokio::time::{Instant, timeout_at};

/// Enable the actual daemon EOF control only on the platform without Unix signals.
pub(super) fn configure_lifecycle(node: &mut toml::Table) {
    #[cfg(windows)]
    node.insert(
        "lifecycle".into(),
        toml::Value::Table(
            [("exit_on_stdin_close".to_owned(), toml::Value::Boolean(true))]
                .into_iter()
                .collect(),
        ),
    );
    #[cfg(not(windows))]
    let _ = node;
}

/// Running validators.
pub(super) struct Peers {
    children: Vec<Child>,
    logs: Vec<RetainedFile>,
    directory: Option<PrivateDirectory>,
}

impl Peers {
    pub(super) fn spawn(
        daemon: &ResolvedProgram,
        directory: &Path,
        node_files: &[PathBuf],
    ) -> Result<Self> {
        let mut peers = Self {
            children: Vec::new(),
            logs: Vec::new(),
            directory: Some(PrivateDirectory::open_exact(directory)?),
        };
        for (index, node_file) in node_files.iter().enumerate() {
            let mut log = |stream: &str| -> Result<Stdio> {
                let original = private_log(&directory.join(format!("peer{index}.{stream}.log")))?;
                let stream = Stdio::from(original.file().try_clone()?);
                peers.logs.push(original);
                Ok(stream)
            };
            let mut command = daemon.command()?;
            command
                .env_clear()
                .current_dir(directory)
                .arg("--config")
                .arg(node_file)
                .stdin(if cfg!(windows) {
                    Stdio::piped()
                } else {
                    Stdio::null()
                })
                .stdout(log("stdout")?)
                .stderr(log("stderr")?)
                .kill_on_drop(true);
            peers.children.push(command.spawn_async()?);
            peers.revalidate()?;
        }
        Ok(peers)
    }

    fn revalidate(&self) -> Result<()> {
        self.directory
            .as_ref()
            .expect("peer original directory")
            .revalidate()?;
        for original in &self.logs {
            original.revalidate()?;
        }
        Ok(())
    }

    pub(super) fn exited(&mut self) -> Result<Option<String>> {
        self.revalidate()?;
        for (index, child) in self.children.iter_mut().enumerate() {
            if let Some(status) = child.try_wait()? {
                return Ok(Some(format!("validator {index} exited: {status}")));
            }
        }
        Ok(None)
    }

    pub(super) async fn stop(&mut self, deadline: Instant) -> Result<()> {
        self.revalidate()?;
        #[cfg(unix)]
        for child in &self.children {
            if let Some(pid) = child.id() {
                nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(i32::try_from(pid)?),
                    nix::sys::signal::Signal::SIGTERM,
                )?;
            }
        }
        #[cfg(windows)]
        for child in &mut self.children {
            if child.try_wait()?.is_none() {
                ensure!(
                    child.stdin.is_some(),
                    "validator lost its original EOF control pipe"
                );
                drop(child.stdin.take());
            }
        }
        for child in &mut self.children {
            timeout_at(deadline, child.wait())
                .await
                .wrap_err("validator did not stop")??;
        }
        for original in &self.logs {
            original.file().sync_all()?;
        }
        self.revalidate()?;
        self.children.clear();
        Ok(())
    }
}

impl Drop for Peers {
    fn drop(&mut self) {
        if self
            .children
            .iter_mut()
            .any(|child| !matches!(child.try_wait(), Ok(Some(_))))
        {
            // The paired original owner retains termination and native reaping after interruption.
            // Without terminal observation do not discard the logs' original ancestry;
            // retain this bounded failed-run evidence until the test process exits.
            std::mem::forget((std::mem::take(&mut self.logs), self.directory.take()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_configuration_matches_the_owned_shutdown_transport() {
        let mut node = toml::Table::new();
        node.insert("profile".into(), "sora-nexus-v1-qual".into());
        configure_lifecycle(&mut node);
        assert_eq!(node["profile"].as_str(), Some("sora-nexus-v1-qual"));
        #[cfg(windows)]
        assert_eq!(
            node["lifecycle"]["exit_on_stdin_close"].as_bool(),
            Some(true)
        );
        #[cfg(unix)]
        assert!(!node.contains_key("lifecycle"));
    }

    #[cfg(windows)]
    const EOF_CHILD: &str = "IROHA_PROFILE_PEER_EOF_CHILD";

    #[cfg(windows)]
    #[test]
    fn eof_child() {
        if std::env::var_os(EOF_CHILD).is_none() {
            return;
        }
        use std::io::{Read as _, Write as _};
        println!("profile-peer-ready");
        std::io::stdout().flush().unwrap();
        let mut bytes = Vec::new();
        std::io::stdin().read_to_end(&mut bytes).unwrap();
        assert!(bytes.is_empty());
        println!("profile-peer-observed-eof");
        std::io::stdout().flush().unwrap();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn stop_closes_the_original_pipe_only_after_ready_and_observes_exit() {
        use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, BufReader};
        let workspace = super::super::profile_files::workspace().unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap()).unwrap();
        command
            .args(["--exact", "profile_peers::tests::eof_child", "--nocapture"])
            .env(EOF_CHILD, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        let mut child = Child::from_development_child(command.spawn().unwrap()).unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut peers = Peers {
            children: vec![child],
            logs: vec![],
            directory: Some(PrivateDirectory::open_exact(workspace.path()).unwrap()),
        };
        let deadline = Instant::now() + std::time::Duration::from_secs(20);
        loop {
            let mut line = String::new();
            assert_ne!(
                timeout_at(deadline, output.read_line(&mut line))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
            if line.contains("profile-peer-ready") {
                break;
            }
        }
        assert!(peers.exited().unwrap().is_none());
        assert!(peers.children[0].stdin.is_some());
        peers.stop(deadline).await.unwrap();
        assert!(peers.children.is_empty());
        let mut tail = String::new();
        timeout_at(deadline, output.read_to_string(&mut tail))
            .await
            .unwrap()
            .unwrap();
        assert!(tail.contains("profile-peer-observed-eof"));
        workspace.revalidate().unwrap();
    }
}
