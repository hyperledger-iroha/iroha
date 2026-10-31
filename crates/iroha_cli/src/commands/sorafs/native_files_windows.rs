//! Local native handshake files and private guard-cache publication.
//!
//! Original file and ancestry owners bracket bounded reads and complete writes.
//! A cache leaf is retained for its bounded read; its parent remains retained
//! through authenticated decoding. This does not hold the leaf through decoding.
//! Existing permissions are validated, never repaired; unsupported namespaces
//! never fall back to pathname metadata or an ordinary unretained file open.

use std::{
    ffi::OsString,
    io::{Read as _, Write as _},
    path::{Component, Path, PathBuf, Prefix},
};

use iroha_fs::{OwnerDirectory, PublishMode, ReaderDirectory, RetainedFile};

use super::{
    GUARD_CACHE_MAX_BYTES_V1, GuardCacheKey, GuardSet, Result, TokenOutputFormat, WrapErr,
    Zeroizing, eyre, validate_handshake_file_length, write_token_encoding,
};

pub(super) fn read_private(
    path: &Path,
    maximum: usize,
    exact: Option<usize>,
    label: &str,
) -> Result<Zeroizing<Vec<u8>>> {
    read_observed(path, maximum, exact, label, true, |_| Ok(()))
}

pub(super) fn read_public(
    path: &Path,
    maximum: usize,
    exact: Option<usize>,
    label: &str,
) -> Result<Vec<u8>> {
    let mut bytes = read_observed(path, maximum, exact, label, false, |_| Ok(()))?;
    Ok(std::mem::take(&mut *bytes))
}

fn read_observed(
    path: &Path,
    maximum: usize,
    exact: Option<usize>,
    label: &str,
    private: bool,
    after_read: impl FnOnce(&RetainedFile) -> Result<()>,
) -> Result<Zeroizing<Vec<u8>>> {
    let mut original = if private {
        RetainedFile::open_private(path)
    } else {
        RetainedFile::open_regular(path)
    }
    .wrap_err_with(|| {
        format!(
            "failed to admit {label} {} with native custody",
            path.display()
        )
    })?;
    let before = original.snapshot()?;
    let result = (|| {
        let length = usize::try_from(before.local_parts().2)
            .map_err(|_| eyre!("{label} length cannot be represented on this host"))?;
        validate_handshake_file_length(length, maximum, exact, label, Some(path))?;
        let mut bytes = Zeroizing::new(Vec::new());
        bytes
            .try_reserve_exact(length)
            .map_err(|error| eyre!("failed to reserve {label} buffer: {error}"))?;
        bytes.resize(length, 0);
        original.file_mut().read_exact(bytes.as_mut_slice())?;
        let mut extra = Zeroizing::new([0_u8; 1]);
        if original.file_mut().read(extra.as_mut())? != 0 {
            return Err(eyre!("{label} {} grew while being read", path.display()));
        }
        after_read(&original)?;
        Ok(bytes)
    })();
    // Evaluate both final checks on ordinary read errors too. Preserve the first
    // read/observation error, but never return bytes after an exit custody refusal.
    let snapshot_check = original
        .snapshot()
        .map_err(eyre::Report::from)
        .and_then(|after| {
            if after != before {
                Err(eyre!("{label} {} changed while being read", path.display()))
            } else {
                Ok(())
            }
        });
    let custody_check = original.revalidate().map_err(eyre::Report::from);
    result.and_then(|bytes| {
        snapshot_check?;
        custody_check?;
        Ok(bytes)
    })
}

pub(super) fn write_token(path: &Path, format: TokenOutputFormat, bytes: &[u8]) -> Result<()> {
    write_token_observed(path, format, bytes, |_| Ok(()))
}

fn write_token_observed(
    path: &Path,
    format: TokenOutputFormat,
    bytes: &[u8],
    after_write: impl FnOnce(&RetainedFile) -> Result<()>,
) -> Result<()> {
    let mut original = RetainedFile::create_new_private(path).wrap_err_with(|| {
        format!(
            "failed to create new owner-private token output {}",
            path.display()
        )
    })?;
    let mut written = None;
    let result = (|| {
        write_token_encoding(original.file_mut(), format, bytes)?;
        original.file_mut().flush()?;
        original.file().sync_all()?;
        written = Some(original.snapshot()?);
        after_write(&original)
    })();
    let snapshot_check = original
        .snapshot()
        .map_err(eyre::Report::from)
        .and_then(|after| {
            if written.is_some_and(|before| before != after) {
                Err(eyre!(
                    "token output {} changed after writing",
                    path.display()
                ))
            } else {
                Ok(())
            }
        });
    let custody_check = original.revalidate().map_err(eyre::Report::from);
    result.and(snapshot_check).and(custody_check)
}

// Resolve relative spelling lexically, then reject nonlocal prefixes before any
// filesystem probe. Every remaining directory component is admitted by its native
// owner. In particular, no NotFound from a late revalidation becomes cache absence.
fn cache_path(path: &Path) -> Result<(PathBuf, Vec<OsString>, OsString)> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(eyre!(
            "guard cache requires a nonempty path without parent traversal"
        ));
    }
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    if absolute.components().count() > 128 {
        return Err(eyre!(
            "guard cache path exceeds the native custody depth bound"
        ));
    }
    let mut components = absolute.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err(eyre!("guard cache requires a local native drive path"));
    };
    if !matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        || !matches!(components.next(), Some(Component::RootDir))
    {
        return Err(eyre!("guard cache requires a local native drive path"));
    }
    let mut root = PathBuf::from(prefix.as_os_str());
    root.push(Path::new(r"\"));
    let mut names = Vec::new();
    for component in components {
        match component {
            Component::Normal(name) => names.push(name.to_owned()),
            Component::CurDir => {}
            _ => return Err(eyre!("guard cache requires normal native components")),
        }
    }
    let leaf = names
        .pop()
        .ok_or_else(|| eyre!("guard cache path must name a regular file"))?;
    Ok((root, names, leaf))
}

pub(super) fn load_guard_set(path: &Path, key: &GuardCacheKey) -> Result<Option<GuardSet>> {
    load_guard_set_observed(path, key, |_| Ok(()))
}

fn load_guard_set_observed(
    path: &Path,
    key: &GuardCacheKey,
    after_read: impl FnOnce(&ReaderDirectory) -> Result<()>,
) -> Result<Option<GuardSet>> {
    let (root, names, leaf) = cache_path(path)?;
    let mut parent = ReaderDirectory::open(root)?;
    for name in names {
        match parent.open_child_optional(name)? {
            Some(child) => parent = child,
            None => {
                parent.revalidate()?;
                return Ok(None);
            }
        }
    }
    parent.require_current_owner()?;
    let result = (|| {
        let Some(bytes) = parent.read_private_optional(leaf, GUARD_CACHE_MAX_BYTES_V1)? else {
            return Ok(None);
        };
        validate_handshake_file_length(
            bytes.len(),
            GUARD_CACHE_MAX_BYTES_V1,
            None,
            "guard cache",
            Some(path),
        )?;
        after_read(&parent)?;
        let guards = GuardSet::decode_authenticated(&bytes, key).map_err(|error| {
            eyre!(
                "failed to decode guard cache from `{}`: {error}",
                path.display()
            )
        })?;
        Ok(Some(guards))
    })();
    let custody = parent.revalidate().map_err(eyre::Report::from);
    result.and_then(|guards| {
        custody?;
        Ok(guards)
    })
}

pub(super) fn persist_guard_cache(path: &Path, payload: &[u8]) -> Result<()> {
    validate_handshake_file_length(
        payload.len(),
        GUARD_CACHE_MAX_BYTES_V1,
        None,
        "encoded guard cache",
        Some(path),
    )?;
    let (mut parent, names, leaf) = cache_path(path)?;
    for name in names {
        parent.push(name);
    }
    let parent = OwnerDirectory::open_or_create(parent)?;
    parent
        .write_private_atomic(leaf, payload, PublishMode::Replace)
        .wrap_err_with(|| format!("failed to publish private guard cache `{}`", path.display()))
}

#[cfg(test)]
#[path = "native_files_windows_tests.rs"]
mod tests;
