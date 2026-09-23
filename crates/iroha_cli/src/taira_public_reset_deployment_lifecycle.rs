//! Physical-host deployment exclusion and durable reset ownership across dispatcher calls.

use super::super::FileSnapshot;
use super::*;

pub(super) const STATE_ROOT: &str = "/var/lib/taira-deployment";
const LOCK_FILE: &str = ".deployment.lock";
const OWNER_FILE: &str = ".reset-owner.json";
const OWNER_SCHEMA: &str = "iroha.taira.deployment.reset-owner.v1";

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ResetOwnerV1 {
    schema: String,
    authorization_nonce: String,
    inventory_sha256: String,
}

fn owner(admitted: &HostAdmission) -> ResetOwnerV1 {
    ResetOwnerV1 {
        schema: OWNER_SCHEMA.into(),
        authorization_nonce: admitted.inventory.authorization_nonce.clone(),
        inventory_sha256: admitted.inventory_sha256.clone(),
    }
}

fn local_targets(admitted: &HostAdmission) -> Vec<HostTarget> {
    let identity = &admitted.target.endpoint().host_identity_sha256;
    admitted
        .inventory
        .validators
        .iter()
        .filter(|validator| &validator.endpoint.host_identity_sha256 == identity)
        .cloned()
        .map(HostTarget::Validator)
        .chain(
            (&admitted.inventory.edge.endpoint.host_identity_sha256 == identity)
                .then(|| HostTarget::Edge(admitted.inventory.edge.clone())),
        )
        .collect()
}

fn owns_validator(admitted: &HostAdmission) -> bool {
    local_targets(admitted)
        .iter()
        .any(|target| matches!(target, HostTarget::Validator(_)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Terminal {
    Sealed,
    Restored,
}
impl Terminal {
    const fn suffix(self) -> &'static str {
        match self {
            Self::Sealed => "sealed",
            Self::Restored => "restored",
        }
    }
    fn admits(self, action: HostAction) -> bool {
        match self {
            Self::Sealed => matches!(action, HostAction::Seal | HostAction::Cleanup),
            Self::Restored => action == HostAction::Rollback,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Retained {
    intent: bool,
    sealed: bool,
    restored: bool,
}
#[derive(Debug, PartialEq, Eq)]
enum OwnerAdmission {
    Create,
    Existing,
    Terminal(Terminal),
}

fn admit_owner(
    expected: &ResetOwnerV1,
    actual: Option<&ResetOwnerV1>,
    retained: Retained,
    action: HostAction,
    recovery_only: bool,
    execution_expired: bool,
) -> Result<OwnerAdmission> {
    if actual.is_some_and(|actual| actual != expected) {
        return Err(eyre!("deployment lifecycle is owned by another operation"));
    }
    if retained.sealed && retained.restored {
        return Err(eyre!(
            "deployment lifecycle has conflicting terminal evidence"
        ));
    }
    let terminal = retained
        .sealed
        .then_some(Terminal::Sealed)
        .or_else(|| retained.restored.then_some(Terminal::Restored));
    if let Some(terminal) = terminal {
        if !retained.intent || !terminal.admits(action) {
            return Err(eyre!("terminal deployment lifecycle rejects this action"));
        }
        return Ok(OwnerAdmission::Terminal(terminal));
    }
    let recovering = recovery_only || execution_expired || action == HostAction::Rollback;
    if actual.is_some() {
        if recovering && !retained.intent {
            return Err(eyre!("deployment recovery lacks its durable intent"));
        }
        return Ok(OwnerAdmission::Existing);
    }
    if retained.intent || recovering || matches!(action, HostAction::Seal | HostAction::Cleanup) {
        return Err(eyre!(
            "deployment recovery cannot recreate missing or lost ownership"
        ));
    }
    Ok(OwnerAdmission::Create)
}

fn persistent_name(admitted: &HostAdmission, suffix: &str) -> String {
    format!(
        "reset-{}-{suffix}.json",
        admitted.inventory.authorization_nonce
    )
}

struct PinnedLock {
    path: PathBuf,
    file: File,
    snapshot: FileSnapshot,
}
impl PinnedLock {
    fn revalidate(&self) -> Result<()> {
        ensure_pinned_unchanged(&self.path, "deployment lock", &self.file, &self.snapshot)
    }
}

/// The common kernel lock remains held for the complete dispatched host action.
pub(super) struct Guard {
    lock: PinnedLock,
    root: File,
    root_device: u64,
    root_inode: u64,
}

impl Guard {
    pub(super) fn revalidate(&self) -> Result<()> {
        require_root_directory(Path::new(STATE_ROOT), true, "deployment lifecycle root")?;
        let held = self.root.metadata()?;
        let named = fs::symlink_metadata(STATE_ROOT)?;
        if held.dev() != self.root_device
            || held.ino() != self.root_inode
            || named.dev() != self.root_device
            || named.ino() != self.root_inode
            || held.uid() != 0
            || held.gid() != 0
            || held.mode() & 0o7777 != 0o700
        {
            return Err(eyre!("deployment lifecycle root changed while held"));
        }
        self.lock.revalidate()
    }

    pub(super) fn revalidate_unowned(&self) -> Result<()> {
        self.revalidate()?;
        match fs::symlink_metadata(Path::new(STATE_ROOT).join(OWNER_FILE)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
            Ok(_) => Err(eyre!("a durable reset owns the deployment lifecycle")),
        }
    }

    fn read_owner(&self, name: &str) -> Result<Option<ResetOwnerV1>> {
        self.revalidate()?;
        let path = Path::new(STATE_ROOT).join(name);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        }
        let (file, snapshot) = open_pinned_regular(&path, "deployment lifecycle record")?;
        if snapshot.uid != 0
            || snapshot.mode & 0o7777 != 0o600
            || snapshot.nlink != 1
            || file.metadata()?.gid() != 0
        {
            return Err(eyre!("deployment lifecycle record has unsafe root custody"));
        }
        let bytes = read_pinned_bytes(&path, "deployment lifecycle record", file, &snapshot, 4096)?;
        let actual: ResetOwnerV1 = json::from_slice(&bytes)?;
        self.revalidate()?;
        Ok(Some(actual))
    }

    fn retained(&self, admitted: &HostAdmission, suffix: &str) -> Result<bool> {
        match self.read_owner(&persistent_name(admitted, suffix))? {
            Some(actual) if actual == owner(admitted) => Ok(true),
            Some(_) => Err(eyre!("retained deployment lifecycle evidence conflicts")),
            None => Ok(false),
        }
    }

    fn require_owner(&self, admitted: &HostAdmission) -> Result<()> {
        if self.read_owner(OWNER_FILE)?.as_ref() != Some(&owner(admitted)) {
            return Err(eyre!("deployment lifecycle owner is missing or foreign"));
        }
        Ok(())
    }

    fn publish(&self, name: &str, value: &ResetOwnerV1) -> Result<()> {
        self.revalidate()?;
        publish_root_private_noreplace(
            Path::new(STATE_ROOT),
            name,
            json::to_json(value)?.as_bytes(),
        )?;
        self.revalidate()
    }
}

fn validate_lock_snapshot(snapshot: &FileSnapshot, gid: u32) -> Result<()> {
    if snapshot.uid != 0
        || gid != 0
        || snapshot.mode & 0o7777 != 0o600
        || snapshot.nlink != 1
        || snapshot.len != 0
    {
        return Err(eyre!(
            "deployment lock requires an empty root0600 single-link file"
        ));
    }
    Ok(())
}

fn acquire_lock(deadline: Instant, create: bool) -> Result<Guard> {
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(eyre!("deployment lifecycle custody requires root"));
    }
    if create {
        ensure_root_private_directory(Path::new(STATE_ROOT))?;
    }
    require_root_directory(Path::new(STATE_ROOT), true, "deployment lifecycle root")?;
    let root = File::from(rustix::fs::open(
        STATE_ROOT,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    let root_metadata = root.metadata()?;
    let path = Path::new(STATE_ROOT).join(LOCK_FILE);
    let mut flags =
        rustix::fs::OFlags::RDWR | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW;
    if create {
        flags |= rustix::fs::OFlags::CREATE;
    }
    let file = File::from(rustix::fs::openat(
        &root,
        LOCK_FILE,
        flags,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )?);
    let snapshot = super::super::file_snapshot(&file.metadata()?)?;
    validate_lock_snapshot(&snapshot, file.metadata()?.gid())?;
    let guard = Guard {
        lock: PinnedLock {
            path,
            file,
            snapshot,
        },
        root,
        root_device: root_metadata.dev(),
        root_inode: root_metadata.ino(),
    };
    guard.revalidate()?;
    loop {
        if Instant::now() >= deadline {
            return Err(eyre!("deployment lock deadline elapsed"));
        }
        match guard.lock.file.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(25)),
            Err(error) => return Err(error).wrap_err("acquire deployment lock"),
        }
    }
    guard.revalidate()?;
    guard.lock.file.sync_all()?;
    guard.root.sync_all()?;
    Ok(guard)
}

/// Read-only dispatcher replacement shares the deployment lock and admits no reset owner.
pub(super) fn acquire_unowned_existing(deadline: Instant) -> Result<Guard> {
    let guard = acquire_lock(deadline, false)?;
    guard.revalidate_unowned()?;
    Ok(guard)
}

/// Admit exact signed reset ownership before any physical-host mutation.
pub(super) fn acquire(admitted: &HostAdmission, action: HostAction) -> Result<Option<Guard>> {
    if !owns_validator(admitted) {
        return Ok(None);
    }
    let guard = acquire_lock(admitted.action_deadline, true)?;
    let actual = guard.read_owner(OWNER_FILE)?;
    let retained = Retained {
        intent: guard.retained(admitted, "intent")?,
        sealed: guard.retained(admitted, "sealed")?,
        restored: guard.retained(admitted, "restored")?,
    };
    match admit_owner(
        &owner(admitted),
        actual.as_ref(),
        retained,
        action,
        admitted.request.recovery_only,
        admitted.execution_expired,
    )? {
        OwnerAdmission::Create => {
            let progress_path = host_coordination_path(admitted)?.join("progress.json");
            match fs::symlink_metadata(&progress_path) {
                Ok(_) => {
                    let (progress, _) = read_private_json::<HostProgressV1>(
                        &progress_path,
                        "initial deployment host progress",
                    )?;
                    require_initial_progress(admitted, &progress)?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            ensure_action_deadline(admitted)?;
            if now_unix_ms()? > admitted.authorization.claims.execution_expires_at_unix_ms {
                return Err(eyre!(
                    "new deployment ownership requires current signed forward authority"
                ));
            }
            guard.publish(OWNER_FILE, &owner(admitted))?;
            guard.require_owner(admitted)?;
            guard.publish(&persistent_name(admitted, "intent"), &owner(admitted))?;
        }
        OwnerAdmission::Existing => {
            guard.require_owner(admitted)?;
            guard.publish(&persistent_name(admitted, "intent"), &owner(admitted))?;
        }
        OwnerAdmission::Terminal(terminal) => {
            let (progress, _) = read_private_json::<HostProgressV1>(
                &host_coordination_path(admitted)?.join("progress.json"),
                "terminal deployment host progress",
            )?;
            if terminal_for_progress(admitted, action, &progress)? != Some(terminal) {
                return Err(eyre!(
                    "terminal deployment replay lacks completed local progress"
                ));
            }
            require_terminal_postconditions(admitted, &progress, terminal)?;
        }
    }
    guard.revalidate()?;
    Ok(Some(guard))
}

fn require_initial_progress(admitted: &HostAdmission, progress: &HostProgressV1) -> Result<()> {
    validate_host_progress(admitted, progress)?;
    if progress.next_forward_ordinal != 0
        || progress.prepared_action.is_some()
        || !progress.touched_hosts.is_empty()
        || progress.rolling_back
        || progress.sealed
    {
        return Err(eyre!(
            "existing deployment progress cannot recreate lost ownership"
        ));
    }
    Ok(())
}

fn terminal_for_progress(
    admitted: &HostAdmission,
    action: HostAction,
    progress: &HostProgressV1,
) -> Result<Option<Terminal>> {
    validate_host_progress(admitted, progress)?;
    let local = local_targets(admitted);
    if progress
        .touched_hosts
        .iter()
        .any(|slug| !local.iter().any(|target| target.slug() == slug))
    {
        return Err(eyre!(
            "deployment terminal evidence names a different physical host"
        ));
    }
    if progress.prepared_action.is_some() {
        return Ok(None);
    }
    if matches!(action, HostAction::Seal | HostAction::Cleanup) && progress.sealed {
        return Ok(Some(Terminal::Sealed));
    }
    if action == HostAction::Rollback
        && progress.rolling_back
        && required_rollback_targets(admitted, progress)? == progress.rolled_back_hosts
    {
        return Ok(Some(Terminal::Restored));
    }
    Ok(None)
}

fn admission_for_target(admitted: &HostAdmission, target: HostTarget) -> Result<HostAdmission> {
    let (guard, bytes) = read_private_json::<HostGuardV1>(
        &Path::new(target.reset_guard()).join("guard.json"),
        "local deployment target guard",
    )?;
    if sha256_hex(&bytes) != target.endpoint().upload_guard_sha256
        || guard.schema != HOST_GUARD_SCHEMA_V1
        || guard.host_slug != target.slug()
        || guard.service_root != target.service_root()
        || guard.state_root != target.state_root()
        || guard.trusted_key_sha256 != admitted.request.trusted_key_sha256
        || guard.dispatcher_path != FIXED_DISPATCHER
        || guard.dispatcher_sha256 != admitted.guard.dispatcher_sha256
        || guard.upload_parent != upload_parent(target.service_root())
    {
        return Err(eyre!(
            "local deployment target guard differs from the signed closure"
        ));
    }
    let mut selected = admitted.clone();
    selected.request.host_slug = target.slug().to_owned();
    selected.target = target;
    selected.guard = guard;
    Ok(selected)
}

fn require_terminal_postconditions(
    admitted: &HostAdmission,
    progress: &HostProgressV1,
    terminal: Terminal,
) -> Result<()> {
    for target in local_targets(admitted) {
        let selected = admission_for_target(admitted, target)?;
        match terminal {
            Terminal::Sealed => verify_success_seal_postcondition(&selected)?,
            Terminal::Restored => {
                if progress
                    .touched_hosts
                    .iter()
                    .any(|slug| slug == selected.target.slug())
                {
                    verify_rollback_postcondition(&selected)?;
                } else {
                    verify_conservative_rollback_absence(&selected)?;
                }
                if let HostTarget::Validator(validator) = &selected.target
                    && !validator.is_vacant()
                {
                    occupied::verify_prior_genesis_hash(&selected, validator)?;
                }
            }
        }
    }
    Ok(())
}

/// Release durable reset ownership only after every local target has exact terminal evidence.
pub(super) fn finish(
    admitted: &HostAdmission,
    action: HostAction,
    progress: &HostProgressV1,
    guard: &Guard,
) -> Result<()> {
    guard.revalidate()?;
    let Some(terminal) = terminal_for_progress(admitted, action, progress)? else {
        return Ok(());
    };
    require_terminal_postconditions(admitted, progress, terminal)?;
    let retained = guard.retained(admitted, terminal.suffix())?;
    let actual = guard.read_owner(OWNER_FILE)?;
    if actual.is_none() && retained {
        guard.root.sync_all()?;
        return Ok(());
    }
    guard.require_owner(admitted)?;
    guard.publish(
        &persistent_name(admitted, terminal.suffix()),
        &owner(admitted),
    )?;
    guard.require_owner(admitted)?;
    guard.revalidate()?;
    rustix::fs::unlinkat(&guard.root, OWNER_FILE, rustix::fs::AtFlags::empty())?;
    guard.root.sync_all()?;
    guard.revalidate()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deployment_owner_rejects_foreign_missing_and_expired_recovery() {
        let admitted = super::super::tests::progress_admission();
        let expected = owner(&admitted);
        assert_eq!(
            admit_owner(
                &expected,
                None,
                Retained::default(),
                HostAction::Stage,
                false,
                false
            )
            .unwrap(),
            OwnerAdmission::Create
        );
        let mut foreign = expected.clone();
        foreign.inventory_sha256 = "f".repeat(64);
        assert!(
            admit_owner(
                &expected,
                Some(&foreign),
                Retained::default(),
                HostAction::Stage,
                false,
                false
            )
            .is_err()
        );
        for (action, recovery, expired, intent) in [
            (HostAction::Stage, true, false, false),
            (HostAction::Stage, false, true, false),
            (HostAction::Rollback, false, false, false),
            (HostAction::Stage, false, false, true),
            (HostAction::Seal, false, false, false),
            (HostAction::Cleanup, false, false, false),
        ] {
            assert!(
                admit_owner(
                    &expected,
                    None,
                    Retained {
                        intent,
                        ..Retained::default()
                    },
                    action,
                    recovery,
                    expired
                )
                .is_err()
            );
        }
        assert!(
            admit_owner(
                &expected,
                Some(&expected),
                Retained::default(),
                HostAction::Rollback,
                false,
                false
            )
            .is_err()
        );
        assert_eq!(
            admit_owner(
                &expected,
                Some(&expected),
                Retained {
                    intent: true,
                    ..Retained::default()
                },
                HostAction::Rollback,
                false,
                true
            )
            .unwrap(),
            OwnerAdmission::Existing
        );
    }

    #[test]
    fn deployment_owner_cannot_restart_from_existing_host_progress() {
        let admitted = super::super::tests::progress_admission();
        let initial = initial_host_progress(&admitted);
        require_initial_progress(&admitted, &initial).unwrap();
        for changed in 0..4 {
            let mut progress = initial.clone();
            match changed {
                0 => progress.next_forward_ordinal = 1,
                1 => progress.prepared_action = Some(host_forward_plan(&admitted)[0].clone()),
                2 => progress
                    .touched_hosts
                    .push(admitted.target.slug().to_owned()),
                _ => progress.rolling_back = true,
            }
            assert!(require_initial_progress(&admitted, &progress).is_err());
        }
    }

    #[test]
    fn deployment_terminal_replay_requires_exact_action_and_retained_intent() {
        let admitted = super::super::tests::progress_admission();
        let expected = owner(&admitted);
        for (terminal, action, wrong) in [
            (Terminal::Sealed, HostAction::Seal, HostAction::Rollback),
            (Terminal::Restored, HostAction::Rollback, HostAction::Start),
        ] {
            let retained = Retained {
                intent: true,
                sealed: terminal == Terminal::Sealed,
                restored: terminal == Terminal::Restored,
            };
            assert_eq!(
                admit_owner(&expected, None, retained, action, false, true).unwrap(),
                OwnerAdmission::Terminal(terminal)
            );
            assert!(admit_owner(&expected, None, retained, wrong, false, true).is_err());
            assert!(
                admit_owner(
                    &expected,
                    None,
                    Retained {
                        intent: false,
                        ..retained
                    },
                    action,
                    false,
                    true
                )
                .is_err()
            );
            let mut foreign = expected.clone();
            foreign.authorization_nonce = "f".repeat(32);
            assert!(admit_owner(&expected, Some(&foreign), retained, action, false, true).is_err());
        }
        assert!(
            admit_owner(
                &expected,
                None,
                Retained {
                    intent: true,
                    sealed: true,
                    restored: true
                },
                HostAction::Seal,
                false,
                true
            )
            .is_err()
        );
        let bytes = json::to_vec(&expected).unwrap();
        assert_eq!(json::from_slice::<ResetOwnerV1>(&bytes).unwrap(), expected);
        let mut retired = json::to_value(&expected).unwrap();
        retired
            .as_object_mut()
            .unwrap()
            .insert("policy_sha256".into(), "f".repeat(64).into());
        assert!(json::from_value::<ResetOwnerV1>(retired).is_err());
    }

    #[test]
    fn deployment_owner_waits_for_all_local_seals_and_rollbacks() {
        let admitted = super::super::tests::progress_admission();
        assert!(owns_validator(&admitted));
        let plan = host_forward_plan(&admitted);
        let first_seal = plan
            .iter()
            .position(|key| key.action == HostAction::Seal.label())
            .unwrap();
        let first_cleanup = plan
            .iter()
            .position(|key| key.action == HostAction::Cleanup.label())
            .unwrap();
        let mut progress = initial_host_progress(&admitted);
        for next in first_seal..first_cleanup {
            progress.next_forward_ordinal = u16::try_from(next).unwrap();
            assert_eq!(
                terminal_for_progress(&admitted, HostAction::Seal, &progress).unwrap(),
                None
            );
        }
        progress.next_forward_ordinal = u16::try_from(first_cleanup).unwrap();
        progress.sealed = true;
        assert_eq!(
            terminal_for_progress(&admitted, HostAction::Seal, &progress).unwrap(),
            Some(Terminal::Sealed)
        );
        progress = initial_host_progress(&admitted);
        progress.rolling_back = true;
        progress.touched_hosts = local_targets(&admitted)
            .iter()
            .map(|target| target.slug().to_owned())
            .collect();
        let required = required_rollback_targets(&admitted, &progress).unwrap();
        for slug in &required {
            assert_eq!(
                terminal_for_progress(&admitted, HostAction::Rollback, &progress).unwrap(),
                None
            );
            progress.rolled_back_hosts.push(slug.clone());
            progress.last_rollback_rank = rollback_rank(&admitted.inventory, slug).unwrap();
        }
        assert_eq!(
            terminal_for_progress(&admitted, HostAction::Rollback, &progress).unwrap(),
            Some(Terminal::Restored)
        );
        let mut separate = admitted.clone();
        separate.inventory.edge.endpoint.host_identity_sha256 = "b".repeat(64);
        separate.target = HostTarget::Edge(separate.inventory.edge.clone());
        assert!(!owns_validator(&separate));
        assert_eq!(local_targets(&separate).len(), 1);
        separate.target = HostTarget::Validator(separate.inventory.validators[1].clone());
        assert!(owns_validator(&separate));
        assert_eq!(local_targets(&separate).len(), 4);
    }

    #[cfg(unix)]
    #[test]
    fn deployment_lock_excludes_other_descriptors_and_rejects_rebinding() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let path = root.join(LOCK_FILE);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        let snapshot = super::super::super::file_snapshot(&file.metadata().unwrap()).unwrap();
        let pinned = PinnedLock {
            path: path.clone(),
            file,
            snapshot,
        };
        pinned.file.try_lock().unwrap();
        let contender = File::open(&path).unwrap();
        assert!(matches!(
            contender.try_lock(),
            Err(std::fs::TryLockError::WouldBlock)
        ));
        pinned.revalidate().unwrap();
        fs::rename(&path, root.join("original.lock")).unwrap();
        File::create(&path).unwrap();
        assert!(pinned.revalidate().is_err());
        let mut expected = pinned.snapshot.clone();
        expected.uid = 0;
        validate_lock_snapshot(&expected, 0).unwrap();
        for changed in 0..5 {
            let mut invalid = expected.clone();
            let mut gid = 0;
            match changed {
                0 => invalid.uid = 1,
                1 => gid = 1,
                2 => invalid.mode = 0o644,
                3 => invalid.nlink = 2,
                _ => invalid.len = 1,
            }
            assert!(validate_lock_snapshot(&invalid, gid).is_err());
        }
    }
}
