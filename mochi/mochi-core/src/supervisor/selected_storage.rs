use super::*;

/// Managed mutable paths bound to the immutable generation selected by
/// `current-generation`.
///
/// This value retains a shared selection lease. Keep it alive for the entire
/// operation that uses its paths; cloning it extends the lease until every
/// clone is dropped.
#[derive(Debug, Clone)]
pub struct SelectedPeerStoragePaths {
    config_generation_id: String,
    storage_generation_id: String,
    storage_dir: PathBuf,
    snapshot_dir: PathBuf,
    _selection_lock: Arc<fs::File>,
}

/// Canonical peer paths validated while the caller holds either the shared
/// generation lease or the exclusive generation transaction.
#[derive(Debug)]
pub(super) struct ValidatedSelectedPeerStoragePaths {
    pub(super) config_generation_id: String,
    pub(super) storage_generation_id: String,
    pub(super) config_path: PathBuf,
    pub(super) storage_dir: PathBuf,
    pub(super) snapshot_dir: PathBuf,
}

impl PartialEq for SelectedPeerStoragePaths {
    fn eq(&self, other: &Self) -> bool {
        self.config_generation_id == other.config_generation_id
            && self.storage_generation_id == other.storage_generation_id
            && self.storage_dir == other.storage_dir
            && self.snapshot_dir == other.snapshot_dir
    }
}

impl Eq for SelectedPeerStoragePaths {}

impl SelectedPeerStoragePaths {
    /// Immutable configuration generation selected by `current-generation`.
    pub fn config_generation_id(&self) -> &str {
        &self.config_generation_id
    }

    /// Generation that owns the mutable storage selected by the peer config.
    pub fn storage_generation_id(&self) -> &str {
        &self.storage_generation_id
    }

    /// Selected mutable storage root for the peer.
    pub fn storage_dir(&self) -> &Path {
        &self.storage_dir
    }

    /// Selected snapshot root nested under the peer storage root.
    pub fn snapshot_dir(&self) -> &Path {
        &self.snapshot_dir
    }
}

/// Resolve a peer's mutable storage only after validating the published
/// immutable generation and every managed directory in the selected path.
///
/// `Ok(None)` means that the network root had no published generation at the
/// initial read. It is an instantaneous absence result and carries no retained
/// lease; callers that need a later answer must resolve again.
pub fn resolve_selected_peer_storage_paths(
    network_root: &Path,
    alias: &str,
) -> Result<Option<SelectedPeerStoragePaths>> {
    resolve_selected_peer_storage_paths_inner(network_root, alias, || {})
}

#[cfg(test)]
pub(super) fn resolve_selected_peer_storage_paths_with_hook<F>(
    network_root: &Path,
    alias: &str,
    after_initial_selection: F,
) -> Result<Option<SelectedPeerStoragePaths>>
where
    F: FnOnce(),
{
    resolve_selected_peer_storage_paths_inner(network_root, alias, after_initial_selection)
}

fn resolve_selected_peer_storage_paths_inner<F>(
    network_root: &Path,
    alias: &str,
    after_initial_selection: F,
) -> Result<Option<SelectedPeerStoragePaths>>
where
    F: FnOnce(),
{
    validate_peer_alias(alias)?;

    let Some(observed_generation_id) = current_generation_id(network_root)? else {
        return Ok(None);
    };
    after_initial_selection();
    let selection_lock = Arc::new(try_lock_generation_selection(network_root)?);
    let selected_after_lock = current_generation_id(network_root)?;
    if selected_after_lock.as_deref() != Some(observed_generation_id.as_str()) {
        return Err(SupervisorError::GenerationSelectionChanged {
            expected: Some(observed_generation_id),
            actual: selected_after_lock,
        });
    }
    let selected = verify_selected_generation(network_root, &observed_generation_id)?;
    let validated =
        validate_selected_peer_storage_paths_under_lock(network_root, alias, &selected)?;

    Ok(Some(SelectedPeerStoragePaths {
        config_generation_id: validated.config_generation_id,
        storage_generation_id: validated.storage_generation_id,
        storage_dir: validated.storage_dir,
        snapshot_dir: validated.snapshot_dir,
        _selection_lock: selection_lock,
    }))
}

fn validate_peer_alias(alias: &str) -> Result<()> {
    let alias_path = Path::new(alias);
    let mut alias_components = alias_path.components();
    if !matches!(
        alias_components.next(),
        Some(std::path::Component::Normal(component)) if !component.is_empty()
    ) || alias_components.next().is_some()
    {
        return Err(SupervisorError::GenerationValidation(format!(
            "peer alias `{alias}` is not one safe path component"
        )));
    }
    Ok(())
}

/// Strictly validate the selected immutable config and mutable peer hierarchy.
///
/// The caller must retain either a shared selection lease or the exclusive
/// generation transaction for the entire call. This helper deliberately does
/// not acquire a nested lock descriptor because dropping one duplicate can
/// release process-associated locks on some supported platforms.
pub(super) fn validate_selected_peer_storage_paths_under_lock(
    network_root: &Path,
    alias: &str,
    selected: &VerifiedGeneration,
) -> Result<ValidatedSelectedPeerStoragePaths> {
    validate_peer_alias(alias)?;
    let config_generation_id = selected.generation_id.clone();

    let immutable_peers = validate_selected_direct_child_directory(
        &selected.root,
        &selected.root.join("peers"),
        "selected immutable peers directory",
    )?;
    let immutable_peer = validate_selected_direct_child_directory(
        &immutable_peers,
        &immutable_peers.join(alias),
        "selected immutable peer directory",
    )?;
    let config_path = immutable_peer.join("config.toml");
    validate_selected_direct_child_file(&immutable_peer, &config_path, "selected peer config")?;
    let source = TomlSource::from_file(&config_path).map_err(|error| {
        SupervisorError::GenerationValidation(format!(
            "selected peer config `{}` failed reading: {error:?}",
            config_path.display()
        ))
    })?;
    let config = actual::Root::from_toml_source(source).map_err(|error| {
        SupervisorError::GenerationValidation(format!(
            "selected peer config `{}` failed parsing: {error:?}",
            config_path.display()
        ))
    })?;

    let generations = selected.root.parent().ok_or_else(|| {
        SupervisorError::GenerationValidation(
            "selected immutable generation has no generations parent".to_owned(),
        )
    })?;
    let canonical_network_root = generations.parent().ok_or_else(|| {
        SupervisorError::GenerationValidation(
            "selected immutable generation has no network root".to_owned(),
        )
    })?;
    let configured_snapshot = config.snapshot.store_dir.resolve_relative_path();
    let configured_snapshot_metadata =
        fs::symlink_metadata(&configured_snapshot).map_err(|error| {
            SupervisorError::GenerationValidation(format!(
                "selected peer config `{}` points at unavailable snapshot storage `{}`: {error}",
                config_path.display(),
                configured_snapshot.display()
            ))
        })?;
    if configured_snapshot_metadata.file_type().is_symlink()
        || !configured_snapshot_metadata.is_dir()
    {
        return Err(SupervisorError::GenerationValidation(format!(
            "selected peer config `{}` snapshot storage `{}` must be a non-symlink directory",
            config_path.display(),
            configured_snapshot.display()
        )));
    }
    let canonical_snapshot = fs::canonicalize(&configured_snapshot)?;
    let configured_storage = canonical_snapshot.parent().ok_or_else(|| {
        SupervisorError::GenerationValidation(format!(
            "selected peer config `{}` snapshot storage has no managed parent",
            config_path.display()
        ))
    })?;
    let relative_storage = configured_storage
        .strip_prefix(canonical_network_root)
        .map_err(|_| {
            SupervisorError::GenerationValidation(format!(
                "selected peer config `{}` storage `{}` escapes the network root",
                config_path.display(),
                configured_storage.display()
            ))
        })?;
    let mut storage_components = relative_storage.components();
    if !matches!(
        storage_components.next(),
        Some(std::path::Component::Normal(component)) if component == "peers"
    ) || !matches!(
        storage_components.next(),
        Some(std::path::Component::Normal(component)) if component == alias
    ) || !matches!(
        storage_components.next(),
        Some(std::path::Component::Normal(component)) if component == "storage-generations"
    ) {
        return Err(SupervisorError::GenerationValidation(format!(
            "selected peer config `{}` storage `{}` is outside the managed peer hierarchy",
            config_path.display(),
            configured_storage.display()
        )));
    }
    let storage_generation_id = match storage_components.next() {
        Some(std::path::Component::Normal(component)) => component.to_str().filter(|id| {
            id.len() == 32
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }),
        _ => None,
    }
    .filter(|_| storage_components.next().is_none())
    .ok_or_else(|| {
        SupervisorError::GenerationValidation(format!(
            "selected peer config `{}` storage `{}` has an unsafe generation id",
            config_path.display(),
            configured_storage.display()
        ))
    })?
    .to_owned();

    let storage_generation = verify_selected_generation(network_root, &storage_generation_id)?;
    if storage_generation.chain_id != selected.chain_id
        || storage_generation.chain_discriminant != selected.chain_discriminant
        || storage_generation.genesis_public_key != selected.genesis_public_key
        || storage_generation.expected_hash != selected.expected_hash
    {
        return Err(SupervisorError::GenerationValidation(format!(
            "selected config generation `{config_generation_id}` and storage generation `{storage_generation_id}` have different genesis metadata"
        )));
    }

    let runtime_peers = validate_selected_direct_child_directory(
        canonical_network_root,
        &canonical_network_root.join("peers"),
        "selected runtime peers directory",
    )?;
    let runtime_peer = validate_selected_direct_child_directory(
        &runtime_peers,
        &runtime_peers.join(alias),
        "selected runtime peer directory",
    )?;
    let storage_generations = validate_selected_direct_child_directory(
        &runtime_peer,
        &runtime_peer.join("storage-generations"),
        "selected storage-generations directory",
    )?;
    let storage_dir = validate_selected_direct_child_directory(
        &storage_generations,
        &storage_generations.join(&storage_generation_id),
        "selected peer storage directory",
    )?;
    let snapshot_dir = validate_selected_direct_child_directory(
        &storage_dir,
        &storage_dir.join("snapshot"),
        "selected peer snapshot directory",
    )?;
    if snapshot_dir != canonical_snapshot || storage_dir != configured_storage {
        return Err(SupervisorError::GenerationValidation(format!(
            "selected peer config `{}` snapshot path is not the canonical selected storage path",
            config_path.display()
        )));
    }
    let immutable_peer_count = fs::read_dir(&immutable_peers)?
        .map(|entry| entry.map(|entry| entry.path().join("config.toml").is_file()))
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .filter(|has_config| *has_config)
        .count();
    validate_managed_peer_paths_against(
        &config,
        &config_path,
        &storage_dir,
        &immutable_peer.join(MANAGED_RANS_TABLE_RELATIVE_PATH),
        immutable_peer_count,
    )?;
    for (label, path) in [
        ("selected Kura directory", storage_dir.join("kura")),
        ("selected Torii directory", storage_dir.join("torii")),
        (
            "selected Torii DA replay directory",
            storage_dir.join("torii/da_replay"),
        ),
        (
            "selected Torii DA manifest directory",
            storage_dir.join("torii/da_manifests"),
        ),
        ("selected SoraFS directory", storage_dir.join("sorafs")),
        (
            "selected streaming directory",
            storage_dir.join("streaming"),
        ),
        (
            "selected SoraNet route spool",
            storage_dir.join("streaming/soranet_routes"),
        ),
        (
            "selected SoraVPN route spool",
            storage_dir.join("streaming/soravpn_routes"),
        ),
    ] {
        validate_optional_managed_descendant_directory(&storage_dir, &path, label)?;
    }
    if immutable_peer_count > 1 {
        validate_optional_managed_descendant_file(
            &storage_dir,
            &storage_dir.join("soranet/ticket_revocations.norito"),
            "selected SoraNet revocation store",
        )?;
    }
    validate_required_managed_descendant_file(
        &immutable_peer,
        &immutable_peer.join(MANAGED_RANS_TABLE_RELATIVE_PATH),
        "selected rANS table",
    )?;

    Ok(ValidatedSelectedPeerStoragePaths {
        config_generation_id,
        storage_generation_id,
        config_path: fs::canonicalize(config_path)?,
        storage_dir,
        snapshot_dir,
    })
}

fn validate_selected_direct_child_directory(
    parent: &Path,
    path: &Path,
    label: &str,
) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        SupervisorError::GenerationValidation(format!(
            "{label} `{}` is unavailable: {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(SupervisorError::GenerationValidation(format!(
            "{label} `{}` must be a non-symlink directory",
            path.display()
        )));
    }
    let canonical_parent = fs::canonicalize(parent)?;
    let canonical = fs::canonicalize(path)?;
    if canonical.parent() != Some(canonical_parent.as_path()) {
        return Err(SupervisorError::GenerationValidation(format!(
            "{label} `{}` escapes its managed parent",
            path.display()
        )));
    }
    Ok(canonical)
}

fn validate_selected_direct_child_file(parent: &Path, path: &Path, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        SupervisorError::GenerationValidation(format!(
            "{label} `{}` is unavailable: {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(SupervisorError::GenerationValidation(format!(
            "{label} `{}` must be a regular non-symlink file",
            path.display()
        )));
    }
    let canonical_parent = fs::canonicalize(parent)?;
    let canonical = fs::canonicalize(path)?;
    if canonical.parent() != Some(canonical_parent.as_path()) {
        return Err(SupervisorError::GenerationValidation(format!(
            "{label} `{}` escapes its managed parent",
            path.display()
        )));
    }
    Ok(())
}

fn validate_optional_managed_descendant_directory(
    root: &Path,
    path: &Path,
    label: &str,
) -> Result<()> {
    validate_managed_descendant(root, path, label, false, false)
}

fn validate_optional_managed_descendant_file(root: &Path, path: &Path, label: &str) -> Result<()> {
    validate_managed_descendant(root, path, label, true, false)
}

fn validate_required_managed_descendant_file(root: &Path, path: &Path, label: &str) -> Result<()> {
    validate_managed_descendant(root, path, label, true, true)
}

fn validate_managed_descendant(
    root: &Path,
    path: &Path,
    label: &str,
    leaf_is_file: bool,
    required: bool,
) -> Result<()> {
    let canonical_root = fs::canonicalize(root)?;
    let relative = path.strip_prefix(root).map_err(|_| {
        SupervisorError::GenerationValidation(format!(
            "{label} `{}` escapes its managed root `{}`",
            path.display(),
            root.display()
        ))
    })?;
    let components = relative.components().collect::<Vec<_>>();
    if components.is_empty()
        || components
            .iter()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(SupervisorError::GenerationValidation(format!(
            "{label} `{}` is not a safe managed descendant",
            path.display()
        )));
    }

    let mut parent = canonical_root;
    for (index, component) in components.iter().enumerate() {
        let child = parent.join(component.as_os_str());
        let is_leaf = index + 1 == components.len();
        match fs::symlink_metadata(&child) {
            Ok(_) if is_leaf && leaf_is_file => {
                validate_selected_direct_child_file(&parent, &child, label)?;
            }
            Ok(_) => {
                parent = validate_selected_direct_child_directory(&parent, &child, label)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound && !required => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(SupervisorError::GenerationValidation(format!(
                    "{label} `{}` is unavailable: {error}",
                    path.display()
                )));
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
