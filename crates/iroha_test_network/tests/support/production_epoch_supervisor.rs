//! Linux acceptance of the shipping worker in the existing four-peer workload.
//! Public journal evidence is compared exactly; only native Kagami reads seeds.
use super::*;
use iroha_core::release_identity::BuildIdentity;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Identity {
    boot_id: String,
    pid: u32,
    start_time_ticks: u64,
}
impl Identity {
    fn read(pid: u32) -> Result<Self> {
        let boot_id = fs::read_to_string("/proc/sys/kernel/random/boot_id")?
            .trim()
            .to_owned();
        let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
        let (named_pid, _) = stat
            .split_once(' ')
            .ok_or_else(|| eyre!("invalid worker stat PID"))?;
        let (_, fields) = stat
            .rsplit_once(") ")
            .ok_or_else(|| eyre!("invalid worker stat fields"))?;
        let fields = fields.split_whitespace().collect::<Vec<_>>();
        ensure!(
            named_pid.parse::<u32>()? == pid
                && fields.len() > 19
                && !matches!(fields[0], "Z" | "X" | "x"),
            "worker process is not live"
        );
        let start_time_ticks = fields[19].parse::<u64>()?;
        ensure!(
            start_time_ticks > 0
                && boot_id.len() == 36
                && fs::read_to_string("/proc/sys/kernel/random/boot_id")?.trim() == boot_id,
            "worker boot/start identity changed"
        );
        Ok(Self {
            boot_id,
            pid,
            start_time_ticks,
        })
    }
    fn value(&self) -> Value {
        norito::json!({"boot_id":(self.boot_id),"pid":(self.pid),"start_time_ticks":(self.start_time_ticks)})
    }
    fn ready_path(&self, state: &Supervisor) -> PathBuf {
        state.worker.join(format!(
            "ready-{}-{}-{}-{}.json",
            state.policy_sha256, self.boot_id, self.pid, self.start_time_ticks
        ))
    }
}

pub(super) struct Supervisor {
    policy: PathBuf,
    custody: PathBuf,
    policy_sha256: String,
    worker: PathBuf,
    original_files: Vec<(PathBuf, Vec<u8>)>,
    original_identity: Option<Identity>,
    restarted_identity: Option<Identity>,
}

fn binary_digest(path: &Path) -> Result<String> {
    let mut input = File::open(path)?;
    let before = input.metadata()?;
    let (digest, bytes) = iroha_crypto::sha256_reader_bounded(&mut input, before.len())?;
    let after = input.metadata()?;
    let stamp = |metadata: &fs::Metadata| {
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
        )
    };
    ensure!(
        bytes == before.len()
            && stamp(&before) == stamp(&after)
            && stamp(&before) == stamp(&fs::symlink_metadata(path)?),
        "admitted public artifact changed during fixture pinning"
    );
    Ok(hex(&digest))
}

pub(super) fn start(
    binary: &Path,
    kagami: &Path,
    prepared: &prepare::Prepared,
    trust: PathBuf,
    build_identity: BuildIdentity,
) -> Result<Maintenance> {
    let directory = prepared.directory.clone();
    let journal = directory.join("epoch-maintenance");
    fs::create_dir(&journal)?;
    fs::set_permissions(&journal, fs::Permissions::from_mode(0o700))?;
    let schedule: Value = json::from_slice(&fs::read(&prepared.epoch_schedule)?)?;
    let owner = iroha::config::Config::load_file(directory.join("client.toml"))
        .map_err(|error| eyre!("native administrator config: {error:?}"))?;
    let policy = norito::json!({
        "schema_version":1,
        "intent":{
            "authorization":"until_stopped", "network_id":(prepared.network_id),
            "administrator":(owner.account),
            "payment_asset":(field(&schedule,"payment_asset")?),
            "transaction_fee_maximum":(field(&schedule,"transaction_fee_maximum")?),
            "first_epoch":1, "batch_epochs":2, "operation_timeout_ms":180000
        },
        "release_source_commit":(build_identity.release_source_commit()?),
        "iroha_sha256":(binary_digest(binary)?),
        "kagami":{"path":(kagami.to_str().ok_or_else(|| eyre!("Kagami fixture path is not UTF-8"))?),"sha256":(binary_digest(kagami)?)},
        "observation_trust_sha256":(hex(&iroha_crypto::sha256(fs::read(&trust)?))),
        "provision_timeout_ms":180000
    });
    let mut seeds = BTreeMap::new();
    for index in 0..4 {
        let peer = config(&directory.join(format!("peer{index}.toml")))?
            .common
            .peer
            .id;
        ensure!(
            seeds
                .insert(
                    peer,
                    directory.join(format!("runtime/mint-finality-signers/peer{index}.seed"))
                )
                .is_none(),
            "duplicate fixture validator custody"
        );
    }
    let custody = norito::json!({"schema_version":1,"seeds":(seeds.iter().map(|(validator,path)| -> Result<Value> {
        Ok(norito::json!({"validator":validator,"path":(path.to_str().ok_or_else(|| eyre!("fixture seed path is not UTF-8"))?)}))}).collect::<Result<Vec<_>>>()?)});
    let policy_bytes = json::to_vec(&policy)?;
    let state = Supervisor {
        policy: journal.join("fixture-policy.json"),
        custody: journal.join("fixture-custody.json"),
        policy_sha256: hex(&iroha_crypto::sha256(&policy_bytes)),
        worker: journal.join(format!("epoch-worker-{}", prepared.network_id)),
        original_files: Vec::new(),
        original_identity: None,
        restarted_identity: None,
    };
    private_file(&state.policy, &policy_bytes)?;
    private_file(&state.custody, &json::to_vec(&custody)?)?;
    let child = worker_command(binary, &directory, &trust, &journal, &state, "first")?.spawn()?;
    Ok(Maintenance {
        binary: binary.into(),
        directory,
        trust,
        schedule: prepared.epoch_schedule.clone(),
        journal,
        network: prepared.network_id,
        child,
        stopped: false,
        supervisor: Some(state),
    })
}

fn worker_command(
    binary: &Path,
    directory: &Path,
    trust: &Path,
    journal: &Path,
    state: &Supervisor,
    run_name: &str,
) -> Result<Command> {
    let mut child = Maintenance::base_command(binary, directory);
    child
        .args(["supervise", "--policy"])
        .arg(&state.policy)
        .arg("--trust")
        .arg(trust)
        .arg("--custody")
        .arg(&state.custody)
        .arg("--journal-dir")
        .arg(journal)
        .arg("--timeout-ms")
        .arg((PHASE_BUDGET * MONITOR_PHASES).as_millis().to_string())
        .stdout(private_file(
            &journal.join(format!("supervisor-{run_name}.stdout.log")),
            &[],
        )?)
        .stderr(private_file(
            &journal.join(format!("supervisor-{run_name}.stderr.log")),
            &[],
        )?);
    Ok(child)
}

fn status_command(
    maintenance: &Maintenance,
    state: &Supervisor,
    identity: &Identity,
    deadline: Instant,
) -> Result<Command> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    ensure!(
        !remaining.is_zero(),
        "supervisor status exceeded its original deadline"
    );
    let mut child = Maintenance::base_command(&maintenance.binary, &maintenance.directory);
    child
        .args(["supervisor-status", "--policy"])
        .arg(&state.policy)
        .arg("--trust")
        .arg(&maintenance.trust)
        .arg("--journal-dir")
        .arg(&maintenance.journal)
        .arg("--boot-id")
        .arg(&identity.boot_id)
        .arg("--pid")
        .arg(identity.pid.to_string())
        .arg("--start-time-ticks")
        .arg(identity.start_time_ticks.to_string())
        .arg("--timeout-ms")
        .arg(remaining.as_millis().to_string());
    Ok(child)
}

async fn ready(
    maintenance: &mut Maintenance,
    identity: &Identity,
    deadline: Instant,
) -> Result<Value> {
    timeout_at(deadline, async {
        let state = maintenance
            .supervisor
            .as_ref()
            .ok_or_else(|| eyre!("supervisor fixture absent"))?;
        let path = identity.ready_path(state);
        while !path.try_exists()? {
            ensure!(
                maintenance.child.try_wait()?.is_none(),
                "owned supervisor exited before process-bound readiness"
            );
            sleep(Duration::from_millis(100)).await;
        }
        ensure!(
            Identity::read(identity.pid)? == *identity,
            "readiness process identity changed"
        );
        let receipt: Value = json::from_slice(&fs::read(&path)?)?;
        ensure!(
            field(&receipt, "schema_version")?.as_u64() == Some(1)
                && field(&receipt, "worker")? == &identity.value()
                && text(&receipt, "policy_sha256")? == state.policy_sha256,
            "ready receipt is not bound to owned process and admitted policy"
        );
        // This command must succeed while the live worker holds its lifetime
        // lock. It authenticates the receipt and current target independently.
        let report: Value = json::from_slice(
            &run(
                status_command(maintenance, state, identity, deadline)?,
                deadline,
            )
            .await?,
        )?;
        ensure!(
            field(&report, "worker")? == &identity.value()
                && text(&report, "policy_sha256")? == state.policy_sha256
                && field(&report, "initial_completion")? == field(&receipt, "completion")?
                && field(field(&report, "current_completion")?, "target_epoch")?.as_u64()
                    == Some(1),
            "native active-worker status did not verify the initial actual target"
        );
        ensure!(
            Identity::read(identity.pid)? == *identity && maintenance.child.try_wait()?.is_none(),
            "worker exited while its readiness was being authenticated"
        );
        Ok(report)
    })
    .await
    .wrap_err("process-bound supervisor readiness exceeded the original phase deadline")?
}

fn immutable_files(
    maintenance: &Maintenance,
    state: &Supervisor,
) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let mut paths = [
        "plan.json",
        "trust.json",
        "prepared.json",
        "submitted.json",
        "submission-result.json",
        "completion.json",
    ]
    .map(|name| maintenance.operation(1).join(name))
    .to_vec();
    paths.extend([
        state.worker.join("plan.json"),
        state.worker.join("schedule-1.json"),
    ]);
    paths
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path)?;
            Ok((path, bytes))
        })
        .collect()
}

fn unchanged(files: &[(PathBuf, Vec<u8>)]) -> Result<()> {
    for (path, bytes) in files {
        ensure!(
            &fs::read(path)? == bytes,
            "restart or duplicate worker replaced retained public evidence at {}",
            path.display()
        );
    }
    Ok(())
}

pub(super) async fn restart(maintenance: &mut Maintenance, deadline: Instant) -> Result<()> {
    let original = Identity::read(
        maintenance
            .child
            .id()
            .ok_or_else(|| eyre!("owned worker has no PID"))?,
    )?;
    let first_status = ready(maintenance, &original, deadline).await?;
    let state = maintenance
        .supervisor
        .as_ref()
        .ok_or_else(|| eyre!("supervisor fixture absent"))?;
    let retained = immutable_files(maintenance, state)?;
    let original_plan: Value = json::from_slice(&fs::read(state.worker.join("plan.json"))?)?;
    ensure!(
        field(&original_plan, "original_trust")?
            == &json::from_slice::<Value>(&fs::read(&maintenance.trust)?)?,
        "worker did not preserve independently selected original trust"
    );
    let first_ready = fs::read(original.ready_path(state))?;
    let entries = || -> Result<BTreeSet<std::ffi::OsString>> {
        fs::read_dir(&state.worker)?
            .map(|entry| Ok(entry?.file_name()))
            .collect()
    };
    let before_entries = entries()?;
    let mut duplicate = worker_command(
        &maintenance.binary,
        &maintenance.directory,
        &maintenance.trust,
        &maintenance.journal,
        state,
        "duplicate",
    )?;
    duplicate.stdout(Stdio::piped()).stderr(Stdio::piped());
    let duplicate = timeout_at(deadline, duplicate.spawn()?.wait_with_output())
        .await
        .wrap_err("duplicate supervisor did not fail within the original deadline")??;
    let diagnostic = String::from_utf8_lossy(&duplicate.stderr);
    ensure!(
        duplicate.status.code() == Some(1)
            && (diagnostic.contains("Resource temporarily unavailable")
                || diagnostic.contains("os error 11")),
        "duplicate supervisor did not fail at the native exclusive worker lock"
    );
    ensure!(
        entries()? == before_entries && maintenance.child.try_wait()?.is_none(),
        "duplicate supervisor changed worker state or displaced its owner"
    );
    unchanged(&retained)?;
    private_file(
        &maintenance
            .journal
            .join("supervisor-duplicate-rejection.json"),
        &json::to_vec(
            &norito::json!({"schema_version":1,"exit_code":1,"worker":(original.value()),"retained_state_unchanged":true}),
        )?,
    )?;
    // Stop/reap only this fixture-owned worker. The four validators and the
    // genuine application workload remain under the original fixture owner.
    maintenance.stop(deadline).await?;
    let state = maintenance.supervisor.as_ref().unwrap();
    let mut stale = status_command(maintenance, state, &original, deadline)?;
    stale.stderr(Stdio::piped());
    let stale = timeout_at(deadline, stale.spawn()?.wait_with_output()).await??;
    ensure!(
        stale.status.code() == Some(1),
        "stopped worker readiness was accepted"
    );
    unchanged(&retained)?;
    maintenance.child = worker_command(
        &maintenance.binary,
        &maintenance.directory,
        &maintenance.trust,
        &maintenance.journal,
        state,
        "restart",
    )?
    .spawn()?;
    maintenance.stopped = false;
    let restarted = Identity::read(
        maintenance
            .child
            .id()
            .ok_or_else(|| eyre!("restarted worker has no PID"))?,
    )?;
    ensure!(
        restarted != original,
        "owned worker restart reused its process incarnation"
    );
    let restarted_status = ready(maintenance, &restarted, deadline).await?;
    let state = maintenance.supervisor.as_mut().unwrap();
    ensure!(
        fs::read(original.ready_path(state))? == first_ready
            && restarted.ready_path(state) != original.ready_path(state),
        "restart replaced or reused old readiness"
    );
    unchanged(&retained)?;
    private_file(
        &maintenance
            .journal
            .join("supervisor-restart-verification.json"),
        &json::to_vec(
            &norito::json!({"schema_version":1,"first":first_status,"restarted":restarted_status,
            "stopped_identity_rejected":true,"original_public_files":(retained.iter().map(|(path,bytes)| -> Result<Value> {
                Ok(norito::json!({"path":(path.to_str().ok_or_else(|| eyre!("fixture evidence path is not UTF-8"))?),"sha256":(hex(&iroha_crypto::sha256(bytes)))}))}).collect::<Result<Vec<_>>>()?)}),
        )?,
    )?;
    state.original_files = retained;
    state.original_identity = Some(original);
    state.restarted_identity = Some(restarted);
    Ok(())
}

pub(super) fn verify(maintenance: &Maintenance, state: &Supervisor, height: u64) -> Result<()> {
    ensure!(
        maintenance.stopped && height >= 2 * EPOCH_LENGTH,
        "Linux supervisor acceptance requires two actual epoch transitions"
    );
    ensure!(
        state.original_identity.is_some()
            && state.restarted_identity.is_some()
            && state.original_identity != state.restarted_identity,
        "supervisor restart was not exercised"
    );
    unchanged(&state.original_files)?;
    let first: Value = json::from_slice(&fs::read(state.worker.join("schedule-1.json"))?)?;
    let second: Value = json::from_slice(&fs::read(state.worker.join("schedule-2.json"))?)?;
    let full: Value = json::from_slice(&fs::read(&maintenance.schedule)?)?;
    let parameters = |value: &Value| -> Result<Vec<Parameter>> {
        Ok(json::from_value(field(value, "parameters")?.clone())?)
    };
    let (first_parameters, second_parameters, full_parameters) = (
        parameters(&first)?,
        parameters(&second)?,
        parameters(&full)?,
    );
    ensure!(
        first_parameters.len() == 2
            && second_parameters.len() == 2
            && first_parameters == full_parameters[..2]
            && second_parameters == full_parameters[1..3]
            && first_parameters[1] == second_parameters[0]
            && field(&first, "genesis_roster")? == field(&second, "genesis_roster")?
            && field(&first, "genesis_roster")? == field(&full, "genesis_roster")?,
        "native rolling schedule did not renew [1,2] to [2,3] with exact genesis binding and overlap"
    );
    // Stopping may interrupt only receipt publication. Reuse the native
    // Status results just authenticated by the shared final audit.
    let verified: Vec<Value> = json::from_slice(&fs::read(
        maintenance
            .journal
            .join("fixture-verified-completions.json"),
    )?)?;
    for epoch in [2, 3] {
        ensure!(
            verified
                .iter()
                .any(|receipt| receipt.get("target_epoch").and_then(Value::as_u64) == Some(epoch)),
            "second rolling batch has not completed genuine next-epoch work"
        );
    }
    private_file(
        &maintenance
            .journal
            .join("supervisor-renewal-verification.json"),
        &json::to_vec(
            &norito::json!({"schema_version":1,"network_id":(maintenance.network),
            "final_height":height,"batch_epochs":2,"first_schedule":[1,2],"second_schedule":[2,3],
            "overlap_equal":true,"retained_dispatch_unchanged":true,
            "worker":(state.restarted_identity.as_ref().unwrap().value())}),
        )?,
    )?;
    Ok(())
}
