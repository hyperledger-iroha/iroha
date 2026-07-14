//! Owner-only filesystem primitives for private Kagami artifacts.

#[cfg(not(unix))]
use std::path::Path;

#[cfg(not(unix))]
use color_eyre::eyre::{Result, eyre};

#[cfg(unix)]
mod unix {
    use std::{
        fs::{self, DirBuilder, File, OpenOptions},
        io::{Read, Write},
        os::unix::fs::{
            DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
        },
        path::{Path, PathBuf},
    };

    use color_eyre::eyre::{Result, WrapErr as _, eyre};
    use rand::{TryRngCore as _, rngs::OsRng};
    use zeroize::Zeroize as _;

    const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
    const PRIVATE_FILE_MODE: u32 = 0o600;
    const MAX_PRIVATE_FILE_BYTES: u64 = 1024 * 1024;

    fn absolute(path: &Path) -> Result<PathBuf> {
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(std::env::current_dir()
                .wrap_err("resolve current directory")?
                .join(path))
        }
    }

    fn reject_symlink_components(path: &Path) -> Result<()> {
        let mut current = absolute(path)?;
        loop {
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(eyre!(
                        "private path contains a symbolic link: {}",
                        current.display()
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).wrap_err_with(|| {
                        format!("inspect private path component {}", current.display())
                    });
                }
            }
            let Some(parent) = current.parent() else {
                break;
            };
            if parent == current {
                break;
            }
            current = parent.to_path_buf();
        }
        Ok(())
    }

    fn current_uid() -> u32 {
        rustix::process::geteuid().as_raw()
    }

    fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
        left.dev() == right.dev()
            && left.ino() == right.ino()
            && left.mode() == right.mode()
            && left.uid() == right.uid()
            && left.gid() == right.gid()
            && left.nlink() == right.nlink()
            && left.size() == right.size()
            && left.mtime() == right.mtime()
            && left.mtime_nsec() == right.mtime_nsec()
            && left.ctime() == right.ctime()
            && left.ctime_nsec() == right.ctime_nsec()
    }

    pub(crate) fn validate_private_directory(path: &Path) -> Result<()> {
        reject_symlink_components(path)?;
        let lexical = fs::symlink_metadata(path)
            .wrap_err_with(|| format!("inspect private directory {}", path.display()))?;
        if !lexical.is_dir()
            || lexical.file_type().is_symlink()
            || lexical.uid() != current_uid()
            || lexical.mode() & 0o777 != PRIVATE_DIRECTORY_MODE
        {
            return Err(eyre!(
                "private directory must be owner-held mode 0700: {}",
                path.display()
            ));
        }
        let opened = File::open(path)
            .wrap_err_with(|| format!("open private directory {}", path.display()))?;
        let observed = opened
            .metadata()
            .wrap_err_with(|| format!("inspect opened directory {}", path.display()))?;
        if lexical.dev() != observed.dev() || lexical.ino() != observed.ino() {
            return Err(eyre!(
                "private directory changed while opening: {}",
                path.display()
            ));
        }
        Ok(())
    }

    pub(crate) fn prepare_empty_private_directory(path: &Path) -> Result<()> {
        reject_symlink_components(path)?;
        if !path.exists() {
            let mut builder = DirBuilder::new();
            builder.mode(PRIVATE_DIRECTORY_MODE);
            builder
                .create(path)
                .wrap_err_with(|| format!("create private directory {}", path.display()))?;
        }
        validate_private_directory(path)?;
        if fs::read_dir(path)
            .wrap_err_with(|| format!("read private directory {}", path.display()))?
            .next()
            .is_some()
        {
            return Err(eyre!(
                "private output directory must be empty: {}",
                path.display()
            ));
        }
        Ok(())
    }

    pub(crate) fn harden_private_tree(path: &Path) -> Result<()> {
        validate_private_directory(path)?;
        let mut pending = vec![path.to_path_buf()];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory)
                .wrap_err_with(|| format!("read private tree {}", directory.display()))?
            {
                let entry = entry.wrap_err("read private tree entry")?;
                let child = entry.path();
                let metadata = fs::symlink_metadata(&child)
                    .wrap_err_with(|| format!("inspect private tree entry {}", child.display()))?;
                if metadata.file_type().is_symlink() || metadata.uid() != current_uid() {
                    return Err(eyre!(
                        "private tree contains an unsafe entry: {}",
                        child.display()
                    ));
                }
                if metadata.is_dir() {
                    fs::set_permissions(&child, fs::Permissions::from_mode(PRIVATE_DIRECTORY_MODE))
                        .wrap_err("harden private directory")?;
                    pending.push(child);
                } else if metadata.is_file() && metadata.nlink() == 1 {
                    fs::set_permissions(&child, fs::Permissions::from_mode(PRIVATE_FILE_MODE))
                        .wrap_err("harden private file")?;
                } else {
                    return Err(eyre!(
                        "private tree contains a special or multi-link entry: {}",
                        child.display()
                    ));
                }
            }
            File::open(&directory)
                .and_then(|opened| opened.sync_all())
                .wrap_err("sync hardened private directory")?;
        }
        Ok(())
    }

    fn random_temporary_path(parent: &Path, target_name: &str) -> Result<PathBuf> {
        let mut random = [0_u8; 16];
        OsRng
            .try_fill_bytes(&mut random)
            .wrap_err("obtain OS entropy for private temporary filename")?;
        let suffix = hex::encode(random);
        random.zeroize();
        Ok(parent.join(format!(".{target_name}.tmp.{suffix}")))
    }

    pub(crate) fn write_private_file_atomic(path: &Path, raw: &[u8]) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| eyre!("private output path has no parent"))?;
        validate_private_directory(parent)?;
        match fs::symlink_metadata(path) {
            Ok(_) => {
                return Err(eyre!(
                    "refusing to overwrite private output: {}",
                    path.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).wrap_err("inspect private output destination"),
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| eyre!("private output filename is not UTF-8"))?;
        let temporary = random_temporary_path(parent, name)?;
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .mode(PRIVATE_FILE_MODE)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
        let mut file = options
            .open(&temporary)
            .wrap_err_with(|| format!("create private temporary file {}", temporary.display()))?;
        let result = (|| -> Result<()> {
            let metadata = file.metadata().wrap_err("inspect private temporary file")?;
            if !metadata.is_file()
                || metadata.uid() != current_uid()
                || metadata.mode() & 0o777 != PRIVATE_FILE_MODE
                || metadata.nlink() != 1
            {
                return Err(eyre!("private temporary file has unsafe custody"));
            }
            file.write_all(raw)
                .wrap_err("write private temporary file")?;
            file.sync_all().wrap_err("sync private temporary file")?;
            fs::hard_link(&temporary, path).wrap_err_with(|| {
                format!("atomically publish private output {}", path.display())
            })?;
            fs::remove_file(&temporary).wrap_err("remove private temporary link")?;
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .wrap_err("sync private output directory")?;
            Ok(())
        })();
        drop(file);
        if temporary.exists() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    pub(crate) fn read_private_file(path: &Path) -> Result<Vec<u8>> {
        reject_symlink_components(path)?;
        let lexical = fs::symlink_metadata(path)
            .wrap_err_with(|| format!("inspect private file {}", path.display()))?;
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
        let mut file = options
            .open(path)
            .wrap_err_with(|| format!("open private file {}", path.display()))?;
        let before = file.metadata().wrap_err("inspect opened private file")?;
        if lexical.file_type().is_symlink()
            || !before.is_file()
            || !same_file(&lexical, &before)
            || before.uid() != current_uid()
            || before.mode() & 0o777 != PRIVATE_FILE_MODE
            || before.nlink() != 1
            || before.size() == 0
            || before.size() > MAX_PRIVATE_FILE_BYTES
        {
            return Err(eyre!(
                "private file must be owner-held mode 0600, single-link regular data: {}",
                path.display()
            ));
        }
        let mut raw = Vec::with_capacity(usize::try_from(before.size()).unwrap_or(0));
        file.read_to_end(&mut raw).wrap_err("read private file")?;
        let after = file.metadata().wrap_err("reinspect private file")?;
        if !same_file(&before, &after) || raw.len() as u64 != before.size() {
            raw.zeroize();
            return Err(eyre!("private file changed while being read"));
        }
        Ok(raw)
    }
}

#[cfg(unix)]
pub(crate) use unix::{
    harden_private_tree, prepare_empty_private_directory, read_private_file,
    write_private_file_atomic,
};

#[cfg(not(unix))]
fn unsupported() -> Result<()> {
    Err(eyre!(
        "owner-only private artifact operations require a Unix platform"
    ))
}

#[cfg(not(unix))]
pub(crate) fn validate_private_directory(_path: &Path) -> Result<()> {
    unsupported()
}

#[cfg(not(unix))]
pub(crate) fn prepare_empty_private_directory(_path: &Path) -> Result<()> {
    unsupported()
}

#[cfg(not(unix))]
pub(crate) fn harden_private_tree(_path: &Path) -> Result<()> {
    unsupported()
}

#[cfg(not(unix))]
pub(crate) fn write_private_file_atomic(_path: &Path, _raw: &[u8]) -> Result<()> {
    unsupported()
}

#[cfg(not(unix))]
pub(crate) fn read_private_file(_path: &Path) -> Result<Vec<u8>> {
    unsupported()?;
    unreachable!()
}
