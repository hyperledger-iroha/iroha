//! Owner-only filesystem primitives for private Kagami artifacts.

#[cfg(any(
    not(unix),
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox"
))]
use std::path::Path;

#[cfg(any(
    not(unix),
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox"
))]
use color_eyre::eyre::{Result, eyre};

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
mod unix {
    use std::{
        ffi::{OsStr, OsString},
        fs::{self, DirBuilder, File, OpenOptions},
        io::{Read, Write},
        os::unix::ffi::OsStrExt as _,
        os::unix::fs::{DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _},
        path::{Component, Path, PathBuf},
    };

    use color_eyre::eyre::{Result, WrapErr as _, eyre};
    use rand::{TryRngCore as _, rngs::OsRng};
    use rustix::fs::{
        AtFlags, Dir, FileType as RustixFileType, Mode, OFlags, fchmod, open, openat, statat,
    };
    use zeroize::Zeroize as _;

    const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
    const PRIVATE_FILE_MODE: u32 = 0o600;
    const MAX_PRIVATE_FILE_BYTES: u64 = 1024 * 1024;
    const MAX_PRIVATE_ROOT_COMPONENTS: usize = 64;
    const MAX_PRIVATE_TREE_DEPTH: usize = 64;
    const MAX_PRIVATE_TREE_ENTRIES: usize = 16_384;

    #[cfg(test)]
    static PRIVATE_TREE_ENTRY_REPLACEMENT: std::sync::Mutex<Option<(PathBuf, PathBuf)>> =
        std::sync::Mutex::new(None);

    #[cfg(test)]
    static PRIVATE_TREE_ANCESTOR_REPLACEMENT: std::sync::Mutex<
        Option<(PathBuf, PathBuf, PathBuf)>,
    > = std::sync::Mutex::new(None);

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

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct PrivateFileIdentity {
        device: u64,
        inode: u64,
    }

    impl PrivateFileIdentity {
        fn from_metadata(metadata: &fs::Metadata) -> Self {
            Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            }
        }

        fn from_stat(stat: &rustix::fs::Stat) -> Self {
            Self {
                device: stat.st_dev as u64,
                inode: stat.st_ino as u64,
            }
        }
    }

    struct PrivateDirectoryLink {
        parent: File,
        name: OsString,
        child_identity: PrivateFileIdentity,
    }

    struct OpenPrivateTreeRoot {
        root: File,
        root_before: fs::Metadata,
        ancestry: Vec<PrivateDirectoryLink>,
    }

    impl OpenPrivateTreeRoot {
        fn verify_ancestry(&self, path: &Path) -> Result<()> {
            for link in &self.ancestry {
                let observed = statat(&link.parent, &link.name, AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(std::io::Error::from)
                    .wrap_err_with(|| {
                        format!("reinspect private path ancestry for {}", path.display())
                    })?;
                if RustixFileType::from_raw_mode(observed.st_mode) != RustixFileType::Directory
                    || PrivateFileIdentity::from_stat(&observed) != link.child_identity
                {
                    return Err(eyre!(
                        "private path ancestry changed during hardening: {}",
                        path.display()
                    ));
                }
            }
            Ok(())
        }
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

    fn open_private_tree_root(path: &Path) -> Result<OpenPrivateTreeRoot> {
        let raw_path = path.as_os_str().as_bytes();
        if raw_path.is_empty()
            || raw_path
                .split(|byte| *byte == b'/')
                .any(|component| component == b"." || component == b"..")
        {
            return Err(eyre!(
                "private tree path must not be empty or contain `.`/`..`: {}",
                path.display()
            ));
        }

        let anchor = if path.is_absolute() {
            Path::new("/")
        } else {
            Path::new(".")
        };
        let mut current = File::from(
            open(
                anchor,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)
            .wrap_err_with(|| format!("open private path anchor for {}", path.display()))?,
        );
        let mut ancestry = Vec::new();
        for component in path.components() {
            let name = match component {
                Component::RootDir => continue,
                Component::Normal(name) => name,
                Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                    return Err(eyre!(
                        "private tree path contains an ambiguous component: {}",
                        path.display()
                    ));
                }
            };
            if ancestry.len() >= MAX_PRIVATE_ROOT_COMPONENTS {
                return Err(eyre!(
                    "private tree path contains too many components: {}",
                    path.display()
                ));
            }
            let before = statat(&current, name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(std::io::Error::from)
                .wrap_err_with(|| {
                    format!("inspect private path component for {}", path.display())
                })?;
            if RustixFileType::from_raw_mode(before.st_mode) != RustixFileType::Directory {
                return Err(eyre!(
                    "private tree path contains a symbolic link or non-directory component: {}",
                    path.display()
                ));
            }
            let child_identity = PrivateFileIdentity::from_stat(&before);
            let child = File::from(
                openat(
                    &current,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(std::io::Error::from)
                .wrap_err_with(|| format!("open private path component for {}", path.display()))?,
            );
            let opened = child.metadata().wrap_err_with(|| {
                format!(
                    "inspect opened private path component for {}",
                    path.display()
                )
            })?;
            let after = statat(&current, name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(std::io::Error::from)
                .wrap_err_with(|| {
                    format!("reinspect private path component for {}", path.display())
                })?;
            if !opened.is_dir()
                || PrivateFileIdentity::from_metadata(&opened) != child_identity
                || RustixFileType::from_raw_mode(after.st_mode) != RustixFileType::Directory
                || PrivateFileIdentity::from_stat(&after) != child_identity
            {
                return Err(eyre!(
                    "private path component changed while opening: {}",
                    path.display()
                ));
            }
            ancestry.push(PrivateDirectoryLink {
                parent: current,
                name: name.to_os_string(),
                child_identity,
            });
            current = child;
        }

        let root_before = current
            .metadata()
            .wrap_err_with(|| format!("inspect private directory {}", path.display()))?;
        if !root_before.is_dir()
            || root_before.uid() != current_uid()
            || root_before.mode() & 0o777 != PRIVATE_DIRECTORY_MODE
        {
            return Err(eyre!(
                "private directory must be owner-held mode 0700: {}",
                path.display()
            ));
        }
        let opened = OpenPrivateTreeRoot {
            root: current,
            root_before,
            ancestry,
        };
        opened.verify_ancestry(path)?;
        Ok(opened)
    }

    #[cfg(test)]
    fn replace_private_tree_ancestor_for_test(root_path: &Path) {
        let replacement = {
            let mut hook = PRIVATE_TREE_ANCESTOR_REPLACEMENT
                .lock()
                .expect("private tree ancestor replacement hook lock");
            match hook.as_ref() {
                Some((expected, _, _)) if expected == root_path => {
                    hook.take().map(|(_, ancestor, moved)| (ancestor, moved))
                }
                _ => None,
            }
        };
        if let Some((ancestor, moved)) = replacement {
            fs::rename(&ancestor, &moved).expect("move private tree ancestor for replacement test");
            std::os::unix::fs::symlink(&moved, &ancestor)
                .expect("replace private tree ancestor with symlink for test");
        }
    }

    fn verify_hardened_file(metadata: &fs::Metadata, identity: PrivateFileIdentity) -> bool {
        metadata.is_file()
            && PrivateFileIdentity::from_metadata(metadata) == identity
            && metadata.uid() == current_uid()
            && metadata.nlink() == 1
            && metadata.mode() & 0o777 == PRIVATE_FILE_MODE
    }

    fn verify_hardened_file_stat(stat: &rustix::fs::Stat, identity: PrivateFileIdentity) -> bool {
        RustixFileType::from_raw_mode(stat.st_mode) == RustixFileType::RegularFile
            && PrivateFileIdentity::from_stat(stat) == identity
            && stat.st_uid == current_uid()
            && stat.st_nlink == 1
            && stat.st_mode as u32 & 0o777 == PRIVATE_FILE_MODE
    }

    fn verify_hardened_directory(metadata: &fs::Metadata, identity: PrivateFileIdentity) -> bool {
        metadata.is_dir()
            && PrivateFileIdentity::from_metadata(metadata) == identity
            && metadata.uid() == current_uid()
            && metadata.mode() & 0o777 == PRIVATE_DIRECTORY_MODE
    }

    fn verify_hardened_directory_stat(
        stat: &rustix::fs::Stat,
        identity: PrivateFileIdentity,
    ) -> bool {
        RustixFileType::from_raw_mode(stat.st_mode) == RustixFileType::Directory
            && PrivateFileIdentity::from_stat(stat) == identity
            && stat.st_uid == current_uid()
            && stat.st_mode as u32 & 0o777 == PRIVATE_DIRECTORY_MODE
    }

    #[cfg(test)]
    fn replace_private_tree_entry_for_test(child_path: &Path) {
        let replacement = {
            let mut hook = PRIVATE_TREE_ENTRY_REPLACEMENT
                .lock()
                .expect("private tree replacement hook lock");
            match hook.as_ref() {
                Some((expected, _)) if expected == child_path => {
                    hook.take().map(|(_, replacement)| replacement)
                }
                _ => None,
            }
        };
        if let Some(replacement) = replacement {
            fs::remove_file(child_path).expect("remove private tree entry for replacement test");
            fs::rename(replacement, child_path)
                .expect("replace private tree entry for replacement test");
        }
    }

    #[allow(clippy::too_many_lines)]
    fn harden_private_directory_contents(
        directory: &File,
        display_path: &Path,
        depth: usize,
        entries_seen: &mut usize,
    ) -> Result<()> {
        if depth > MAX_PRIVATE_TREE_DEPTH {
            return Err(eyre!(
                "private tree exceeds the maximum depth at {}",
                display_path.display()
            ));
        }
        let directory_before = directory
            .metadata()
            .wrap_err_with(|| format!("inspect private tree {}", display_path.display()))?;
        let mut entries = Dir::read_from(directory)
            .map_err(std::io::Error::from)
            .wrap_err_with(|| format!("read private tree {}", display_path.display()))?;
        for entry in &mut entries {
            let entry = entry
                .map_err(std::io::Error::from)
                .wrap_err("read private tree entry")?;
            let name = entry.file_name();
            if matches!(name.to_bytes(), b"." | b"..") {
                continue;
            }
            *entries_seen = entries_seen
                .checked_add(1)
                .ok_or_else(|| eyre!("private tree entry count overflow"))?;
            if *entries_seen > MAX_PRIVATE_TREE_ENTRIES {
                return Err(eyre!("private tree contains too many entries"));
            }
            let child_path = display_path.join(OsStr::from_bytes(name.to_bytes()));
            let before = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(std::io::Error::from)
                .wrap_err_with(|| format!("inspect private tree entry {}", child_path.display()))?;
            if before.st_uid != current_uid() {
                return Err(eyre!(
                    "private tree contains an entry owned by another user: {}",
                    child_path.display()
                ));
            }
            let identity = PrivateFileIdentity::from_stat(&before);
            #[cfg(test)]
            replace_private_tree_entry_for_test(&child_path);
            match RustixFileType::from_raw_mode(before.st_mode) {
                RustixFileType::Directory => {
                    let child = File::from(
                        openat(
                            directory,
                            name,
                            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                            Mode::empty(),
                        )
                        .map_err(std::io::Error::from)
                        .wrap_err_with(|| {
                            format!("open private directory {}", child_path.display())
                        })?,
                    );
                    let opened = child.metadata().wrap_err_with(|| {
                        format!("inspect opened private directory {}", child_path.display())
                    })?;
                    if !opened.is_dir()
                        || PrivateFileIdentity::from_metadata(&opened) != identity
                        || opened.uid() != current_uid()
                    {
                        return Err(eyre!(
                            "private directory changed while opening: {}",
                            child_path.display()
                        ));
                    }
                    fchmod(&child, Mode::from_raw_mode(PRIVATE_DIRECTORY_MODE as _))
                        .map_err(std::io::Error::from)
                        .wrap_err_with(|| {
                            format!("harden private directory {}", child_path.display())
                        })?;
                    let hardened = child.metadata().wrap_err_with(|| {
                        format!("reinspect hardened directory {}", child_path.display())
                    })?;
                    let parent_entry = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
                        .map_err(std::io::Error::from)
                        .wrap_err_with(|| {
                            format!("reinspect private directory {}", child_path.display())
                        })?;
                    if !verify_hardened_directory(&hardened, identity)
                        || !verify_hardened_directory_stat(&parent_entry, identity)
                    {
                        return Err(eyre!(
                            "private directory changed while hardening: {}",
                            child_path.display()
                        ));
                    }
                    harden_private_directory_contents(
                        &child,
                        &child_path,
                        depth + 1,
                        entries_seen,
                    )?;
                    let parent_entry = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
                        .map_err(std::io::Error::from)
                        .wrap_err_with(|| {
                            format!("reinspect private directory {}", child_path.display())
                        })?;
                    if !verify_hardened_directory_stat(&parent_entry, identity) {
                        return Err(eyre!(
                            "private directory changed during traversal: {}",
                            child_path.display()
                        ));
                    }
                }
                RustixFileType::RegularFile if before.st_nlink == 1 => {
                    let child = File::from(
                        openat(
                            directory,
                            name,
                            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                            Mode::empty(),
                        )
                        .map_err(std::io::Error::from)
                        .wrap_err_with(|| format!("open private file {}", child_path.display()))?,
                    );
                    let opened = child.metadata().wrap_err_with(|| {
                        format!("inspect opened private file {}", child_path.display())
                    })?;
                    if !opened.is_file()
                        || PrivateFileIdentity::from_metadata(&opened) != identity
                        || opened.uid() != current_uid()
                        || opened.nlink() != 1
                    {
                        return Err(eyre!(
                            "private file changed while opening: {}",
                            child_path.display()
                        ));
                    }
                    fchmod(&child, Mode::from_raw_mode(PRIVATE_FILE_MODE as _))
                        .map_err(std::io::Error::from)
                        .wrap_err_with(|| {
                            format!("harden private file {}", child_path.display())
                        })?;
                    let hardened = child.metadata().wrap_err_with(|| {
                        format!("reinspect hardened file {}", child_path.display())
                    })?;
                    let parent_entry = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
                        .map_err(std::io::Error::from)
                        .wrap_err_with(|| {
                            format!("reinspect private file {}", child_path.display())
                        })?;
                    if !verify_hardened_file(&hardened, identity)
                        || !verify_hardened_file_stat(&parent_entry, identity)
                    {
                        return Err(eyre!(
                            "private file changed while hardening: {}",
                            child_path.display()
                        ));
                    }
                }
                _ => {
                    return Err(eyre!(
                        "private tree contains a special or multi-link entry: {}",
                        child_path.display()
                    ));
                }
            }
        }
        drop(entries);
        directory
            .sync_all()
            .wrap_err_with(|| format!("sync hardened private tree {}", display_path.display()))?;
        let directory_after = directory
            .metadata()
            .wrap_err_with(|| format!("reinspect private tree {}", display_path.display()))?;
        if !same_file(&directory_before, &directory_after) {
            return Err(eyre!(
                "private directory changed during traversal: {}",
                display_path.display()
            ));
        }
        Ok(())
    }

    pub(crate) fn harden_private_tree(path: &Path) -> Result<()> {
        let opened_root = open_private_tree_root(path)?;
        #[cfg(test)]
        replace_private_tree_ancestor_for_test(path);
        let mut entries_seen = 0;
        harden_private_directory_contents(&opened_root.root, path, 0, &mut entries_seen)?;
        let opened_after = opened_root
            .root
            .metadata()
            .wrap_err_with(|| format!("reinspect private directory {}", path.display()))?;
        if !same_file(&opened_root.root_before, &opened_after) {
            return Err(eyre!(
                "private directory changed during hardening: {}",
                path.display()
            ));
        }
        opened_root.verify_ancestry(path)?;
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

    #[cfg(test)]
    mod tests {
        use std::os::unix::{
            fs::{MetadataExt as _, PermissionsExt as _, symlink},
            net::UnixListener,
        };

        use super::*;

        fn set_mode(path: &Path, mode: u32) {
            fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("set mode");
        }

        fn mode(path: &Path) -> u32 {
            fs::symlink_metadata(path).expect("read metadata").mode() & 0o777
        }

        fn private_root() -> (tempfile::TempDir, PathBuf) {
            let guard = tempfile::tempdir().expect("create private root");
            set_mode(guard.path(), PRIVATE_DIRECTORY_MODE);
            let root = fs::canonicalize(guard.path()).expect("canonicalize private root");
            (guard, root)
        }

        #[test]
        fn harden_private_tree_normalizes_directories_and_every_regular_file_to_private_modes() {
            let (_root_guard, root) = private_root();
            let nested = root.join("nested");
            fs::create_dir(&nested).expect("create nested directory");
            set_mode(&nested, 0o755);
            let executable = root.join("start.sh");
            let regular = nested.join("client.toml");
            fs::write(&executable, b"#!/usr/bin/env bash\nexit 0\n").expect("write script");
            fs::write(&regular, b"private_key = 'secret'\n").expect("write config");
            set_mode(&executable, 0o755);
            set_mode(&regular, 0o644);

            harden_private_tree(&root).expect("harden private tree");

            assert_eq!(mode(&root), PRIVATE_DIRECTORY_MODE);
            assert_eq!(mode(&nested), PRIVATE_DIRECTORY_MODE);
            assert_eq!(mode(&executable), PRIVATE_FILE_MODE);
            assert_eq!(mode(&regular), PRIVATE_FILE_MODE);
            assert_eq!(
                fs::read(&executable).expect("read script"),
                b"#!/usr/bin/env bash\nexit 0\n"
            );
        }

        #[test]
        fn harden_private_tree_rejects_symlinks_without_changing_their_target() {
            let (_root_guard, root) = private_root();
            let outside = tempfile::NamedTempFile::new().expect("create outside file");
            set_mode(outside.path(), 0o640);
            symlink(outside.path(), root.join("linked-secret")).expect("create symlink");

            let error = harden_private_tree(&root).expect_err("symlink must fail closed");

            assert!(error.to_string().contains("special or multi-link entry"));
            assert_eq!(mode(outside.path()), 0o640);
        }

        #[test]
        fn harden_private_tree_rejects_special_files() {
            let (_root_guard, root) = private_root();
            let socket_path = root.join("control.sock");
            let _listener = UnixListener::bind(&socket_path).expect("bind Unix socket");

            let error = harden_private_tree(&root).expect_err("socket must fail closed");

            assert!(error.to_string().contains("special or multi-link entry"));
        }

        #[test]
        fn harden_private_tree_rejects_hard_linked_regular_files() {
            let (_root_guard, root) = private_root();
            let first = root.join("first");
            let second = root.join("second");
            fs::write(&first, b"secret").expect("write first link");
            set_mode(&first, 0o640);
            fs::hard_link(&first, &second).expect("create second hard link");

            let error = harden_private_tree(&root).expect_err("hard links must fail closed");

            assert!(error.to_string().contains("special or multi-link entry"));
            assert_eq!(mode(&first), 0o640);
            assert_eq!(mode(&second), 0o640);
        }

        #[test]
        fn harden_private_tree_rejects_a_regular_file_replaced_after_inspection() {
            let (_root_guard, root) = private_root();
            let victim = root.join("victim");
            fs::write(&victim, b"original").expect("write original entry");
            set_mode(&victim, 0o755);
            let replacements = tempfile::tempdir().expect("create replacement directory");
            let replacement = replacements.path().join("replacement");
            fs::write(&replacement, b"replacement").expect("write replacement entry");
            set_mode(&replacement, 0o640);
            {
                let mut hook = PRIVATE_TREE_ENTRY_REPLACEMENT
                    .lock()
                    .expect("private tree replacement hook lock");
                assert!(hook.is_none(), "replacement hook must start empty");
                *hook = Some((victim.clone(), replacement));
            }

            let error = harden_private_tree(&root)
                .expect_err("descriptor identity mismatch must fail closed");

            assert!(error.to_string().contains("changed while opening"));
            assert_eq!(mode(&victim), 0o640);
            assert_eq!(fs::read(&victim).expect("read replacement"), b"replacement");
        }

        #[test]
        fn harden_private_tree_rejects_an_intermediate_ancestor_replaced_with_a_symlink() {
            let sandbox = tempfile::tempdir().expect("create ancestor replacement sandbox");
            let sandbox_root =
                fs::canonicalize(sandbox.path()).expect("canonicalize replacement sandbox");
            let ancestor = sandbox_root.join("ancestor");
            let moved = sandbox_root.join("moved-ancestor");
            let root = ancestor.join("private");
            fs::create_dir_all(&root).expect("create nested private root");
            set_mode(&root, PRIVATE_DIRECTORY_MODE);
            {
                let mut hook = PRIVATE_TREE_ANCESTOR_REPLACEMENT
                    .lock()
                    .expect("private tree ancestor replacement hook lock");
                assert!(hook.is_none(), "ancestor replacement hook must start empty");
                *hook = Some((root.clone(), ancestor.clone(), moved.clone()));
            }

            let error = harden_private_tree(&root)
                .expect_err("an intermediate ancestor symlink must fail closed");

            assert!(error.to_string().contains("path ancestry changed"));
            assert!(
                fs::symlink_metadata(&ancestor)
                    .expect("inspect replaced ancestor")
                    .file_type()
                    .is_symlink()
            );
            assert!(moved.join("private").is_dir());
        }

        #[test]
        fn harden_private_tree_rejects_a_preexisting_intermediate_ancestor_symlink() {
            let sandbox = tempfile::tempdir().expect("create ancestor symlink sandbox");
            let sandbox_root =
                fs::canonicalize(sandbox.path()).expect("canonicalize symlink sandbox");
            let actual_ancestor = sandbox_root.join("actual-ancestor");
            let actual_root = actual_ancestor.join("private");
            fs::create_dir_all(&actual_root).expect("create actual private root");
            set_mode(&actual_root, PRIVATE_DIRECTORY_MODE);
            let alias = sandbox_root.join("ancestor-alias");
            symlink(&actual_ancestor, &alias).expect("create intermediate ancestor symlink");
            let spelled_root = alias.join("private");

            let error = harden_private_tree(&spelled_root)
                .expect_err("a preexisting intermediate ancestor symlink must fail closed");

            assert!(
                error
                    .to_string()
                    .contains("symbolic link or non-directory component")
            );
            assert_eq!(mode(&actual_root), PRIVATE_DIRECTORY_MODE);
        }
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
pub(crate) use unix::{
    harden_private_tree, prepare_empty_private_directory, read_private_file,
    write_private_file_atomic,
};

#[cfg(any(
    not(unix),
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox"
))]
fn unsupported() -> Result<()> {
    Err(eyre!(
        "owner-only private artifact operations require a supported Unix platform"
    ))
}

#[cfg(any(
    not(unix),
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox"
))]
pub(crate) fn validate_private_directory(_path: &Path) -> Result<()> {
    unsupported()
}

#[cfg(any(
    not(unix),
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox"
))]
pub(crate) fn prepare_empty_private_directory(_path: &Path) -> Result<()> {
    unsupported()
}

#[cfg(any(
    not(unix),
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox"
))]
pub(crate) fn harden_private_tree(_path: &Path) -> Result<()> {
    unsupported()
}

#[cfg(any(
    not(unix),
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox"
))]
pub(crate) fn write_private_file_atomic(_path: &Path, _raw: &[u8]) -> Result<()> {
    unsupported()
}

#[cfg(any(
    not(unix),
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox"
))]
pub(crate) fn read_private_file(_path: &Path) -> Result<Vec<u8>> {
    unsupported()?;
    unreachable!()
}
