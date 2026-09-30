//! One-use Sumeragi bootstrap authority for an authorization-created reset state.
//!
//! Reset publishes the token before Start can prepare its durable manager intent.
//! Once that intent exists, neither recovery nor missing history can rearm a key.

use super::*;

pub(super) const TOKEN: &str = "sumeragi-first-boot";
pub(super) const TOKEN_STAGING: &str = ".sumeragi-first-boot.next";
pub(super) const ASSERT_FRESH_KEY: &str = "--sumeragi-assert-fresh-key";
const RUNTIME_ENTRIES: [&str; 3] = [
    "sumeragi-records",
    "sumeragi-installation.log",
    "storage-sumeragi-bodies",
];

fn start_intent_path(admitted: &HostAdmission) -> Result<PathBuf> {
    Ok(Path::new(admitted.target.reset_guard())
        .join("receipts")
        .join(&admitted.inventory.authorization_nonce)
        .join(manager_intent_name("start")?))
}

fn require_no_start_intent(path: &Path) -> Result<()> {
    require_path_absent(path, "first-boot manager intent")?;
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| eyre!("first-boot manager intent has no filename"))?;
    require_path_absent(
        &path.with_file_name(format!(".{name}.next")),
        "unpublished first-boot manager intent",
    )
}

fn require_no_runtime_history(state: &Path) -> Result<()> {
    for name in RUNTIME_ENTRIES {
        require_path_absent(&state.join(name), "first-boot native runtime history")?;
    }
    Ok(())
}

pub(super) fn arm_fresh_state(state: &Path, admitted: &HostAdmission) -> Result<()> {
    require_root_directory(state, true, "first-boot state root")?;
    verify_generated_marker(state, admitted, "fresh_state")?;
    // The intent is durable before systemd can start the launcher. Even if the
    // daemon dies before creating history, a later Reset must not mint a token.
    require_armable_state(state, &start_intent_path(admitted)?)?;
    publish_root_private_noreplace(state, TOKEN, &[])?;
    verify_token(state)
}

fn require_armable_state(state: &Path, start_intent: &Path) -> Result<()> {
    require_no_start_intent(start_intent)?;
    require_no_runtime_history(state)
}

pub(super) fn verify_token(state: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(state.join(TOKEN))?;
    validate_runtime_metadata(TOKEN, &metadata, 0)
}

fn validate_runtime_metadata(name: &str, metadata: &fs::Metadata, owner: u32) -> Result<()> {
    let mode = metadata.mode() & 0o7777;
    let valid_kind_and_mode = match name {
        TOKEN | TOKEN_STAGING => {
            metadata.is_file() && metadata.nlink() == 1 && metadata.len() == 0 && mode == 0o600
        }
        "sumeragi-installation.log" => {
            metadata.is_file() && metadata.nlink() == 1 && matches!(mode, 0o600 | 0o640 | 0o644)
        }
        "sumeragi-records" | "storage-sumeragi-bodies" => {
            metadata.is_dir() && matches!(mode, 0o700 | 0o750 | 0o755)
        }
        _ => false,
    };
    if metadata.uid() != owner || metadata.file_type().is_symlink() || !valid_kind_and_mode {
        return Err(eyre!("Sumeragi reset artifact `{name}` has unsafe custody"));
    }
    Ok(())
}

/// Remove only inspected native runtime artifacts from the exact reset closure.
pub(super) fn validate_runtime_entries(
    state: &Path,
    entries: &mut BTreeSet<OsString>,
) -> Result<()> {
    validate_runtime_entries_for_owner(state, entries, 0)
}

fn validate_runtime_entries_for_owner(
    state: &Path,
    entries: &mut BTreeSet<OsString>,
    owner: u32,
) -> Result<()> {
    if entries.contains(OsStr::new(TOKEN)) && entries.contains(OsStr::new(TOKEN_STAGING)) {
        return Err(eyre!("first-boot token has conflicting publication slots"));
    }
    if entries.contains(OsStr::new(TOKEN)) || entries.contains(OsStr::new(TOKEN_STAGING)) {
        require_no_runtime_history(state)?;
    }
    for name in [TOKEN, TOKEN_STAGING].into_iter().chain(RUNTIME_ENTRIES) {
        if entries.remove(OsStr::new(name)) {
            validate_runtime_metadata(name, &fs::symlink_metadata(state.join(name))?, owner)?;
        }
    }
    Ok(())
}

fn start_was_prepared(admitted: &HostAdmission, validator: &ValidatorV1) -> Result<bool> {
    let path = start_intent_path(admitted)?;
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // A partial intent cannot have submitted a process. Start may let
            // the existing publisher recover it, but must still require the
            // original token; Reset separately refuses to rearm over any slot.
            Ok(false)
        }
        Err(error) => Err(error).wrap_err("inspect first-boot manager intent"),
        Ok(_) => {
            let (intent, _) =
                read_private_json::<ManagerIntentV1>(&path, "first-boot manager intent")?;
            validate_manager_intent_for_session(
                admitted,
                "start",
                "start",
                &validator.systemd_unit,
                &intent,
            )?;
            Ok(true)
        }
    }
}

pub(super) fn verify_start(admitted: &HostAdmission, validator: &ValidatorV1) -> Result<()> {
    let state = Path::new(&validator.state_root);
    verify_populated_fresh_state_for_quarantine(state, admitted)?;
    require_path_absent(&state.join(TOKEN_STAGING), "unpublished first-boot token")?;
    if !start_was_prepared(admitted, validator)? {
        require_unit_stopped(&validator.systemd_unit, admitted.action_deadline)?;
        require_no_runtime_history(state)?;
        verify_token(state)?;
    }
    // A prepared retry may see a token, an executing launcher, or a consumed
    // token. It only resumes the existing manager operation; it never arms.
    Ok(())
}

pub(super) fn verify_consumed(admitted: &HostAdmission, validator: &ValidatorV1) -> Result<()> {
    let state = Path::new(&validator.state_root);
    require_root_directory(state, true, "first-boot state root")?;
    verify_generated_marker(state, admitted, "fresh_state")?;
    if !start_was_prepared(admitted, validator)? {
        return Err(eyre!(
            "validator first boot has no authorized manager intent"
        ));
    }
    require_consumed_token(state)
}

fn require_consumed_token(state: &Path) -> Result<()> {
    require_path_absent(&state.join(TOKEN), "consumed first-boot token")?;
    require_path_absent(
        &state.join(TOKEN_STAGING),
        "consumed first-boot token staging",
    )
}

pub(super) fn require_initial_process(admitted: &HostAdmission) -> Result<()> {
    let start = start_intent_path(admitted)?;
    require_initial_process_at(
        start
            .parent()
            .ok_or_else(|| eyre!("first-boot intent has no directory"))?,
    )
}

fn require_initial_process_at(receipts: &Path) -> Result<()> {
    // Any published or partial successor operation ends the initial process
    // phase. Beacon activation also stops and starts the unit before Restart.
    for label in ["beacon-stop", "beacon-start", "restart"] {
        require_no_start_intent(&receipts.join(manager_intent_name(label)?))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_boot_never_rearms_after_a_prepared_or_partial_start() {
        let directory = tempfile::tempdir().unwrap();
        let intent = directory.path().join("manager-start.intent.json");
        let state = directory.path().join("state");
        fs::create_dir(&state).unwrap();
        require_armable_state(&state, &intent).unwrap();
        fs::write(state.join(TOKEN), b"").unwrap();
        fs::remove_file(state.join(TOKEN)).unwrap();
        // Model a daemon that consumed the token and crashed before creating
        // any native history, or a later loss of all three history artifacts.
        require_no_runtime_history(&state).unwrap();
        for path in [
            intent.clone(),
            directory.path().join(".manager-start.intent.json.next"),
        ] {
            fs::write(&path, b"").unwrap();
            let _ = require_armable_state(&state, &intent)
                .expect_err("a consumed token cannot be recreated despite missing history");
            assert!(!state.join(TOKEN).exists());
            fs::remove_file(path).unwrap();
        }
        symlink(directory.path().join("absent"), &intent).unwrap();
        let _ = require_armable_state(&state, &intent)
            .expect_err("dangling intent must not be treated as absence");
    }

    #[test]
    fn first_boot_attestation_requires_both_token_slots_consumed() {
        let directory = tempfile::tempdir().unwrap();
        require_consumed_token(directory.path()).unwrap();
        for name in [TOKEN, TOKEN_STAGING] {
            let token = directory.path().join(name);
            fs::write(&token, b"").unwrap();
            assert!(require_consumed_token(directory.path()).is_err());
            fs::remove_file(&token).unwrap();
            symlink(directory.path().join("absent"), &token).unwrap();
            assert!(require_consumed_token(directory.path()).is_err());
            fs::remove_file(token).unwrap();
        }
        require_consumed_token(directory.path()).unwrap();
    }

    #[test]
    fn first_boot_assertion_ends_at_every_prepared_successor_process() {
        let directory = tempfile::tempdir().unwrap();
        require_initial_process_at(directory.path()).unwrap();
        for label in ["beacon-stop", "beacon-start", "restart"] {
            let name = manager_intent_name(label).unwrap();
            for name in [name.clone(), format!(".{name}.next")] {
                let path = directory.path().join(name);
                fs::write(&path, b"").unwrap();
                assert!(require_initial_process_at(directory.path()).is_err());
                fs::remove_file(path).unwrap();
            }
        }
        require_initial_process_at(directory.path()).unwrap();
    }

    #[test]
    fn first_boot_runtime_closure_allows_only_inspected_native_entries() {
        let directory = tempfile::tempdir().unwrap();
        let state = directory.path();
        let owner = state.metadata().unwrap().uid();
        for name in RUNTIME_ENTRIES {
            let path = state.join(name);
            if name == "sumeragi-installation.log" {
                fs::write(&path, b"runtime history").unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
            } else {
                fs::create_dir(&path).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
        }
        let entries = BTreeSet::from_iter(RUNTIME_ENTRIES.map(OsString::from));
        let mut remaining = entries.clone();
        remaining.insert(OsString::from("foreign"));
        validate_runtime_entries_for_owner(state, &mut remaining, owner).unwrap();
        assert_eq!(remaining, BTreeSet::from([OsString::from("foreign")]));
        for name in [TOKEN, TOKEN_STAGING] {
            let token = state.join(name);
            fs::write(&token, b"").unwrap();
            fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
            let mut token_and_history = entries.clone();
            token_and_history.insert(OsString::from(name));
            assert!(
                validate_runtime_entries_for_owner(state, &mut token_and_history, owner).is_err()
            );
            fs::remove_file(token).unwrap();
        }
        for name in RUNTIME_ENTRIES {
            let path = state.join(name);
            if path.is_dir() {
                fs::remove_dir(path).unwrap();
            } else {
                fs::remove_file(path).unwrap();
            }
        }
        for name in [TOKEN, TOKEN_STAGING] {
            let token = state.join(name);
            fs::write(&token, b"").unwrap();
            fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
            let mut token_only = BTreeSet::from([OsString::from(name)]);
            validate_runtime_entries_for_owner(state, &mut token_only, owner).unwrap();
            assert!(token_only.is_empty());
            fs::remove_file(token).unwrap();
        }
        let mut conflicting =
            BTreeSet::from([OsString::from(TOKEN), OsString::from(TOKEN_STAGING)]);
        assert!(validate_runtime_entries_for_owner(state, &mut conflicting, owner).is_err());
    }

    #[test]
    fn first_boot_history_absence_includes_body_store_and_dangling_paths() {
        let directory = tempfile::tempdir().unwrap();
        require_no_runtime_history(directory.path()).unwrap();
        for name in RUNTIME_ENTRIES {
            let path = directory.path().join(name);
            symlink(directory.path().join("absent"), &path).unwrap();
            let _ = require_no_runtime_history(directory.path())
                .expect_err("no native history may be replaced");
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn first_boot_runtime_artifacts_enforce_exact_types_modes_and_custody() {
        let directory = tempfile::tempdir().unwrap();
        let owner = directory.path().metadata().unwrap().uid();
        for name in [TOKEN, TOKEN_STAGING, "sumeragi-installation.log"] {
            let path = directory.path().join(name);
            fs::write(&path, b"").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            let metadata = fs::symlink_metadata(&path).unwrap();
            validate_runtime_metadata(name, &metadata, owner).unwrap();
            assert!(validate_runtime_metadata(name, &metadata, owner.wrapping_add(1)).is_err());
            assert!(validate_runtime_metadata("foreign", &metadata, owner).is_err());
            fs::hard_link(&path, directory.path().join("alias")).unwrap();
            assert!(validate_runtime_metadata(name, &path.metadata().unwrap(), owner).is_err());
            fs::remove_file(directory.path().join("alias")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
            assert!(validate_runtime_metadata(name, &path.metadata().unwrap(), owner).is_err());
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            fs::write(&path, b"not a token").unwrap();
            assert_eq!(
                validate_runtime_metadata(name, &path.metadata().unwrap(), owner).is_ok(),
                name == "sumeragi-installation.log"
            );
            fs::remove_file(&path).unwrap();
            fs::create_dir(&path).unwrap();
            assert!(validate_runtime_metadata(name, &path.metadata().unwrap(), owner).is_err());
        }
        for name in ["sumeragi-records", "storage-sumeragi-bodies"] {
            let path = directory.path().join(name);
            fs::create_dir(&path).unwrap();
            for mode in [0o700, 0o750, 0o755] {
                fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
                validate_runtime_metadata(name, &path.metadata().unwrap(), owner).unwrap();
            }
            fs::set_permissions(&path, fs::Permissions::from_mode(0o777)).unwrap();
            assert!(validate_runtime_metadata(name, &path.metadata().unwrap(), owner).is_err());
            fs::remove_dir(&path).unwrap();
            symlink(directory.path(), &path).unwrap();
            assert!(
                validate_runtime_metadata(name, &fs::symlink_metadata(path).unwrap(), owner)
                    .is_err()
            );
        }
    }
}
