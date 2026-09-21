//! Six-file transaction with an absent dispatcher barrier and immutable recovery records.
use super::*;
use std::io::Seek as _;

#[path = "taira_public_reset_dispatcher_transition_storage_copy.rs"]
pub(super) mod copy;
use copy::copy_exact;

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Identity {
    device: u64,
    inode: u64,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Identities {
    plan_sha256: String,
    root: Identity,
    old_dispatcher: Identity,
    original_dispatcher: Identity,
    new_dispatcher: Identity,
    old_guards: Vec<Identity>,
    original_guards: Vec<Identity>,
    new_guards: Vec<Identity>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Restoration {
    plan_sha256: String,
    dispatcher: Identity,
    guards: Vec<Identity>,
}
fn identity(path: &Path) -> Result<Identity> {
    let meta = fs::symlink_metadata(path)?;
    need(
        !meta.file_type().is_symlink(),
        "identity cannot be a symlink",
    )?;
    Ok(Identity {
        device: meta.dev(),
        inode: meta.ino(),
    })
}
fn same(path: &Path, expected: &Identity) -> Result<()> {
    let actual = identity(path)?;
    need(
        actual.device == expected.device && actual.inode == expected.inode,
        "transition inode was replaced",
    )
}
fn census(root: &Path) -> Result<()> {
    let mut allowed: BTreeSet<String> = [
        "plan.json",
        "identities.json",
        "restoration.json",
        "old-dispatcher",
        "new-dispatcher",
        "removed-old-dispatcher",
        "removed-new-dispatcher",
        "rollback-dispatcher",
        "prepared.json",
        "barrier.json",
        "applied.json",
        "rollback-requested.json",
        "rolled-back.json",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for i in 0..5 {
        for p in [
            "old-guard",
            "new-guard",
            "removed-old-guard",
            "removed-new-guard",
            "rollback-guard",
        ] {
            allowed.insert(format!("{p}-{i}"));
        }
        for p in ["applied-guard", "restored-guard"] {
            allowed.insert(format!("{p}-{i}.json"));
        }
    }
    let partials: Vec<_> = allowed.iter().map(|s| format!(".{s}.partial")).collect();
    allowed.extend(partials);
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let info = entry.file_type()?;
        need(
            info.is_file() && allowed.contains(entry.file_name().to_str().unwrap_or("")),
            "unknown transition namespace entry",
        )?;
    }
    Ok(())
}
fn load_identities(root: &Path, bytes: &[u8]) -> Result<Identities> {
    let raw = bounded_record(&root.join("identities.json"))?;
    let ids: Identities = json::from_slice(&raw)?;
    need(
        ids.plan_sha256 == sha256_hex(bytes)
            && ids.old_guards.len() == 5
            && ids.original_guards.len() == 5
            && ids.new_guards.len() == 5,
        "transition identity intent differs",
    )?;
    same(root, &ids.root)?;
    Ok(ids)
}
fn bounded_record(path: &Path) -> Result<Vec<u8>> {
    let (file, snapshot) = open_pinned_regular(path, "transition identity intent")?;
    need(
        snapshot.len <= 1024 * 1024 && snapshot.mode & 0o7777 == 0o600,
        "transition intent custody differs",
    )?;
    read_pinned_bytes(
        path,
        "transition identity intent",
        file,
        &snapshot,
        1024 * 1024,
    )
}
fn load_restoration(root: &Path, bytes: &[u8]) -> Result<Option<Restoration>> {
    if !exists(&root.join("restoration.json"))? {
        return Ok(None);
    }
    let value: Restoration = json::from_slice(&bounded_record(&root.join("restoration.json"))?)?;
    need(
        value.plan_sha256 == sha256_hex(bytes) && value.guards.len() == 5,
        "restoration identity differs",
    )?;
    Ok(Some(value))
}
fn verify_custody(
    plan: &Plan,
    root: &Path,
    new: &[Vec<u8>],
    ids: &Identities,
    restore: Option<&Restoration>,
) -> Result<()> {
    same(root, &ids.root)?;
    census(root)?;
    same(&root.join("old-dispatcher"), &ids.old_dispatcher)?;
    for i in 0..5 {
        same(&root.join(format!("old-guard-{i}")), &ids.old_guards[i])?;
    }
    let live = Path::new(&plan.predecessor.dispatcher.path);
    if !exists(live)? {
        need(
            exists(&root.join("removed-old-dispatcher"))?
                || exists(&root.join("removed-new-dispatcher"))?,
            "unowned absent dispatcher",
        )?;
    }
    if exists(live)? {
        if checked(live, &plan.candidate.executable).is_ok() {
            same(live, &ids.new_dispatcher)?;
        } else {
            checked(live, &plan.predecessor.dispatcher)?;
            if same(live, &ids.original_dispatcher).is_err() {
                same(
                    live,
                    &restore
                        .ok_or_else(|| eyre!("foreign restored dispatcher"))?
                        .dispatcher,
                )?;
            }
        }
    }
    for i in 0..5 {
        let live = Path::new(&plan.predecessor.guards[i].path);
        if !exists(live)? {
            need(
                exists(&root.join(format!("removed-old-guard-{i}")))?
                    || exists(&root.join(format!("removed-new-guard-{i}")))?,
                "unowned absent guard",
            )?;
            continue;
        }
        if checked(live, &guard_pin(plan, new, i, true)).is_ok() {
            same(live, &ids.new_guards[i])?;
        } else {
            checked(live, &plan.predecessor.guards[i])?;
            if same(live, &ids.original_guards[i]).is_err() {
                same(
                    live,
                    &restore
                        .ok_or_else(|| eyre!("foreign restored guard"))?
                        .guards[i],
                )?;
            }
        }
    }
    for (name, pin, id) in [
        (
            "new-dispatcher",
            &plan.candidate.executable,
            &ids.new_dispatcher,
        ),
        (
            "removed-old-dispatcher",
            &plan.predecessor.dispatcher,
            &ids.original_dispatcher,
        ),
        (
            "removed-new-dispatcher",
            &plan.candidate.executable,
            &ids.new_dispatcher,
        ),
    ] {
        let path = root.join(name);
        if exists(&path)? {
            checked(&path, pin)?;
            same(&path, id)?;
        }
    }
    for i in 0..5 {
        for (prefix, id, successor) in [
            ("new-guard", &ids.new_guards[i], true),
            ("removed-old-guard", &ids.original_guards[i], false),
            ("removed-new-guard", &ids.new_guards[i], true),
        ] {
            let path = root.join(format!("{prefix}-{i}"));
            if exists(&path)? {
                checked(&path, &guard_pin(plan, new, i, successor))?;
                same(&path, id)?;
            }
        }
    }
    Ok(())
}

fn checked(path: &Path, pin: &Pin) -> Result<File> {
    let (mut file, snapshot) = open_pinned_regular(path, "transition payload")?;
    need(
        snapshot.len == pin.size && snapshot.mode & 0o7777 == pin.mode,
        "transition payload custody differs",
    )?;
    need(
        hash_reader(&mut file)? == pin.sha256,
        "transition payload differs",
    )?;
    ensure_pinned_unchanged(path, "transition payload", &file, &snapshot)?;
    file.rewind()?;
    Ok(file)
}
pub(super) fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}
fn pin_bytes(path: &Path, bytes: &[u8], mode: u32) -> Pin {
    Pin {
        path: path.to_string_lossy().into_owned(),
        sha256: sha256_hex(bytes),
        size: bytes.len() as u64,
        mode,
    }
}
fn direct_directory(path: &Path) -> Result<File> {
    #[cfg(not(test))]
    require_root_no_symlink_ancestors(&path.join("entry"), "transition directory")?;
    let before = fs::symlink_metadata(path)?;
    need(
        before.is_dir() && !before.file_type().is_symlink() && before.mode() & 0o022 == 0,
        "unsafe transition directory",
    )?;
    let file = File::from(rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    let after = file.metadata()?;
    need(
        before.dev() == after.dev() && before.ino() == after.ino(),
        "transition directory changed",
    )?;
    Ok(file)
}
fn check_directory(path: &Path, held: &File) -> Result<()> {
    let now = direct_directory(path)?.metadata()?;
    let before = held.metadata()?;
    need(
        now.dev() == before.dev() && now.ino() == before.ino(),
        "transition parent was replaced",
    )
}
fn move_exact(source: &Path, target: &Path, pin: &Pin) -> Result<()> {
    let source_parent = source
        .parent()
        .ok_or_else(|| eyre!("source parent missing"))?;
    let target_parent = target
        .parent()
        .ok_or_else(|| eyre!("destination parent missing"))?;
    let src = direct_directory(source_parent)?;
    let dst = direct_directory(target_parent)?;
    let held = checked(source, pin)?;
    let before = held.metadata()?;
    need(!exists(target)?, "transition destination already exists")?;
    check_directory(source_parent, &src)?;
    check_directory(target_parent, &dst)?;
    rustix::fs::renameat_with(
        &src,
        source
            .file_name()
            .ok_or_else(|| eyre!("source name missing"))?,
        &dst,
        target
            .file_name()
            .ok_or_else(|| eyre!("destination name missing"))?,
        rustix::fs::RenameFlags::NOREPLACE,
    )?;
    src.sync_all()?;
    dst.sync_all()?;
    check_directory(source_parent, &src)?;
    check_directory(target_parent, &dst)?;
    let after = checked(target, pin)?.metadata()?;
    need(
        before.dev() == after.dev() && before.ino() == after.ino(),
        "transition rename identity differs",
    )
}

fn bytes_exact(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    if exists(path)? {
        checked(path, &pin_bytes(path, bytes, mode))?;
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("record parent missing"))?;
    let held = direct_directory(parent)?;
    let temporary = path.with_file_name(format!(
        ".{}.partial",
        path.file_name()
            .ok_or_else(|| eyre!("record name missing"))?
            .to_string_lossy()
    ));
    if exists(&temporary)? {
        let (mut partial, snapshot) = open_pinned_regular(&temporary, "partial transition record")?;
        need(
            snapshot.len <= bytes.len() as u64 && snapshot.mode & 0o7777 == mode,
            "partial record custody differs",
        )?;
        let mut prefix = Vec::new();
        partial.read_to_end(&mut prefix)?;
        need(bytes.starts_with(&prefix), "partial record prefix differs")?;
        let mut output = OpenOptions::new()
            .append(true)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(&temporary)?;
        need(
            output.metadata()?.ino() == snapshot.ino && output.metadata()?.dev() == snapshot.dev,
            "partial record changed",
        )?;
        output.write_all(&bytes[prefix.len()..])?;
        output.sync_all()?;
    } else {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(&temporary)?;
        file.set_permissions(fs::Permissions::from_mode(mode))?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    check_directory(parent, &held)?;
    move_exact(&temporary, path, &pin_bytes(path, bytes, mode))
}
fn event(root: &Path, name: &str, plan_bytes: &[u8]) -> Result<()> {
    bytes_exact(
        &root.join(name),
        format!("{}\n", sha256_hex(plan_bytes)).as_bytes(),
        0o600,
    )
}
fn verify_event(root: &Path, name: &str, bytes: &[u8]) -> Result<bool> {
    let path = root.join(name);
    if !exists(&path)? {
        return Ok(false);
    }
    checked(
        &path,
        &pin_bytes(&path, format!("{}\n", sha256_hex(bytes)).as_bytes(), 0o600),
    )?;
    Ok(true)
}
fn guard_pin(plan: &Plan, new: &[Vec<u8>], index: usize, successor: bool) -> Pin {
    if successor {
        pin_bytes(
            Path::new(&plan.predecessor.guards[index].path),
            &new[index],
            0o600,
        )
    } else {
        plan.predecessor.guards[index].clone()
    }
}
fn validate_live(plan: &Plan, new: &[Vec<u8>], successor: bool) -> Result<()> {
    checked(
        Path::new(&plan.predecessor.dispatcher.path),
        if successor {
            &plan.candidate.executable
        } else {
            &plan.predecessor.dispatcher
        },
    )?;
    for index in 0..5 {
        checked(
            Path::new(&plan.predecessor.guards[index].path),
            &guard_pin(plan, new, index, successor),
        )?;
    }
    Ok(())
}
fn validate_backups(plan: &Plan, root: &Path) -> Result<()> {
    checked(&root.join("old-dispatcher"), &plan.predecessor.dispatcher)?;
    for (index, pin) in plan.predecessor.guards.iter().enumerate() {
        checked(&root.join(format!("old-guard-{index}")), pin)?;
    }
    Ok(())
}

pub(super) fn check(plan: &Plan, bytes: &[u8], root: &Path, new: &[Vec<u8>]) -> Result<()> {
    if !exists(root)? {
        need(
            !exists(&operation_stage(root)?)? && !exists(&ownership_path(root)?)?,
            "operation staging requires apply recovery before a completed check",
        )?;
        return validate_live(plan, new, false);
    }
    verify_ownership(root, bytes)?;
    direct_directory(root)?;
    census(root)?;
    checked(
        &root.join("plan.json"),
        &pin_bytes(&root.join("plan.json"), bytes, 0o600),
    )?;
    if !verify_event(root, "prepared.json", bytes)? {
        return validate_live(plan, new, false);
    }
    let ids = load_identities(root, bytes)?;
    let restore = load_restoration(root, bytes)?;
    verify_custody(plan, root, new, &ids, restore.as_ref())?;
    if verify_event(root, "rolled-back.json", bytes)? {
        validate_backups(plan, root)?;
        return validate_live(plan, new, false);
    }
    if verify_event(root, "applied.json", bytes)? && !exists(&root.join("rollback-requested.json"))?
    {
        validate_backups(plan, root)?;
        return validate_live(plan, new, true);
    }
    need(
        !exists(Path::new(&plan.predecessor.dispatcher.path))?,
        "partial transition still exposes a dispatcher",
    )?;
    validate_backups(plan, root)
}

fn barrier(plan: &Plan, root: &Path, rollback: bool) -> Result<()> {
    let live = Path::new(&plan.predecessor.dispatcher.path);
    if exists(live)? {
        let (name, pin) = if checked(live, &plan.predecessor.dispatcher).is_ok() {
            ("removed-old-dispatcher", &plan.predecessor.dispatcher)
        } else if rollback {
            ("removed-new-dispatcher", &plan.candidate.executable)
        } else {
            return Err(eyre!("foreign dispatcher at barrier"));
        };
        move_exact(live, &root.join(name), pin)?;
    }
    need(!exists(live)?, "dispatcher barrier was not established")?;
    for (name, pin) in [
        ("removed-old-dispatcher", &plan.predecessor.dispatcher),
        ("removed-new-dispatcher", &plan.candidate.executable),
    ] {
        let path = root.join(name);
        if exists(&path)? {
            let file = checked(&path, pin)?;
            let m = file.metadata()?;
            drop(file);
            no_references(m.dev(), m.ino())?;
        }
    }
    Ok(())
}

fn ownership_path(root: &Path) -> Result<PathBuf> {
    Ok(root.with_file_name(format!(
        ".{}.ownership.json",
        root.file_name()
            .ok_or_else(|| eyre!("operation name missing"))?
            .to_string_lossy()
    )))
}
fn operation_stage(root: &Path) -> Result<PathBuf> {
    Ok(root.with_file_name(format!(
        ".{}.staging",
        root.file_name()
            .ok_or_else(|| eyre!("operation name missing"))?
            .to_string_lossy()
    )))
}
fn ownership_intent(root: &Path, bytes: &[u8]) -> Result<Vec<u8>> {
    Ok(json::to_vec(&norito::json!({
        "schema": "iroha.taira.dispatcher-transition-ownership.v1",
        "operation_root": (root.to_string_lossy().as_ref()),
        "staging_root": (operation_stage(root)?.to_string_lossy().as_ref()),
        "plan_sha256": (sha256_hex(bytes)),
    }))?)
}
fn verify_ownership(root: &Path, bytes: &[u8]) -> Result<()> {
    let path = ownership_path(root)?;
    checked(
        &path,
        &pin_bytes(&path, &ownership_intent(root, bytes)?, 0o600),
    )?;
    need(
        !exists(&operation_stage(root)?)?,
        "staging namespace remained after operation publication",
    )?;
    need(
        !exists(&path.with_file_name(format!(
            ".{}.partial",
            path.file_name().unwrap().to_string_lossy()
        )))?,
        "ownership publication has a foreign trailing partial",
    )
}
/// The sibling ownership record precedes creation of the operation staging directory.
pub(super) fn initialize_operation(
    root: &Path,
    bytes: &[u8],
    mut checkpoint: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let parent = root
        .parent()
        .ok_or_else(|| eyre!("operation parent missing"))?;
    let held = direct_directory(parent)?;
    let stage = operation_stage(root)?;
    let owner = ownership_path(root)?;
    let intent = ownership_intent(root, bytes)?;
    if !exists(&owner)? {
        need(
            !exists(&stage)? && !exists(root)?,
            "operation namespace exists without ownership",
        )?;
    }
    bytes_exact(&owner, &intent, 0o600)?;
    checkpoint("ownership")?;
    if !exists(&stage)? {
        fs::create_dir(&stage)?;
        fs::set_permissions(&stage, fs::Permissions::from_mode(0o700))?;
        held.sync_all()?;
    }
    direct_directory(&stage)?;
    for entry in fs::read_dir(&stage)? {
        let entry = entry?;
        need(
            entry.file_type()?.is_file()
                && matches!(
                    entry.file_name().to_str(),
                    Some("plan.json" | ".plan.json.partial")
                ),
            "operation staging namespace differs",
        )?;
    }
    checkpoint("stage-created")?;
    bytes_exact(&stage.join("plan.json"), bytes, 0o600)?;
    checkpoint("plan-written")?;
    need(
        fs::read_dir(&stage)?.count() == 1,
        "unexpected completed staging entry",
    )?;
    check_directory(parent, &held)?;
    rustix::fs::renameat_with(
        &held,
        stage
            .file_name()
            .ok_or_else(|| eyre!("stage name missing"))?,
        &held,
        root.file_name()
            .ok_or_else(|| eyre!("operation name missing"))?,
        rustix::fs::RenameFlags::NOREPLACE,
    )?;
    held.sync_all()?;
    check_directory(parent, &held)
}

pub(super) fn transition(
    plan: &Plan,
    bytes: &[u8],
    root: &Path,
    new: &[Vec<u8>],
    action: Action,
    mut revalidate: impl FnMut() -> Result<()>,
) -> Result<()> {
    need(action != Action::Check, "check cannot mutate")?;
    if exists(root)? {
        verify_ownership(root, bytes)?;
        direct_directory(root)?;
        census(root)?;
        checked(
            &root.join("plan.json"),
            &pin_bytes(&root.join("plan.json"), bytes, 0o600),
        )?;
        if verify_event(root, "prepared.json", bytes)? {
            let ids = load_identities(root, bytes)?;
            let restore = load_restoration(root, bytes)?;
            verify_custody(plan, root, new, &ids, restore.as_ref())?;
        }
        if verify_event(root, "rolled-back.json", bytes)? {
            need(
                action == Action::Rollback,
                "rolled-back transition cannot be reapplied",
            )?;
            validate_backups(plan, root)?;
            return validate_live(plan, new, false);
        }
        if action == Action::Apply && verify_event(root, "applied.json", bytes)? {
            need(
                !exists(&root.join("rollback-requested.json"))?,
                "rollback already began",
            )?;
            validate_backups(plan, root)?;
            return validate_live(plan, new, true);
        }
    } else {
        need(
            action == Action::Apply,
            "rollback requires an existing admitted transition",
        )?;
        validate_live(plan, new, false)?;
        let parent = root
            .parent()
            .ok_or_else(|| eyre!("operation parent missing"))?;
        if !exists(parent)? {
            let ancestor = direct_directory(
                parent
                    .parent()
                    .ok_or_else(|| eyre!("operation ancestor missing"))?,
            )?;
            fs::create_dir(parent)?;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
            ancestor.sync_all()?;
        }
        initialize_operation(root, bytes, |_| Ok(()))?;
    }
    let root_hold = direct_directory(root)?;
    if action == Action::Apply {
        need(
            !exists(&root.join("rollback-requested.json"))?,
            "rollback already began",
        )?;
    }
    if !verify_event(root, "prepared.json", bytes)? {
        validate_live(plan, new, false)?;
        // Reserve the existing deployment headroom as well as both exact copies.
        #[cfg(not(test))]
        {
            let v = rustix::fs::statvfs(root)?;
            let available = u128::from(v.f_bavail) * u128::from(v.f_frsize);
            need(
                available
                    >= u128::from(plan.predecessor.dispatcher.size)
                        + u128::from(plan.candidate.executable.size)
                        + 2 * 1024 * 1024 * 1024,
                "insufficient dispatcher preparation capacity",
            )?;
        }
        copy_exact(
            Path::new(&plan.predecessor.dispatcher.path),
            &root.join("old-dispatcher"),
            &plan.predecessor.dispatcher,
        )?;
        copy_exact(
            Path::new(&plan.candidate.executable.path),
            &root.join("new-dispatcher"),
            &plan.candidate.executable,
        )?;
        for index in 0..5 {
            copy_exact(
                Path::new(&plan.predecessor.guards[index].path),
                &root.join(format!("old-guard-{index}")),
                &plan.predecessor.guards[index],
            )?;
            bytes_exact(&root.join(format!("new-guard-{index}")), &new[index], 0o600)?;
        }
        let ids = Identities {
            plan_sha256: sha256_hex(bytes),
            root: identity(root)?,
            old_dispatcher: identity(&root.join("old-dispatcher"))?,
            original_dispatcher: identity(Path::new(&plan.predecessor.dispatcher.path))?,
            new_dispatcher: identity(&root.join("new-dispatcher"))?,
            old_guards: (0..5)
                .map(|i| identity(&root.join(format!("old-guard-{i}"))))
                .collect::<Result<_>>()?,
            original_guards: plan
                .predecessor
                .guards
                .iter()
                .map(|p| identity(Path::new(&p.path)))
                .collect::<Result<_>>()?,
            new_guards: (0..5)
                .map(|i| identity(&root.join(format!("new-guard-{i}"))))
                .collect::<Result<_>>()?,
        };
        bytes_exact(&root.join("identities.json"), &json::to_vec(&ids)?, 0o600)?;
        revalidate()?;
        event(root, "prepared.json", bytes)?;
    }
    validate_backups(plan, root)?;
    let ids = load_identities(root, bytes)?;
    let mut restore = load_restoration(root, bytes)?;
    verify_custody(plan, root, new, &ids, restore.as_ref())?;
    revalidate()?;
    if action == Action::Apply && validate_live(plan, new, true).is_ok() {
        need(
            !exists(&root.join("new-dispatcher"))? && exists(&root.join("removed-old-dispatcher"))?,
            "unowned completed publication",
        )?;
        return event(root, "applied.json", bytes);
    }
    if action == Action::Rollback {
        if restore.is_none() {
            copy_exact(
                &root.join("old-dispatcher"),
                &root.join("rollback-dispatcher"),
                &plan.predecessor.dispatcher,
            )?;
            for i in 0..5 {
                copy_exact(
                    &root.join(format!("old-guard-{i}")),
                    &root.join(format!("rollback-guard-{i}")),
                    &plan.predecessor.guards[i],
                )?;
            }
            let value = Restoration {
                plan_sha256: sha256_hex(bytes),
                dispatcher: identity(&root.join("rollback-dispatcher"))?,
                guards: (0..5)
                    .map(|i| identity(&root.join(format!("rollback-guard-{i}"))))
                    .collect::<Result<_>>()?,
            };
            bytes_exact(
                &root.join("restoration.json"),
                &json::to_vec(&value)?,
                0o600,
            )?;
            restore = Some(value);
        }
        if validate_live(plan, new, false).is_ok() && !exists(&root.join("rollback-dispatcher"))? {
            return event(root, "rolled-back.json", bytes);
        }
        event(root, "rollback-requested.json", bytes)?;
    }
    barrier(plan, root, action == Action::Rollback)?;
    event(root, "barrier.json", bytes)?;
    for index in 0..5 {
        revalidate()?;
        check_directory(root, &root_hold)?;
        verify_custody(plan, root, new, &ids, restore.as_ref())?;
        let successor = action == Action::Apply;
        let target = Path::new(&plan.predecessor.guards[index].path);
        let desired = guard_pin(plan, new, index, successor);
        if exists(target)? {
            if checked(target, &desired).is_ok() {
                continue;
            }
            let prior = guard_pin(plan, new, index, !successor);
            checked(target, &prior)?;
            let removed = root.join(format!(
                "{}-guard-{index}",
                if successor {
                    "removed-old"
                } else {
                    "removed-new"
                }
            ));
            move_exact(target, &removed, &prior)?;
            revalidate()?;
            verify_custody(plan, root, new, &ids, restore.as_ref())?;
        }
        let staging = root.join(format!(
            "{}-guard-{index}",
            if successor { "new" } else { "rollback" }
        ));
        if !successor {
            same(
                &staging,
                &restore
                    .as_ref()
                    .ok_or_else(|| eyre!("restoration missing"))?
                    .guards[index],
            )?;
        } else {
            same(&staging, &ids.new_guards[index])?;
        }
        move_exact(&staging, target, &desired)?;
        event(
            root,
            &format!(
                "{}-guard-{index}.json",
                if successor { "applied" } else { "restored" }
            ),
            bytes,
        )?;
    }
    revalidate()?;
    check_directory(root, &root_hold)?;
    verify_custody(plan, root, new, &ids, restore.as_ref())?;
    barrier(plan, root, action == Action::Rollback)?;
    let (staged, pin) = if action == Action::Apply {
        (root.join("new-dispatcher"), &plan.candidate.executable)
    } else {
        let stage = root.join("rollback-dispatcher");
        same(
            &stage,
            &restore
                .as_ref()
                .ok_or_else(|| eyre!("restoration missing"))?
                .dispatcher,
        )?;
        (stage, &plan.predecessor.dispatcher)
    };
    move_exact(&staged, Path::new(&plan.predecessor.dispatcher.path), pin)?;
    validate_live(plan, new, action == Action::Apply)?;
    revalidate()?;
    event(
        root,
        if action == Action::Apply {
            "applied.json"
        } else {
            "rolled-back.json"
        },
        bytes,
    )
}

#[cfg(test)]
thread_local! { static TEST_PROC_ROOT: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) }; }
#[cfg(test)]
pub(super) fn test_process_root(path: PathBuf) {
    TEST_PROC_ROOT.with(|root| *root.borrow_mut() = Some(path));
}
#[cfg(target_os = "linux")]
fn no_references(device: u64, inode: u64) -> Result<()> {
    #[cfg(test)]
    let root = TEST_PROC_ROOT
        .with(|path| path.borrow().clone())
        .ok_or_else(|| eyre!("fixture process root missing"))?;
    #[cfg(not(test))]
    let root = PathBuf::from("/proc");
    no_references_at(&root, device, inode)
}
#[cfg(target_os = "linux")]
pub(super) fn no_references_at(proc_root: &Path, device: u64, inode: u64) -> Result<()> {
    let mut count = 0usize;
    for entry in fs::read_dir(proc_root)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let _ = pid;
        count += 1;
        need(count <= 65536, "process census exceeds bound")?;
        let process = entry.path();
        let matches = |path: &Path| -> Result<bool> {
            match fs::metadata(path) {
                Ok(m) => Ok(m.dev() == device && m.ino() == inode),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(e) => Err(e.into()),
            }
        };
        need(
            !matches(&process.join("exe"))?,
            "old dispatcher process remains",
        )?;
        match fs::read_dir(process.join("fd")) {
            Ok(fds) => {
                for (index, fd) in fds.enumerate() {
                    need(index < 65536, "process FD census exceeds bound")?;
                    need(!matches(&fd?.path())?, "dispatcher descriptor remains")?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        }
        let input = match File::open(process.join("maps")) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        let mut maps = String::new();
        input.take(16 * 1024 * 1024 + 1).read_to_string(&mut maps)?;
        need(
            maps.len() <= 16 * 1024 * 1024,
            "process map census exceeds bound",
        )?;
        for line in maps.lines() {
            let fields = line.split_whitespace().take(5).collect::<Vec<_>>();
            need(fields.len() == 5, "invalid process map")?;
            let Some((major, minor)) = fields[3].split_once(':') else {
                return Err(eyre!("invalid map device"));
            };
            let dev = rustix::fs::makedev(
                u32::from_str_radix(major, 16)?,
                u32::from_str_radix(minor, 16)?,
            ) as u64;
            need(
                !(dev == device && fields[4].parse::<u64>()? == inode),
                "dispatcher executable mapping remains",
            )?;
        }
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn no_references(_device: u64, _inode: u64) -> Result<()> {
    #[cfg(test)]
    {
        Ok(())
    }
    #[cfg(not(test))]
    {
        Err(eyre!("dispatcher census requires Linux"))
    }
}
