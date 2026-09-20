//! Retained ownership of the exact filesystem operations for one lane transition.

use super::*;
use crate::secure_file_metadata::{self, SecureMetadata};
use std::{
    io::ErrorKind,
    sync::{Arc, OnceLock},
};

#[derive(Debug)]
struct Directory {
    file: fs::File,
    identity: SecureMetadata,
}

impl Directory {
    fn capture(path: &Path) -> Result<Self> {
        let before = secure_file_metadata::from_path(path)?;
        eyre::ensure!(
            before.is_dir() && !before.file_type().is_symlink(),
            "tiered-state: expected an original directory at {}",
            path.display()
        );
        let file = open_directory(path)?;
        let identity = secure_file_metadata::from_file(&file)?;
        let after = secure_file_metadata::from_path(path)?;
        eyre::ensure!(
            same_identity(&before, &identity) && same_identity(&identity, &after),
            "tiered-state: directory changed during capture at {}",
            path.display()
        );
        Ok(Self { file, identity })
    }

    fn authenticate(&self, path: &Path) -> Result<()> {
        let named = secure_file_metadata::from_path(path)?;
        eyre::ensure!(
            named.is_dir()
                && !named.file_type().is_symlink()
                && same_identity(&self.identity, &named)
                && same_identity(
                    &self.identity,
                    &secure_file_metadata::from_file(&self.file)?
                ),
            "tiered-state: original directory was replaced at {}",
            path.display()
        );
        Ok(())
    }
}

#[cfg(unix)]
fn same_identity(left: &SecureMetadata, right: &SecureMetadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(windows)]
fn same_identity(left: &SecureMetadata, right: &SecureMetadata) -> bool {
    left.volume_serial_number().is_some()
        && left.file_index().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
}

#[cfg(not(any(unix, windows)))]
fn same_identity(_left: &SecureMetadata, _right: &SecureMetadata) -> bool {
    false
}

#[cfg(unix)]
fn open_directory(path: &Path) -> std::io::Result<fs::File> {
    rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(fs::File::from)
    .map_err(std::io::Error::from)
}

#[cfg(windows)]
fn open_directory(path: &Path) -> std::io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(0x0200_0000 | 0x0020_0000)
        .open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_directory(_path: &Path) -> std::io::Result<fs::File> {
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "tiered geometry requires stable directory identities",
    ))
}

// A planned directory has one assignment, performed immediately after its mkdir.
// Every subsequent operation refers to this same physical custody object.
type DirectoryRef = Arc<OnceLock<Directory>>;

fn captured_directory(path: &Path) -> Result<DirectoryRef> {
    Ok(Arc::new(OnceLock::from(Directory::capture(path)?)))
}

fn directory(reference: &DirectoryRef) -> Result<&Directory> {
    reference.get().ok_or_else(|| {
        eyre::eyre!("tiered-state: directory creation requires recovery before publication")
    })
}

#[derive(Clone, Debug)]
struct Parent {
    path: PathBuf,
    directory: DirectoryRef,
}

impl Parent {
    fn authenticate(&self) -> Result<&Directory> {
        let owned = directory(&self.directory)?;
        owned.authenticate(&self.path)?;
        Ok(owned)
    }

    fn sync(&self, faults: &mut SyncFault) -> Result<()> {
        let owned = self.authenticate()?;
        faults.before_sync()?;
        owned.file.sync_all().wrap_err_with(|| {
            format!(
                "failed to sync tiered geometry directory {}",
                self.path.display()
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationPhase {
    Pending,
    Changed,
    DestinationSynced,
    Applied,
    Reversed,
    ReverseDestinationSynced,
    RolledBack,
}

#[derive(Debug)]
enum OperationKind {
    Create {
        path: PathBuf,
        parent: Parent,
        created: DirectoryRef,
    },
    Rename {
        source: PathBuf,
        target: PathBuf,
        source_parent: Parent,
        target_parent: Parent,
        original: DirectoryRef,
    },
}

#[derive(Debug)]
struct Operation {
    kind: OperationKind,
    phase: OperationPhase,
}

impl Operation {
    fn resume(&mut self, faults: &mut SyncFault) -> Result<()> {
        match &self.kind {
            OperationKind::Create {
                path,
                parent,
                created,
            } => {
                if self.phase == OperationPhase::Pending {
                    let parent_directory = parent.authenticate()?;
                    ensure_absent(path)?;
                    create_directory_at(&parent_directory.file, leaf(path)?)?;
                    // Remember that mkdir succeeded even if custody acquisition fails.
                    // Such a failure cannot justify adopting a later same-name object.
                    self.phase = OperationPhase::Changed;
                    created.set(Directory::capture(path)?).map_err(|_| {
                        eyre::eyre!("tiered-state: directory custody was already assigned")
                    })?;
                }
                if self.phase == OperationPhase::Changed {
                    directory(created)?.authenticate(path)?;
                    parent.sync(faults)?;
                    self.phase = OperationPhase::Applied;
                }
            }
            OperationKind::Rename {
                source,
                target,
                source_parent,
                target_parent,
                original,
            } => {
                if self.phase == OperationPhase::Pending {
                    let from = source_parent.authenticate()?;
                    let to = target_parent.authenticate()?;
                    directory(original)?.authenticate(source)?;
                    ensure_absent(target)?;
                    rename_directory_at(&from.file, leaf(source)?, &to.file, leaf(target)?)?;
                    self.phase = OperationPhase::Changed;
                }
                if matches!(
                    self.phase,
                    OperationPhase::Changed | OperationPhase::DestinationSynced
                ) {
                    directory(original)?.authenticate(target)?;
                    ensure_absent(source)?;
                    source_parent.authenticate()?;
                    target_parent.authenticate()?;
                }
                if self.phase == OperationPhase::Changed {
                    target_parent.sync(faults)?;
                    self.phase = OperationPhase::DestinationSynced;
                }
                if self.phase == OperationPhase::DestinationSynced {
                    if source_parent.path != target_parent.path {
                        source_parent.sync(faults)?;
                    }
                    self.phase = OperationPhase::Applied;
                }
            }
        }
        eyre::ensure!(
            self.phase == OperationPhase::Applied,
            "tiered-state: rolled-back geometry cannot be resumed"
        );
        Ok(())
    }

    fn rollback(&mut self, faults: &mut SyncFault) -> Result<()> {
        if self.phase == OperationPhase::Pending {
            self.phase = OperationPhase::RolledBack;
        }
        if self.phase == OperationPhase::RolledBack {
            return Ok(());
        }
        match &self.kind {
            OperationKind::Create {
                path,
                parent,
                created,
            } => {
                if matches!(
                    self.phase,
                    OperationPhase::Changed | OperationPhase::Applied
                ) {
                    let parent_directory = parent.authenticate()?;
                    directory(created)?.authenticate(path)?;
                    // remove_dir refuses populated replacements; never recursively delete.
                    remove_directory_at(&parent_directory.file, leaf(path)?)?;
                    self.phase = OperationPhase::Reversed;
                }
                if self.phase == OperationPhase::Reversed {
                    ensure_absent(path)?;
                    parent.sync(faults)?;
                    self.phase = OperationPhase::RolledBack;
                }
            }
            OperationKind::Rename {
                source,
                target,
                source_parent,
                target_parent,
                original,
            } => {
                if matches!(
                    self.phase,
                    OperationPhase::Changed
                        | OperationPhase::DestinationSynced
                        | OperationPhase::Applied
                ) {
                    let from = target_parent.authenticate()?;
                    let to = source_parent.authenticate()?;
                    directory(original)?.authenticate(target)?;
                    ensure_absent(source)?;
                    rename_directory_at(&from.file, leaf(target)?, &to.file, leaf(source)?)?;
                    self.phase = OperationPhase::Reversed;
                }
                if matches!(
                    self.phase,
                    OperationPhase::Reversed | OperationPhase::ReverseDestinationSynced
                ) {
                    directory(original)?.authenticate(source)?;
                    ensure_absent(target)?;
                    source_parent.authenticate()?;
                    target_parent.authenticate()?;
                }
                if self.phase == OperationPhase::Reversed {
                    source_parent.sync(faults)?;
                    self.phase = OperationPhase::ReverseDestinationSynced;
                }
                if self.phase == OperationPhase::ReverseDestinationSynced {
                    if source_parent.path != target_parent.path {
                        target_parent.sync(faults)?;
                    }
                    self.phase = OperationPhase::RolledBack;
                }
            }
        }
        eyre::ensure!(
            self.phase == OperationPhase::RolledBack,
            "tiered-state: geometry operation has no owned rollback"
        );
        Ok(())
    }
}

fn leaf(path: &Path) -> Result<&std::ffi::OsStr> {
    path.file_name().ok_or_else(|| {
        eyre::eyre!("tiered-state: geometry operation cannot replace a filesystem root")
    })
}

fn ensure_absent(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => Err(eyre::eyre!(
            "tiered-state: geometry destination is occupied: {}",
            path.display()
        )),
    }
}

#[cfg(unix)]
fn create_directory_at(parent: &fs::File, name: &std::ffi::OsStr) -> std::io::Result<()> {
    rustix::fs::mkdirat(parent, name, rustix::fs::Mode::from_raw_mode(0o755))
        .map_err(std::io::Error::from)
}

#[cfg(unix)]
fn remove_directory_at(parent: &fs::File, name: &std::ffi::OsStr) -> std::io::Result<()> {
    rustix::fs::unlinkat(parent, name, rustix::fs::AtFlags::REMOVEDIR).map_err(std::io::Error::from)
}

#[cfg(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox"
))]
fn rename_directory_at(
    source_parent: &fs::File,
    source: &std::ffi::OsStr,
    target_parent: &fs::File,
    target: &std::ffi::OsStr,
) -> std::io::Result<()> {
    rustix::fs::renameat_with(
        source_parent,
        source,
        target_parent,
        target,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)
}

#[cfg(not(unix))]
fn create_directory_at(_parent: &fs::File, _name: &std::ffi::OsStr) -> std::io::Result<()> {
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "descriptor-relative tiered geometry creation is unavailable",
    ))
}

#[cfg(not(unix))]
fn remove_directory_at(_parent: &fs::File, _name: &std::ffi::OsStr) -> std::io::Result<()> {
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "descriptor-relative tiered geometry removal is unavailable",
    ))
}

#[cfg(not(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox"
)))]
fn rename_directory_at(
    _source_parent: &fs::File,
    _source: &std::ffi::OsStr,
    _target_parent: &fs::File,
    _target: &std::ffi::OsStr,
) -> std::io::Result<()> {
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "atomic descriptor-relative tiered geometry rename is unavailable",
    ))
}

#[derive(Debug, Default)]
struct SyncFault {
    #[cfg(test)]
    remaining: Option<usize>,
}

impl SyncFault {
    fn before_sync(&mut self) -> Result<()> {
        #[cfg(test)]
        if let Some(remaining) = &mut self.remaining {
            if *remaining == 0 {
                self.remaining = None;
                return Err(eyre::eyre!(
                    "injected tiered geometry directory sync failure"
                ));
            }
            *remaining -= 1;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct RootBinding {
    configured: PathBuf,
    resolved: PathBuf,
    directory: DirectoryRef,
    parent: Parent,
}

/// Move-only original operation plan, including partial rename/sync and reverse progress.
///
/// No filesystem mutation occurs while preparing this owner. The caller must retain it
/// until publication or completed rollback; a failed resume never chooses new paths.
#[derive(Debug)]
pub(crate) struct TieredGeometryAttempt {
    enabled: bool,
    cold_store_root: Option<PathBuf>,
    da_store_root: Option<PathBuf>,
    roots: Vec<RootBinding>,
    operations: Vec<Operation>,
    original_paths: BTreeMap<PathBuf, Option<DirectoryRef>>,
    final_paths: BTreeMap<PathBuf, Option<DirectoryRef>>,
    cursor: usize,
    rollback_cursor: Option<usize>,
    applied: bool,
    rolled_back: bool,
    faults: SyncFault,
}

impl TieredGeometryAttempt {
    /// Check retained roots without effects before another owned storage step.
    pub(crate) fn authenticate_backend(&self, backend: &TieredStateBackend) -> Result<()> {
        eyre::ensure!(
            self.enabled == backend.enabled
                && self.cold_store_root == backend.cold_store_root
                && self.da_store_root == backend.da_store_root,
            "tiered-state: retained geometry belongs to different backend roots"
        );
        for root in &self.roots {
            eyre::ensure!(
                resolve_path(&root.configured)? == root.resolved,
                "tiered-state: configured root changed physical location: {}",
                root.configured.display()
            );
            root.parent.authenticate()?;
            let removed = self.operations.iter().any(|operation| {
                matches!(&operation.kind, OperationKind::Create { path, .. } if path == &root.resolved)
                    && matches!(operation.phase, OperationPhase::Reversed | OperationPhase::RolledBack)
            });
            if removed || root.directory.get().is_none() {
                ensure_absent(&root.resolved)?;
            } else {
                directory(&root.directory)?.authenticate(&root.resolved)?;
            }
        }
        Ok(())
    }

    /// Resume only the exact retained operation and its remaining directory syncs.
    pub(crate) fn resume(&mut self, backend: &mut TieredStateBackend) -> Result<()> {
        self.applied = false;
        self.authenticate_backend(backend)?;
        eyre::ensure!(
            self.rollback_cursor.is_none(),
            "tiered-state: geometry rollback has already started"
        );
        while self.cursor < self.operations.len() {
            self.operations[self.cursor].resume(&mut self.faults)?;
            self.cursor += 1;
        }
        authenticate_paths(&self.final_paths)?;
        self.applied = true;
        Ok(())
    }

    /// Reauthenticate completed physical effects without changing retained progress.
    /// A temporary identity refusal must not reopen a published operation plan.
    pub(crate) fn authenticate_applied(&self, backend: &TieredStateBackend) -> Result<()> {
        eyre::ensure!(
            self.applied && self.rollback_cursor.is_none() && self.cursor == self.operations.len(),
            "tiered-state: retained geometry has not completed its forward operations"
        );
        self.authenticate_backend(backend)?;
        authenticate_paths(&self.final_paths)
    }

    /// Reverse only this owner's completed effects, retaining partial reverse progress.
    pub(crate) fn rollback(&mut self, backend: &mut TieredStateBackend) -> Result<()> {
        self.applied = false;
        self.rolled_back = false;
        self.authenticate_backend(backend)?;
        let mut cursor = *self.rollback_cursor.get_or_insert(self.operations.len());
        while cursor > 0 {
            self.operations[cursor - 1].rollback(&mut self.faults)?;
            cursor -= 1;
            self.rollback_cursor = Some(cursor);
        }
        authenticate_paths(&self.original_paths)?;
        self.rolled_back = true;
        Ok(())
    }

    /// Whether all original forward operations and their syncs have completed.
    pub(crate) fn is_applied(&self) -> bool {
        self.applied
    }

    /// Whether all effects owned by this attempt have been reversed and synced.
    #[cfg(test)]
    pub(crate) fn is_rolled_back(&self) -> bool {
        self.rolled_back
    }
}

#[derive(Default)]
struct Plan {
    original_paths: BTreeMap<PathBuf, Option<DirectoryRef>>,
    paths: BTreeMap<PathBuf, Option<DirectoryRef>>,
    operations: Vec<Operation>,
}

impl Plan {
    fn lookup(&mut self, path: &Path) -> Result<Option<DirectoryRef>> {
        if let Some(found) = self.paths.get(path) {
            return Ok(found.clone());
        }
        let found = match fs::symlink_metadata(path) {
            Ok(_) => Some(captured_directory(path)?),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        self.original_paths
            .insert(path.to_path_buf(), found.clone());
        self.paths.insert(path.to_path_buf(), found.clone());
        Ok(found)
    }

    fn ensure(&mut self, path: &Path) -> Result<DirectoryRef> {
        if let Some(found) = self.lookup(path)? {
            return Ok(found);
        }
        let parent = self.parent(path)?;
        let created = Arc::new(OnceLock::new());
        self.operations.push(Operation {
            kind: OperationKind::Create {
                path: path.to_path_buf(),
                parent,
                created: Arc::clone(&created),
            },
            phase: OperationPhase::Pending,
        });
        self.paths
            .insert(path.to_path_buf(), Some(Arc::clone(&created)));
        Ok(created)
    }

    fn parent(&mut self, path: &Path) -> Result<Parent> {
        let path = path.parent().ok_or_else(|| {
            eyre::eyre!("tiered-state: geometry operation has no parent directory")
        })?;
        Ok(Parent {
            path: path.to_path_buf(),
            directory: self.ensure(path)?,
        })
    }

    fn existing_anchor(&mut self, path: &Path) -> Result<Parent> {
        for ancestor in path.ancestors().skip(1) {
            if let Some(directory) = self.lookup(ancestor)?
                && directory.get().is_some()
            {
                return Ok(Parent {
                    path: ancestor.to_path_buf(),
                    directory,
                });
            }
        }
        Err(eyre::eyre!(
            "tiered-state: geometry root has no original directory anchor"
        ))
    }

    fn rename(&mut self, source: &Path, target: &Path) -> Result<()> {
        if source == target {
            return Ok(());
        }
        let Some(original) = self.lookup(source)? else {
            return Ok(());
        };
        eyre::ensure!(
            self.lookup(target)?.is_none(),
            "tiered-state: lane snapshot rename target already exists: {}",
            target.display()
        );
        let source_parent = self.parent(source)?;
        let target_parent = self.parent(target)?;
        self.operations.push(Operation {
            kind: OperationKind::Rename {
                source: source.to_path_buf(),
                target: target.to_path_buf(),
                source_parent,
                target_parent,
                original: Arc::clone(&original),
            },
            phase: OperationPhase::Pending,
        });
        self.paths.insert(source.to_path_buf(), None);
        self.paths.insert(target.to_path_buf(), Some(original));
        Ok(())
    }

    fn retire(&mut self, root: &Path, lanes_root: &Path, entry: &LaneConfigEntry) -> Result<()> {
        let source = lane_snapshot_dir(lanes_root, entry);
        if self.lookup(&source)?.is_none() {
            return Ok(());
        }
        let retired_root = root.join("retired").join("lanes");
        self.ensure(&retired_root)?;
        let base = unique_retired_lane_path(&retired_root, &entry.kura_segment);
        let mut destination = base.clone();
        let mut suffix = 0usize;
        while self.lookup(&destination)?.is_some() {
            suffix = suffix
                .checked_add(1)
                .ok_or_else(|| eyre::eyre!("tiered-state: exhausted retirement path candidates"))?;
            destination = PathBuf::from(format!("{}_{}", base.display(), suffix));
        }
        self.rename(&source, &destination)
    }
}

fn authenticate_paths(paths: &BTreeMap<PathBuf, Option<DirectoryRef>>) -> Result<()> {
    for (path, expected) in paths {
        if let Some(expected) = expected {
            directory(expected)?.authenticate(path)?;
        } else {
            ensure_absent(path)?;
        }
    }
    Ok(())
}

fn resolve_path(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut ancestor = absolute.as_path();
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                suffix.push(leaf(ancestor)?.to_owned());
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| eyre::eyre!("tiered-state: root has no existing ancestor"))?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    let mut resolved = fs::canonicalize(ancestor)?;
    for component in suffix.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

impl TieredStateBackend {
    /// Capture one lane transition's exact directory plan without filesystem effects.
    pub(crate) fn prepare_lane_geometry_attempt(
        &self,
        previous: &LaneConfig,
        current: &LaneConfig,
        replacements: &[(&LaneConfigEntry, &LaneConfigEntry)],
        relabelled: &[(&LaneConfigEntry, &LaneConfigEntry)],
    ) -> Result<TieredGeometryAttempt> {
        self.preflight_lane_geometry(previous, current, replacements, relabelled)?;
        let mut attempt = TieredGeometryAttempt {
            enabled: self.enabled,
            cold_store_root: self.cold_store_root.clone(),
            da_store_root: self.da_store_root.clone(),
            roots: Vec::new(),
            operations: Vec::new(),
            original_paths: BTreeMap::new(),
            final_paths: BTreeMap::new(),
            cursor: 0,
            rollback_cursor: None,
            applied: false,
            rolled_back: false,
            faults: SyncFault::default(),
        };
        if !self.enabled {
            return Ok(attempt);
        }
        let Some(root) = self.primary_cold_root() else {
            return Ok(attempt);
        };
        let root = resolve_path(root)?;
        let mut plan = Plan::default();
        for configured in self.cold_store_root.iter().chain(self.da_store_root.iter()) {
            let resolved = resolve_path(configured)?;
            let directory = plan.ensure(&resolved)?;
            let parent = plan.existing_anchor(&resolved)?;
            attempt.roots.push(RootBinding {
                configured: configured.clone(),
                resolved,
                directory,
                parent,
            });
        }
        let lanes_root = root.join("lanes");
        plan.ensure(&lanes_root)?;
        let previous_map: BTreeMap<_, _> = previous
            .entries()
            .iter()
            .map(|entry| (entry.lane_id, entry))
            .collect();
        let current_map: BTreeMap<_, _> = current
            .entries()
            .iter()
            .map(|entry| (entry.lane_id, entry))
            .collect();
        let replacement_ids: BTreeSet<_> = replacements
            .iter()
            .map(|(_, current)| current.lane_id)
            .collect();
        for (previous, _) in replacements {
            plan.retire(&root, &lanes_root, previous)?;
        }
        for entry in current.entries() {
            if replacement_ids.contains(&entry.lane_id) {
                continue;
            }
            let target = lane_snapshot_dir(&lanes_root, entry);
            if plan.lookup(&target)?.is_some() {
                continue;
            }
            if let Some(previous) = previous_map.get(&entry.lane_id)
                && plan
                    .lookup(&lane_snapshot_dir(&lanes_root, previous))?
                    .is_some()
            {
                continue;
            }
            plan.ensure(&target)?;
        }
        for (_, current) in replacements {
            plan.ensure(&lane_snapshot_dir(&lanes_root, current))?;
        }
        for (id, entry) in previous_map {
            if !current_map.contains_key(&id) && !replacement_ids.contains(&id) {
                plan.retire(&root, &lanes_root, entry)?;
            }
        }
        for (previous, current) in relabelled {
            plan.rename(
                &lane_snapshot_dir(&lanes_root, previous),
                &lane_snapshot_dir(&lanes_root, current),
            )?;
        }
        attempt.operations = plan.operations;
        attempt.original_paths = plan.original_paths;
        attempt.final_paths = plan.paths;
        Ok(attempt)
    }
}

#[cfg(test)]
mod tests;
