//! Native process owner and honest readiness checks for a retained four-peer generation.

use super::*;
use iroha::blocking::Client;
use iroha_data_model::{Level, isi::Log, transaction::FeePaymentIntent};
use iroha_fs::{PrivateDirectory, PublishMode};
use std::{
    fs::File,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Instant,
};

/// Run the long-lived private localnet worker inside the installed Kagami executable.
///
/// The CLI dispatches its internal `_managed-worker` entry point here. The worker retains the
/// ownership lock for its full lifetime and transfers a clone into each child's standard input.
/// A lost controller therefore cannot allow a second controller to adopt unverified PIDs.
///
/// # Errors
/// Invalid custody or metadata, competing ownership, changed executables, process failure or
/// readiness failure. Every failure stops only the children this invocation actually created.
pub fn run_worker(store: &ManagedStore, name: &str, startup_timeout: Duration) -> Result<()> {
    transport::supported()?;
    if startup_timeout.is_zero() || startup_timeout > Duration::from_secs(600) {
        return Err(Error::Invalid(
            "worker timeout is outside its bounded startup contract".into(),
        ));
    }
    let directory = store.directory(name)?;
    let ownership = store::acquire(&directory, "runtime.lock", name)?;
    let retained: RetainedLocalnet = decode(&directory.read(MANIFEST, MAX_METADATA)?)?;
    store::validate_prepared(name, directory.path(), &retained.prepared)?;
    store::verify_binary(&retained.launcher)?;
    store::verify_binary(&retained.daemon)?;
    let current = store::pin_binary(&std::env::current_exe()?)?;
    if current.blake3 != retained.launcher.blake3 {
        return Err(Error::Invalid(
            "worker executable does not match the retained launcher".into(),
        ));
    }
    let listener = transport::Listener::bind(&directory)?;
    let worker = WorkerRecord {
        token: store::random_token(),
    };
    directory.write_atomic(WORKER, &encode(&worker)?, PublishMode::Replace)?;
    let mut status = ManagedStatus {
        context: retained.prepared.context.clone(),
        phase: ManagedPhase::Starting,
        running_peers: 0,
        failure: None,
    };
    publish(&directory, &status)?;
    let mut processes = PeerProcesses::default();
    if processes.start(&directory, &retained, &ownership).is_err() {
        processes.stop()?;
        status.phase = ManagedPhase::Failed;
        status.failure = Some("a validator could not start; inspect its retained logs".into());
        publish(&directory, &status)?;
        return Err(Error::Invalid(status.failure.unwrap_or_default()));
    }
    status.running_peers = processes.children.len();
    let cancelled = Arc::new(AtomicBool::new(false));
    let readiness_cancelled = Arc::clone(&cancelled);
    let prepared = retained.prepared;
    let (sender, receiver) = mpsc::sync_channel(1);
    let started = Instant::now();
    thread::spawn(move || {
        let result = prove_readiness(&prepared, startup_timeout, &readiness_cancelled);
        let _ = sender.send(result);
    });
    loop {
        if let Some(mut connection) = listener.accept()? {
            if let Ok(request) = connection.receive()
                && same_token(&request.token, &worker.token)
            {
                match request.action.as_str() {
                    "status" => {
                        let _ = connection.reply(&status);
                    }
                    "down" => {
                        cancelled.store(true, Ordering::Release);
                        processes.stop()?;
                        status.phase = ManagedPhase::Stopped;
                        status.running_peers = 0;
                        publish(&directory, &status)?;
                        let _ = connection.reply(&status);
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
        if processes.any_exited()? {
            cancelled.store(true, Ordering::Release);
            processes.stop()?;
            status.phase = ManagedPhase::Failed;
            status.running_peers = 0;
            status.failure = Some("a supervised validator exited; inspect its retained log".into());
            publish(&directory, &status)?;
            return Err(Error::Invalid(status.failure.unwrap_or_default()));
        }
        if status.phase == ManagedPhase::Starting {
            match receiver.try_recv() {
                Ok(Ok(())) => {
                    status.phase = ManagedPhase::Ready;
                    publish(&directory, &status)?;
                }
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                    cancelled.store(true, Ordering::Release);
                    processes.stop()?;
                    status.phase = ManagedPhase::Failed;
                    status.running_peers = 0;
                    status.failure =
                        Some("signed readiness was not confirmed on all four validators".into());
                    publish(&directory, &status)?;
                    return Err(Error::Invalid(status.failure.unwrap_or_default()));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
            if started.elapsed() >= startup_timeout {
                cancelled.store(true, Ordering::Release);
                processes.stop()?;
                status.phase = ManagedPhase::Failed;
                status.running_peers = 0;
                status.failure = Some("startup readiness deadline expired".into());
                publish(&directory, &status)?;
                return Err(Error::Timeout(startup_timeout));
            }
        }
        thread::sleep(POLL);
    }
}

fn publish(directory: &PrivateDirectory, status: &ManagedStatus) -> Result<()> {
    directory.write_atomic(STATUS, &encode(status)?, PublishMode::Replace)?;
    Ok(())
}

fn same_token(left: &str, right: &str) -> bool {
    left.len() == 64
        && right.len() == 64
        && left
            .bytes()
            .zip(right.bytes())
            .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

#[derive(Default)]
struct PeerProcesses {
    children: Vec<Child>,
}

impl PeerProcesses {
    fn start(
        &mut self,
        directory: &PrivateDirectory,
        retained: &RetainedLocalnet,
        ownership: &File,
    ) -> Result<()> {
        for (index, peer) in retained.prepared.peers.iter().enumerate() {
            let log = directory.open_append(&peer.log_name)?;
            let mut command = Command::new(&retained.daemon.path);
            command
                .arg("--config")
                .arg(&peer.config_path)
                .current_dir(
                    peer.config_path
                        .parent()
                        .ok_or_else(|| Error::Invalid("node configuration has no parent".into()))?,
                )
                .stdin(Stdio::from(ownership.try_clone()?))
                .stdout(log.try_clone()?)
                .stderr(log);
            self.children
                .push(spawn_with_launch_fence(directory, index, &mut command)?);
        }
        Ok(())
    }

    fn any_exited(&mut self) -> Result<bool> {
        for child in &mut self.children {
            if child.try_wait()?.is_some() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn stop(&mut self) -> Result<()> {
        // Only this owner's unreaped Child handles are eligible for signalling. No persisted PID
        // is ever used. Send all graceful requests before sharing a single bounded grace period.
        for child in &mut self.children {
            if child.try_wait()?.is_some() {
                continue;
            }
            #[cfg(unix)]
            if let Some(pid) = rustix::process::Pid::from_raw(child.id() as i32) {
                let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
            }
            #[cfg(not(unix))]
            child.kill()?;
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let mut remaining = false;
            for child in &mut self.children {
                if child.try_wait()?.is_none() {
                    remaining = true;
                }
            }
            if !remaining || Instant::now() >= deadline {
                break;
            }
            thread::sleep(POLL);
        }
        for child in &mut self.children {
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
            child.wait()?;
        }
        self.children.clear();
        Ok(())
    }
}

fn spawn_with_launch_fence(
    directory: &PrivateDirectory,
    index: usize,
    command: &mut Command,
) -> Result<Child> {
    let marker = format!("peer{index}.launch");
    let fresh = match directory.read(&marker, 1) {
        Ok(bytes) if bytes.as_slice() == b"1" => false,
        Ok(bytes) if bytes.as_slice() == b"0" => {
            directory.write_atomic(&marker, b"1", PublishMode::Replace)?;
            true
        }
        Ok(_) => return Err(Error::Invalid("invalid retained key launch marker".into())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // Persist before exec. A lost controller must never repeat the assertion for keys
            // that might have signed. Only a definite spawn failure below can undo this fence.
            directory.write_atomic(&marker, b"1", PublishMode::CreateNew)?;
            true
        }
        Err(error) => return Err(error.into()),
    };
    if fresh {
        command.arg("--sumeragi-assert-fresh-key");
    }
    match command.spawn() {
        Ok(child) => Ok(child),
        Err(error) => {
            if fresh {
                directory.write_atomic(&marker, b"0", PublishMode::Replace)?;
            }
            Err(error.into())
        }
    }
}

impl Drop for PeerProcesses {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn prove_readiness(
    prepared: &PreparedLocalnet,
    timeout: Duration,
    cancelled: &AtomicBool,
) -> Result<()> {
    let started = Instant::now();
    let mut config = prepared.context.load_client_config()?;
    config.torii_request_timeout = Duration::from_millis(750);
    config.transaction_status_timeout = timeout;
    config.transaction_ttl = timeout.max(Duration::from_secs(60));
    let mut clients = Vec::with_capacity(4);
    for peer in &prepared.peers {
        let mut peer_config = config.clone();
        peer_config.torii_api_url = peer
            .torii_url
            .parse()
            .map_err(|_| Error::Invalid("invalid peer URL".into()))?;
        clients.push(
            Client::new(peer_config)
                .map_err(|_| Error::Invalid("cannot construct managed readiness client".into()))?,
        );
    }
    loop {
        remaining(started, timeout, cancelled)?;
        if clients.iter().all(|client| {
            client
                .status()
                .get()
                .is_ok_and(|status| status.blocks > 0 && status.peers >= 3)
        }) {
            break;
        }
        thread::sleep(POLL);
    }
    // Exactly one attempt. Ambiguous submission never produces a replacement transaction.
    let budget = remaining(started, timeout, cancelled)?;
    config.transaction_status_timeout = budget;
    let submitter = Client::new(config)
        .map_err(|_| Error::Invalid("cannot construct readiness signer".into()))?;
    let hash = submitter
        .submit(
            Log::new(
                Level::INFO,
                format!("managed localnet readiness {}", store::random_token()),
            ),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .map_err(|_| Error::Invalid("readiness transaction was not confirmed".into()))?;
    for client in clients {
        let budget = remaining(started, timeout, cancelled)?;
        client
            .wait_for_transaction_applied(
                hash,
                iroha::client::TransactionWaitOptions {
                    timeout: budget,
                    poll_interval: POLL,
                },
            )
            .map_err(|_| {
                Error::Invalid(
                    "readiness transaction was not applied on every managed validator".into(),
                )
            })?;
    }
    remaining(started, timeout, cancelled)?;
    Ok(())
}

fn remaining(started: Instant, timeout: Duration, cancelled: &AtomicBool) -> Result<Duration> {
    if cancelled.load(Ordering::Acquire) {
        return Err(Error::Invalid("readiness was cancelled".into()));
    }
    timeout
        .checked_sub(started.elapsed())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(Error::Timeout(timeout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_authentication_rejects_short_and_different_tokens() {
        assert!(same_token(&"a".repeat(64), &"a".repeat(64)));
        assert!(!same_token("", ""));
        assert!(!same_token(&"a".repeat(64), &"b".repeat(64)));
        assert!(!same_token(&"a".repeat(63), &"a".repeat(63)));
    }

    #[test]
    fn readiness_deadline_and_cancellation_fail_closed() {
        let cancelled = AtomicBool::new(false);
        assert!(remaining(Instant::now(), Duration::from_secs(1), &cancelled).is_ok());
        assert!(remaining(Instant::now(), Duration::ZERO, &cancelled).is_err());
        cancelled.store(true, Ordering::Release);
        assert!(remaining(Instant::now(), Duration::from_secs(1), &cancelled).is_err());
    }

    #[test]
    fn definite_spawn_failure_retains_a_retryable_fresh_key_fence() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = PrivateDirectory::open_or_create(temporary.path().join("network")).unwrap();
        for _ in 0..2 {
            let mut command = Command::new(temporary.path().join("absent-daemon"));
            assert!(spawn_with_launch_fence(&directory, 0, &mut command).is_err());
            assert_eq!(directory.read("peer0.launch", 1).unwrap().as_slice(), b"0");
            assert_eq!(
                command.get_args().collect::<Vec<_>>(),
                ["--sumeragi-assert-fresh-key"]
            );
        }
        directory
            .write_atomic("peer0.launch", b"1", PublishMode::Replace)
            .unwrap();
        let mut command = Command::new(temporary.path().join("absent-daemon"));
        assert!(spawn_with_launch_fence(&directory, 0, &mut command).is_err());
        assert_eq!(directory.read("peer0.launch", 1).unwrap().as_slice(), b"1");
        assert_eq!(command.get_args().len(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn owned_child_retains_the_runtime_lock_and_stop_leaves_other_children_alive() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = PrivateDirectory::open_or_create(temporary.path().join("network")).unwrap();
        let ownership = store::acquire(&directory, "runtime.lock", "fixture").unwrap();
        let child = Command::new("/bin/sleep")
            .arg("30")
            .stdin(Stdio::from(ownership.try_clone().unwrap()))
            .spawn()
            .unwrap();
        let other = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let mut owned = PeerProcesses {
            children: vec![child],
        };
        let mut sentinel = PeerProcesses {
            children: vec![other],
        };
        drop(ownership);
        assert!(matches!(
            store::acquire(&directory, "runtime.lock", "fixture"),
            Err(Error::Busy(_))
        ));
        owned.stop().unwrap();
        assert!(
            !sentinel.any_exited().unwrap(),
            "unrelated process must remain untouched"
        );
        store::acquire(&directory, "runtime.lock", "fixture").unwrap();
        sentinel.stop().unwrap();
    }
}
