//! Descriptor-owned journal creation and no-replace trace publication.
//!
//! Parent identities remain bound through each operation. Failure retains partial
//! diagnostics; no cleanup follows untrusted pathnames. NOREPLACE protects a
//! destination racer. The OS cannot condition rename on a source inode, so a
//! same-uid source replacement in the final check/rename interval is detected
//! afterward, never qualified or blindly removed. Filesystem calls themselves
//! have no promised wall-clock latency bound. Final bundle verification remains
//! required before evidence qualification.

#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
pub(super) use supported::{TraceOutput, new_owned_file};

#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
mod supported {
    use eyre::{Result, ensure, eyre};
    use rustix::fs::{AtFlags, FileType, Mode, OFlags, RenameFlags, Stat};
    use std::{
        ffi::OsString,
        fs::File,
        os::unix::ffi::OsStrExt as _,
        path::{Component, Path, PathBuf},
    };

    const DIRECTORY_FLAGS: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::NONBLOCK)
        .union(OFlags::CLOEXEC);
    const FILE_FLAGS: OFlags = OFlags::WRONLY
        .union(OFlags::CREATE)
        .union(OFlags::EXCL)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::NONBLOCK)
        .union(OFlags::CLOEXEC);
    const MAX_PATH_BYTES: usize = 4096;
    const MAX_COMPONENTS: usize = 64;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Identity {
        dev: u64,
        ino: u64,
        mode: u32,
        uid: u32,
        gid: u32,
    }
    impl Identity {
        fn from_stat(stat: Stat) -> Self {
            Self {
                dev: stat.st_dev as u64,
                ino: stat.st_ino as u64,
                mode: stat.st_mode as u32,
                uid: stat.st_uid,
                gid: stat.st_gid,
            }
        }
        fn directory(self) -> bool {
            FileType::from_raw_mode(self.mode as rustix::fs::RawMode) == FileType::Directory
        }
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FileState {
        identity: Identity,
        links: u64,
        size: u64,
        modified: (i64, i64),
        changed: (i64, i64),
    }
    impl FileState {
        fn from_stat(stat: Stat) -> Result<Self> {
            Ok(Self {
                identity: Identity::from_stat(stat),
                links: stat.st_nlink as u64,
                size: u64::try_from(stat.st_size)?,
                modified: (stat.st_mtime as i64, stat.st_mtime_nsec as i64),
                changed: (stat.st_ctime as i64, stat.st_ctime_nsec as i64),
            })
        }
        fn owned_regular(self, uid: u32) -> bool {
            FileType::from_raw_mode(self.identity.mode as rustix::fs::RawMode)
                == FileType::RegularFile
                && self.identity.uid == uid
                && self.identity.mode & 0o7777 == 0o600
                && self.links == 1
        }
    }
    // Directory byte counts and timestamps are not admission authorities.
    fn held_identity(file: &File) -> Result<Identity> {
        Ok(Identity::from_stat(rustix::fs::fstat(file)?))
    }
    fn named_identity(parent: &File, name: &std::ffi::OsStr) -> Result<Identity> {
        Ok(Identity::from_stat(rustix::fs::statat(
            parent,
            name,
            AtFlags::SYMLINK_NOFOLLOW,
        )?))
    }
    fn held(file: &File) -> Result<FileState> {
        FileState::from_stat(rustix::fs::fstat(file)?)
    }
    fn named(parent: &File, name: &std::ffi::OsStr) -> Result<FileState> {
        FileState::from_stat(rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?)
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Phase {
        ParentAdmitted,
        ParentRetained,
        BeforeCreate,
        AfterCreate,
        BeforeDirectorySync,
        AfterDirectorySync,
        BeforeFileSync,
        AfterFileSync,
        BeforeRename,
        RenameReady,
        AfterRename,
    }
    #[derive(Debug)]
    struct Directory {
        file: File,
        name: Option<OsString>,
        identity: Identity,
    }
    #[derive(Debug)]
    struct Parent {
        directories: Vec<Directory>,
        uid: u32,
    }
    impl Parent {
        fn capture(
            path: &Path,
            hook: &mut impl FnMut(Phase) -> Result<()>,
        ) -> Result<(Self, OsString)> {
            ensure!(
                path.is_absolute() && path.as_os_str().as_bytes().len() <= MAX_PATH_BYTES,
                "output requires a bounded absolute path"
            );
            let mut normalized = PathBuf::new();
            let mut names = Vec::new();
            for component in path.components() {
                match component {
                    Component::RootDir => normalized.push(component.as_os_str()),
                    Component::Normal(name) => {
                        normalized.push(name);
                        names.push(name.to_os_string());
                    }
                    _ => return Err(eyre!("output path must be lexical and normalized")),
                }
            }
            ensure!(
                normalized.as_os_str() == path.as_os_str()
                    && !names.is_empty()
                    && names.len() <= MAX_COMPONENTS,
                "output path components are invalid"
            );
            let leaf = names.pop().ok_or_else(|| eyre!("output leaf missing"))?;
            let file = File::from(rustix::fs::open("/", DIRECTORY_FLAGS, Mode::empty())?);
            let root = held_identity(&file)?;
            ensure!(
                root.directory(),
                "output filesystem root is not a directory"
            );
            let mut directories = vec![Directory {
                file,
                name: None,
                identity: root,
            }];
            for (index, name) in names.iter().enumerate() {
                let parent = &directories
                    .last()
                    .ok_or_else(|| eyre!("output parent missing"))?
                    .file;
                let before = named_identity(parent, name)?;
                ensure!(
                    before.directory(),
                    "output ancestor is not a non-symlink directory"
                );
                if index + 1 == names.len() {
                    hook(Phase::ParentAdmitted)?;
                }
                let file = File::from(rustix::fs::openat(
                    parent,
                    name,
                    DIRECTORY_FLAGS,
                    Mode::empty(),
                )?);
                ensure!(
                    held_identity(&file)? == before && named_identity(parent, name)? == before,
                    "output directory changed during admission"
                );
                directories.push(Directory {
                    file,
                    name: Some(name.clone()),
                    identity: before,
                });
            }
            let owner = Self {
                directories,
                uid: rustix::process::geteuid().as_raw(),
            };
            owner.check()?;
            hook(Phase::ParentRetained)?;
            owner.check()?;
            Ok((owner, leaf))
        }
        fn file(&self) -> &File {
            &self
                .directories
                .last()
                .expect("retained filesystem root")
                .file
        }
        fn check(&self) -> Result<()> {
            for (index, directory) in self.directories.iter().enumerate() {
                ensure!(
                    held_identity(&directory.file)? == directory.identity,
                    "held output directory changed"
                );
                if let Some(name) = &directory.name {
                    ensure!(
                        index > 0
                            && named_identity(&self.directories[index - 1].file, name)?
                                == directory.identity,
                        "named output ancestor changed"
                    );
                }
            }
            Ok(())
        }
        fn create(
            &self,
            name: &std::ffi::OsStr,
            hook: &mut impl FnMut(Phase) -> Result<()>,
        ) -> Result<(File, Identity)> {
            self.check()?;
            hook(Phase::BeforeCreate)?;
            self.check()?;
            let file = File::from(rustix::fs::openat(
                self.file(),
                name,
                FILE_FLAGS,
                Mode::RUSR | Mode::WUSR,
            )?);
            let state = held(&file)?;
            ensure!(
                state.owned_regular(self.uid)
                    && state.size == 0
                    && named(self.file(), name)? == state,
                "created output is not the exact owned empty regular file"
            );
            hook(Phase::AfterCreate)?;
            self.check_file(&file, name, state.identity, 0)?;
            self.sync(hook)?;
            self.check_file(&file, name, state.identity, 0)?;
            Ok((file, state.identity))
        }
        fn check_file(
            &self,
            file: &File,
            name: &std::ffi::OsStr,
            identity: Identity,
            maximum: u64,
        ) -> Result<FileState> {
            self.check()?;
            let state = held(file)?;
            ensure!(
                state.identity == identity
                    && state.owned_regular(self.uid)
                    && state.size <= maximum
                    && named(self.file(), name)? == state,
                "held or named output file changed or exceeded allocation"
            );
            Ok(state)
        }
        fn require_absent(&self, name: &std::ffi::OsStr) -> Result<()> {
            self.check()?;
            ensure!(
                matches!(
                    rustix::fs::statat(self.file(), name, AtFlags::SYMLINK_NOFOLLOW),
                    Err(rustix::io::Errno::NOENT)
                ),
                "trace stage name reappeared after publication"
            );
            Ok(())
        }
        fn sync(&self, hook: &mut impl FnMut(Phase) -> Result<()>) -> Result<()> {
            self.check()?;
            hook(Phase::BeforeDirectorySync)?;
            self.check()?;
            self.file().sync_all()?;
            hook(Phase::AfterDirectorySync)?;
            self.check()
        }
    }
    pub(crate) fn new_owned_file(path: &Path) -> Result<File> {
        new_owned_file_with_hook(path, |_| Ok(()))
    }
    fn new_owned_file_with_hook(
        path: &Path,
        mut hook: impl FnMut(Phase) -> Result<()>,
    ) -> Result<File> {
        let (parent, name) = Parent::capture(path, &mut hook)?;
        let (file, _) = parent.create(&name, &mut hook)?;
        Ok(file)
    }
    /// Owns stage descriptor and its original namespace until consuming publication.
    pub(crate) struct TraceOutput {
        parent: Parent,
        file: File,
        identity: Identity,
        stage: OsString,
        destination: OsString,
        maximum: u64,
    }
    impl TraceOutput {
        pub(crate) fn create(path: &Path, maximum: usize) -> Result<Self> {
            Self::create_with_hook(path, maximum, |_| Ok(()))
        }
        fn create_with_hook(
            path: &Path,
            maximum: usize,
            mut hook: impl FnMut(Phase) -> Result<()>,
        ) -> Result<Self> {
            ensure!(
                maximum > 0 && maximum <= super::super::MAX_FILE_BYTES,
                "invalid trace allocation"
            );
            ensure!(
                path.is_absolute() && path.as_os_str().as_bytes().len() <= MAX_PATH_BYTES,
                "trace requires a bounded absolute path"
            );
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| eyre!("trace output name must be UTF-8"))?;
            let stage = OsString::from(format!("{name}.collecting"));
            ensure!(
                path.as_os_str()
                    .as_bytes()
                    .len()
                    .checked_add(".collecting".len())
                    .is_some_and(|n| n <= MAX_PATH_BYTES),
                "trace stage path exceeds bound"
            );
            let (parent, destination) = Parent::capture(path, &mut hook)?;
            let (file, identity) = parent.create(&stage, &mut hook)?;
            Ok(Self {
                parent,
                file,
                identity,
                stage,
                destination,
                maximum: maximum as u64,
            })
        }
        pub(crate) fn file_mut(&mut self) -> &mut File {
            &mut self.file
        }
        pub(crate) fn publish(self) -> Result<()> {
            self.publish_with_hook(|_| Ok(()))
        }
        fn publish_with_hook(self, mut hook: impl FnMut(Phase) -> Result<()>) -> Result<()> {
            let before =
                self.parent
                    .check_file(&self.file, &self.stage, self.identity, self.maximum)?;
            hook(Phase::BeforeFileSync)?;
            ensure!(
                self.parent
                    .check_file(&self.file, &self.stage, self.identity, self.maximum)?
                    == before,
                "trace changed before file sync"
            );
            self.file.sync_all()?;
            hook(Phase::AfterFileSync)?;
            ensure!(
                self.parent
                    .check_file(&self.file, &self.stage, self.identity, self.maximum)?
                    == before,
                "trace changed during file sync"
            );
            hook(Phase::BeforeRename)?;
            ensure!(
                self.parent
                    .check_file(&self.file, &self.stage, self.identity, self.maximum)?
                    == before,
                "trace changed before publication"
            );
            // Tests can expose the irreducible check/rename window explicitly.
            hook(Phase::RenameReady)?;
            rustix::fs::renameat_with(
                self.parent.file(),
                &self.stage,
                self.parent.file(),
                &self.destination,
                RenameFlags::NOREPLACE,
            )?;
            let published = self.parent.check_file(
                &self.file,
                &self.destination,
                self.identity,
                self.maximum,
            )?;
            // Rename may update ctime. Its full new state is then pinned through directory sync.
            ensure!(
                published.size == before.size && published.modified == before.modified,
                "trace bytes changed during rename"
            );
            hook(Phase::AfterRename)?;
            ensure!(
                self.parent.check_file(
                    &self.file,
                    &self.destination,
                    self.identity,
                    self.maximum
                )? == published,
                "trace changed after rename"
            );
            self.parent.require_absent(&self.stage)?;
            self.parent.sync(&mut hook)?;
            ensure!(
                self.parent.check_file(
                    &self.file,
                    &self.destination,
                    self.identity,
                    self.maximum
                )? == published,
                "trace changed during publication sync"
            );
            self.parent.require_absent(&self.stage)?;
            Ok(())
        }
    }
    #[cfg(test)]
    include!("output/tests.rs");
}

#[cfg(not(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
)))]
mod unsupported {
    use eyre::{Result, eyre};
    use std::{fs::File, path::Path};
    // TODO: implement retained parent handles and atomic no-replace publication on Windows/other targets.
    // No pathname reopen, overwrite, hard-link fallback or unsafe compatibility route is accepted.
    pub(crate) fn new_owned_file(_: &Path) -> Result<File> {
        Err(eyre!(
            "descriptor-owned output is unsupported on this platform"
        ))
    }
    pub(crate) struct TraceOutput {
        file: File,
    }
    impl TraceOutput {
        pub(crate) fn create(_: &Path, _: usize) -> Result<Self> {
            Err(eyre!(
                "descriptor-owned output is unsupported on this platform"
            ))
        }
        pub(crate) fn file_mut(&mut self) -> &mut File {
            &mut self.file
        }
        pub(crate) fn publish(self) -> Result<()> {
            Err(eyre!(
                "descriptor-owned output is unsupported on this platform"
            ))
        }
    }
}
#[cfg(not(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
)))]
pub(super) use unsupported::{TraceOutput, new_owned_file};
