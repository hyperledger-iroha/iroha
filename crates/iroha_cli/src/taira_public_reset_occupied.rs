//! Exact occupied-runtime custody and reversible validator unit publication.
//!
//! The configuration selector is independent of each prior executable. A signed
//! artifact path is never reconstructed from another role's source revision.

use super::super::{
    OCCUPIED_VALIDATOR_ARTIFACT_ROLES, OccupiedArtifactV1, artifact_role_policy,
    validate_absolute_normal_path, validate_lower_hex,
};
use super::*;

const UNIT_TRANSITION_SCHEMA: &str = "iroha.taira.public-reset.validator-unit-transition.v1";
const UNIT_FORWARD_INTENT: &str = "validator-unit-forward.intent.json";
const UNIT_ROLLBACK_INTENT: &str = "validator-unit-rollback.intent.json";
const UNIT_BACKUP: &str = "validator-unit.before";

pub(in super::super) fn validate_occupied_binding(validator: &ValidatorV1) -> Result<()> {
    let prior = validator.admitted_release()?;
    prior.service_state.validate()?;
    if prior.artifacts.len() != OCCUPIED_VALIDATOR_ARTIFACT_ROLES.len() {
        return Err(eyre!(
            "occupied runtime requires exactly {} artifact roles",
            OCCUPIED_VALIDATOR_ARTIFACT_ROLES.len()
        ));
    }
    let mut paths = BTreeSet::new();
    for (entry, role) in prior
        .artifacts
        .iter()
        .zip(OCCUPIED_VALIDATOR_ARTIFACT_ROLES)
    {
        if entry.role != role || !paths.insert(&entry.path) {
            return Err(eyre!(
                "occupied artifact roles/paths are not exact, ordered and unique"
            ));
        }
        validate_lower_hex("occupied artifact SHA-256", &entry.sha256, 64)?;
        validate_lower_hex(
            "occupied artifact source revision",
            &entry.source_commit,
            40,
        )?;
        validate_absolute_normal_path(Path::new(&entry.path), "occupied artifact path")?;
        let (mode, maximum) = artifact_role_policy(role)?;
        if entry.mode != mode || entry.size == 0 || entry.size > maximum {
            return Err(eyre!("occupied artifact size or mode violates its role"));
        }
        match role {
            "iroha3d" => {
                let basename = "iroha3d_taira";
                let path = Path::new(&entry.path);
                let root = path
                    .parent()
                    .and_then(Path::parent)
                    .ok_or_else(|| eyre!("occupied executable has no release root"))?;
                if path != root.join("bin").join(basename) {
                    return Err(eyre!("occupied executable has the wrong role basename"));
                }
                let service_release = format!(
                    "{}/releases/{}",
                    validator.service_root, entry.source_commit
                );
                let private_parent = Path::new("/private/runtime/taira-public-reset");
                let prefix = format!("release-{}-", entry.source_commit);
                let private_release = root.parent() == Some(private_parent)
                    && root
                        .file_name()
                        .and_then(OsStr::to_str)
                        .and_then(|name| name.strip_prefix(&prefix))
                        .is_some_and(|suffix| {
                            !suffix.is_empty()
                                && suffix.len() <= 128
                                && suffix.bytes().all(|c| {
                                    c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'
                                })
                        });
                if root != Path::new(&service_release) && !private_release {
                    return Err(eyre!(
                        "occupied executable path does not bind its exact role source revision"
                    ));
                }
            }
            "config" => {
                if entry.path != format!("{}/config/config.toml", prior.release_root)
                    || entry.source_commit != prior.commit
                {
                    return Err(eyre!(
                        "occupied configuration does not bind the selected configuration release"
                    ));
                }
            }
            "genesis" | "genesis_hash" => {
                let basename = if role == "genesis" {
                    "genesis.json"
                } else {
                    "genesis.sha256"
                };
                let path = Path::new(&entry.path);
                if path.file_name() != Some(OsStr::new(basename))
                    || !(path.starts_with(Path::new(&validator.service_root).join("releases"))
                        || path.starts_with("/private/runtime/taira-public-reset"))
                {
                    return Err(eyre!(
                        "occupied genesis role escaped its admitted runtime namespace"
                    ));
                }
            }
            "validator_unit" => {
                if entry.path != format!("/etc/systemd/system/{}", validator.systemd_unit) {
                    return Err(eyre!(
                        "occupied validator unit is not its exact systemd fragment"
                    ));
                }
            }
            _ => unreachable!("closed role set checked above"),
        }
    }
    let daemon = prior.artifact("iroha3d")?;
    let stable = format!("{}/current/bin/iroha3d_taira", validator.service_root);
    let stable_resolves_to_daemon =
        daemon.path == format!("{}/bin/iroha3d_taira", prior.release_root);
    if prior.argv.len() != 4
        || !(prior.argv[0] == daemon.path || (prior.argv[0] == stable && stable_resolves_to_daemon))
        || prior.argv[1] != "--config"
        || prior.argv[2] != format!("{}/current/config/config.toml", validator.service_root)
        || prior.argv[3] != "--sora"
    {
        return Err(eyre!(
            "occupied daemon argv is not its exact executable and stable configuration closure"
        ));
    }
    Ok(())
}

fn verify_occupied_artifact_at(entry: &OccupiedArtifactV1, path: &Path, mode: u16) -> Result<()> {
    require_root_no_symlink_ancestors(path, "occupied artifact")?;
    let (mut file, snapshot) = open_pinned_regular(path, "occupied artifact")?;
    #[cfg(unix)]
    if snapshot.uid != 0 || snapshot.mode & 0o7777 != u32::from(mode) {
        return Err(eyre!(
            "occupied artifact root ownership or exact mode drifted"
        ));
    }
    if snapshot.len != entry.size || super::super::sha256_reader(&mut file, path)? != entry.sha256 {
        return Err(eyre!("occupied artifact bytes or size drifted"));
    }
    ensure_pinned_unchanged(path, "occupied artifact", &file, &snapshot)
}

pub(super) fn verify_prior_artifacts(validator: &ValidatorV1, include_unit: bool) -> Result<()> {
    validate_occupied_binding(validator)?;
    require_root_directory(
        Path::new(&validator.admitted_release()?.release_root),
        false,
        "occupied configuration release",
    )?;
    for entry in &validator.admitted_release()?.artifacts {
        if include_unit || entry.role != "validator_unit" {
            verify_occupied_artifact_at(entry, Path::new(&entry.path), entry.mode)?;
        }
    }
    Ok(())
}

pub(super) struct ProcessBinding {
    pub(super) release_root: PathBuf,
    pub(super) executable: PathBuf,
    pub(super) argv: Vec<String>,
    pub(super) config: PathBuf,
    pub(super) config_sha256: String,
    pub(super) genesis: PathBuf,
    pub(super) genesis_sha256: String,
    pub(super) unit_sha256: String,
}

pub(super) fn process_binding(
    admitted: &HostAdmission,
    validator: &ValidatorV1,
    fresh: bool,
) -> Result<ProcessBinding> {
    if fresh {
        let root = Path::new(&validator.service_root)
            .join("releases")
            .join(&admitted.inventory.revision.commit);
        let stable = Path::new(&validator.service_root).join("current");
        let active = beacon::active_binding(admitted, validator)?;
        let initial_config = artifact(&validator.artifacts, "config")?;
        let config_name = if active.is_some() {
            "beacon.toml"
        } else {
            "config.toml"
        };
        Ok(ProcessBinding {
            release_root: root.clone(),
            executable: root.join("bin/iroha3d_taira"),
            argv: vec![
                stable
                    .join("bin/iroha3d_taira")
                    .to_string_lossy()
                    .into_owned(),
                "--config".to_owned(),
                stable
                    .join("config")
                    .join(config_name)
                    .to_string_lossy()
                    .into_owned(),
                "--sora".to_owned(),
            ],
            config: active
                .as_ref()
                .map(|binding| binding.config.clone())
                .unwrap_or_else(|| PathBuf::from(&initial_config.remote_path)),
            config_sha256: active
                .as_ref()
                .map(|binding| binding.config_sha256.clone())
                .unwrap_or_else(|| initial_config.sha256.clone()),
            genesis: PathBuf::from(&artifact(&validator.artifacts, "genesis")?.remote_path),
            genesis_sha256: artifact(&validator.artifacts, "genesis")?.sha256.clone(),
            unit_sha256: active
                .map(|binding| binding.unit_sha256)
                .unwrap_or_else(|| validator.systemd_unit_sha256.clone()),
        })
    } else {
        validate_occupied_binding(validator)?;
        let prior = validator.admitted_release()?;
        Ok(ProcessBinding {
            release_root: PathBuf::from(&prior.release_root),
            executable: PathBuf::from(&prior.artifact("iroha3d")?.path),
            argv: prior.argv.clone(),
            config: PathBuf::from(&prior.artifact("config")?.path),
            config_sha256: prior.artifact("config")?.sha256.clone(),
            genesis: PathBuf::from(&prior.artifact("genesis")?.path),
            genesis_sha256: prior.artifact("genesis")?.sha256.clone(),
            unit_sha256: prior.artifact("validator_unit")?.sha256.clone(),
        })
    }
}

pub(super) fn verify_prior_genesis_hash(
    admitted: &HostAdmission,
    validator: &ValidatorV1,
) -> Result<()> {
    let entry = validator.admitted_release()?.artifact("genesis_hash")?;
    let path = Path::new(&entry.path);
    let (file, snapshot) = open_pinned_regular(path, "occupied genesis hash")?;
    let bytes = read_pinned_bytes(path, "occupied genesis hash", file, &snapshot, 65)?;
    if bytes != format!("{}\n", admitted.inventory.previous_genesis_hash).as_bytes()
        || sha256_hex(&bytes) != entry.sha256
    {
        return Err(eyre!(
            "occupied genesis-hash artifact does not bind the previous network"
        ));
    }
    Ok(())
}

pub(super) fn protects_prior_artifact(target: &HostTarget, root: &Path) -> bool {
    match target {
        HostTarget::Validator(validator) => validator.admitted_release().is_ok_and(|prior| {
            Path::new(&prior.release_root).starts_with(root)
                || prior
                    .artifacts
                    .iter()
                    .any(|entry| Path::new(&entry.path).starts_with(root))
        }),
        HostTarget::Edge(edge) => edge
            .admitted_release()
            .is_ok_and(|prior| Path::new(&prior.release_root).starts_with(root)),
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize, PartialEq, Eq)]
#[norito(deny_unknown_fields)]
struct UnitTransitionIntent {
    schema: String,
    authorization_nonce: String,
    inventory_sha256: String,
    host_slug: String,
    destination: String,
    prior_sha256: String,
    candidate_sha256: String,
    restoring: bool,
}

fn unit_intent(
    admitted: &HostAdmission,
    validator: &ValidatorV1,
    restoring: bool,
) -> Result<UnitTransitionIntent> {
    Ok(UnitTransitionIntent {
        schema: UNIT_TRANSITION_SCHEMA.to_owned(),
        authorization_nonce: admitted.inventory.authorization_nonce.clone(),
        inventory_sha256: sha256_hex(&super::super::canonical_inventory_bytes(
            &admitted.inventory,
        )?),
        host_slug: validator.slug.clone(),
        destination: validator
            .admitted_release()?
            .artifact("validator_unit")?
            .path
            .clone(),
        prior_sha256: validator
            .admitted_release()?
            .artifact("validator_unit")?
            .sha256
            .clone(),
        candidate_sha256: validator.systemd_unit_sha256.clone(),
        restoring,
    })
}

fn load_unit_intent(directory: &Path, expected: &UnitTransitionIntent) -> Result<()> {
    let name = if expected.restoring {
        UNIT_ROLLBACK_INTENT
    } else {
        UNIT_FORWARD_INTENT
    };
    let (actual, _) = read_private_json::<UnitTransitionIntent>(
        &directory.join(name),
        "validator unit transition",
    )?;
    validate_unit_intent(&actual, expected)
}

fn validate_unit_intent(
    actual: &UnitTransitionIntent,
    expected: &UnitTransitionIntent,
) -> Result<()> {
    if actual != expected {
        return Err(eyre!(
            "validator unit transition does not bind the exact admitted old/new closure"
        ));
    }
    Ok(())
}

fn publish_unit_intent(directory: &Path, expected: &UnitTransitionIntent) -> Result<()> {
    let name = if expected.restoring {
        UNIT_ROLLBACK_INTENT
    } else {
        UNIT_FORWARD_INTENT
    };
    if !directory.join(name).exists() {
        publish_root_private_noreplace(directory, name, json::to_json(expected)?.as_bytes())?;
    }
    load_unit_intent(directory, expected)
}

/// A prior/candidate choice is allowed only after the exact forward intent and
/// retained prior bytes have been authenticated. Other bytes never authorize a retry.
fn classify_unit_publication(actual: &str, intent: &UnitTransitionIntent) -> Result<bool> {
    if actual == intent.prior_sha256 {
        Ok(false)
    } else if actual == intent.candidate_sha256 {
        Ok(true)
    } else {
        Err(eyre!(
            "validator unit is outside its durable exact transition"
        ))
    }
}

fn current_unit_hash(path: &Path) -> Result<String> {
    require_root_no_symlink_ancestors(path, "validator unit fragment")?;
    let (mut file, snapshot) = open_pinned_regular(path, "validator unit fragment")?;
    #[cfg(unix)]
    if snapshot.uid != 0 || snapshot.mode & 0o7777 != 0o644 {
        return Err(eyre!(
            "validator unit fragment lost root-owned 0644 custody"
        ));
    }
    if snapshot.len == 0 || snapshot.len > 1024 * 1024 {
        return Err(eyre!("validator unit fragment exceeds its exact bound"));
    }
    let hash = super::super::sha256_reader(&mut file, path)?;
    ensure_pinned_unchanged(path, "validator unit fragment", &file, &snapshot)?;
    Ok(hash)
}

pub(super) fn verify_unit_fragment(path: &Path, expected_sha256: &str) -> Result<()> {
    if current_unit_hash(path)? != expected_sha256 {
        return Err(eyre!(
            "loaded validator unit differs from its exact phase hash"
        ));
    }
    Ok(())
}

/// Bind the loaded unit during stopped-owner cleanup to its exact admitted publication phase.
/// The prior fragment is valid before Install; a successor requires its retained native intent.
pub(super) fn stopped_unit_hash(
    admitted: &HostAdmission,
    validator: &ValidatorV1,
) -> Result<String> {
    let destination = Path::new("/etc/systemd/system").join(&validator.systemd_unit);
    let actual = current_unit_hash(&destination)?;
    let prior = if validator.is_vacant() {
        None
    } else {
        Some(validator.admitted_release()?.artifact("validator_unit")?)
    };
    admit_stopped_unit_hash(
        &actual,
        prior.map(|entry| entry.sha256.as_str()),
        &validator.systemd_unit_sha256,
        || {
            let prior = prior.ok_or_else(|| eyre!("vacant unit has no prior transition"))?;
            let directory = Path::new(&validator.reset_guard)
                .join("rollback")
                .join(&admitted.inventory.authorization_nonce);
            require_root_directory(&directory, true, "retained validator unit transition")?;
            verify_generated_marker(&directory, admitted, "rollback")?;
            load_unit_intent(&directory, &unit_intent(admitted, validator, false)?)?;
            verify_occupied_artifact_at(prior, &directory.join(UNIT_BACKUP), 0o600)
        },
        || beacon::prepared_unit_hash(admitted, validator),
    )?;
    Ok(actual)
}

fn admit_stopped_unit_hash(
    actual: &str,
    prior: Option<&str>,
    initial: &str,
    verify_forward: impl FnOnce() -> Result<()>,
    verified_beacon: impl FnOnce() -> Result<Option<String>>,
) -> Result<()> {
    if actual == prior.unwrap_or(initial) {
        return Ok(());
    }
    if prior.is_some() {
        verify_forward()?;
        if actual == initial {
            return Ok(());
        }
    }
    if verified_beacon()?.as_deref() != Some(actual) {
        return Err(eyre!(
            "stopped validator unit is outside its exact admitted publication phases"
        ));
    }
    Ok(())
}

/// Publish one exact unit while retaining both source and destination descriptors
/// through the final rename. The caller has authenticated the phase intent and
/// root-owned parents; this function also rejects intervening inode/byte drift.
pub(super) fn publish_unit_bytes_with(
    source: &Path,
    destination: &Path,
    desired: &ArtifactV1,
    previous_sha256: &str,
    restoring: bool,
    before_rename: impl Fn() -> Result<()>,
    sync_parent: impl Fn(&Path) -> Result<()>,
) -> Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| eyre!("unit has no parent"))?;
    let (mut previous, previous_snapshot) =
        open_pinned_regular(destination, "unit publication destination")?;
    if super::super::sha256_reader(&mut previous, destination)? != previous_sha256 {
        return Err(eyre!(
            "unit publication destination no longer has its exact preceding bytes"
        ));
    }
    let name = destination
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| eyre!("unit name is not UTF-8"))?;
    let phase = if restoring { "rollback" } else { "forward" };
    let next = parent.join(format!(".{name}.public-reset-{phase}.next"));
    if next.exists() {
        let (file, snapshot) = open_pinned_regular(&next, "unit publication staging")?;
        #[cfg(unix)]
        if snapshot.uid != previous_snapshot.uid
            || snapshot.mode & 0o7777 != u32::from(desired.mode)
        {
            return Err(eyre!("unit staging custody drifted"));
        }
        if snapshot.len < desired.size {
            ensure_pinned_unchanged(&next, "unit publication staging", &file, &snapshot)?;
            fs::remove_file(&next)?;
            sync_parent(parent)?;
        } else if snapshot.len != desired.size {
            return Err(eyre!(
                "unit staging length is outside its exact publication"
            ));
        }
    }
    copy_verified_file(source, &next, desired)?;
    let (next_file, next_snapshot) = open_pinned_regular(&next, "unit publication staging")?;
    before_rename()?;
    ensure_pinned_unchanged(
        destination,
        "unit publication destination",
        &previous,
        &previous_snapshot,
    )?;
    ensure_pinned_unchanged(
        &next,
        "unit publication staging",
        &next_file,
        &next_snapshot,
    )?;
    fs::rename(&next, destination)?;
    sync_parent(parent)
}

pub(super) fn install_validator_unit(admitted: &HostAdmission) -> Result<()> {
    let HostTarget::Validator(validator) = &admitted.target else {
        return Err(eyre!("unit install requires a validator"));
    };
    if validator.is_vacant() {
        return attest_loaded_systemd_unit(
            validator,
            &validator.systemd_unit_sha256,
            admitted.action_deadline,
        );
    }
    let directory = rollback_nonce_root(admitted)?;
    if directory.join(UNIT_ROLLBACK_INTENT).exists() {
        return Err(eyre!(
            "unit forward publication cannot resume after rollback intent"
        ));
    }
    let prior = validator.admitted_release()?.artifact("validator_unit")?;
    let destination = Path::new(&prior.path);
    let backup = directory.join(UNIT_BACKUP);
    let intent = unit_intent(admitted, validator, false)?;
    if !directory.join(UNIT_FORWARD_INTENT).exists() {
        require_unit_stopped(&validator.systemd_unit, admitted.action_deadline)?;
        require_session_manager_operation_applied(
            admitted,
            "stop",
            "stop",
            &validator.systemd_unit,
        )?;
        verify_occupied_artifact_at(prior, destination, prior.mode)?;
        attest_loaded_systemd_unit(validator, &prior.sha256, admitted.action_deadline)?;
        snapshot_root_file(admitted, destination, &backup, &prior.sha256)?;
        verify_occupied_artifact_at(prior, &backup, 0o600)?;
        publish_unit_intent(&directory, &intent)?;
    }
    load_unit_intent(&directory, &intent)?;
    verify_occupied_artifact_at(prior, &backup, 0o600)?;
    let candidate = artifact(&validator.artifacts, "validator_unit")?;
    let published = classify_unit_publication(&current_unit_hash(destination)?, &intent)?;
    if !published {
        require_unit_stopped(&validator.systemd_unit, admitted.action_deadline)?;
        publish_unit_bytes_with(
            Path::new(&candidate.remote_path),
            destination,
            candidate,
            &prior.sha256,
            false,
            || ensure_action_deadline(admitted),
            sync_directory,
        )?;
    } else {
        sync_existing_file_publication(
            destination,
            &candidate.sha256,
            destination.parent().unwrap(),
            sync_directory,
        )?;
    }
    run_durable_manager_operation(admitted, "install-unit-reload", "daemon-reload", "")?;
    attest_loaded_systemd_unit(validator, &candidate.sha256, admitted.action_deadline)
}

pub(super) fn restore_validator_unit(admitted: &HostAdmission) -> Result<()> {
    let HostTarget::Validator(validator) = &admitted.target else {
        return Err(eyre!("unit restore requires a validator"));
    };
    let prior = validator.admitted_release()?.artifact("validator_unit")?;
    let destination = Path::new(&prior.path);
    let directory = rollback_nonce_root(admitted)?;
    if !directory.join(UNIT_FORWARD_INTENT).exists() {
        verify_occupied_artifact_at(prior, destination, prior.mode)?;
        return attest_loaded_systemd_unit(validator, &prior.sha256, admitted.action_deadline);
    }
    let forward = unit_intent(admitted, validator, false)?;
    load_unit_intent(&directory, &forward)?;
    let backup = directory.join(UNIT_BACKUP);
    verify_occupied_artifact_at(prior, &backup, 0o600)?;
    require_unit_stopped(&validator.systemd_unit, admitted.action_deadline)?;
    publish_unit_intent(&directory, &unit_intent(admitted, validator, true)?)?;
    let current_hash = current_unit_hash(destination)?;
    let published =
        if current_hash == forward.prior_sha256 || current_hash == forward.candidate_sha256 {
            classify_unit_publication(&current_hash, &forward)?
        } else if beacon::prepared_unit_hash(admitted, validator)?.as_deref()
            == Some(current_hash.as_str())
        {
            true
        } else {
            return Err(eyre!(
                "rollback unit is outside both exact signed publication phases"
            ));
        };
    if published {
        let restored = ArtifactV1 {
            role: prior.role.clone(),
            local_path: backup.to_string_lossy().into_owned(),
            remote_path: prior.path.clone(),
            sha256: prior.sha256.clone(),
            size: prior.size,
            mode: prior.mode,
            source_commit: prior.source_commit.clone(),
            target: super::super::BUILD_TARGET.to_owned(),
        };
        publish_unit_bytes_with(
            &backup,
            destination,
            &restored,
            &current_hash,
            true,
            || ensure_action_deadline(admitted),
            sync_directory,
        )?;
    } else {
        sync_existing_file_publication(
            destination,
            &prior.sha256,
            destination.parent().unwrap(),
            sync_directory,
        )?;
    }
    run_durable_manager_operation(admitted, "rollback-unit-reload", "daemon-reload", "")?;
    verify_occupied_artifact_at(prior, destination, prior.mode)?;
    attest_loaded_systemd_unit(validator, &prior.sha256, admitted.action_deadline)
}

pub(super) fn verify_restored_validator_unit(admitted: &HostAdmission) -> Result<()> {
    let HostTarget::Validator(validator) = &admitted.target else {
        return Err(eyre!("unit rollback proof requires a validator"));
    };
    let directory = rollback_nonce_root(admitted)?;
    if directory.join(UNIT_FORWARD_INTENT).exists() {
        load_unit_intent(&directory, &unit_intent(admitted, validator, false)?)?;
        load_unit_intent(&directory, &unit_intent(admitted, validator, true)?)?;
        verify_occupied_artifact_at(
            validator.admitted_release()?.artifact("validator_unit")?,
            &directory.join(UNIT_BACKUP),
            0o600,
        )?;
        require_session_manager_operation_applied(
            admitted,
            "rollback-unit-reload",
            "daemon-reload",
            "",
        )?;
    }
    verify_prior_artifacts(validator, true)
}

#[cfg(test)]
mod tests {
    use super::super::super::{
        ValidatorInitialStateV1, sample_inventory_fixture, validate_validator,
    };
    use super::*;

    fn split_validator() -> ValidatorV1 {
        let mut validator = sample_inventory_fixture().validators.remove(0);
        let ValidatorInitialStateV1::AdmittedRelease(prior) = &mut validator.initial_state else {
            unreachable!()
        };
        let daemon = prior
            .artifacts
            .iter_mut()
            .find(|entry| entry.role == "iroha3d")
            .unwrap();
        daemon.source_commit = "5".repeat(40);
        daemon.path = format!(
            "/private/runtime/taira-public-reset/release-{}-update-0123456789abcdef/bin/iroha3d_taira",
            daemon.source_commit
        );
        prior.argv[0] = prior.artifact("iroha3d").unwrap().path.clone();
        for role in ["genesis", "genesis_hash"] {
            let entry = prior
                .artifacts
                .iter_mut()
                .find(|entry| entry.role == role)
                .unwrap();
            entry.source_commit = "6".repeat(40);
            let basename = if role == "genesis" {
                "genesis.json"
            } else {
                "genesis.sha256"
            };
            entry.path =
                format!("/private/runtime/taira-public-reset/continuation/genesis/{basename}");
        }
        validator
    }

    #[test]
    fn occupied_runtime_accepts_split_source_and_configuration_binding() {
        let inventory = sample_inventory_fixture();
        // Admit all current occupied roles in a single release before checking
        // independent executable and configuration source revisions.
        validate_occupied_binding(&inventory.validators[0]).unwrap();
        let validator = split_validator();
        validate_validator(
            &validator,
            &validator.slug,
            &inventory.revision,
            inventory.qualification_scope,
        )
        .unwrap();
        let prior = validator.admitted_release().unwrap();
        assert_eq!(
            prior
                .artifacts
                .iter()
                .map(|entry| entry.role.as_str())
                .collect::<Vec<_>>(),
            OCCUPIED_VALIDATOR_ARTIFACT_ROLES
        );
        assert_ne!(
            prior.artifact("iroha3d").unwrap().source_commit,
            prior.commit
        );
        assert_eq!(
            prior.artifact("config").unwrap().source_commit,
            prior.commit
        );
        assert_eq!(
            inventory.validators[0].artifacts.len(),
            super::super::super::VALIDATOR_ARTIFACT_ROLES.len()
        );
        assert_ne!(
            prior.artifact("genesis").unwrap().source_commit,
            prior.commit
        );
        let encoded = json::to_json(&validator).unwrap();
        let decoded: ValidatorV1 = json::from_str(&encoded).unwrap();
        validate_validator(
            &decoded,
            &decoded.slug,
            &inventory.revision,
            inventory.qualification_scope,
        )
        .unwrap();
        assert_eq!(encoded, json::to_json(&decoded).unwrap());
    }

    #[test]
    fn occupied_runtime_rejects_incomplete_or_foreign_artifact_custody() {
        for change in 0..17 {
            let mut validator = split_validator();
            let ValidatorInitialStateV1::AdmittedRelease(prior) = &mut validator.initial_state
            else {
                unreachable!()
            };
            let role_index = |role: &str| {
                prior
                    .artifacts
                    .iter()
                    .position(|entry| entry.role == role)
                    .unwrap()
            };
            let daemon = role_index("iroha3d");
            let config = role_index("config");
            let unit = role_index("validator_unit");
            let genesis = role_index("genesis");
            match change {
                0 => {
                    prior.artifacts.pop();
                }
                1 => prior.artifacts.swap(daemon, config),
                2 => prior.artifacts[daemon].role = "iroha_cli".to_owned(),
                3 => prior.artifacts[daemon].source_commit = "7".repeat(40),
                4 => prior.artifacts[daemon].path = prior.artifacts[config].path.clone(),
                5 => prior.artifacts[daemon].sha256 = "xyz".to_owned(),
                6 => prior.artifacts[daemon].size = 0,
                7 => prior.artifacts[config].mode = 0o644,
                8 => prior.artifacts[config].source_commit = "7".repeat(40),
                9 => prior.artifacts[unit].path = "/etc/systemd/system/foreign.service".to_owned(),
                10 => {
                    prior.argv[0] = format!("{}/current/bin/iroha3d_taira", validator.service_root)
                }
                11 => prior.argv[2] = prior.artifact("config").unwrap().path.clone(),
                12 => prior.argv.push("--extra".to_owned()),
                13 => prior.artifacts[genesis].path = "/tmp/genesis.json".to_owned(),
                14 => {
                    prior.artifacts[daemon].path = Path::new(&prior.artifacts[daemon].path)
                        .with_file_name("iroha")
                        .to_string_lossy()
                        .into_owned();
                }
                15 => prior.artifacts[daemon].source_commit = prior.commit.clone(),
                16 => prior.artifacts[daemon].role = "sorafs_node".to_owned(),
                _ => unreachable!(),
            }
            let error = validate_occupied_binding(&validator).expect_err(&format!(
                "change {change} escaped the native occupied closure"
            ));
            let expected = match change {
                14 => Some("occupied executable has the wrong role basename"),
                15 => Some("occupied executable path does not bind its exact role source revision"),
                16 => Some("occupied artifact roles/paths are not exact, ordered and unique"),
                _ => None,
            };
            if let Some(expected) = expected {
                assert!(
                    error.to_string().contains(expected),
                    "change {change}: {error}"
                );
            }
        }
    }

    #[test]
    fn occupied_runtime_rejects_builder_tools_and_each_missing_runtime_role() {
        for role in OCCUPIED_VALIDATOR_ARTIFACT_ROLES {
            let mut validator = split_validator();
            let ValidatorInitialStateV1::AdmittedRelease(prior) = &mut validator.initial_state
            else {
                unreachable!()
            };
            prior.artifacts.retain(|entry| entry.role != role);
            assert!(
                validate_occupied_binding(&validator)
                    .unwrap_err()
                    .to_string()
                    .contains("exactly 5 artifact roles"),
                "missing {role}"
            );
        }
        let mut validator = split_validator();
        let ValidatorInitialStateV1::AdmittedRelease(prior) = &mut validator.initial_state else {
            unreachable!()
        };
        let daemon = prior.artifact("iroha3d").unwrap().clone();
        for (offset, (role, name)) in [
            ("iroha_cli", "iroha"),
            ("kagami", "kagami"),
            ("sorafs_node", "sorafs-node"),
        ]
        .into_iter()
        .enumerate()
        {
            let mut tool = daemon.clone();
            tool.role = role.into();
            tool.path = Path::new(&daemon.path)
                .with_file_name(name)
                .display()
                .to_string();
            prior.artifacts.insert(offset + 1, tool);
        }
        assert_eq!(
            prior
                .artifacts
                .iter()
                .map(|entry| entry.role.as_str())
                .collect::<Vec<_>>(),
            super::super::super::VALIDATOR_ARTIFACT_ROLES
        );
        assert!(
            validate_occupied_binding(&validator)
                .unwrap_err()
                .to_string()
                .contains("exactly 5 artifact roles")
        );
    }

    #[test]
    fn occupied_runtime_wire_requires_explicit_artifacts_and_argv() {
        let validator = split_validator();
        let encoded = json::to_value(validator.admitted_release().unwrap()).unwrap();
        for field in ["artifacts", "argv"] {
            let mut missing = encoded.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                json::from_value::<super::super::super::ValidatorAdmittedReleaseV1>(missing)
                    .is_err()
            );
        }
        let mut retired = encoded;
        retired.as_object_mut().unwrap().insert(
            "iroha3d_sha256".to_owned(),
            json::Value::String("1".repeat(64)),
        );
        assert!(
            json::from_value::<super::super::super::ValidatorAdmittedReleaseV1>(retired).is_err()
        );
        let entry = validator
            .admitted_release()
            .unwrap()
            .artifact("iroha3d")
            .unwrap();
        for field in ["path", "sha256", "size", "mode", "source_commit"] {
            let mut missing = json::to_value(entry).unwrap();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                json::from_value::<OccupiedArtifactV1>(missing).is_err(),
                "missing {field}"
            );
        }
    }

    #[test]
    fn occupied_runtime_process_binding_distinguishes_prior_and_candidate() {
        let admitted = super::super::tests::progress_admission();
        let validator = split_validator();
        let before = process_binding(&admitted, &validator, false).unwrap();
        let after = process_binding(&admitted, &validator, true).unwrap();
        assert_eq!(
            before.executable,
            PathBuf::from(
                &validator
                    .admitted_release()
                    .unwrap()
                    .artifact("iroha3d")
                    .unwrap()
                    .path
            )
        );
        assert_eq!(before.argv, validator.admitted_release().unwrap().argv);
        assert_ne!(before.executable, after.executable);
        assert_ne!(before.release_root, after.release_root);
        assert_ne!(before.genesis, after.genesis);
        assert_ne!(before.unit_sha256, after.unit_sha256);
        assert_eq!(
            after.executable,
            after.release_root.join("bin/iroha3d_taira")
        );
        assert_eq!(
            after.unit_sha256,
            artifact(&validator.artifacts, "validator_unit")
                .unwrap()
                .sha256
        );
    }

    #[test]
    fn occupied_runtime_cleanup_preserves_every_prior_artifact_root() {
        let validator = split_validator();
        let target = HostTarget::Validator(validator.clone());
        for entry in &validator.admitted_release().unwrap().artifacts {
            let root = Path::new(&entry.path).parent().unwrap();
            assert!(
                protects_prior_artifact(&target, root),
                "unprotected {}",
                entry.role
            );
        }
        let prior = validator.admitted_release().unwrap();
        let daemon_root = Path::new(&prior.artifact("iroha3d").unwrap().path)
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        // The source42 CLI is a daemon sibling; retained SoraFS is within the
        // selected configuration release. Neither is a validator dependency.
        for retained_tool in [
            daemon_root.join("bin/iroha"),
            Path::new(&prior.release_root).join("bin/sorafs-node"),
        ] {
            let release = retained_tool.parent().unwrap().parent().unwrap();
            assert!(
                protects_prior_artifact(&target, release),
                "{}",
                retained_tool.display()
            );
        }
        assert!(!protects_prior_artifact(
            &target,
            Path::new(
                "/srv/taira/taira-validator-1/releases/ffffffffffffffffffffffffffffffffffffffff"
            )
        ));
    }

    #[test]
    fn stopped_unit_admission_requires_the_exact_prior_or_durable_successor() {
        let prior = "1".repeat(64);
        let initial = "2".repeat(64);
        let beacon = "3".repeat(64);
        admit_stopped_unit_hash(
            &prior,
            Some(&prior),
            &initial,
            || panic!("old unit before Install requires no candidate transition"),
            || panic!("old unit must not require a new beacon"),
        )
        .unwrap();
        admit_stopped_unit_hash(
            &initial,
            None,
            &initial,
            || panic!("vacant target has no previous unit"),
            || panic!("initial vacant unit needs no beacon"),
        )
        .unwrap();
        assert!(
            admit_stopped_unit_hash(
                &initial,
                Some(&prior),
                &initial,
                || Err(eyre!("missing exact forward intent or prior backup")),
                || panic!("failed transition cannot fall back to a beacon")
            )
            .is_err()
        );
        let authenticated = std::cell::Cell::new(false);
        admit_stopped_unit_hash(
            &initial,
            Some(&prior),
            &initial,
            || {
                authenticated.set(true);
                Ok(())
            },
            || panic!("initial candidate is not a beacon unit"),
        )
        .unwrap();
        assert!(authenticated.get());
        admit_stopped_unit_hash(
            &beacon,
            Some(&prior),
            &initial,
            || Ok(()),
            || Ok(Some(beacon.clone())),
        )
        .unwrap();
        for observed in [None, Some(initial.clone()), Some("4".repeat(64))] {
            assert!(
                admit_stopped_unit_hash(
                    &beacon,
                    Some(&prior),
                    &initial,
                    || Ok(()),
                    || Ok(observed)
                )
                .is_err()
            );
        }
    }

    #[test]
    fn occupied_unit_transition_rejects_unbound_recovery_and_preserves_rollback() {
        let admitted = super::super::tests::progress_admission();
        let validator = split_validator();
        let forward = unit_intent(&admitted, &validator, false).unwrap();
        let rollback = unit_intent(&admitted, &validator, true).unwrap();
        assert_eq!(rollback.prior_sha256, forward.prior_sha256);
        assert_eq!(rollback.candidate_sha256, forward.candidate_sha256);
        assert!(validate_unit_intent(&forward, &rollback).is_err());
        for change in 0..5 {
            let mut altered = forward.clone();
            match change {
                0 => altered.authorization_nonce.push('0'),
                1 => altered.inventory_sha256 = "0".repeat(64),
                2 => altered.destination.push_str(".foreign"),
                3 => altered.prior_sha256 = altered.candidate_sha256.clone(),
                4 => altered.restoring = true,
                _ => unreachable!(),
            }
            assert!(validate_unit_intent(&altered, &forward).is_err());
        }
        assert!(!classify_unit_publication(&forward.prior_sha256, &forward).unwrap());
        assert!(classify_unit_publication(&forward.candidate_sha256, &forward).unwrap());
        assert!(classify_unit_publication(&"0".repeat(64), &forward).is_err());
    }

    #[cfg(unix)]
    fn unit_fixture(root: &Path, name: &str, bytes: &[u8]) -> ArtifactV1 {
        let path = root.join(name);
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        ArtifactV1 {
            role: "validator_unit".to_owned(),
            local_path: path.to_string_lossy().into_owned(),
            remote_path: path.to_string_lossy().into_owned(),
            sha256: sha256_hex(bytes),
            size: bytes.len() as u64,
            mode: 0o644,
            source_commit: "1".repeat(40),
            target: super::super::super::BUILD_TARGET.to_owned(),
        }
    }

    #[cfg(unix)]
    #[test]
    fn occupied_unit_publication_recovers_interrupted_forward_and_rollback() {
        let directory = super::super::super::private_custody_test_dir("unit-transition-");
        let prior = unit_fixture(
            directory.path(),
            "prior.service",
            b"[Service]\nExecStart=/exact/prior --config /exact/current/config --sora\n",
        );
        let candidate = unit_fixture(directory.path(), "candidate.service", b"[Service]\nExecStart=/exact/current/bin/iroha3d_taira --config /exact/current/config --sora\n");
        let destination = directory.path().join("installed.service");
        fs::copy(&prior.local_path, &destination).unwrap();
        // Interruption after staging must leave the exact prior unit live.
        assert!(
            publish_unit_bytes_with(
                Path::new(&candidate.local_path),
                &destination,
                &candidate,
                &prior.sha256,
                false,
                || Err(eyre!("interrupted before unit rename")),
                sync_directory
            )
            .is_err()
        );
        verify_regular_hash(&destination, &prior.sha256).unwrap();
        // Reuse the exact staged inode. An interruption after rename still
        // cannot authorize manager reload until the directory barrier succeeds.
        assert!(
            publish_unit_bytes_with(
                Path::new(&candidate.local_path),
                &destination,
                &candidate,
                &prior.sha256,
                false,
                || Ok(()),
                |_| Err(eyre!("interrupted after unit rename"))
            )
            .is_err()
        );
        verify_regular_hash(&destination, &candidate.sha256).unwrap();
        sync_existing_file_publication(
            &destination,
            &candidate.sha256,
            directory.path(),
            sync_directory,
        )
        .unwrap();
        // Rollback uses its own staging name and restores the retained prior bytes.
        assert!(
            publish_unit_bytes_with(
                Path::new(&prior.local_path),
                &destination,
                &prior,
                &candidate.sha256,
                true,
                || Err(eyre!("interrupted rollback publication")),
                sync_directory
            )
            .is_err()
        );
        verify_regular_hash(&destination, &candidate.sha256).unwrap();
        publish_unit_bytes_with(
            Path::new(&prior.local_path),
            &destination,
            &prior,
            &candidate.sha256,
            true,
            || Ok(()),
            sync_directory,
        )
        .unwrap();
        verify_regular_hash(&destination, &prior.sha256).unwrap();
        assert_eq!(fs::metadata(&destination).unwrap().mode() & 0o7777, 0o644);
    }

    #[cfg(unix)]
    #[test]
    fn occupied_unit_publication_rejects_destination_or_staging_drift() {
        let directory = super::super::super::private_custody_test_dir("unit-drift-");
        let prior = unit_fixture(directory.path(), "prior.service", b"prior unit\n");
        let candidate = unit_fixture(directory.path(), "candidate.service", b"candidate unit\n");
        let destination = directory.path().join("installed.service");
        fs::copy(&prior.local_path, &destination).unwrap();
        let error = publish_unit_bytes_with(
            Path::new(&candidate.local_path),
            &destination,
            &candidate,
            &prior.sha256,
            false,
            || {
                fs::write(&destination, b"foreign unit\n")?;
                Ok(())
            },
            sync_directory,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("changed during descriptor-bound validation"),
            "{error:#}"
        );
        assert_eq!(fs::read(&destination).unwrap(), b"foreign unit\n");
        fs::copy(&prior.local_path, &destination).unwrap();
        let staging = directory
            .path()
            .join(".installed.service.public-reset-forward.next");
        fs::write(&staging, vec![b'x'; candidate.size as usize]).unwrap();
        assert!(
            publish_unit_bytes_with(
                Path::new(&candidate.local_path),
                &destination,
                &candidate,
                &prior.sha256,
                false,
                || Ok(()),
                sync_directory
            )
            .is_err()
        );
        verify_regular_hash(&destination, &prior.sha256).unwrap();
    }

    #[test]
    fn occupied_unit_reload_requires_exact_terminal_manager_evidence() {
        assert_eq!(
            manager_command_arguments("daemon-reload", "").unwrap(),
            ["daemon-reload"]
        );
        for (verb, target) in [
            ("daemon-reload", "foreign.service"),
            ("start", ""),
            ("reload-or-restart", "unit.service"),
        ] {
            assert!(manager_command_arguments(verb, target).is_err());
        }
        let intent = ManagerIntentV1 {
            schema: MANAGER_INTENT_SCHEMA_V1.to_owned(),
            action: "install-unit-reload".to_owned(),
            host_slug: "taira-validator-1".to_owned(),
            request_sha256: "1".repeat(64),
            authorization_nonce: "2".repeat(32),
            boot_id: "01234567-89ab-cdef-0123-456789abcdef".to_owned(),
            operation_unit: "exact-unit.service".to_owned(),
            verb: "daemon-reload".to_owned(),
            target_unit: String::new(),
            created_at_unix_ms: 1,
            action_deadline_unix_ms: 100,
        };
        let applied = "LoadState=loaded\nActiveState=active\nSubState=exited\nResult=success\nExecMainCode=1\nExecMainStatus=0\nInvocationID=0123456789abcdef0123456789abcdef\nExecStart={ path=/usr/bin/systemctl ; argv[]=/usr/bin/systemctl daemon-reload ; ignore_errors=no }\nJob=\n";
        assert_eq!(
            classify_manager_operation_evidence(applied.as_bytes(), &intent).unwrap(),
            ManagerOperationEvidence::Applied
        );
        let pending = applied.replace("Job=\n", "Job=42\n");
        assert_eq!(
            classify_manager_operation_evidence(pending.as_bytes(), &intent).unwrap(),
            ManagerOperationEvidence::Pending
        );
        let failed = applied.replace("ExecMainStatus=0", "ExecMainStatus=1");
        assert_eq!(
            classify_manager_operation_evidence(failed.as_bytes(), &intent).unwrap(),
            ManagerOperationEvidence::Rejected
        );
        let foreign = applied.replace("daemon-reload ;", "daemon-reload foreign.service ;");
        assert!(classify_manager_operation_evidence(foreign.as_bytes(), &intent).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn occupied_unit_copy_preserves_exact_mode_and_rejects_retained_mode_drift() {
        let directory = super::super::super::private_custody_test_dir("unit-copy-mode-");
        let source = unit_fixture(directory.path(), "source.service", b"exact unit\n");
        let destination = directory.path().join("copy.service");
        copy_verified_file(Path::new(&source.local_path), &destination, &source).unwrap();
        assert_eq!(fs::metadata(&destination).unwrap().mode() & 0o7777, 0o644);
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(copy_verified_file(Path::new(&source.local_path), &destination, &source).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"exact unit\n");
    }

    #[cfg(unix)]
    #[test]
    fn pinned_reader_rejects_oversized_snapshot_before_allocation_and_allows_empty() {
        let directory = super::super::super::private_custody_test_dir("bounded-pinned-reader-");
        let path = directory.path().join("empty");
        fs::write(&path, []).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let (file, mut snapshot) = open_pinned_regular(&path, "bounded test").unwrap();
        snapshot.len = u64::MAX;
        let error = read_pinned_bytes(&path, "bounded test", file, &snapshot, 128).unwrap_err();
        assert_eq!(
            error.to_string(),
            "bounded test exceeds the 128-byte V1 limit"
        );
        let (file, snapshot) = open_pinned_regular(&path, "empty test").unwrap();
        assert!(
            read_pinned_bytes(&path, "empty test", file, &snapshot, 0)
                .unwrap()
                .is_empty()
        );
    }
}
