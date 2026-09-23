//! Shared deployment exclusion and durable reset ownership across dispatcher calls.

use super::*;

const ROOT: &str = "/var/lib/taira";
const OWNER_FILE: &str = ".reset-owner.json";

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ResetOwnerV1 {
    schema: String,
    authorization_nonce: String,
    inventory_sha256: String,
    authorization_sha256: String,
}

fn owner(admitted: &HostAdmission) -> ResetOwnerV1 {
    ResetOwnerV1 {
        schema: "iroha.taira.deployment.reset-owner.v1".into(),
        authorization_nonce: admitted.inventory.authorization_nonce.clone(),
        inventory_sha256: admitted.inventory_sha256.clone(),
        authorization_sha256: admitted.authorization_sha256.clone(),
    }
}

fn evidence_name(admitted: &HostAdmission, suffix: &str) -> String {
    format!(
        ".reset-{}-{suffix}.json",
        admitted.inventory.authorization_nonce
    )
}

fn exact_evidence(admitted: &HostAdmission, name: &str) -> Result<bool> {
    let path = Path::new(ROOT).join(name);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
        Ok(_) => {
            let (actual, _) =
                read_private_json::<ResetOwnerV1>(&path, "deployment reset ownership")?;
            if actual != owner(admitted) {
                return Err(eyre!(
                    "deployment reset ownership belongs to another operation"
                ));
            }
            Ok(true)
        }
    }
}

fn publish_evidence(admitted: &HostAdmission, name: &str) -> Result<()> {
    publish_root_private_noreplace_with_directory_custody(
        Path::new(ROOT),
        name,
        json::to_json(&owner(admitted))?.as_bytes(),
        false,
    )
}

/// Hold the shared kernel lock for this dispatch and persist ownership between dispatches.
pub(super) fn lock(admitted: &HostAdmission) -> Result<File> {
    require_root_directory(Path::new(ROOT), false, "deployment coordination root")?;
    reject_retired_epoch_worker_paths(
        Path::new("/var/lib/taira-epoch-supervisor"),
        Path::new("/etc/systemd/system/iroha-taira-epoch-supervisor.service"),
    )?;
    let path = Path::new(ROOT).join(".deployment.lock");
    let file = File::from(rustix::fs::open(
        &path,
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )?);
    let held = file.metadata()?;
    #[cfg(unix)]
    if !held.is_file()
        || held.uid() != 0
        || held.gid() != 0
        || held.mode() & 0o7777 != 0o600
        || held.nlink() != 1
        || held.len() != 0
    {
        return Err(eyre!("deployment lock has unsafe root custody"));
    }
    loop {
        match file.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) => {
                let remaining = admitted
                    .action_deadline
                    .saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(eyre!("deployment lock is held by another operation"));
                }
                std::thread::sleep(PROCESS_POLL_INTERVAL.min(remaining));
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
        }
    }
    let named = fs::symlink_metadata(&path)?;
    #[cfg(unix)]
    if named.file_type().is_symlink() || named.dev() != held.dev() || named.ino() != held.ino() {
        return Err(eyre!("deployment lock changed during acquisition"));
    }
    Ok(file)
}

/// Bind the held deployment lock after the existing host lease has been admitted.
pub(super) fn admit_owner(admitted: &HostAdmission, action: HostAction) -> Result<()> {
    if !exact_evidence(admitted, OWNER_FILE)? {
        let sealed = exact_evidence(admitted, &evidence_name(admitted, "sealed"))?;
        let restored = exact_evidence(admitted, &evidence_name(admitted, "restored"))?;
        if terminal_replay_allowed(action, sealed, restored) {
            return Ok(());
        }
        if exact_evidence(admitted, &evidence_name(admitted, "intent"))?
            || action == HostAction::Rollback
            || admitted.execution_expired
            || admitted.request.recovery_only
        {
            return Err(eyre!(
                "deployment recovery cannot recreate missing reset ownership"
            ));
        }
        publish_evidence(admitted, OWNER_FILE)?;
    }
    publish_evidence(admitted, &evidence_name(admitted, "intent"))?;
    Ok(())
}

fn terminal_replay_allowed(action: HostAction, sealed: bool, restored: bool) -> bool {
    (sealed && matches!(action, HostAction::Seal | HostAction::Cleanup))
        || (restored && action == HostAction::Rollback)
}

/// Release only after the authenticated host frontier proves terminal success or rollback.
pub(super) fn finish_terminal(admitted: &HostAdmission) -> Result<()> {
    let progress = load_or_create_host_progress(admitted)?;
    let terminal = if progress.prepared_action.is_some() {
        None
    } else if progress.sealed {
        Some("sealed")
    } else if progress.rolling_back
        && progress.rolled_back_hosts == required_rollback_targets(admitted, &progress)?
    {
        Some("restored")
    } else {
        None
    };
    let Some(terminal) = terminal else {
        return Ok(());
    };
    let evidence = evidence_name(admitted, terminal);
    if !exact_evidence(admitted, OWNER_FILE)? {
        if exact_evidence(admitted, &evidence)? {
            return Ok(());
        }
        return Err(eyre!("terminal deployment reset ownership is missing"));
    }
    publish_evidence(admitted, &evidence)?;
    exact_evidence(admitted, OWNER_FILE)?;
    let parent = File::from(rustix::fs::open(
        Path::new(ROOT),
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    rustix::fs::unlinkat(&parent, OWNER_FILE, rustix::fs::AtFlags::empty())?;
    parent.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_evidence_permits_only_exact_terminal_replays() {
        for action in [
            HostAction::Upload,
            HostAction::Stop,
            HostAction::Start,
            HostAction::Restart,
            HostAction::MutationReserve,
            HostAction::Seal,
            HostAction::Cleanup,
            HostAction::Rollback,
        ] {
            assert!(!terminal_replay_allowed(action, false, false));
            assert_eq!(
                terminal_replay_allowed(action, true, false),
                matches!(action, HostAction::Seal | HostAction::Cleanup)
            );
            assert_eq!(
                terminal_replay_allowed(action, false, true),
                action == HostAction::Rollback
            );
        }
    }

    #[test]
    fn reset_owner_requires_exact_authorization_without_worker_policy() {
        let value = ResetOwnerV1 {
            schema: "iroha.taira.deployment.reset-owner.v1".into(),
            authorization_nonce: "1".repeat(32),
            inventory_sha256: "2".repeat(64),
            authorization_sha256: "3".repeat(64),
        };
        let encoded = json::to_vec(&value).unwrap();
        assert_eq!(json::from_slice::<ResetOwnerV1>(&encoded).unwrap(), value);
        let mut wire: json::Value = json::from_slice(&encoded).unwrap();
        wire.as_object_mut()
            .unwrap()
            .insert("policy_sha256".into(), json::Value::String("4".repeat(64)));
        assert!(json::from_value::<ResetOwnerV1>(wire).is_err());
        let mut foreign = value.clone();
        foreign.authorization_sha256 = "5".repeat(64);
        assert_ne!(foreign, value);
    }
}
