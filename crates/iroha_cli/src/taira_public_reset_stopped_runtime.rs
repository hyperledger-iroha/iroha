//! Reconcile an authenticated validator's stopped Inrou owner before publishing stop evidence.

use super::*;

pub(in crate::taira_public_reset) fn validate_config_slot(
    slug: &str,
    inrou: &iroha_config::parameters::actual::SoracloudRuntimeInrou,
) -> Result<()> {
    let slot = owner_slot(slug)?;
    let identity = iroha_config::parameters::defaults::soracloud_runtime::INROU_PORTABLE_VM_ID_BASE
        + u32::try_from(slot)?;
    if !inrou.enabled
        || inrou.portable_vm_uid.map(std::num::NonZeroU32::get) != Some(identity)
        || inrou.portable_vm_gid.map(std::num::NonZeroU32::get) != Some(identity)
    {
        return Err(eyre!(
            "validator Inrou identity differs from its canonical owner slot"
        ));
    }
    Ok(())
}

fn owner_slot(slug: &str) -> Result<usize> {
    super::super::VALIDATOR_SLUGS
        .iter()
        .position(|expected| *expected == slug)
        .ok_or_else(|| eyre!("stopped Inrou cleanup requires an exact validator owner"))
}

#[cfg(any(target_os = "linux", all(test, unix)))]
pub(super) fn reconcile(admitted: &HostAdmission, cleanup: bool) -> Result<()> {
    let HostTarget::Validator(validator) = &admitted.target else {
        return Ok(());
    };
    let slot = owner_slot(&validator.slug)?;
    let vacant = || require_vacant_unit(admitted, true);
    // The candidate scope cannot erase ownership left by the admitted old runtime.
    // A core-only candidate never starts Inrou, so a vacant/disabled prior runtime
    // needs no guest tooling. Still prove absence of the reserved worker identity
    // and owner paths before accepting this boundary.
    if !admitted.inventory.qualification_scope.includes_inrou()
        && !prior_inrou_enabled(validator)?
        && !retained_owner_lock(slot)?
    {
        vacant()?;
        let check = || stopped_owner_deadline(admitted.action_deadline);
        let identity =
            iroha_config::parameters::defaults::soracloud_runtime::INROU_PORTABLE_VM_ID_BASE
                + u32::try_from(slot)?;
        require_identity_absent(Path::new("/proc"), identity, &check)?;
        preflight_cgroups(
            Path::new("/sys/fs/cgroup/iroha-inrou-v1"),
            slot,
            false,
            &stopped_cgroup_custody,
            &check,
        )?;
        return vacant();
    }
    let plan = preflight_stopped_owner(slot, cleanup, cleanup, admitted.action_deadline, &vacant)?;
    apply_stopped_owner(&plan, cleanup, &vacant)
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn retained_owner_lock(slot: usize) -> Result<bool> {
    let path = PathBuf::from(format!("/run/iroha-inrou-firewall-v1-slot-{slot}.lock"));
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true), // Full custody validation happens before any cleanup.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn prior_inrou_enabled(validator: &ValidatorV1) -> Result<bool> {
    use iroha_config::{
        base::toml::{MAX_TOML_SOURCE_BYTES, TomlSource},
        parameters::actual,
    };
    if validator.is_vacant() {
        return Ok(false);
    }
    let entry = validator.admitted_release()?.artifact("config")?;
    let path = Path::new(&entry.path);
    require_root_no_symlink_ancestors(path, "stopped prior validator config")?;
    let (file, snapshot) = open_pinned_regular(path, "stopped prior validator config")?;
    if snapshot.uid != 0
        || snapshot.mode & 0o7777 != u32::from(entry.mode)
        || snapshot.len != entry.size
    {
        return Err(eyre!("stopped prior validator config custody drifted"));
    }
    let bytes = zeroize::Zeroizing::new(read_pinned_bytes(
        path,
        "stopped prior validator config",
        file,
        &snapshot,
        MAX_TOML_SOURCE_BYTES as u64,
    )?);
    if sha256_hex(&bytes) != entry.sha256 {
        return Err(eyre!(
            "stopped prior validator config differs from the admitted bytes"
        ));
    }
    let text =
        std::str::from_utf8(&bytes).map_err(|_| eyre!("prior validator config is not UTF-8"))?;
    let table: toml::Table =
        toml::from_str(text).map_err(|_| eyre!("prior validator config is not TOML"))?;
    let config = actual::Root::from_toml_source(TomlSource::new_sensitive(
        path.to_path_buf(),
        table,
        crate::soracloud::zeroize_taira_toml_table,
    ))
    .map_err(|_| eyre!("prior validator config failed typed admission"))?;
    prior_inrou_config_enabled(&validator.slug, &config.soracloud_runtime.inrou)
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn prior_inrou_config_enabled(
    slug: &str,
    inrou: &iroha_config::parameters::actual::SoracloudRuntimeInrou,
) -> Result<bool> {
    if inrou.enabled {
        validate_config_slot(slug, inrou)?;
    }
    Ok(inrou.enabled)
}

/// One authenticated stopped owner held across cohort preflight and mutation.
#[cfg(any(target_os = "linux", all(test, unix)))]
pub(super) struct StoppedOwnerPlan {
    slot: usize,
    deadline: Instant,
    _lock: File,
    cgroups: StoppedCgroupPlan,
    firewall: StoppedFirewallState,
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn stopped_owner_deadline(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(eyre!("stopped Inrou owner deadline elapsed"));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn stopped_cgroup_custody(path: &Path) -> Result<()> {
    require_root_directory(path, false, "stopped Inrou cgroup")?;
    if rustix::fs::statfs(path)?.f_type as u64 != 0x6367_7270 {
        return Err(eyre!(
            "stopped Inrou hierarchy is not the kernel cgroup-v2 filesystem"
        ));
    }
    Ok(())
}

/// Admit both worker and firewall state before either can be changed.
#[cfg(any(target_os = "linux", all(test, unix)))]
pub(super) fn preflight_stopped_owner(
    slot: usize,
    cleanup: bool,
    create_lock: bool,
    deadline: Instant,
    require_vacant: &impl Fn() -> Result<()>,
) -> Result<StoppedOwnerPlan> {
    if slot >= 4 {
        return Err(eyre!("stopped Inrou owner slot is invalid"));
    }
    stopped_owner_deadline(deadline)?;
    let lock_path = PathBuf::from(format!("/run/iroha-inrou-firewall-v1-slot-{slot}.lock"));
    require_root_no_symlink_ancestors(&lock_path, "stopped Inrou owner lock")?;
    let lock = acquire_slot_lock(&lock_path, 0, create_lock)?;
    require_vacant()?;
    let identity = iroha_config::parameters::defaults::soracloud_runtime::INROU_PORTABLE_VM_ID_BASE
        + u32::try_from(slot)?;
    let check = || stopped_owner_deadline(deadline);
    require_identity_absent(Path::new("/proc"), identity, &check)?;
    let cgroups = preflight_cgroups(
        Path::new("/sys/fs/cgroup/iroha-inrou-v1"),
        slot,
        cleanup,
        &stopped_cgroup_custody,
        &check,
    )?;
    let program = stopped_firewall_program()?;
    let firewall = parse_stopped_firewall(
        slot,
        &run_host_command(program, &["-w", "5", "-S"], deadline)?,
    )?;
    if !cleanup && firewall.present {
        return Err(eyre!("stopped Inrou owner retains its firewall chain"));
    }
    Ok(StoppedOwnerPlan {
        slot,
        deadline,
        _lock: lock,
        cgroups,
        firewall,
    })
}

/// Consume only the preflighted owner while retaining its exclusive slot lock.
#[cfg(any(target_os = "linux", all(test, unix)))]
pub(super) fn apply_stopped_owner(
    plan: &StoppedOwnerPlan,
    cleanup: bool,
    require_vacant: &impl Fn() -> Result<()>,
) -> Result<()> {
    let check = || stopped_owner_deadline(plan.deadline);
    check()?;
    let program = stopped_firewall_program()?;
    if parse_stopped_firewall(
        plan.slot,
        &run_host_command(program, &["-w", "5", "-S"], plan.deadline)?,
    )? != plan.firewall
    {
        return Err(eyre!(
            "stopped Inrou firewall changed after cohort preflight"
        ));
    }
    let identity = iroha_config::parameters::defaults::soracloud_runtime::INROU_PORTABLE_VM_ID_BASE
        + u32::try_from(plan.slot)?;
    run_stopped_owner_boundary(
        &|| {
            require_vacant()?;
            require_identity_absent(Path::new("/proc"), identity, &check)
        },
        &|| {
            apply_cgroup_plan(
                &plan.cgroups,
                cleanup,
                &stopped_cgroup_custody,
                &check,
                &|parent, name| {
                    rustix::fs::unlinkat(parent, name, rustix::fs::AtFlags::REMOVEDIR)?;
                    Ok(())
                },
            )
        },
        // The firewall remains closed until both the supervisor and every process
        // with its dedicated identity are absent and all own cgroups are released.
        &|| reconcile_firewall(plan.slot, cleanup, plan.deadline),
    )
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn run_stopped_owner_boundary(
    require_vacant: &impl Fn() -> Result<()>,
    reconcile_workers: &impl Fn() -> Result<()>,
    reconcile_firewall: &impl Fn() -> Result<()>,
) -> Result<()> {
    require_vacant()?;
    reconcile_workers()?;
    require_vacant()?;
    reconcile_firewall()
}

#[cfg(not(any(target_os = "linux", all(test, unix))))]
pub(super) fn reconcile(admitted: &HostAdmission, _cleanup: bool) -> Result<()> {
    if matches!(admitted.target, HostTarget::Edge(_)) {
        Ok(())
    } else {
        Err(eyre!("stopped Inrou owner cleanup requires Linux"))
    }
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn acquire_slot_lock(path: &Path, owner: u32, create: bool) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(create)
        .mode(0o600)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC).bits() as i32)
        .open(path)?;
    let opened = file.metadata()?;
    let named = fs::symlink_metadata(path)?;
    if !opened.is_file()
        || opened.nlink() != 1
        || opened.uid() != owner
        || (owner == 0 && opened.gid() != 0)
        || opened.mode() & 0o7777 != 0o600
        || named.file_type().is_symlink()
        || (opened.dev(), opened.ino()) != (named.dev(), named.ino())
    {
        return Err(eyre!(
            "stopped Inrou owner lock has unsafe or changed custody"
        ));
    }
    rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
        .wrap_err("Inrou owner is still held by a supervisor")?;
    Ok(file)
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn bounded_text_at(parent: &File, name: &OsStr) -> Result<String> {
    let file = File::from(rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    if !file.metadata()?.is_file() {
        return Err(eyre!("stopped Inrou evidence is not a regular kernel file"));
    }
    let mut text = String::new();
    file.take(65_537).read_to_string(&mut text)?;
    if text.len() > 65_536 {
        return Err(eyre!("stopped Inrou evidence exceeds its bound"));
    }
    Ok(text)
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn require_identity_absent(
    root: &Path,
    identity: u32,
    check: &impl Fn() -> Result<()>,
) -> Result<()> {
    let mut count = 0;
    for entry in fs::read_dir(root)? {
        check()?;
        let entry = entry?;
        if entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
            .is_none()
        {
            continue;
        }
        count += 1;
        if count > 65_536 {
            return Err(eyre!("stopped Inrou process census exceeds its bound"));
        }
        let status = (|| -> Result<String> {
            let process = open_directory(&entry.path())?;
            bounded_text_at(&process, OsStr::new("status"))
        })();
        match status {
            Ok(status) => require_status_identity_absent(&status, identity)?,
            Err(error)
                if error.downcast_ref::<rustix::io::Errno>() == Some(&rustix::io::Errno::NOENT) => {
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn require_status_identity_absent(status: &str, identity: u32) -> Result<()> {
    for field in ["Uid:", "Gid:", "Groups:"] {
        let lines = status
            .lines()
            .filter_map(|line| line.strip_prefix(field))
            .collect::<Vec<_>>();
        if lines.len() != 1 {
            return Err(eyre!(
                "stopped Inrou process identity evidence is incomplete or duplicated"
            ));
        }
        let values = lines[0]
            .split_ascii_whitespace()
            .map(str::parse::<u32>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if (field != "Groups:" && values.len() != 4) || values.contains(&identity) {
            return Err(eyre!(
                "stopped Inrou owner still has a process or malformed identity evidence"
            ));
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn open_directory(path: &Path) -> Result<File> {
    Ok(File::from(rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?))
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn revalidate_directory(path: &Path, file: &File) -> Result<()> {
    let named = fs::symlink_metadata(path)?;
    let opened = file.metadata()?;
    if !named.is_dir()
        || named.file_type().is_symlink()
        || (named.dev(), named.ino()) != (opened.dev(), opened.ino())
    {
        return Err(eyre!("stopped Inrou cgroup changed identity"));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn worker_slot(name: &str) -> Result<usize> {
    let (slot, hash) = name
        .strip_prefix("worker-")
        .and_then(|rest| rest.split_once('-'))
        .ok_or_else(|| eyre!("stopped Inrou cgroup has a noncanonical worker name"))?;
    if !matches!(slot, "0" | "1" | "2" | "3")
        || hash.len() != 64
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(eyre!(
            "stopped Inrou cgroup has a noncanonical worker owner"
        ));
    }
    Ok(slot.parse()?)
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn require_empty_cgroup(file: &File) -> Result<()> {
    let events = bounded_text_at(file, OsStr::new("cgroup.events"))?;
    if events
        .lines()
        .filter(|line| line.starts_with("populated "))
        .collect::<Vec<_>>()
        != ["populated 0"]
        || !bounded_text_at(file, OsStr::new("cgroup.procs"))?
            .trim()
            .is_empty()
    {
        return Err(eyre!("stopped Inrou worker is still populated"));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", all(test, unix)))]
struct StoppedCgroupPlan {
    root: PathBuf,
    slot: usize,
    parent: Option<File>,
    workers: Vec<(OsString, File)>,
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn preflight_cgroups(
    root: &Path,
    slot: usize,
    cleanup: bool,
    custody: &impl Fn(&Path) -> Result<()>,
    check: &impl Fn() -> Result<()>,
) -> Result<StoppedCgroupPlan> {
    match fs::symlink_metadata(root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = root.parent() {
                custody(parent)?;
            }
            return Ok(StoppedCgroupPlan {
                root: root.to_path_buf(),
                slot,
                parent: None,
                workers: Vec::new(),
            });
        }
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }
    custody(root)?;
    let parent = open_directory(root)?;
    revalidate_directory(root, &parent)?;
    if !bounded_text_at(&parent, OsStr::new("cgroup.procs"))?
        .trim()
        .is_empty()
    {
        return Err(eyre!("stopped Inrou hierarchy retains an unscoped process"));
    }
    let mut workers = Vec::new();
    for (index, entry) in fs::read_dir(root)?.enumerate() {
        check()?;
        if index >= 1_024 {
            return Err(eyre!("stopped Inrou cgroup scan exceeds its bound"));
        }
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() || (!kind.is_dir() && !kind.is_file()) {
            return Err(eyre!("stopped Inrou cgroup root contains an unsafe entry"));
        }
        if !kind.is_dir() {
            continue;
        }
        custody(&entry.path())?;
        let name = entry.file_name();
        let name_text = name
            .to_str()
            .ok_or_else(|| eyre!("stopped Inrou worker name is not UTF-8"))?;
        if worker_slot(name_text)? != slot {
            continue;
        }
        if !cleanup {
            return Err(eyre!("stopped Inrou owner still retains a worker cgroup"));
        }
        let file = open_directory(&entry.path())?;
        revalidate_directory(&entry.path(), &file)?;
        require_empty_cgroup(&file)?;
        // No recursive removal. A descendant group remains an ownership conflict.
        for child in fs::read_dir(entry.path())? {
            if !child?.file_type()?.is_file() {
                return Err(eyre!(
                    "stopped Inrou worker contains a nested or unsafe entry"
                ));
            }
        }
        workers.push((name, file));
    }
    revalidate_directory(root, &parent)?;
    Ok(StoppedCgroupPlan {
        root: root.to_path_buf(),
        slot,
        parent: Some(parent),
        workers,
    })
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn apply_cgroup_plan(
    plan: &StoppedCgroupPlan,
    cleanup: bool,
    custody: &impl Fn(&Path) -> Result<()>,
    check: &impl Fn() -> Result<()>,
    remove: &impl Fn(&File, &OsStr) -> Result<()>,
) -> Result<()> {
    let root = &plan.root;
    let Some(parent) = &plan.parent else {
        preflight_cgroups(root, plan.slot, false, custody, check)?;
        return Ok(());
    };
    // Admit the complete bounded census before the first mutation.
    for (name, file) in &plan.workers {
        check()?;
        custody(root)?;
        revalidate_directory(root, parent)?;
        let path = root.join(name);
        custody(&path)?;
        revalidate_directory(&path, file)?;
        require_empty_cgroup(file)?;
        remove(parent, name)?;
    }
    revalidate_directory(root, parent)?;
    if cleanup {
        preflight_cgroups(root, plan.slot, false, custody, check)?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
fn reconcile_cgroups(
    root: &Path,
    slot: usize,
    cleanup: bool,
    custody: &impl Fn(&Path) -> Result<()>,
    check: &impl Fn() -> Result<()>,
    remove: &impl Fn(&File, &OsStr) -> Result<()>,
) -> Result<()> {
    let plan = preflight_cgroups(root, slot, cleanup, custody, check)?;
    apply_cgroup_plan(&plan, cleanup, custody, check, remove)
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn stopped_firewall_program() -> Result<&'static str> {
    [
        "/usr/sbin/iptables",
        "/sbin/iptables",
        "/usr/bin/iptables",
        "/bin/iptables",
    ]
    .into_iter()
    .find(|path| validate_firewall_program(Path::new(path)).is_ok())
    .ok_or_else(|| eyre!("stopped Inrou cleanup requires a fixed root-custodied iptables entry"))
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn reconcile_firewall(slot: usize, cleanup: bool, deadline: Instant) -> Result<()> {
    let program = stopped_firewall_program()?;
    reconcile_firewall_with(slot, cleanup, &mut |arguments| {
        validate_firewall_program(Path::new(program))?;
        let arguments = arguments.iter().map(String::as_str).collect::<Vec<_>>();
        run_host_command(program, &arguments, deadline)
    })
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn validate_firewall_program(entry: &Path) -> Result<()> {
    let mut path = entry.to_path_buf();
    for _ in 0..40 {
        require_root_no_symlink_ancestors(&path, "stopped Inrou iptables entry")?;
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.uid() != 0 || metadata.gid() != 0 || metadata.nlink() != 1 {
            return Err(eyre!("stopped Inrou iptables entry has unsafe custody"));
        }
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path)?;
            path = if target.is_absolute() {
                target
            } else {
                path.parent()
                    .ok_or_else(|| eyre!("iptables entry has no parent"))?
                    .join(target)
            };
            continue;
        }
        if !metadata.is_file() || metadata.mode() & 0o111 == 0 || metadata.mode() & 0o7022 != 0 {
            return Err(eyre!(
                "stopped Inrou iptables entry is not a trusted executable"
            ));
        }
        // Keep the admitted entry name: the xtables multi-call binary chooses
        // iptables behavior from argv[0], not from its canonical target path.
        return Ok(());
    }
    Err(eyre!(
        "stopped Inrou iptables entry exceeds the symlink bound"
    ))
}

#[cfg(any(target_os = "linux", all(test, unix)))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct StoppedFirewallState {
    present: bool,
    jump: bool,
    port: Option<u16>,
    marker: bool,
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn firewall_tokens(line: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut started = false;
    for character in line.chars() {
        if character.is_control() && character != '\t' {
            return Err(eyre!("iptables snapshot contains control characters"));
        }
        if escaped {
            token.push(character);
            escaped = false;
            started = true;
        } else if character == '\\' {
            escaped = true;
            started = true;
        } else if character == '"' {
            quoted = !quoted;
            started = true;
        } else if character.is_ascii_whitespace() && !quoted {
            if started {
                tokens.push(std::mem::take(&mut token));
                started = false;
            }
        } else {
            token.push(character);
            started = true;
        }
    }
    if quoted || escaped {
        return Err(eyre!("iptables snapshot contains unterminated quoting"));
    }
    if started {
        tokens.push(token);
    }
    Ok(tokens)
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn firewall_marker_rule(chain: &str, slot: usize) -> Vec<String> {
    [
        "-A",
        chain,
        "-m",
        "comment",
        "--comment",
        &format!("iroha-inrou-owned-v1-slot-{slot}"),
        "-j",
        "RETURN",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn firewall_reject_rule(chain: &str, port: u16) -> Vec<String> {
    [
        "-A",
        chain,
        "-d",
        "127.0.0.1/32",
        "-o",
        "lo",
        "-p",
        "tcp",
        "-m",
        "tcp",
        "--dport",
        &port.to_string(),
        "-m",
        "owner",
        "!",
        "--uid-owner",
        "0",
        "-j",
        "REJECT",
        "--reject-with",
        "tcp-reset",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn parse_stopped_firewall(slot: usize, bytes: &[u8]) -> Result<StoppedFirewallState> {
    if slot >= 4 || bytes.len() > MAX_PROCESS_OUTPUT {
        return Err(eyre!(
            "stopped Inrou firewall owner or snapshot is outside its bound"
        ));
    }
    let chain = format!("IROHA_INROU_S{slot}_V1");
    let text = std::str::from_utf8(bytes)?;
    let mut state = StoppedFirewallState {
        present: false,
        jump: false,
        port: None,
        marker: false,
    };
    let mut policies = std::collections::BTreeSet::new();
    let mut rules = Vec::new();
    for line in text.lines() {
        let tokens = firewall_tokens(line)?;
        let words = tokens.iter().map(String::as_str).collect::<Vec<_>>();
        match words.as_slice() {
            [
                "-P",
                name @ ("INPUT" | "FORWARD" | "OUTPUT"),
                "ACCEPT" | "DROP",
            ] => {
                if !policies.insert((*name).to_owned()) {
                    return Err(eyre!("iptables snapshot repeats a filter policy"));
                }
            }
            ["-N", name] => {
                if *name == chain {
                    if state.present {
                        return Err(eyre!("iptables snapshot repeats the owned chain"));
                    }
                    state.present = true;
                }
            }
            ["-A", _, _, ..] => {
                let references = words.iter().enumerate().any(|(index, word)| {
                    if index > 0 && words[index - 1] == "--comment" {
                        return false;
                    }
                    (matches!(*word, "-j" | "--jump" | "-g" | "--goto")
                        && words.get(index + 1) == Some(&chain.as_str()))
                        || *word == format!("--jump={chain}")
                        || *word == format!("--goto={chain}")
                });
                if words[1] == chain {
                    rules.push(tokens);
                } else if references {
                    if words != ["-A", "OUTPUT", "-j", chain.as_str()] || state.jump {
                        return Err(eyre!(
                            "owned Inrou firewall has a foreign or repeated reference"
                        ));
                    }
                    state.jump = true;
                }
            }
            _ => {
                return Err(eyre!(
                    "iptables snapshot is not a complete canonical filter listing"
                ));
            }
        }
    }
    if policies.len() != 3 {
        return Err(eyre!("iptables snapshot omitted filter policies"));
    }
    if let Some(last) = rules.last() {
        if *last != firewall_marker_rule(&chain, slot) {
            return Err(eyre!(
                "owned Inrou firewall lacks its exact final ownership marker"
            ));
        }
        state.marker = true;
        rules.pop();
    }
    if let Some(rule) = rules.first() {
        let port = rule
            .get(11)
            .and_then(|port| port.parse::<u16>().ok())
            .filter(|port| *port != 0)
            .ok_or_else(|| eyre!("owned Inrou firewall has an invalid listener port"))?;
        if rules.len() != 1 || *rule != firewall_reject_rule(&chain, port) {
            return Err(eyre!(
                "owned Inrou firewall has a foreign or changed listener rule"
            ));
        }
        state.port = Some(port);
    }
    if (!state.present && (state.jump || state.marker || state.port.is_some()))
        || (state.jump && (state.port.is_none() || !state.marker))
    {
        return Err(eyre!(
            "owned Inrou firewall is not a valid construction or cleanup cut"
        ));
    }
    Ok(state)
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn reconcile_firewall_with(
    slot: usize,
    cleanup: bool,
    run: &mut impl FnMut(&[String]) -> Result<Vec<u8>>,
) -> Result<()> {
    if slot >= 4 {
        return Err(eyre!("stopped Inrou firewall slot is invalid"));
    }
    let list = ["-w", "5", "-S"].map(str::to_owned);
    let mut state = parse_stopped_firewall(slot, &run(&list)?)?;
    if !cleanup && state.present {
        return Err(eyre!("stopped Inrou owner retains its firewall chain"));
    }
    let chain = format!("IROHA_INROU_S{slot}_V1");
    for _ in 0..4 {
        if !state.present {
            return Ok(());
        }
        let mut expected = state.clone();
        let mut operation = if state.jump {
            expected.jump = false;
            vec![
                "-D".to_owned(),
                "OUTPUT".to_owned(),
                "-j".to_owned(),
                chain.clone(),
            ]
        } else if let Some(port) = state.port {
            expected.port = None;
            firewall_reject_rule(&chain, port)
        } else if state.marker {
            expected.marker = false;
            firewall_marker_rule(&chain, slot)
        } else {
            // This exact reserved, empty and unreferenced chain can be left by
            // a crash immediately after -N or immediately before -X. Removing
            // it changes no traffic policy; no unrelated chain is eligible.
            expected.present = false;
            vec!["-X".to_owned(), chain.clone()]
        };
        if operation[0] == "-A" {
            operation[0] = "-D".to_owned();
        }
        let mut arguments = vec!["-w".to_owned(), "5".to_owned()];
        arguments.extend(operation);
        run(&arguments)?;
        let observed = parse_stopped_firewall(slot, &run(&list)?)?;
        if observed != expected {
            return Err(eyre!("owned Inrou firewall changed during exact cleanup"));
        }
        state = observed;
    }
    if state.present {
        return Err(eyre!(
            "owned Inrou firewall cleanup exceeded its exact operation bound"
        ));
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    fn custody(path: &Path) -> Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(eyre!("fixture directory custody changed"));
        }
        Ok(())
    }

    fn worker(root: &Path, slot: usize, tag: char) -> PathBuf {
        if !root.join("cgroup.procs").exists() {
            fs::write(root.join("cgroup.procs"), "").unwrap();
        }
        let path = root.join(format!("worker-{slot}-{}", tag.to_string().repeat(64)));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("cgroup.events"), "populated 0\nfrozen 0\n").unwrap();
        fs::write(path.join("cgroup.procs"), "").unwrap();
        path
    }

    fn remove_fixture(root: &Path, name: &OsStr) -> Result<()> {
        let path = root.join(name);
        fs::remove_file(path.join("cgroup.events"))?;
        fs::remove_file(path.join("cgroup.procs"))?;
        fs::remove_dir(path)?;
        Ok(())
    }

    #[test]
    fn stopped_owner_cohort_preflight_preserves_workers_until_every_slot_is_admitted() -> Result<()>
    {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        let first = worker(root, 0, 'a');
        let last = worker(root, 3, 'b');
        let admitted = preflight_cgroups(root, 0, true, &custody, &|| Ok(()))?;
        assert!(first.exists());
        fs::write(last.join("cgroup.procs"), "123\n")?;
        assert!(preflight_cgroups(root, 3, true, &custody, &|| Ok(())).is_err());
        assert!(first.exists());
        // A worker replaced between cohort admission and apply is never removed.
        fs::rename(&first, root.join("retained"))?;
        worker(root, 0, 'a');
        let removed = Cell::new(false);
        assert!(
            apply_cgroup_plan(&admitted, true, &custody, &|| Ok(()), &|_, _| {
                removed.set(true);
                Ok(())
            })
            .is_err()
        );
        assert!(!removed.get());
        Ok(())
    }

    #[test]
    fn stopped_owner_cleanup_releases_only_empty_own_workers_and_replays() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        let own = worker(root, 2, 'a');
        let foreign = worker(root, 3, 'b');
        fs::write(foreign.join("cgroup.events"), "populated 1\n")?;
        fs::write(foreign.join("cgroup.procs"), "123\n")?;
        let removed = Cell::new(0);
        let remove = |_: &File, name: &OsStr| {
            removed.set(removed.get() + 1);
            remove_fixture(root, name)
        };
        assert!(reconcile_cgroups(root, 2, false, &custody, &|| Ok(()), &remove).is_err());
        let events = RefCell::new(Vec::new());
        let lock_path = root.join("slot.lock");
        let uid = rustix::process::geteuid().as_raw();
        let lock = acquire_slot_lock(&lock_path, uid, true)?;
        run_stopped_owner_boundary(
            &|| {
                events.borrow_mut().push("vacant");
                Ok(())
            },
            &|| {
                events.borrow_mut().push("workers");
                reconcile_cgroups(root, 2, true, &custody, &|| Ok(()), &remove)
            },
            &|| {
                assert!(!own.exists());
                assert!(acquire_slot_lock(&lock_path, uid, true).is_err());
                events.borrow_mut().push("firewall");
                Ok(())
            },
        )?;
        assert_eq!(
            *events.borrow(),
            ["vacant", "workers", "vacant", "firewall"]
        );
        drop(lock);
        assert!(acquire_slot_lock(&lock_path, uid, true).is_ok());
        assert!(foreign.exists());
        assert_eq!(fs::read_to_string(foreign.join("cgroup.procs"))?, "123\n");
        reconcile_cgroups(root, 2, true, &custody, &|| Ok(()), &remove)?;
        assert_eq!(removed.get(), 1);
        Ok(())
    }

    #[test]
    fn stopped_owner_cleanup_rejects_live_nested_forged_and_replaced_workers() -> Result<()> {
        for kind in [
            "populated",
            "pid",
            "nested",
            "symlink",
            "name",
            "replaced",
            "deadline",
        ] {
            let directory = tempfile::tempdir()?;
            let root = directory.path();
            let own = worker(root, 2, 'a');
            match kind {
                "populated" => fs::write(own.join("cgroup.events"), "populated 1\n")?,
                "pid" => fs::write(own.join("cgroup.procs"), "123\n")?,
                "nested" => fs::create_dir(own.join("nested"))?,
                "symlink" => {
                    fs::remove_file(own.join("cgroup.procs"))?;
                    symlink("cgroup.events", own.join("cgroup.procs"))?;
                }
                "name" => {
                    fs::rename(&own, root.join("worker-02-invalid"))?;
                }
                _ => {}
            }
            let checks = Cell::new(0);
            let removed = Cell::new(false);
            let firewall = Cell::new(false);
            let result = run_stopped_owner_boundary(
                &|| Ok(()),
                &|| {
                    reconcile_cgroups(
                        root,
                        2,
                        true,
                        &custody,
                        &|| {
                            checks.set(checks.get() + 1);
                            if checks.get() == 3 {
                                if kind == "deadline" {
                                    return Err(eyre!("expired fixture deadline"));
                                }
                                if kind == "replaced" {
                                    fs::rename(&own, root.join("retained"))?;
                                    worker(root, 2, 'a');
                                }
                            }
                            Ok(())
                        },
                        &|_, _| {
                            removed.set(true);
                            Ok(())
                        },
                    )
                },
                &|| {
                    firewall.set(true);
                    Ok(())
                },
            );
            assert!(result.is_err(), "{kind}");
            assert!(!removed.get(), "{kind}");
            assert!(!firewall.get(), "{kind}");
        }
        Ok(())
    }

    #[test]
    fn stopped_owner_cleanup_keeps_barriers_when_process_absence_is_unproven() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        let process = root.join("123");
        fs::create_dir(&process)?;
        let status = process.join("status");
        for identity in [
            "Uid:\t0 70002 0 0\nGid:\t0 0 0 0\nGroups:\t\n",
            "Uid:\t0 0 0 0\nGid:\t0 0 70002 0\nGroups:\t\n",
            "Uid:\t0 0 0 0\nGid:\t0 0 0 0\nGroups:\t70002\n",
            "Uid:\t0 0 0 0\nGid:\t0 0 0 0\n",
        ] {
            fs::write(&status, identity)?;
            let touched = Cell::new(false);
            assert!(
                run_stopped_owner_boundary(
                    &|| require_identity_absent(root, 70002, &|| Ok(())),
                    &|| {
                        touched.set(true);
                        Ok(())
                    },
                    &|| {
                        touched.set(true);
                        Ok(())
                    },
                )
                .is_err()
            );
            assert!(!touched.get());
        }
        fs::write(&status, "Uid:\t0 0 0 0\nGid:\t0 0 0 0\nGroups:\t70003\n")?;
        require_identity_absent(root, 70002, &|| Ok(()))?;
        let checks = Cell::new(0);
        let firewall = Cell::new(false);
        assert!(
            run_stopped_owner_boundary(
                &|| {
                    checks.set(checks.get() + 1);
                    if checks.get() == 2 {
                        Err(eyre!("owner returned"))
                    } else {
                        Ok(())
                    }
                },
                &|| Ok(()),
                &|| {
                    firewall.set(true);
                    Ok(())
                },
            )
            .is_err()
        );
        assert!(!firewall.get());
        Ok(())
    }

    #[test]
    fn stopped_owner_cleanup_lock_rejects_replaced_or_shared_custody() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("slot.lock");
        let uid = rustix::process::geteuid().as_raw();
        drop(acquire_slot_lock(&path, uid, true)?);
        fs::hard_link(&path, directory.path().join("alias"))?;
        assert!(acquire_slot_lock(&path, uid, true).is_err());
        fs::remove_file(&path)?;
        symlink("alias", &path)?;
        assert!(acquire_slot_lock(&path, uid, true).is_err());
        Ok(())
    }

    #[test]
    fn stopped_owner_cleanup_authority_requires_exact_config_slot() -> Result<()> {
        use iroha_config::parameters::actual::SoracloudRuntimeInrou;
        use std::num::NonZeroU32;
        for (slot, slug) in super::super::super::VALIDATOR_SLUGS.iter().enumerate() {
            let identity = NonZeroU32::new(70000 + u32::try_from(slot)?);
            let mut config = SoracloudRuntimeInrou {
                enabled: true,
                portable_vm_uid: identity,
                portable_vm_gid: identity,
                ..Default::default()
            };
            validate_config_slot(slug, &config)?;
            assert!(prior_inrou_config_enabled(slug, &config)?);
            config.portable_vm_gid = NonZeroU32::new(70000 + (u32::try_from(slot)? + 1) % 4);
            assert!(validate_config_slot(slug, &config).is_err());
            assert!(prior_inrou_config_enabled(slug, &config).is_err());
            config.portable_vm_gid = identity;
            config.enabled = false;
            assert!(validate_config_slot(slug, &config).is_err());
            assert!(!prior_inrou_config_enabled(slug, &config)?);
        }
        assert!(owner_slot("taira-validator-0").is_err());
        Ok(())
    }

    fn firewall_snapshot(
        slot: usize,
        present: bool,
        jump: bool,
        port: Option<u16>,
        marker: bool,
    ) -> String {
        let chain = format!("IROHA_INROU_S{slot}_V1");
        let mut lines = vec![
            "-P INPUT ACCEPT".to_owned(),
            "-P FORWARD DROP".to_owned(),
            "-P OUTPUT ACCEPT".to_owned(),
            "-N FOREIGN".to_owned(),
            "-A FOREIGN -m comment --comment \"-j IROHA_INROU_S2_V1\" -j RETURN".to_owned(),
        ];
        if present {
            lines.push(format!("-N {chain}"));
        }
        if jump {
            lines.push(format!("-A OUTPUT -j {chain}"));
        }
        if let Some(port) = port {
            lines.push(firewall_reject_rule(&chain, port).join(" "));
        }
        if marker {
            lines.push(firewall_marker_rule(&chain, slot).join(" "));
        }
        lines.join("\n") + "\n"
    }

    #[test]
    fn stopped_firewall_accepts_only_exact_crash_cuts() -> Result<()> {
        for (present, jump, port, marker) in [
            (false, false, None, false),
            (true, false, None, false),
            (true, false, None, true),
            (true, false, Some(43291), true),
            (true, true, Some(43291), true),
        ] {
            let snapshot = firewall_snapshot(2, present, jump, port, marker);
            let state = parse_stopped_firewall(2, snapshot.as_bytes())?;
            assert_eq!(
                state,
                StoppedFirewallState {
                    present,
                    jump,
                    port,
                    marker
                }
            );
        }
        for (jump, port, marker) in [
            (false, Some(43291), false),
            (true, None, false),
            (true, None, true),
            (true, Some(43291), false),
        ] {
            assert!(
                parse_stopped_firewall(
                    2,
                    firewall_snapshot(2, true, jump, port, marker).as_bytes()
                )
                .is_err()
            );
        }
        let quoted = firewall_snapshot(2, true, true, Some(43291), true).replace(
            "--comment iroha-inrou-owned-v1-slot-2",
            "--comment \"iroha-inrou-owned-v1-slot-2\"",
        );
        parse_stopped_firewall(2, quoted.as_bytes())?;
        assert!(parse_stopped_firewall(4, quoted.as_bytes()).is_err());
        assert!(parse_stopped_firewall(2, b"").is_err());
        Ok(())
    }

    #[test]
    fn stopped_firewall_rejects_foreign_references_and_rule_drift() -> Result<()> {
        let full = firewall_snapshot(2, true, true, Some(43291), true);
        for suffix in [
            "-A FORWARD -j IROHA_INROU_S2_V1\n",
            "-A FOREIGN -g IROHA_INROU_S2_V1\n",
            "-A FOREIGN --goto IROHA_INROU_S2_V1\n",
            "-A FOREIGN --jump=IROHA_INROU_S2_V1\n",
            "-A OUTPUT -j IROHA_INROU_S2_V1\n",
            "-A IROHA_INROU_S2_V1 -j ACCEPT\n",
            "-N IROHA_INROU_S2_V1\n",
        ] {
            assert!(
                parse_stopped_firewall(2, (full.clone() + suffix).as_bytes()).is_err(),
                "{suffix}"
            );
        }
        for (before, after) in [
            ("127.0.0.1/32", "127.0.0.0/8"),
            ("-o lo", "-o eth0"),
            ("! --uid-owner 0", "--uid-owner 0"),
            ("--uid-owner 0", "--uid-owner 70002"),
            ("--dport 43291", "--dport 043291"),
            ("--dport 43291", "--dport 0"),
            ("--dport 43291", "--dport 43291:43292"),
            (
                "--reject-with tcp-reset",
                "--reject-with icmp-port-unreachable",
            ),
            ("iroha-inrou-owned-v1-slot-2", "iroha-inrou-owned-v1-slot-3"),
        ] {
            assert!(
                parse_stopped_firewall(2, full.replace(before, after).as_bytes()).is_err(),
                "{after}"
            );
        }
        let comment = full.clone()
            + "-A FOREIGN -m comment --comment \"--jump=IROHA_INROU_S2_V1\" -j RETURN\n";
        parse_stopped_firewall(2, comment.as_bytes())?;
        let mut lines = full.lines().map(str::to_owned).collect::<Vec<_>>();
        let length = lines.len();
        lines.swap(length - 1, length - 2);
        assert!(parse_stopped_firewall(2, lines.join("\n").as_bytes()).is_err());
        assert!(firewall_tokens("-A FOREIGN --comment \"unterminated").is_err());
        Ok(())
    }

    #[test]
    fn stopped_firewall_cleanup_is_exact_and_idempotent() -> Result<()> {
        for (jump, port, marker) in [
            (false, None, false),
            (false, None, true),
            (false, Some(43291), true),
            (true, Some(43291), true),
        ] {
            let mut snapshot = firewall_snapshot(2, true, jump, port, marker);
            let foreign = "-N IROHA_INROU_S3_V1\n-A IROHA_INROU_S3_V1 -j ACCEPT\n";
            snapshot.push_str(foreign);
            let mut mutations = Vec::new();
            let mut run = |args: &[String]| -> Result<Vec<u8>> {
                if args == ["-w", "5", "-S"] {
                    return Ok(snapshot.as_bytes().to_vec());
                }
                assert_eq!(&args[..2], ["-w", "5"]);
                assert!(matches!(args[2].as_str(), "-D" | "-X"));
                mutations.push(args.to_vec());
                let mut removed = args[2..].to_vec();
                removed[0] = if args[2] == "-D" { "-A" } else { "-N" }.to_owned();
                let line = removed.join(" ");
                assert_eq!(
                    snapshot
                        .lines()
                        .filter(|candidate| *candidate == line)
                        .count(),
                    1
                );
                snapshot = snapshot
                    .lines()
                    .filter(|candidate| *candidate != line)
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n";
                Ok(vec![])
            };
            reconcile_firewall_with(2, true, &mut run)?;
            reconcile_firewall_with(2, true, &mut run)?;
            drop(run);
            assert!(snapshot.contains(foreign));
            assert!(!parse_stopped_firewall(2, snapshot.as_bytes())?.present);
            assert_eq!(
                mutations.len(),
                1 + usize::from(jump) + usize::from(port.is_some()) + usize::from(marker)
            );
        }
        Ok(())
    }

    #[test]
    fn stopped_firewall_stops_after_command_failure_or_snapshot_drift() -> Result<()> {
        for failure in ["command", "snapshot"] {
            let initial = firewall_snapshot(2, true, true, Some(43291), true);
            let mut mutations = 0;
            let mut scans = 0;
            let result = reconcile_firewall_with(2, true, &mut |args| {
                if args == ["-w", "5", "-S"] {
                    scans += 1;
                    return Ok(if scans == 1 {
                        initial.as_bytes().to_vec()
                    } else {
                        firewall_snapshot(2, true, false, Some(12345), true).into_bytes()
                    });
                }
                mutations += 1;
                if failure == "command" {
                    Err(eyre!("fixture command failed"))
                } else {
                    Ok(vec![])
                }
            });
            assert!(result.is_err());
            assert_eq!(mutations, 1);
            assert_eq!(scans, if failure == "command" { 1 } else { 2 });
        }
        Ok(())
    }

    #[test]
    fn stopped_firewall_read_only_never_mutates() -> Result<()> {
        for present in [false, true] {
            let mut scans = 0;
            let result = reconcile_firewall_with(2, false, &mut |args| {
                assert_eq!(args, ["-w", "5", "-S"]);
                scans += 1;
                Ok(firewall_snapshot(2, present, false, None, false).into_bytes())
            });
            assert_eq!(result.is_ok(), !present);
            assert_eq!(scans, 1);
        }
        let mut called = false;
        assert!(
            reconcile_firewall_with(4, true, &mut |_| {
                called = true;
                Ok(vec![])
            })
            .is_err()
        );
        assert!(!called);
        let directory = tempfile::tempdir()?;
        assert!(validate_firewall_program(directory.path()).is_err());
        Ok(())
    }
}
