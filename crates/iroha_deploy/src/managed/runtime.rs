//! Native process owner and honest readiness checks for a retained four-peer generation.

use super::*;
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

mod activation;
mod maintenance;
mod owned;
mod progress;
mod readiness;
mod renewal;
use owned::PeerProcesses;

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
    let started = Instant::now();
    transport::supported()?;
    if startup_timeout.is_zero() || startup_timeout > Duration::from_secs(600) {
        return Err(Error::Invalid(
            "worker timeout is outside its bounded startup contract".into(),
        ));
    }
    let directory = store.directory(name)?;
    let ownership = store::acquire(&directory, "runtime.lock", name)?;
    let retained = generation::read(&directory)?;
    store::validate_prepared(
        name,
        directory.path(),
        &retained.prepared,
        &retained.root_kind,
    )?;
    store::verify_binary(&retained.launcher)?;
    store::verify_binary(&retained.daemon)?;
    let current = store::pin_binary(&std::env::current_exe()?)?;
    if current.blake3 != retained.launcher.blake3 {
        return Err(Error::Invalid(
            "worker executable does not match the retained launcher".into(),
        ));
    }
    startup_remaining(started, startup_timeout)?;
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
    let cancelled = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(progress::Progress::default());
    let mut budget = Arc::new(activation::Budget {
        started,
        timeout: startup_timeout,
        utc_ceiling_unix_ms: None,
        cancelled: Arc::clone(&cancelled),
        progress: Arc::clone(&progress),
    });
    let prepared = retained.prepared.clone();
    let mut processes = PeerProcesses::default();
    let _cancel_on_exit = CancelOnExit(Arc::clone(&cancelled));
    let activation::Startup {
        launch,
        authorization,
    } = match activation::prepare(&prepared, &budget) {
        Ok(startup) => startup,
        Err(failure) => {
            return fail_worker(&directory, &mut status, &mut processes, &cancelled, failure);
        }
    };
    if processes
        .start(&directory, &retained, &ownership, launch)
        .is_err()
    {
        return fail_worker(
            &directory,
            &mut status,
            &mut processes,
            &cancelled,
            progress.unconfirmed(),
        );
    }
    status.running_peers = processes.children.len();
    let generated = match processes.generated() {
        Ok(generated) => generated,
        Err(_) => {
            return fail_worker(
                &directory,
                &mut status,
                &mut processes,
                &cancelled,
                progress.unconfirmed(),
            );
        }
    };
    let background_prepared = prepared.clone();
    let background_budget = Arc::clone(&budget);
    let (sender, mut receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let result = activation::initial(
            &background_prepared,
            &background_budget,
            generated,
            authorization,
        );
        let _ = sender.send(result);
    });
    let mut service_expiry: Option<maintenance::Observation> = None;
    let mut refresh: Option<
        mpsc::Receiver<std::result::Result<maintenance::Observation, progress::Failure>>,
    > = None;
    let mut attachment: Option<remote::AttachmentWorker> = None;
    let mut attachment_attempted = false;
    loop {
        if service_expiry
            .as_ref()
            .is_some_and(|expiry| !expiry.current().unwrap_or(false))
        {
            return fail_worker(
                &directory,
                &mut status,
                &mut processes,
                &cancelled,
                progress::Failure::ObservationExpired,
            );
        }
        if let Some(mut connection) = listener.accept()? {
            if let Ok(request) = connection.receive()
                && same_token(&request.token, &worker.token)
            {
                match request.action.as_str() {
                    "status" => {
                        let _ = connection.reply(&status);
                    }
                    action if action.starts_with("startup_receipt:") => {
                        if status.phase == ManagedPhase::Ready
                            && !processes.any_exited()?
                            && let Some(challenge) = action.strip_prefix("startup_receipt:")
                        {
                            let children = processes.live_children()?;
                            if let Some(launch) = processes.launch_snapshot.as_ref()
                                && let Ok(receipt) = startup_receipt::capture(
                                    store.root(),
                                    &directory,
                                    &retained,
                                    challenge,
                                    &children,
                                    launch,
                                )
                                && !processes.any_exited()?
                            {
                                let _ = connection.reply(&receipt);
                            }
                        }
                    }
                    "attachment_start" => {
                        if status.phase == ManagedPhase::Ready
                            && attachment
                                .as_ref()
                                .is_none_or(remote::AttachmentWorker::is_finished)
                        {
                            attachment = remote::AttachmentWorker::start(
                                store,
                                name,
                                &prepared,
                                Arc::clone(&cancelled),
                            )
                            .ok()
                            .flatten();
                            attachment_attempted = true;
                        }
                        let _ = connection.reply(&status);
                    }
                    "attachment_status" => {
                        let observation = attachment
                            .as_ref()
                            .map(remote::AttachmentWorker::status)
                            .or_else(|| {
                                remote::inactive_status(store, name, &prepared)
                                    .ok()
                                    .flatten()
                            });
                        if let Some(attachment) = observation {
                            let _ = connection.reply(&ManagedDataspaceStatus {
                                local: status.clone(),
                                attachment,
                            });
                        }
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
                    "startup_expired" => {
                        // The foreground budget starts before this worker is spawned. Its
                        // authenticated expiry must retain the last safe phase rather than
                        // converting an unproved attempt into an ordinary user-requested stop.
                        let failure = progress.deadline();
                        cancelled.store(true, Ordering::Release);
                        processes.stop()?;
                        expire_startup_status(&mut status, failure);
                        publish(&directory, &status)?;
                        let _ = connection.reply(&status);
                        return Err(Error::Invalid(status.failure.unwrap_or_default()));
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
            // Deadline wins over a proof queued just before the worker observed it. Never
            // publish a transient Ready after the original startup budget was exhausted.
            let result = match budget.check() {
                Err(failure) => Some(Err(failure)),
                Ok(_) => {
                    observe_readiness(&receiver, &budget.progress, budget.started, budget.timeout)
                }
            };
            match result {
                Some(Err(failure)) => {
                    return fail_worker(
                        &directory,
                        &mut status,
                        &mut processes,
                        &cancelled,
                        failure,
                    );
                }
                Some(Ok(activation::Outcome::Restart(restart))) => {
                    let next = (|| {
                        let (launch, recheck) = restart.into_launch(&prepared, &budget)?;
                        // Old HTTP guards become inactive before any owned child is signalled.
                        processes.stop().map_err(|_| progress.unconfirmed())?;
                        budget.check()?;
                        processes
                            .start(&directory, &retained, &ownership, Some(launch))
                            .map_err(|_| progress.unconfirmed())?;
                        budget.check()?;
                        let live = processes.gateways().map_err(|_| progress.unconfirmed())?;
                        status.running_peers = processes.children.len();
                        let selected = prepared.clone();
                        let background_budget = Arc::clone(&budget);
                        let (sender, receiver) = mpsc::sync_channel(1);
                        thread::spawn(move || {
                            let _ =
                                sender.send(recheck.finish(&selected, live, &background_budget));
                        });
                        Ok::<_, progress::Failure>(receiver)
                    })();
                    match next {
                        Ok(next) => receiver = next,
                        Err(failure) => {
                            return fail_worker(
                                &directory,
                                &mut status,
                                &mut processes,
                                &cancelled,
                                failure,
                            );
                        }
                    }
                }
                Some(Ok(activation::Outcome::Complete(expiry))) => {
                    service_expiry = expiry;
                    if service_expiry
                        .as_ref()
                        .is_some_and(|expiry| !expiry.current().unwrap_or(false))
                    {
                        return fail_worker(
                            &directory,
                            &mut status,
                            &mut processes,
                            &cancelled,
                            progress::Failure::ObservationExpired,
                        );
                    }
                    // Successful handoffs are checked against the original deadline again above.
                    status.phase = ManagedPhase::Ready;
                    publish(&directory, &status)?;
                }
                None => {}
            }
        }
        if status.phase == ManagedPhase::Ready
            && let Some(observation) = service_expiry.as_mut()
        {
            let result = refresh
                .as_ref()
                .and_then(|receiver| match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => Some(Err(progress.unconfirmed())),
                });
            if let Some(result) = result {
                refresh = None;
                // The old observation wins over queued success; a slow refresh cannot bridge
                // an interval in which the worker had no current authenticated observation.
                if !observation.current().unwrap_or(false) {
                    return fail_worker(
                        &directory,
                        &mut status,
                        &mut processes,
                        &cancelled,
                        progress::Failure::ObservationExpired,
                    );
                }
                match result {
                    Ok(next) if next.current().unwrap_or(false) => *observation = next,
                    _ => {
                        if observation.retry().is_err() {
                            return fail_worker(
                                &directory,
                                &mut status,
                                &mut processes,
                                &cancelled,
                                progress::Failure::ObservationExpired,
                            );
                        }
                    }
                }
            }
            if refresh.is_none() && observation.due().unwrap_or(true) {
                let next = (|| -> Result<_> {
                    let (selection, turn) = observation.begin(Arc::clone(&cancelled))?;
                    let live = processes.gateway(selection.provider())?;
                    let selected = prepared.clone();
                    let (sender, receiver) = mpsc::sync_channel(1);
                    thread::spawn(move || {
                        let _ =
                            sender.send(maintenance::refresh(&selected, selection, live, &turn));
                    });
                    Ok(receiver)
                })();
                match next {
                    Ok(receiver) => refresh = Some(receiver),
                    Err(_) => {
                        if observation.retry().is_err() {
                            return fail_worker(
                                &directory,
                                &mut status,
                                &mut processes,
                                &cancelled,
                                progress::Failure::ObservationExpired,
                            );
                        }
                    }
                }
            }
        }
        if status.phase == ManagedPhase::Ready && refresh.is_none() {
            let turn = service_expiry
                .as_ref()
                .map(|observation| {
                    observation.renewal(Arc::clone(&cancelled), Arc::clone(&progress))
                })
                .transpose();
            match turn {
                Ok(Some(Some(turn))) => {
                    let live = match processes.gateway(turn.provider()) {
                        Ok(live) => live,
                        Err(_) => {
                            return fail_worker(
                                &directory,
                                &mut status,
                                &mut processes,
                                &cancelled,
                                progress.unconfirmed(),
                            );
                        }
                    };
                    // Withdraw Ready durably before the background owner can mutate custody.
                    // No stale observation survives across a native enrollment head change.
                    status.phase = ManagedPhase::Starting;
                    publish(&directory, &status)?;
                    service_expiry = None;
                    budget = Arc::clone(&turn.budget);
                    let selected = prepared.clone();
                    let (sender, next) = mpsc::sync_channel(1);
                    thread::spawn(move || {
                        let _ = sender.send(renewal::advance(&selected, turn, live));
                    });
                    receiver = next;
                }
                Ok(_) => {}
                Err(_) => {
                    return fail_worker(
                        &directory,
                        &mut status,
                        &mut processes,
                        &cancelled,
                        progress::Failure::ObservationExpired,
                    );
                }
            }
        }
        if status.phase == ManagedPhase::Ready && !attachment_attempted {
            // Parent custody and transport failures cannot change private execution readiness.
            // A later authenticated activation can retry a previously absent binding.
            attachment =
                remote::AttachmentWorker::start(store, name, &prepared, Arc::clone(&cancelled))
                    .ok()
                    .flatten();
            attachment_attempted = true;
        }
        thread::sleep(POLL);
    }
}

fn publish(directory: &PrivateDirectory, status: &ManagedStatus) -> Result<()> {
    directory.write_atomic(STATUS, &encode(status)?, PublishMode::Replace)?;
    Ok(())
}

fn expire_startup_status(status: &mut ManagedStatus, failure: progress::Failure) {
    status.phase = ManagedPhase::Failed;
    status.running_peers = 0;
    status.failure = Some(failure.message());
}

fn observe_readiness<T>(
    receiver: &mpsc::Receiver<std::result::Result<T, progress::Failure>>,
    progress: &progress::Progress,
    started: Instant,
    timeout: Duration,
) -> Option<std::result::Result<T, progress::Failure>> {
    let observed = match receiver.try_recv() {
        Ok(result) => Some(result),
        Err(mpsc::TryRecvError::Disconnected) => Some(Err(progress.unconfirmed())),
        Err(mpsc::TryRecvError::Empty) => None,
    };
    if started.elapsed() >= timeout {
        Some(Err(progress.deadline()))
    } else {
        observed
    }
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

pub(super) fn daemon_command(
    daemon: &std::path::Path,
    config: &std::path::Path,
    root_kind: &RootKind,
    config_digest: Option<&str>,
) -> Result<Command> {
    let mut command = Command::new(daemon);
    if matches!(root_kind, RootKind::Private { .. }) {
        // The native daemon requires this profile for any explicitly scoped Nexus topology,
        // including a single private lane. The signed root identity still owns its scope.
        command.arg("--sora");
    }
    command.arg("--config").arg(config).current_dir(
        config
            .parent()
            .ok_or_else(|| Error::Invalid("node configuration has no parent".into()))?,
    );
    match (root_kind, config_digest) {
        (RootKind::Private { .. }, Some(digest))
            if digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) =>
        {
            command.arg("--config-blake3").arg(digest);
        }
        (RootKind::Global, None) => {}
        _ => {
            return Err(Error::Invalid(
                "private launch requires its original configuration digest".into(),
            ));
        }
    }
    Ok(command)
}

pub(super) fn command_argv(command: &Command) -> Result<Vec<String>> {
    std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|part| {
            part.to_str()
                .map(str::to_owned)
                .ok_or_else(|| Error::Invalid("managed startup argv is not canonical UTF-8".into()))
        })
        .collect()
}

fn retained_launch_directory(directory: &PrivateDirectory) -> Result<PrivateDirectory> {
    let generation = directory.open_child(generation::DIRECTORY)?;
    // Refuse incomplete published custody before any peer can start. The per-peer transition
    // below rereads through this same original directory authority before changing its fence.
    for index in 0..4 {
        read_launch_fence(&generation, index)?;
    }
    Ok(generation)
}

fn read_launch_fence(directory: &PrivateDirectory, index: usize) -> Result<bool> {
    if index >= 4 {
        return Err(Error::Invalid("managed peer index is out of bounds".into()));
    }
    let marker = format!("peer{index}.launch");
    match directory.read(&marker, 1) {
        Ok(bytes) if bytes.as_slice() == b"1" => Ok(false),
        Ok(bytes) if bytes.as_slice() == b"0" => Ok(true),
        Ok(_) => Err(Error::Invalid("invalid retained key launch marker".into())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(Error::Invalid(
            "published generation is missing its retained key launch marker".into(),
        )),
        Err(error) => Err(error.into()),
    }
}

fn spawn_with_launch_fence(
    directory: &PrivateDirectory,
    index: usize,
    command: &mut Command,
) -> Result<Child> {
    let marker = format!("peer{index}.launch");
    let fresh = read_launch_fence(directory, index)?;
    if fresh {
        // Persist before exec. A lost controller must never repeat the assertion for keys
        // that might have signed. Only a definite spawn failure below can undo this fence.
        directory.write_atomic(&marker, b"1", PublishMode::Replace)?;
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

/// Any early error also cancels background work before owned children are dropped.
struct CancelOnExit(Arc<AtomicBool>);
impl Drop for CancelOnExit {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

fn fail_worker(
    directory: &PrivateDirectory,
    status: &mut ManagedStatus,
    processes: &mut PeerProcesses,
    cancelled: &AtomicBool,
    failure: progress::Failure,
) -> Result<()> {
    cancelled.store(true, Ordering::Release);
    processes.stop()?;
    expire_startup_status(status, failure);
    publish(directory, status)?;
    Err(Error::Invalid(failure.message()))
}

/// Charge every startup phase to the same finite budget, including custody and binary checks.
pub(super) fn startup_remaining(started: Instant, timeout: Duration) -> Result<Duration> {
    timeout
        .checked_sub(started.elapsed())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(Error::Timeout(timeout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_daemon_uses_the_explicit_nexus_profile_without_changing_global_launch() {
        let temporary = tempfile::tempdir().unwrap();
        let daemon = temporary.path().join("iroha3d");
        let config = temporary.path().join("peer0.toml");
        let digest = "a".repeat(64);
        let global = daemon_command(&daemon, &config, &RootKind::Global, None).unwrap();
        assert_eq!(
            global.get_args().collect::<Vec<_>>(),
            ["--config".as_ref(), config.as_os_str()]
        );
        let private = daemon_command(
            &daemon,
            &config,
            &RootKind::Private {
                spec: super::super::tests::private_spec(),
            },
            Some(&digest),
        )
        .unwrap();
        assert_eq!(
            private.get_args().collect::<Vec<_>>(),
            [
                "--sora".as_ref(),
                "--config".as_ref(),
                config.as_os_str(),
                "--config-blake3".as_ref(),
                digest.as_ref()
            ]
        );
        assert_eq!(private.get_current_dir(), Some(temporary.path()));
        let private_kind = RootKind::Private {
            spec: super::super::tests::private_spec(),
        };
        let uppercase = "A".repeat(64);
        let short = "a".repeat(63);
        for invalid in [
            None,
            Some("ready"),
            Some(uppercase.as_str()),
            Some(short.as_str()),
        ] {
            assert!(daemon_command(&daemon, &config, &private_kind, invalid).is_err());
        }
        assert!(daemon_command(&daemon, &config, &RootKind::Global, Some(&digest)).is_err());
    }

    #[test]
    fn control_authentication_rejects_short_and_different_tokens() {
        assert!(same_token(&"a".repeat(64), &"a".repeat(64)));
        assert!(!same_token("", ""));
        assert!(!same_token(&"a".repeat(64), &"b".repeat(64)));
        assert!(!same_token(&"a".repeat(63), &"a".repeat(63)));
    }

    #[test]
    fn startup_budget_charges_foreground_and_worker_verification_before_readiness() {
        let original = Duration::from_secs(100);
        let foreground_started = Instant::now() - Duration::from_secs(90);
        let transferred = startup_remaining(foreground_started, original).unwrap();
        assert!(transferred <= Duration::from_secs(10));
        let worker_started = Instant::now() - Duration::from_secs(7);
        let readiness = startup_remaining(worker_started, transferred).unwrap();
        assert!(readiness <= Duration::from_secs(3));
        assert!(matches!(
            startup_remaining(Instant::now() - Duration::from_secs(101), original),
            Err(Error::Timeout(value)) if value == original
        ));
        assert!(matches!(
            startup_remaining(Instant::now() - Duration::from_secs(11), transferred),
            Err(Error::Timeout(value)) if value == transferred
        ));
    }

    #[test]
    fn queued_success_after_original_deadline_is_a_retained_failure() {
        let progress = progress::Progress::default();
        let (sender, receiver) = mpsc::sync_channel(1);
        sender.send(Ok(())).unwrap();
        let failure = observe_readiness(
            &receiver,
            &progress,
            Instant::now() - Duration::from_secs(2),
            Duration::from_secs(1),
        )
        .unwrap()
        .unwrap_err();
        assert_eq!(failure, progress.deadline());
        let _resources = super::super::native_test_guard();
        let temporary = tempfile::tempdir().unwrap();
        let (_, directory, prepared) =
            super::super::tests::fixture(&temporary.path().join("managed"), "local");
        // A caller's earlier budget can expire just after the worker proved readiness.
        // Cleanup must not preserve that late Ready or erase the phase as a normal stop.
        for phase in [ManagedPhase::Starting, ManagedPhase::Ready] {
            let mut status = ManagedStatus {
                context: prepared.context.clone(),
                phase,
                running_peers: 4,
                failure: None,
            };
            expire_startup_status(&mut status, failure);
            publish(&directory, &status).unwrap();
            let retained: ManagedStatus =
                decode(&directory.read(STATUS, MAX_METADATA).unwrap()).unwrap();
            assert_eq!(retained.phase, ManagedPhase::Failed);
            assert_eq!(retained.running_peers, 0);
            assert_eq!(retained.failure, Some(progress.deadline().message()));
        }
    }

    #[test]
    fn handoff_channel_preserves_noncopy_payload_and_discards_it_after_deadline() {
        struct Handoff(Arc<std::sync::atomic::AtomicUsize>);
        impl Drop for Handoff {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let drops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let progress = progress::Progress::default();
        progress.enter(progress::Phase::Restart);
        let (sender, receiver) = mpsc::sync_channel(1);
        sender.send(Ok(Handoff(Arc::clone(&drops)))).ok().unwrap();
        let success = observe_readiness(
            &receiver,
            &progress,
            Instant::now(),
            Duration::from_secs(30),
        );
        assert!(matches!(success, Some(Ok(_))));
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(success);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        sender.send(Ok(Handoff(Arc::clone(&drops)))).ok().unwrap();
        let late = observe_readiness(
            &receiver,
            &progress,
            Instant::now() - Duration::from_secs(2),
            Duration::from_secs(1),
        );
        assert!(matches!(late, Some(Err(failure)) if failure == progress.deadline()));
        assert_eq!(drops.load(Ordering::SeqCst), 2);
        drop(sender);
        assert!(
            matches!(observe_readiness(&receiver, &progress, Instant::now(), Duration::from_secs(30)),
            Some(Err(failure)) if failure == progress.unconfirmed())
        );
    }

    #[test]
    fn scope_exit_cancels_background_work_without_changing_its_budget() {
        let cancelled = Arc::new(AtomicBool::new(false));
        {
            let _guard = CancelOnExit(Arc::clone(&cancelled));
        }
        assert!(cancelled.load(Ordering::Acquire));
    }

    #[test]
    fn definite_spawn_failure_retains_a_retryable_fresh_key_fence() {
        let (temporary, _context, directory) = launch_fence_fixture();
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

    // These fixture processes exercise only the native launch owner and OS exec boundary;
    // none is admitted as an Iroha daemon or treated as evidence of consensus readiness.
    fn launch_fence_fixture() -> (tempfile::TempDir, PrivateDirectory, PrivateDirectory) {
        let temporary = tempfile::tempdir().unwrap();
        let context = PrivateDirectory::open_or_create(temporary.path().join("network")).unwrap();
        let generation = context.create_child(generation::DIRECTORY).unwrap();
        for index in 0..4 {
            generation
                .write_atomic(format!("peer{index}.launch"), b"0", PublishMode::CreateNew)
                .unwrap();
        }
        (temporary, context, generation)
    }

    #[cfg(unix)]
    #[test]
    fn fresh_launch_spends_fence_before_exec_and_restart_never_reasserts() {
        let (_temporary, context, original) = launch_fence_fixture();
        let generation = retained_launch_directory(&context).unwrap();
        assert_eq!(generation.identity().unwrap(), original.identity().unwrap());
        // No safety-record directory is present. Its absence can never rearm a spent fence.
        assert!(!generation.path().join("state").exists());
        for index in 0..4 {
            let mut first = Command::new("/bin/sh");
            first
                .arg("-c")
                .arg("test \"$0\" = --sumeragi-assert-fresh-key");
            let mut child = spawn_with_launch_fence(&generation, index, &mut first).unwrap();
            assert_eq!(
                generation
                    .read(format!("peer{index}.launch"), 1)
                    .unwrap()
                    .as_slice(),
                b"1"
            );
            assert!(
                child.wait().unwrap().success(),
                "actual first exec receives its flag"
            );
            assert_eq!(
                first.get_args().last().unwrap(),
                "--sumeragi-assert-fresh-key"
            );
            context
                .write_atomic(format!("peer{index}.launch"), b"0", PublishMode::CreateNew)
                .unwrap();
            let mut restarted = Command::new("/bin/sh");
            restarted.arg("-c").arg("test \"$0\" = /bin/sh");
            let mut child = spawn_with_launch_fence(&generation, index, &mut restarted).unwrap();
            assert!(
                child.wait().unwrap().success(),
                "actual restart has no freshness flag"
            );
            assert_eq!(restarted.get_args().len(), 2);
            assert_eq!(
                generation
                    .read(format!("peer{index}.launch"), 1)
                    .unwrap()
                    .as_slice(),
                b"1"
            );
            assert_eq!(
                context
                    .read(format!("peer{index}.launch"), 1)
                    .unwrap()
                    .as_slice(),
                b"0",
                "an old outer armed marker cannot reassert an inner spent key"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn child_exec_failure_never_rearms_a_spent_fence() {
        let (temporary, context, generation) = launch_fence_fixture();
        let mut failed_child = Command::new("/bin/sh");
        failed_child.arg("-c").arg("exit 13");
        let mut child = spawn_with_launch_fence(&generation, 0, &mut failed_child).unwrap();
        assert_eq!(child.wait().unwrap().code(), Some(13));
        assert_eq!(generation.read("peer0.launch", 1).unwrap().as_slice(), b"1");
        let held = retained_launch_directory(&context).unwrap();
        let mut restarted = Command::new(temporary.path().join("absent-daemon"));
        assert!(spawn_with_launch_fence(&held, 0, &mut restarted).is_err());
        assert_eq!(restarted.get_args().len(), 0);
        assert_eq!(held.read("peer0.launch", 1).unwrap().as_slice(), b"1");
    }

    #[test]
    fn lost_fence_refuses_without_adopting_an_old_outer_marker() {
        let (temporary, context, generation) = launch_fence_fixture();
        generation
            .write_atomic("peer3.launch", b"1", PublishMode::Replace)
            .unwrap();
        std::fs::remove_file(generation.path().join("peer3.launch")).unwrap();
        context
            .write_atomic("peer3.launch", b"0", PublishMode::CreateNew)
            .unwrap();
        assert!(matches!(
            retained_launch_directory(&context),
            Err(Error::Invalid(_))
        ));
        let mut command = Command::new(temporary.path().join("absent-daemon"));
        assert!(matches!(
            spawn_with_launch_fence(&generation, 3, &mut command),
            Err(Error::Invalid(_))
        ));
        assert_eq!(command.get_args().len(), 0);
        assert!(!generation.path().join("peer3.launch").exists());
        assert_eq!(context.read("peer3.launch", 1).unwrap().as_slice(), b"0");
    }

    #[test]
    fn missing_generation_refuses_without_reconstructing_launch_custody() {
        let (_temporary, context, generation) = launch_fence_fixture();
        std::fs::rename(
            generation.path(),
            context.path().join("displaced-generation"),
        )
        .unwrap();
        for index in 0..4 {
            context
                .write_atomic(format!("peer{index}.launch"), b"0", PublishMode::CreateNew)
                .unwrap();
        }
        assert!(retained_launch_directory(&context).is_err());
        assert!(!context.path().join(generation::DIRECTORY).exists());
        for index in 0..4 {
            assert_eq!(
                context
                    .read(format!("peer{index}.launch"), 1)
                    .unwrap()
                    .as_slice(),
                b"0"
            );
        }
    }

    #[test]
    fn invalid_or_oversized_fences_and_peer_indices_never_assert_or_spawn() {
        let (temporary, context, generation) = launch_fence_fixture();
        for bytes in [b"".as_slice(), b"2".as_slice(), b"00".as_slice()] {
            generation
                .write_atomic("peer0.launch", bytes, PublishMode::Replace)
                .unwrap();
            assert!(retained_launch_directory(&context).is_err());
            let mut command = Command::new(temporary.path().join("absent-daemon"));
            assert!(spawn_with_launch_fence(&generation, 0, &mut command).is_err());
            assert_eq!(command.get_args().len(), 0);
            assert_eq!(
                std::fs::read(generation.path().join("peer0.launch")).unwrap(),
                bytes
            );
        }
        let mut command = Command::new(temporary.path().join("absent-daemon"));
        assert!(matches!(
            spawn_with_launch_fence(&generation, 4, &mut command),
            Err(Error::Invalid(_))
        ));
        assert_eq!(command.get_args().len(), 0);
        assert!(!generation.path().join("peer4.launch").exists());
    }

    #[cfg(unix)]
    #[test]
    fn linked_fences_and_replaced_generation_ancestors_refuse_before_spawn() {
        use std::os::unix::fs::symlink;
        for hard_link in [false, true] {
            let (temporary, context, generation) = launch_fence_fixture();
            let fence = generation.path().join("peer0.launch");
            let original = generation.path().join("foreign-launch");
            std::fs::rename(&fence, &original).unwrap();
            if hard_link {
                std::fs::hard_link(&original, &fence).unwrap();
            } else {
                symlink(&original, &fence).unwrap();
            }
            assert!(retained_launch_directory(&context).is_err());
            let mut command = Command::new(temporary.path().join("absent-daemon"));
            assert!(spawn_with_launch_fence(&generation, 0, &mut command).is_err());
            assert_eq!(command.get_args().len(), 0);
            assert_eq!(std::fs::read(&original).unwrap(), b"0");
        }
        let (temporary, context, generation) = launch_fence_fixture();
        std::fs::rename(
            generation.path(),
            context.path().join("displaced-generation"),
        )
        .unwrap();
        let replacement = context.create_child(generation::DIRECTORY).unwrap();
        replacement
            .write_atomic("peer0.launch", b"0", PublishMode::CreateNew)
            .unwrap();
        let mut command = Command::new(temporary.path().join("absent-daemon"));
        assert!(spawn_with_launch_fence(&generation, 0, &mut command).is_err());
        assert_eq!(command.get_args().len(), 0);
        assert_eq!(
            replacement.read("peer0.launch", 1).unwrap().as_slice(),
            b"0"
        );
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
        let mut owned = PeerProcesses::from_children(vec![child]);
        let mut sentinel = PeerProcesses::from_children(vec![other]);
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
    #[test]
    fn later_renewal_receivers_use_their_own_finite_budget_and_reject_late_success() {
        let progress = progress::Progress::default();
        progress.enter(progress::Phase::CustodyRenewal);
        let (sender, receiver) = mpsc::sync_channel(1);
        sender.send(Ok(())).unwrap();
        let original_start = Instant::now() - Duration::from_secs(86_400);
        assert!(original_start.elapsed() > Duration::from_secs(600));
        let renewal_start = Instant::now();
        assert!(matches!(
            observe_readiness(
                &receiver,
                &progress,
                renewal_start,
                Duration::from_secs(120)
            ),
            Some(Ok(()))
        ));
        sender.send(Ok(())).unwrap();
        assert_eq!(
            observe_readiness(
                &receiver,
                &progress,
                renewal_start - Duration::from_secs(121),
                Duration::from_secs(120)
            ),
            Some(Err(progress.deadline()))
        );
    }
}
