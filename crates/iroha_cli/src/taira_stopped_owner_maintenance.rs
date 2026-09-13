//! Routine root-owned updater adapter for the shared stopped Inrou owner boundary.
//!
//! This command carries no reset or signing authority. The live parent updater,
//! its exclusive flock, and its retained public plan delimit routine maintenance.

use super::*;
use base64::Engine as _;
#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt as _;

const REQUEST_SCHEMA: &str = "taira.stopped-owner-maintenance.request.v1";
const RESULT_SCHEMA: &str = "taira.stopped-owner-maintenance.result.v1";

/// Credential-free stopped-owner maintenance for the active routine updater.
#[derive(clap::Args, Debug)]
pub(crate) struct StoppedOwnerMaintenance {
    /// Read the bounded root-owned public request from this inherited read-only descriptor.
    #[arg(long)]
    request_fd: u32,
}

#[derive(Debug, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct MaintenanceRequest {
    schema: String,
    operation_directory: String,
    owner: MaintenanceOwner,
}

#[derive(Debug, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct MaintenanceOwner {
    pid: u32,
    start_time_ticks: u64,
    argv: Vec<String>,
    lock: MaintenanceLock,
}

#[derive(Debug, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct MaintenanceLock {
    device: u64,
    inode: u64,
}

impl StoppedOwnerMaintenance {
    /// Run without opening client configuration or signing inputs.
    pub(crate) fn run_without_client_config(&self, mut output: impl Write) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            if rustix::process::geteuid().as_raw() != 0 {
                return Err(eyre!("stopped-owner maintenance requires OS root"));
            }
            let bytes = crate::client_config::read_inherited_private_file(
                self.request_fd,
                16_384,
                "stopped-owner maintenance request",
            )?;
            let request: MaintenanceRequest = json::from_slice(&bytes)?;
            let report = run_maintenance(&request)?;
            output.write_all(&json::to_vec(&report)?)?;
            output.write_all(b"\n")?;
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = &mut output;
            Err(eyre!("stopped-owner maintenance requires Linux"))
        }
    }
}

#[cfg(any(target_os = "linux", test))]
fn text_field<'a>(value: &'a json::Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(json::Value::as_str)
        .ok_or_else(|| eyre!("maintenance public plan is missing a required text field"))
}

#[cfg(any(target_os = "linux", test))]
struct MaintenanceScope {
    operation: String,
    runtime: PathBuf,
    config: PathBuf,
    state: PathBuf,
    previous_daemon: PathBuf,
    candidate_cli: PathBuf,
    units: Vec<Vec<u8>>,
}

#[cfg(any(target_os = "linux", test))]
fn maintenance_scope(request: &MaintenanceRequest, plan: &json::Value) -> Result<MaintenanceScope> {
    let operation = text_field(plan, "operation")?;
    if request.schema != REQUEST_SCHEMA
        || text_field(plan, "schema")? != "taira.daemon-update.plan.v1"
        || !operation.strip_prefix("update-").is_some_and(|suffix| {
            suffix.len() == 32
                && suffix
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        || request.owner.pid == 0
        || request.owner.start_time_ticks == 0
        || request.owner.argv != ["/usr/bin/python3", "-I", "-"]
    {
        return Err(eyre!(
            "maintenance request or routine updater identity differs"
        ));
    }
    let deployment = plan
        .get("deployment")
        .ok_or_else(|| eyre!("maintenance deployment is absent"))?;
    let runtime = PathBuf::from(text_field(deployment, "runtime_root")?);
    let config = PathBuf::from(text_field(deployment, "config_root")?);
    let state = PathBuf::from(text_field(deployment, "state_root")?);
    let installed = match plan.get("failed_start") {
        Some(failed) => failed.get("installed"),
        None => deployment.get("current"),
    }
    .ok_or_else(|| eyre!("maintenance installed runtime is absent"))?;
    let previous_daemon = PathBuf::from(text_field(installed, "daemon")?);
    let commit = text_field(plan, "commit")?;
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(eyre!("maintenance candidate identity differs"));
    }
    for path in [
        &runtime,
        &config,
        &state,
        &previous_daemon,
        Path::new(&request.operation_directory),
    ] {
        super::super::validate_absolute_normal_path(path, "maintenance public path")?;
    }
    if runtime.join(operation) != Path::new(&request.operation_directory)
        || !previous_daemon.starts_with(&runtime)
        || previous_daemon.file_name() != Some(OsStr::new("iroha3d_taira"))
        || config.starts_with(&runtime)
        || runtime.starts_with(&config)
        || state.starts_with(&runtime)
        || runtime.starts_with(&state)
        || state.starts_with(&config)
        || config.starts_with(&state)
    {
        return Err(eyre!("maintenance public roots or installed daemon differ"));
    }
    let roles = deployment
        .get("roles")
        .and_then(json::Value::as_array)
        .ok_or_else(|| eyre!("maintenance roles are absent"))?;
    let rows = plan
        .get("units")
        .and_then(json::Value::as_array)
        .ok_or_else(|| eyre!("maintenance units are absent"))?;
    if roles.len() != 4 || rows.len() != 4 {
        return Err(eyre!("maintenance requires all four ordered owner slots"));
    }
    let mut units = Vec::new();
    for (index, slug) in super::super::VALIDATOR_SLUGS.iter().enumerate() {
        if roles[index].as_str() != Some(*slug) || text_field(&rows[index], "role")? != *slug {
            return Err(eyre!("maintenance owner ordering differs"));
        }
        let raw = BASE64.decode(text_field(&rows[index], "before")?)?;
        if raw.is_empty()
            || raw.len() > 1024 * 1024
            || sha256_hex(&raw) != text_field(&rows[index], "before_sha256")?
        {
            return Err(eyre!("maintenance installed unit bytes differ"));
        }
        units.push(raw);
    }
    Ok(MaintenanceScope {
        operation: operation.to_owned(),
        candidate_cli: runtime.join(format!("release-{commit}-{operation}/bin/iroha")),
        runtime,
        config,
        state,
        previous_daemon,
        units,
    })
}

#[cfg(target_os = "linux")]
fn public_bytes(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    require_root_no_symlink_ancestors(path, "maintenance public evidence")?;
    let pinned = pin_owner_private_file(path, "maintenance public evidence")?;
    if pinned.snapshot.len > maximum {
        return Err(eyre!("maintenance public evidence exceeds its bound"));
    }
    read_pinned_bytes(
        path,
        "maintenance public evidence",
        pinned.file,
        &pinned.snapshot,
        maximum,
    )
}

#[cfg(target_os = "linux")]
fn proc_bytes(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(maximum + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(eyre!("maintenance process evidence exceeds its bound"));
    }
    Ok(bytes)
}

#[cfg(any(target_os = "linux", test))]
fn process_identity(bytes: &[u8]) -> Result<(u32, u32, u64)> {
    let text = std::str::from_utf8(bytes)?;
    let (prefix, tail) = text
        .rsplit_once(") ")
        .ok_or_else(|| eyre!("maintenance process stat is malformed"))?;
    let pid = prefix
        .split_once(' ')
        .ok_or_else(|| eyre!("maintenance process PID is absent"))?
        .0
        .parse()?;
    let fields: Vec<_> = tail.split_ascii_whitespace().collect();
    if fields.len() < 20 || matches!(fields[0], "Z" | "X") {
        return Err(eyre!("maintenance updater process is not live"));
    }
    Ok((pid, fields[1].parse()?, fields[19].parse()?))
}

#[cfg(any(target_os = "linux", test))]
fn require_updater_flock(bytes: &[u8], owner: &MaintenanceOwner) -> Result<()> {
    let mut matches = 0;
    for line in std::str::from_utf8(bytes)?.lines() {
        let row: Vec<_> = line.split_ascii_whitespace().collect();
        if row.len() != 8
            || row[1..4] != ["FLOCK", "ADVISORY", "WRITE"]
            || row[4].parse::<u32>().ok() != Some(owner.pid)
            || row[6..] != ["0", "EOF"]
        {
            continue;
        }
        let identity: Vec<_> = row[5].split(':').collect();
        if identity.len() == 3
            && u32::from_str_radix(identity[0], 16).ok()
                == Some(rustix::fs::major(owner.lock.device as rustix::fs::Dev))
            && u32::from_str_radix(identity[1], 16).ok()
                == Some(rustix::fs::minor(owner.lock.device as rustix::fs::Dev))
            && identity[2].parse::<u64>().ok() == Some(owner.lock.inode)
        {
            matches += 1;
        }
    }
    if matches != 1 {
        return Err(eyre!(
            "maintenance updater no longer holds the exact exclusive flock"
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn verify_owner(
    request: &MaintenanceRequest,
    scope: &MaintenanceScope,
    deadline: Instant,
) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(eyre!("maintenance deadline elapsed"));
    }
    let directory = Path::new(&request.operation_directory);
    require_root_directory(directory, true, "maintenance operation")?;
    for terminal in [
        "result.json",
        "failure.json",
        "rollback.json",
        "start-intent.json",
    ] {
        match fs::symlink_metadata(directory.join(terminal)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(eyre!("maintenance operation has started or terminated")),
        }
    }
    let lock_path = scope.runtime.join(".routine-update.lock");
    require_root_no_symlink_ancestors(&lock_path, "maintenance updater lock")?;
    let lock = fs::symlink_metadata(&lock_path)?;
    if !lock.is_file()
        || lock.uid() != 0
        || lock.gid() != 0
        || lock.nlink() != 1
        || lock.mode() & 0o7777 != 0o600
        || (lock.dev(), lock.ino()) != (request.owner.lock.device, request.owner.lock.inode)
    {
        return Err(eyre!("maintenance updater lock custody differs"));
    }
    let own = process_identity(&proc_bytes(Path::new("/proc/self/stat"), 4096)?)?;
    let process = PathBuf::from(format!("/proc/{}", request.owner.pid));
    let expected = process_identity(&proc_bytes(&process.join("stat"), 4096)?)?;
    if own.1 != request.owner.pid
        || expected.0 != request.owner.pid
        || expected.2 != request.owner.start_time_ticks
        || proc_bytes(&process.join("cmdline"), 4096)? != b"/usr/bin/python3\0-I\0-\0"
    {
        return Err(eyre!(
            "maintenance caller is not the retained live parent updater"
        ));
    }
    require_updater_flock(
        &proc_bytes(Path::new("/proc/locks"), 1024 * 1024)?,
        &request.owner,
    )
}

#[cfg(target_os = "linux")]
fn require_cohort_vacant(scope: &MaintenanceScope, deadline: Instant) -> Result<()> {
    require_root_directory(&scope.config, false, "maintenance config root")?;
    require_root_directory(&scope.state, false, "maintenance state root")?;
    require_root_no_symlink_ancestors(&scope.previous_daemon, "maintenance installed daemon")?;
    let _ = open_pinned_regular(&scope.previous_daemon, "maintenance installed daemon")?;
    for (slot, slug) in super::super::VALIDATOR_SLUGS.iter().enumerate() {
        let unit = format!("iroha3d-{slug}.service");
        let fragment = Path::new("/etc/systemd/system").join(&unit);
        require_root_no_symlink_ancestors(&fragment, "maintenance installed unit")?;
        let (file, snapshot) = open_pinned_regular(&fragment, "maintenance installed unit")?;
        if snapshot.len > 1024 * 1024 {
            return Err(eyre!("maintenance unit exceeds its bound"));
        }
        // Unit files may be world-readable, but no non-root writer may alter them.
        let metadata = fs::symlink_metadata(&fragment)?;
        if metadata.uid() != 0
            || metadata.mode() & 0o022 != 0
            || read_pinned_bytes(
                &fragment,
                "maintenance installed unit",
                file,
                &snapshot,
                1024 * 1024,
            )? != scope.units[slot]
        {
            return Err(eyre!(
                "maintenance installed unit differs from the retained plan"
            ));
        }
        let loaded = run_host_command(
            SYSTEMCTL,
            &[
                "show",
                "--all",
                "--property=FragmentPath",
                "--property=DropInPaths",
                "--property=NeedDaemonReload",
                &unit,
            ],
            deadline,
        )?;
        validate_loaded_unit_evidence(&loaded, &fragment)?;
        let evidence = run_host_command(
            SYSTEMCTL,
            &[
                "show",
                "--all",
                "--property=ActiveState",
                "--property=SubState",
                "--property=MainPID",
                "--property=ControlPID",
                "--property=ControlGroup",
                "--property=Job",
                &unit,
            ],
            deadline,
        )?;
        if let Some(cgroup) = validate_vacant_unit_evidence(&evidence, true)? {
            require_root_directory(&cgroup, false, "maintenance supervisor cgroup")?;
            if rustix::fs::statfs(&cgroup)?.f_type as u64 != 0x6367_7270
                || fs::read_to_string(cgroup.join("cgroup.events"))?
                    .lines()
                    .filter(|line| line.starts_with("populated "))
                    .collect::<Vec<_>>()
                    != ["populated 0"]
            {
                return Err(eyre!("maintenance supervisor cgroup is not kernel-empty"));
            }
        }
    }
    require_no_live_path_references(
        &[&scope.config, &scope.state, &scope.previous_daemon],
        deadline,
    )
}

#[cfg(target_os = "linux")]
fn run_maintenance(request: &MaintenanceRequest) -> Result<json::Value> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let directory = Path::new(&request.operation_directory);
    require_root_directory(directory, true, "maintenance operation")?;
    let plan_bytes = public_bytes(&directory.join("intent.json"), 8 * 1024 * 1024)?;
    let plan: json::Value = json::from_slice(&plan_bytes)?;
    let scope = maintenance_scope(request, &plan)?;
    require_root_directory(&scope.runtime, true, "maintenance runtime")?;
    if std::env::current_exe()? != scope.candidate_cli {
        return Err(eyre!(
            "maintenance executable is not the planned candidate CLI"
        ));
    }
    let vacant = || {
        verify_owner(request, &scope, deadline)?;
        if public_bytes(&directory.join("intent.json"), 8 * 1024 * 1024)? != plan_bytes {
            return Err(eyre!("maintenance public plan changed"));
        }
        require_cohort_vacant(&scope, deadline)
    };
    vacant()?;
    // Keep every slot lock alive. A later owner's failed preflight prevents any
    // earlier owner mutation, and no supervisor can claim a preflighted slot.
    let plans = (0..4)
        .map(|slot| stopped_runtime::preflight_stopped_owner(slot, true, false, deadline, &vacant))
        .collect::<Result<Vec<_>>>()?;
    let publish = |name: &str, value: &json::Value| {
        super::super::inputs::write_new_private(&directory.join(name), &json::to_vec(value)?)
    };
    publish(
        "stopped-owner-maintenance-intent.json",
        &norito::json!({
            "schema": "taira.stopped-owner-maintenance.intent.v1", "operation": (scope.operation.clone()),
            "all_four_preflighted": true, "kills_or_flushes": false,
        }),
    )?;
    for (slot, plan) in plans.iter().enumerate() {
        stopped_runtime::apply_stopped_owner(plan, true, &vacant)?;
        publish(
            &format!("stopped-owner-maintenance-slot-{slot}.json"),
            &norito::json!({
                "slot": slot, "stopped_owner_clean": true,
            }),
        )?;
    }
    vacant()?;
    let result = norito::json!({"schema": RESULT_SCHEMA, "operation": (scope.operation),
        "all_four_stopped_owners_clean": true});
    publish("stopped-owner-maintenance-result.json", &result)?;
    Ok(result)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn fixture() -> (MaintenanceRequest, json::Value) {
        let operation = format!("update-{}", "a".repeat(32));
        let units = super::super::super::VALIDATOR_SLUGS
            .iter()
            .map(|slug| {
                let raw = format!("[Unit]\nDescription={slug}\n");
                norito::json!({"role": (*slug), "before": (BASE64.encode(raw.as_bytes())),
                "before_sha256": (sha256_hex(raw.as_bytes()))})
            })
            .collect::<Vec<_>>();
        (
            MaintenanceRequest {
                schema: REQUEST_SCHEMA.into(),
                operation_directory: format!("/private/runtime/taira/{operation}"),
                owner: MaintenanceOwner {
                    pid: 123,
                    start_time_ticks: 456,
                    argv: vec!["/usr/bin/python3".into(), "-I".into(), "-".into()],
                    lock: MaintenanceLock {
                        device: rustix::fs::makedev(8, 1) as u64,
                        inode: 789,
                    },
                },
            },
            norito::json!({"schema": "taira.daemon-update.plan.v1", "operation": operation,
                "commit": ("b".repeat(40)), "units": units,
                "deployment": {"runtime_root": "/private/runtime/taira", "config_root": "/srv/taira",
                    "state_root": "/var/lib/taira", "roles": (super::super::super::VALIDATOR_SLUGS),
                    "current": {"daemon": "/private/runtime/taira/selected/bin/iroha3d_taira"}}}),
        )
    }

    #[test]
    fn maintenance_scope_binds_all_four_units_and_failed_installed_runtime() -> Result<()> {
        let (request, mut plan) = fixture();
        let scope = maintenance_scope(&request, &plan)?;
        assert_eq!(scope.units.len(), 4);
        assert_eq!(
            scope.previous_daemon,
            Path::new("/private/runtime/taira/selected/bin/iroha3d_taira")
        );
        plan.as_object_mut().unwrap().insert(
            "failed_start".into(),
            norito::json!({"installed": {
            "daemon": "/private/runtime/taira/failed/bin/iroha3d_taira"}}),
        );
        assert_eq!(
            maintenance_scope(&request, &plan)?.previous_daemon,
            Path::new("/private/runtime/taira/failed/bin/iroha3d_taira")
        );
        *plan
            .get_mut("units")
            .unwrap()
            .get_mut(3_usize)
            .unwrap()
            .get_mut("role")
            .unwrap() = norito::json!("taira-validator-3");
        assert!(maintenance_scope(&request, &plan).is_err());
        let (request, mut plan) = fixture();
        *plan
            .get_mut("units")
            .unwrap()
            .get_mut(3_usize)
            .unwrap()
            .get_mut("before")
            .unwrap() = norito::json!(BASE64.encode(b"changed"));
        assert!(maintenance_scope(&request, &plan).is_err());
        let (mut request, plan) = fixture();
        request.operation_directory.push_str("/extra");
        assert!(maintenance_scope(&request, &plan).is_err());
        let (request, mut plan) = fixture();
        *plan
            .get_mut("deployment")
            .unwrap()
            .get_mut("state_root")
            .unwrap() = norito::json!("/private/runtime/taira");
        assert!(maintenance_scope(&request, &plan).is_err());
        Ok(())
    }

    #[test]
    fn maintenance_flock_requires_one_exact_live_updater_owner() -> Result<()> {
        let (request, _) = fixture();
        let valid = b"1: FLOCK ADVISORY WRITE 123 08:01:789 0 EOF\n";
        require_updater_flock(valid, &request.owner)?;
        for bad in [
            "1: FLOCK ADVISORY WRITE 124 08:01:789 0 EOF\n",
            "1: FLOCK ADVISORY READ 123 08:01:789 0 EOF\n",
            "1: POSIX ADVISORY WRITE 123 08:01:789 0 EOF\n",
            "1: FLOCK ADVISORY WRITE 123 08:01:790 0 EOF\n",
            "1: FLOCK ADVISORY WRITE 123 08:02:789 0 EOF\n",
            "1: -> FLOCK ADVISORY WRITE 123 08:01:789 0 EOF\n",
        ] {
            assert!(require_updater_flock(bad.as_bytes(), &request.owner).is_err());
        }
        assert!(require_updater_flock(&valid.repeat(2), &request.owner).is_err());
        Ok(())
    }

    #[test]
    fn maintenance_process_identity_handles_names_and_rejects_dead_owner() -> Result<()> {
        let mut fields = vec!["S", "42"];
        fields.extend(std::iter::repeat_n("0", 17));
        fields.push("456");
        let live = format!("123 (python ) worker) {}", fields.join(" "));
        assert_eq!(process_identity(live.as_bytes())?, (123, 42, 456));
        fields[0] = "Z";
        assert!(process_identity(format!("123 (python) {}", fields.join(" ")).as_bytes()).is_err());
        assert!(process_identity(b"123 (python) S 42").is_err());
        Ok(())
    }
}
