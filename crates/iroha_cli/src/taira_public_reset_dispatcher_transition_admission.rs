//! Closed candidate/predecessor admission and read-only runtime custody.
use super::super::super::FileSnapshot;
use super::*;
use std::io::Seek as _;
#[path = "taira_public_reset_dispatcher_transition_qualification.rs"]
pub(super) mod qualification;

pub(super) struct Held {
    files: Vec<(Pin, File, FileSnapshot)>,
}
fn pin(value: &Pin, maximum: u64) -> Result<(File, FileSnapshot)> {
    validate_absolute_normal_path(Path::new(&value.path), "transition pin")?;
    require_lower_sha256(&value.sha256, "transition pin digest")?;
    need(
        value.size > 0 && value.size <= maximum,
        "input size exceeds bound",
    )?;
    require_root_no_symlink_ancestors(Path::new(&value.path), "transition input")?;
    let (mut file, snapshot) = open_pinned_regular(Path::new(&value.path), "transition input")?;
    need(
        snapshot.uid == 0 && snapshot.mode & 0o7777 == value.mode && snapshot.len == value.size,
        "input custody or size differs",
    )?;
    need(
        hash_reader(&mut file)? == value.sha256,
        "input digest differs",
    )?;
    ensure_pinned_unchanged(Path::new(&value.path), "transition input", &file, &snapshot)?;
    file.rewind()?;
    Ok((file, snapshot))
}
pub(super) fn read(value: &Pin) -> Result<Vec<u8>> {
    let (file, snapshot) = pin(value, MAX_PROOF)?;
    read_pinned_bytes(
        Path::new(&value.path),
        "transition public record",
        file,
        &snapshot,
        MAX_PROOF,
    )
}
fn record(value: &Pin) -> Result<Value> {
    Ok(json::from_slice(&read(value)?)?)
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("missing string {key}"))
}
fn flag(value: &Value, key: &str, expected: bool) -> Result<()> {
    need(
        value.get(key).and_then(Value::as_bool) == Some(expected),
        &format!("invalid {key}"),
    )
}

pub(super) fn validate_plan(plan: &Plan) -> Result<()> {
    need(plan.schema == SCHEMA, "exact transition schema required")?;
    validate_lower_hex("operation ID", &plan.operation_id, 32)?;
    require_lower_sha256(&plan.host_identity_sha256, "host identity")?;
    let p = &plan.predecessor;
    require_lower_sha256(&p.inventory_sha256, "predecessor inventory")?;
    require_lower_sha256(&p.authorization_sha256, "predecessor authorization")?;
    super::super::super::validate_nonce(&p.authorization_nonce)?;
    need(
        p.completed_next_step > 0
            && p.completed_next_step <= 256
            && p.sealed_forward_ordinal > 0
            && p.sealed_forward_ordinal <= 4096,
        "invalid retained completion counters",
    )?;
    let coordination = coordination_root(plan);
    need(
        p.lease.path == coordination.join("lease.json").to_string_lossy()
            && p.progress.path == coordination.join("progress.json").to_string_lossy()
            && p.completed.path
                == format!(
                    "{RUNTIME}/journal-v1/completed/{}.json",
                    p.authorization_sha256
                )
            && p.dispatcher.path == FIXED_DISPATCHER
            && p.dispatcher.mode == 0o755,
        "predecessor control paths differ",
    )?;
    need(
        p.guards.len() == 5 && p.occupied.len() == 5,
        "five exact role closures required",
    )?;
    for ((guard, role), slug) in p.guards.iter().zip(&p.occupied).zip(SLUGS) {
        let directory = if slug == "taira-edge" { "edge" } else { slug };
        need(
            guard.path == format!("{CONTROL}/{slug}/guard.json")
                && guard.mode == 0o600
                && role.slug == slug
                && role.state.path == format!("/var/lib/taira/{directory}")
                && role.state.inode > 0
                && role.selector.path == format!("/srv/taira/{directory}/current")
                && role.selector.inode > 0,
            "role paths or identities differ",
        )?;
        let release = Path::new(&role.selector.target);
        need(
            release.parent() == Some(Path::new(&format!("/srv/taira/{directory}/releases"))),
            "selector release escaped",
        )?;
        validate_lower_hex(
            "occupied configuration commit",
            release.file_name().and_then(OsStr::to_str).unwrap_or(""),
            40,
        )?;
        let paths: BTreeSet<&str> = role.files.iter().map(|p| p.path.as_str()).collect();
        need(paths.len() == role.files.len(), "duplicate protected path")?;
        if slug == "taira-edge" {
            let expected = [
                format!("{}/bin/iroha", role.selector.target),
                format!("{}/taira.conf", role.selector.target),
                "/etc/nginx/conf.d/taira.conf".into(),
                "/etc/systemd/system/nginx.service".into(),
            ];
            need(
                paths == expected.iter().map(String::as_str).collect(),
                "edge protected closure differs",
            )?;
        } else {
            need(
                role.files.len() == 5
                    && paths
                        .contains(format!("{}/config/config.toml", role.selector.target).as_str())
                    && paths
                        .contains(format!("/etc/systemd/system/iroha3d-{slug}.service").as_str()),
                "validator protected closure incomplete",
            )?;
            for (index, basename) in [
                "iroha3d_taira",
                "config.toml",
                "genesis.json",
                "genesis.sha256",
                "",
            ]
            .iter()
            .enumerate()
            {
                if !basename.is_empty() {
                    need(
                        Path::new(&role.files[index].path).file_name()
                            == Some(OsStr::new(basename)),
                        "protected role order differs",
                    )?;
                }
            }
            for file in &role.files[..4] {
                need(
                    Path::new(&file.path).starts_with(&role.selector.target)
                        || Path::new(&file.path).starts_with(RUNTIME),
                    "protected artifact escaped runtime",
                )?;
            }
        }
    }
    Ok(())
}

pub(super) struct Locks {
    pinned: Vec<(PathBuf, File, FileSnapshot)>,
    deployment: deployment_lifecycle::Guard,
}
impl Locks {
    pub(super) fn revalidate(&self) -> Result<()> {
        for (path, file, snapshot) in &self.pinned {
            require_root_no_symlink_ancestors(path, "held transition lock")?;
            ensure_pinned_unchanged(path, "held transition lock", file, snapshot)?;
        }
        self.deployment.revalidate_unowned()
    }
}
pub(super) fn locks(plan: &Plan) -> Result<Locks> {
    locks_for_host(&plan.host_identity_sha256)
}
pub(super) fn locks_for_host(host_identity: &str) -> Result<Locks> {
    require_lower_sha256(host_identity, "host identity")?;
    let mut held = Vec::new();
    for path in [
        Path::new(RUNTIME).join(".routine-update.lock"),
        Path::new(RUNTIME).join("journal-v1/public-reset.lock"),
        Path::new(CONTROL)
            .join("hosts")
            .join(host_identity)
            .join("action.lock"),
    ] {
        require_root_no_symlink_ancestors(&path, "transition lock")?;
        let (file, snapshot) = open_pinned_regular(&path, "existing transition lock")?;
        need(
            snapshot.uid == 0 && snapshot.len == 0 && snapshot.mode & 0o7777 == 0o600,
            "unsafe existing lock",
        )?;
        file.try_lock()
            .wrap_err("deployment or host action owns a transition lock")?;
        ensure_pinned_unchanged(&path, "transition lock", &file, &snapshot)?;
        held.push((path, file, snapshot));
    }
    let deployment =
        deployment_lifecycle::acquire_unowned_existing(Instant::now() + Duration::from_secs(30))?;
    Ok(Locks {
        pinned: held,
        deployment,
    })
}

fn sealed(plan: &Plan) -> Result<()> {
    let p = &plan.predecessor;
    let lease: HostLeaseV1 = json::from_slice(&read(&p.lease)?)?;
    let progress: HostProgressV1 = json::from_slice(&read(&p.progress)?)?;
    validate_sealed_records(plan, &lease, &progress, &record(&p.completed)?)?;
    for name in [
        "progress.successor.json",
        ".progress.json.next",
        ".progress.successor.json.next",
    ] {
        need(
            !storage::exists(&coordination_root(plan).join(name))?,
            "unfinished host progress publication exists",
        )?;
    }
    Ok(())
}

pub(super) fn validate_sealed_records(
    plan: &Plan,
    lease: &HostLeaseV1,
    progress: &HostProgressV1,
    terminal: &Value,
) -> Result<()> {
    let p = &plan.predecessor;
    need(
        lease.schema == LEASE_SCHEMA_V1
            && lease.inventory_sha256 == p.inventory_sha256
            && lease.authorization_semantic_sha256 == p.authorization_sha256
            && lease.authorization_nonce == p.authorization_nonce,
        "predecessor lease differs",
    )?;
    need(
        progress.schema == HOST_PROGRESS_SCHEMA_V1
            && progress.inventory_sha256 == p.inventory_sha256
            && progress.authorization_sha256 == p.authorization_sha256
            && progress.authorization_nonce == p.authorization_nonce
            && progress.sealed
            && progress.prepared_action.is_none()
            && !progress.rolling_back
            && progress.next_forward_ordinal == p.sealed_forward_ordinal
            && progress.last_rollback_rank == 0
            && progress.rolled_back_hosts.is_empty(),
        "predecessor session is not sealed",
    )?;
    need(
        progress
            .touched_hosts
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            == SLUGS.into_iter().collect()
            && progress.touched_hosts.len() == 5,
        "sealed session role closure differs",
    )?;
    need(
        now_unix_ms()? > lease.execution_expires_at_unix_ms,
        "predecessor lease has not expired",
    )?;
    qualification::names(
        &terminal,
        "schema qualification_scope deployment_id inventory_sha256 authorization_sha256 authorization_nonce status phase next_step recovery_intent touched_validators edge_touched edge_rollback_complete rollback_next_validator failure_summary rollback_failures",
    )?;
    let _: super::super::super::executor_model::JournalV1 = json::from_value(terminal.clone())?;
    need(
        terminal.get("next_step").and_then(Value::as_u64) == Some(u64::from(p.completed_next_step))
            && terminal
                .get("rollback_next_validator")
                .and_then(Value::as_u64)
                == Some(0)
            && terminal.get("edge_touched").and_then(Value::as_bool) == Some(true)
            && terminal
                .get("edge_rollback_complete")
                .and_then(Value::as_bool)
                == Some(false)
            && terminal.get("failure_summary").and_then(Value::as_str) == Some("")
            && terminal
                .get("rollback_failures")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty)
            && terminal.get("touched_validators") == Some(&json::to_value(&SLUGS[..4].to_vec())?),
        "completed predecessor shape differs",
    )?;
    for (name, expected) in [
        ("schema", super::super::super::JOURNAL_SCHEMA_V1),
        ("inventory_sha256", p.inventory_sha256.as_str()),
        ("authorization_sha256", p.authorization_sha256.as_str()),
        ("authorization_nonce", p.authorization_nonce.as_str()),
        ("status", "completed"),
        ("phase", "completed"),
    ] {
        need(
            text(&terminal, name)? == expected,
            "completed predecessor receipt differs",
        )?;
    }
    need(
        terminal.get("recovery_intent") == Some(&Value::Null),
        "terminal recovery remains",
    )?;
    Ok(())
}

fn protected(plan: &Plan) -> Result<()> {
    for role in &plan.predecessor.occupied {
        let state = Path::new(&role.state.path);
        require_root_directory(state, role.slug != "taira-edge", "protected stopped state")?;
        let meta = fs::symlink_metadata(state)?;
        need(
            meta.dev() == role.state.device && meta.ino() == role.state.inode,
            "protected state identity changed",
        )?;
        let selector = Path::new(&role.selector.path);
        require_root_no_symlink_ancestors(selector, "protected selector")?;
        let info = fs::symlink_metadata(selector)?;
        need(
            info.file_type().is_symlink()
                && info.uid() == 0
                && info.dev() == role.selector.device
                && info.ino() == role.selector.inode
                && fs::read_link(selector)? == Path::new(&role.selector.target),
            "protected selector changed",
        )?;
        for value in &role.files {
            pin(value, MAX_BINARY)?;
        }
        let (unit, active, substate) = if role.slug == "taira-edge" {
            ("nginx.service".into(), "active", "running")
        } else {
            (format!("iroha3d-{}.service", role.slug), "inactive", "dead")
        };
        let bytes = run_host_command(
            SYSTEMCTL,
            &[
                "show",
                "--property=LoadState,ActiveState,SubState,FragmentPath,DropInPaths,NeedDaemonReload,ControlPID,MainPID,Job",
                &unit,
            ],
            Instant::now() + Duration::from_secs(30),
        )?;
        let mut parsed = std::str::from_utf8(&bytes)?
            .lines()
            .map(|line| {
                line.split_once('=')
                    .ok_or_else(|| eyre!("malformed unit state"))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let main_pid = parsed
            .remove("MainPID")
            .ok_or_else(|| eyre!("missing MainPID"))?
            .parse::<u32>()?;
        need(
            if role.slug == "taira-edge" {
                main_pid > 0
            } else {
                main_pid == 0
            },
            "protected MainPID changed",
        )?;
        let fragment = format!("/etc/systemd/system/{unit}");
        let expected = BTreeMap::from([
            ("LoadState", "loaded"),
            ("ActiveState", active),
            ("SubState", substate),
            ("FragmentPath", fragment.as_str()),
            ("DropInPaths", ""),
            ("NeedDaemonReload", "no"),
            ("ControlPID", "0"),
            ("Job", ""),
        ]);
        need(parsed == expected, "protected loaded unit state changed")?;
    }
    Ok(())
}

pub(super) fn admit(plan: &Plan) -> Result<Held> {
    let binaries = qualification::admit(&plan.candidate)?;
    sealed(plan)?;
    protected(plan)?;
    let mut held = Held { files: Vec::new() };
    let p = &plan.predecessor;
    let c = &plan.candidate;
    for value in [
        &plan.trusted_public_key,
        &p.completed,
        &p.lease,
        &p.progress,
        &c.preparation,
        &c.request,
        &c.checks,
        &c.capture,
        &c.binary_transfer,
        &c.source_transfer,
        &c.transfer_request,
        &c.transfer_completed,
        &c.executable,
    ]
    .into_iter()
    .chain(p.occupied.iter().flat_map(|r| r.files.iter()))
    .chain(binaries.iter())
    {
        let (file, snapshot) = pin(value, MAX_BINARY)?;
        held.files.push((value.clone(), file, snapshot));
    }
    Ok(held)
}
pub(super) fn revalidate(plan: &Plan, held: &Held) -> Result<()> {
    for (value, file, snapshot) in &held.files {
        ensure_pinned_unchanged(
            Path::new(&value.path),
            "preserved transition input",
            file,
            snapshot,
        )?;
    }
    sealed(plan)?;
    protected(plan)
}
pub(super) fn new_guards(plan: &Plan, root: &Path) -> Result<Vec<Vec<u8>>> {
    let mut result = Vec::new();
    for (index, slug) in SLUGS.iter().enumerate() {
        let _ = slug;
        let original = &plan.predecessor.guards[index];
        let archived = root.join(format!("old-guard-{index}"));
        let pin = if storage::exists(&archived)? {
            Pin {
                path: archived.to_string_lossy().into_owned(),
                ..original.clone()
            }
        } else {
            original.clone()
        };
        result.push(derive_guard(plan, index, &read(&pin)?)?);
    }
    Ok(result)
}

pub(super) fn derive_guard(plan: &Plan, index: usize, bytes: &[u8]) -> Result<Vec<u8>> {
    let slug = &SLUGS[index];
    let directory = if *slug == "taira-edge" { "edge" } else { *slug };
    need(
        sha256_hex(bytes) == plan.predecessor.guards[index].sha256,
        "old guard bytes differ",
    )?;
    let mut guard: HostGuardV1 = json::from_slice(bytes)?;
    need(
        guard.schema == HOST_GUARD_SCHEMA_V1
            && guard.host_slug == *slug
            && guard.service_root == format!("/srv/taira/{directory}")
            && guard.state_root == format!("/var/lib/taira/{directory}")
            && guard.upload_parent == format!("/srv/taira/{directory}/.public-reset-upload-v1")
            && guard.dispatcher_path == FIXED_DISPATCHER
            && guard.dispatcher_sha256 == plan.predecessor.dispatcher.sha256
            && guard.trusted_key_sha256 == plan.trusted_public_key.sha256,
        "existing guard authority differs",
    )?;
    guard
        .dispatcher_sha256
        .clone_from(&plan.candidate.executable.sha256);
    Ok(json::to_vec(&guard)?)
}
