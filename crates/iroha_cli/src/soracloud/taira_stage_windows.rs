//! Native custody for portable Taira workspace construction and read-only stage verification.

use super::*;
use iroha_fs::{
    FileMetadata, FileSnapshot, OwnerDirectory, PrivateDirectory, ReaderDirectory, RetainedFile,
};
use std::io::{Seek as _, SeekFrom};

/// A captured source keeps its original and complete native snapshot through streaming.
#[derive(Debug)]
pub(super) struct Source {
    path: PathBuf,
    original: RetainedFile,
    metadata: FileMetadata,
    snapshot: FileSnapshot,
}

impl Source {
    fn capture(path: &Path, original: RetainedFile) -> Result<Self> {
        let metadata = FileMetadata::of(original.file())?;
        let snapshot = original.snapshot()?;
        if metadata.identity() != original.identity()? || metadata.links() != 1 {
            return Err(eyre!("Taira source identity changed during capture"));
        }
        original.revalidate()?;
        Ok(Self {
            path: path.to_owned(),
            original,
            metadata,
            snapshot,
        })
    }

    pub(super) fn len(&self) -> u64 {
        self.metadata.len()
    }

    fn revalidate(&self) -> Result<()> {
        if self.original.snapshot()? != self.snapshot {
            return Err(eyre!(
                "Taira source {} changed after capture",
                self.path.display()
            ));
        }
        self.original.revalidate()?;
        Ok(())
    }
}

fn join_check<T>(result: Result<T>, check: Result<()>) -> Result<T> {
    match (result, check) {
        (Err(first), Err(final_error)) => Err(first)
            .wrap_err_with(|| format!("Taira final native custody also refused: {final_error:#}")),
        (Err(first), Ok(())) => Err(first),
        (Ok(_), Err(error)) => Err(error),
        (Ok(value), Ok(())) => Ok(value),
    }
}

pub(super) fn finish_directory<T>(directory: &PrivateDirectory, result: Result<T>) -> Result<T> {
    join_check(result, directory.revalidate().map_err(Into::into))
}

pub(super) fn finish_sources<T>(sources: &[Source], mut result: Result<T>) -> Result<T> {
    for source in sources {
        result = join_check(result, source.revalidate());
    }
    result
}

pub(super) fn finish_workspace<T>(
    directory: &PrivateDirectory,
    sources: [&Source; 3],
    mut result: Result<T>,
) -> Result<T> {
    for source in sources {
        result = join_check(result, source.revalidate());
    }
    finish_directory(directory, result).wrap_err(
        "Taira workspace verification; a failed native creation may leave private partial output (no pathname cleanup)",
    )
}

pub(super) fn require_distinct_sources(sources: [&Source; 3]) -> Result<()> {
    let identities = sources.map(|source| source.metadata.identity());
    if identities[0] == identities[1]
        || identities[0] == identities[2]
        || identities[1] == identities[2]
    {
        return Err(eyre!(
            "Taira AArch64 kernel, rootfs, and initrd sources must be three distinct files"
        ));
    }
    Ok(())
}

fn absolute_route(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(eyre!(
            "Taira native path must be nonempty and contain no parent traversal"
        ));
    }
    Ok(if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    })
}

pub(super) fn create_taira_owner_only_directory(
    path: &Path,
    description: &str,
) -> Result<PrivateDirectory> {
    let path = absolute_route(path)?;
    let name = path
        .file_name()
        .ok_or_else(|| eyre!("{description} must name one concrete directory"))?;
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("{description} must have a parent"))?;
    let parent = OwnerDirectory::open(parent)?;
    let result = parent.create_private_child(name).map_err(Into::into);
    let child =
        join_check(result, parent.revalidate().map_err(Into::into)).wrap_err_with(|| {
            format!(
                "create fresh {description} {}; existing directories are never reused",
                path.display()
            )
        })?;
    child.revalidate()?;
    Ok(child)
}

pub(super) fn create_taira_owner_only_subdirectory(
    path: &Path,
    description: &str,
) -> Result<PrivateDirectory> {
    create_taira_owner_only_directory(path, description)
}

pub(super) fn create_taira_stage_member_parent(
    root: &Path,
    logical_path: &[String],
) -> Result<(PathBuf, PrivateDirectory)> {
    let mut directory = PrivateDirectory::open(root)?;
    let mut current = root.to_owned();
    for component in logical_path
        .iter()
        .take(logical_path.len().saturating_sub(1))
    {
        directory = directory.ensure_child(component)?;
        current.push(component);
    }
    directory.revalidate()?;
    Ok((current, directory))
}

pub(super) fn write_taira_stage_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut original = RetainedFile::create_new_private(path)
        .wrap_err_with(|| format!("create Taira Inrou staged file {}", path.display()))?;
    let result = (|| -> Result<()> {
        original.file_mut().write_all(bytes)?;
        original.file().sync_all()?;
        Ok(())
    })();
    join_check(result, original.revalidate().map_err(Into::into))?;
    let sealed = original.seal()?;
    sealed.revalidate()?;
    Ok(())
}

pub(super) fn set_taira_stage_permissions(path: &Path, mode: u32) -> Result<()> {
    // Native objects are private from creation. Existing objects are validated, never repaired.
    match mode {
        0o700 => validate_taira_stage_owned_entry(path, true, "Taira private directory"),
        0o400 | 0o600 => validate_taira_stage_owned_entry(path, false, "Taira private file"),
        _ => Err(eyre!(
            "Unix executable or other mode transitions are not a native Taira staging operation"
        )),
    }
}

pub(super) fn taira_inrou_workspace_source_metadata(
    path: &Path,
    description: &str,
) -> Result<Source> {
    let original = RetainedFile::open_regular(path)
        .wrap_err_with(|| format!("retain {description} {}", path.display()))?;
    let source = Source::capture(path, original)?;
    if source.len() == 0 {
        return Err(eyre!("{description} {} must not be empty", path.display()));
    }
    Ok(source)
}

pub(super) fn taira_direct_member_metadata(
    root: &Path,
    logical_path: &[String],
    description: &str,
) -> Result<Source> {
    let (leaf, parents) = logical_path
        .split_last()
        .ok_or_else(|| eyre!("{description} path must not be empty"))?;
    let mut directory = ReaderDirectory::open(root)?;
    for component in parents {
        directory = directory.open_child(component)?;
    }
    let original = directory.open_retained_regular(leaf)?;
    let source = Source::capture(&root.join(logical_path.join("/")), original)?;
    directory.revalidate()?;
    Ok(source)
}

pub(super) fn copy_taira_stage_source_file(
    source_path: &Path,
    destination_path: &Path,
    captured: &Source,
) -> Result<u64> {
    if source_path != captured.path.as_path() {
        return Err(eyre!("Taira copy must use its captured source route"));
    }
    captured.revalidate()?;
    let mut destination =
        RetainedFile::create_new_private(destination_path).wrap_err_with(|| {
            format!(
                "create Taira guest-image staged file {}",
                destination_path.display()
            )
        })?;
    let result = (|| -> Result<u64> {
        let limit = captured
            .len()
            .checked_add(1)
            .ok_or_else(|| eyre!("Taira guest-image source length overflow"))?;
        let mut source = captured.original.file();
        source.seek(SeekFrom::Start(0))?;
        let mut bounded = io::Read::take(&mut source, limit);
        let copied = io::copy(&mut bounded, destination.file_mut())?;
        destination.file().sync_all()?;
        if copied != captured.len() || FileMetadata::of(destination.file())?.len() != copied {
            return Err(eyre!(
                "Taira guest-image source {} changed during descriptor-bound copy",
                source_path.display()
            ));
        }
        Ok(copied)
    })();
    let result = join_check(result, captured.revalidate());
    let copied = join_check(result, destination.revalidate().map_err(Into::into))?;
    let sealed = destination.seal()?;
    sealed.revalidate()?;
    captured.revalidate()?;
    Ok(copied)
}

fn read_bytes(path: &Path, description: &str, maximum: u64, private: bool) -> Result<Vec<u8>> {
    let bound = usize::try_from(maximum).wrap_err("Taira read bound exceeds host width")?;
    let result = if private {
        iroha_fs::read_private(path, bound)
    } else {
        iroha_fs::read_regular(path, bound)
    };
    let mut bytes = result.wrap_err_with(|| {
        format!(
            "read {description} {}; maximum is {maximum}",
            path.display()
        )
    })?;
    if bytes.is_empty() {
        return Err(eyre!("{description} {} must not be empty", path.display()));
    }
    Ok(std::mem::take(&mut *bytes))
}

pub(super) fn taira_stage_regular_file_bytes(
    path: &Path,
    description: &str,
    max_bytes: u64,
) -> Result<Vec<u8>> {
    read_bytes(path, description, max_bytes, false)
}

pub(super) fn taira_stage_owned_file_bytes(
    path: &Path,
    description: &str,
    max_bytes: u64,
) -> Result<Vec<u8>> {
    read_bytes(path, description, max_bytes, true)
}

pub(super) fn validate_taira_stage_owned_entry(
    path: &Path,
    directory: bool,
    description: &str,
) -> Result<()> {
    if directory {
        PrivateDirectory::open(path).and_then(|owner| owner.revalidate())
    } else {
        RetainedFile::open_private(path).and_then(|owner| owner.revalidate())
    }
    .wrap_err_with(|| format!("validate native private {description} {}", path.display()))
}

pub(super) fn validate_taira_inrou_workspace_entry(
    path: &Path,
    directory: bool,
    description: &str,
) -> Result<fs::Metadata> {
    if directory {
        let owner = PrivateDirectory::open(path)?;
        let metadata = FileMetadata::symlink_metadata(path)?;
        if !metadata.is_dir() || metadata.identity() != owner.identity()? {
            return Err(eyre!("{description} directory original changed"));
        }
        owner.revalidate()?;
        Ok(metadata.metadata().clone())
    } else {
        let owner = RetainedFile::open_private(path)?;
        let metadata = FileMetadata::of(owner.file())?;
        owner.revalidate()?;
        Ok(metadata.metadata().clone())
    }
}

pub(super) fn validate_taira_inrou_workspace_members(
    directory: &Path,
    expected: &[&str],
) -> Result<()> {
    let owner = PrivateDirectory::open(directory)?;
    let result = (|| -> Result<()> {
        let actual = owner
            .entries(expected.len())?
            .into_iter()
            .map(|name| {
                name.into_string()
                    .map_err(|_| eyre!("Taira workspace has a non-UTF-8 member"))
            })
            .collect::<Result<BTreeSet<_>>>()?;
        let expected = expected
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(eyre!(
                "Taira Inrou workspace {} must contain exactly {expected:?}, found {actual:?}",
                directory.display()
            ));
        }
        Ok(())
    })();
    finish_directory(&owner, result)
}

pub(super) fn taira_inrou_source_workspace(
    container_path: &Path,
    service_path: &Path,
    bundle_file: &Path,
) -> Result<PathBuf> {
    let mut roots = Vec::new();
    let mut originals = Vec::new();
    for path in [container_path, service_path, bundle_file] {
        let route = absolute_route(path)?;
        let parent = route
            .parent()
            .ok_or_else(|| eyre!("Taira Inrou source has no workspace"))?;
        let root = ReaderDirectory::open(parent)?;
        let original = RetainedFile::open_regular(&route)?;
        if let Some(first) = roots.first() {
            let first: &ReaderDirectory = first;
            if first.identity()? != root.identity()? {
                return Err(eyre!(
                    "Taira Inrou container, service, bundle, and inrou directory must share one workspace"
                ));
            }
        }
        roots.push(root);
        originals.push(original);
    }
    let root = &roots[0];
    let inrou = root.open_child("inrou")?;
    // Only normalize a route after all original ancestors are retained and admitted.
    let canonical = fs::canonicalize(root.path())?;
    for original in originals {
        original.revalidate()?;
    }
    for root in &roots {
        root.revalidate()?;
    }
    inrou.revalidate()?;
    Ok(canonical)
}

pub(super) fn visit_private_tree(
    directory: &PrivateDirectory,
    relative_dir: &Path,
    expected: &BTreeMap<PathBuf, bool>,
    seen: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    let result = (|| -> Result<()> {
        for name in directory.entries(expected.len())? {
            let relative = relative_dir.join(&name);
            let Some(is_directory) = expected.get(&relative).copied() else {
                return Err(eyre!(
                    "Taira Inrou stage contains unexpected entry `{}`",
                    relative.display()
                ));
            };
            if !seen.insert(relative.clone()) {
                return Err(eyre!(
                    "Taira Inrou stage contains duplicate entry `{}`",
                    relative.display()
                ));
            }
            if is_directory {
                let child = directory.open_child(&name)?;
                visit_private_tree(&child, &relative, expected, seen)?;
            } else {
                let file = directory.open_retained_private(&name)?;
                file.revalidate()?;
            }
        }
        Ok(())
    })();
    finish_directory(directory, result)
}
