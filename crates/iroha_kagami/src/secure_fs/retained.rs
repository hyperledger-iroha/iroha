// Descriptor and namespace custody for generated originals and their final receipt.
const MAX_RETAINED_PRIVATE_FILES: usize = 80;
const MAX_RETAINED_PRIVATE_BYTES: u64 = 64 * 1024 * 1024;

struct RetainedPrivateFile {
    name: String,
    file: File,
    metadata: fs::Metadata,
    bytes: zeroize::Zeroizing<Vec<u8>>,
    digest: [u8; 32],
}

struct RetainedPrivateDirectory {
    path: String,
    name: String,
    parent: Option<usize>,
    file: File,
    metadata: fs::Metadata,
}

struct PrivatePositionalReader<'a> {
    file: &'a File,
    offset: u64,
}
impl Read for PrivatePositionalReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        use std::os::unix::fs::FileExt as _;
        let count = self.file.read_at(bytes, self.offset)?;
        self.offset = self
            .offset
            .checked_add(count as u64)
            .ok_or_else(|| std::io::Error::other("private read offset overflow"))?;
        Ok(count)
    }
}

/// Bounded generated originals with descriptor and complete ancestor custody.
///
/// File bytes, including secret-bearing configs, are erased on every drop path.
/// This owner never follows symlinks or replaces an existing publication.
pub struct RetainedPrivateFiles {
    root: OpenPrivateTreeRoot,
    path: PathBuf,
    files: Vec<RetainedPrivateFile>,
    total: u64,
    directories: Vec<RetainedPrivateDirectory>,
    sealed_namespace: bool,
}
impl RetainedPrivateFiles {
    /// Retain an existing absolute owner-only output directory and its ancestry.
    pub fn new(path: &Path) -> Result<Self> {
        if !path.is_absolute() || path.as_os_str().as_bytes().len() > 4096 {
            return Err(eyre!(
                "retained private root must be a bounded absolute path"
            ));
        }
        Ok(Self {
            root: open_private_tree_root(path)?,
            path: path.to_owned(),
            files: Vec::new(),
            total: 0,
            directories: Vec::new(),
            sealed_namespace: false,
        })
    }

    fn leaf(name: &str) -> Result<()> {
        if name.is_empty()
            || name.len() > 128
            || name == "."
            || name == ".."
            || !name.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
            })
        {
            return Err(eyre!("retained private artifact name is not a fixed leaf"));
        }
        Ok(())
    }

    fn open_leaf(&self, name: &str) -> Result<File> {
        Self::leaf(name)?;
        Ok(File::from(
            openat(
                &self.root.root,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)?,
        ))
    }

    /// Retain one already-created directory in parent-before-child order.
    /// Only a bounded fixed relative path under the original output root is accepted.
    pub fn capture_directory(&mut self, path: &str) -> Result<()> {
        self.check()?;
        if self.sealed_namespace || self.directories.len() >= 32 || path.len() > 128 {
            return Err(eyre!("retained directory reservation is invalid"));
        }
        for name in path.split('/') {
            Self::leaf(name)?;
        }
        if self.directories.iter().any(|entry| entry.path == path) {
            return Err(eyre!("duplicate retained directory"));
        }
        let (parent, name) = if let Some((parent, name)) = path.rsplit_once('/') {
            (
                Some(
                    self.directories
                        .iter()
                        .position(|entry| entry.path == parent)
                        .ok_or_else(|| eyre!("retained directory parent was not admitted"))?,
                ),
                name,
            )
        } else {
            (None, path)
        };
        let parent_file = parent.map_or(&self.root.root, |index| &self.directories[index].file);
        let file = File::from(
            openat(
                parent_file,
                name,
                OFlags::RDONLY
                    | OFlags::DIRECTORY
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)?,
        );
        let metadata = file.metadata()?;
        if !metadata.is_dir()
            || metadata.uid() != current_uid()
            || metadata.mode() & 0o7777 != PRIVATE_DIRECTORY_MODE
            || self.directories.iter().any(|entry| {
                entry.metadata.dev() == metadata.dev() && entry.metadata.ino() == metadata.ino()
            })
        {
            return Err(eyre!("retained directory custody is invalid"));
        }
        self.directories.push(RetainedPrivateDirectory {
            path: path.to_owned(),
            name: name.to_owned(),
            parent,
            file,
            metadata,
        });
        self.check()
    }

    fn directory_entries(file: &File, expected: BTreeSet<&str>) -> Result<()> {
        let mut entries = Dir::read_from(file).map_err(std::io::Error::from)?;
        let mut seen = BTreeSet::new();
        for entry in &mut entries {
            let entry = entry.map_err(std::io::Error::from)?;
            let bytes = entry.file_name().to_bytes();
            if matches!(bytes, b"." | b"..") {
                continue;
            }
            let name = std::str::from_utf8(bytes)
                .map_err(|_| eyre!("retained namespace name is not UTF-8"))?;
            if seen.len() >= expected.len()
                || !expected.contains(name)
                || !seen.insert(name.to_owned())
            {
                return Err(eyre!("retained namespace contains an unexpected entry"));
            }
        }
        if seen.len() != expected.len() {
            return Err(eyre!("retained namespace is incomplete"));
        }
        Ok(())
    }

    /// Seal the exact immutable files and admitted initial empty runtime directory tree.
    /// No further publication is allowed after this point.
    pub fn seal_namespace(&mut self) -> Result<()> {
        if self.sealed_namespace {
            return Err(eyre!("retained namespace is already sealed"));
        }
        self.sealed_namespace = true;
        self.check()
    }

    /// Capture one original, optionally comparing it with the bytes just generated.
    /// The per-file limit and aggregate 64-MiB limit are checked before allocation.
    pub fn capture(&mut self, name: &str, maximum: u64, expected: Option<&[u8]>) -> Result<usize> {
        self.check()?;
        if self.sealed_namespace
            || self.files.len() >= MAX_RETAINED_PRIVATE_FILES
            || maximum == 0
            || maximum > MAX_RETAINED_PRIVATE_BYTES
            || self.files.iter().any(|file| file.name == name)
        {
            return Err(eyre!("retained private artifact reservation is invalid"));
        }
        let file = self.open_leaf(name)?;
        let before = file.metadata()?;
        let total = self
            .total
            .checked_add(before.len())
            .ok_or_else(|| eyre!("retained private byte overflow"))?;
        if !before.is_file()
            || before.uid() != current_uid()
            || before.mode() & 0o7777 != PRIVATE_FILE_MODE
            || before.nlink() != 1
            || before.len() == 0
            || before.len() > maximum
            || total > MAX_RETAINED_PRIVATE_BYTES
            || self.files.iter().any(|other| {
                other.metadata.dev() == before.dev() && other.metadata.ino() == before.ino()
            })
        {
            return Err(eyre!(
                "retained private artifact custody or size is invalid"
            ));
        }
        let mut bytes = zeroize::Zeroizing::new(Vec::new());
        bytes.try_reserve_exact(usize::try_from(before.len())?)?;
        PrivatePositionalReader {
            file: &file,
            offset: 0,
        }
        .take(before.len() + 1)
        .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != before.len()
            || !same_file(&before, &file.metadata()?)
            || expected.is_some_and(|expected| expected != bytes.as_slice())
        {
            return Err(eyre!(
                "retained private artifact changed or differs from generated bytes"
            ));
        }
        let digest = iroha_crypto::sha256(bytes.as_slice());
        self.files.push(RetainedPrivateFile {
            name: name.to_owned(),
            file,
            metadata: before,
            bytes,
            digest,
        });
        self.total = total;
        self.check()?;
        Ok(self.files.len() - 1)
    }

    /// Borrow exact admitted bytes while the descriptor and ancestors remain held.
    pub fn bytes(&self, index: usize) -> Result<&[u8]> {
        self.files
            .get(index)
            .map(|file| file.bytes.as_slice())
            .ok_or_else(|| eyre!("retained private artifact index is invalid"))
    }

    /// Public raw identities; no original content or filesystem path is exposed.
    pub fn identities(&self) -> impl Iterator<Item = (&str, [u8; 32], u64)> {
        self.files
            .iter()
            .map(|file| (file.name.as_str(), file.digest, file.metadata.len()))
    }

    /// Recheck every descriptor, named leaf, full file metadata, digest and ancestor.
    pub fn check(&self) -> Result<()> {
        self.root.verify_ancestry(&self.path)?;
        let root = self.root.root.metadata()?;
        if !root.is_dir()
            || root.uid() != current_uid()
            || root.mode() & 0o7777 != PRIVATE_DIRECTORY_MODE
            || PrivateFileIdentity::from_metadata(&root)
                != PrivateFileIdentity::from_metadata(&self.root.root_before)
        {
            return Err(eyre!("retained private root changed"));
        }
        for file in &self.files {
            #[cfg(test)]
            retained_test_event(&self.path, "check", &file.name);
            if !same_file(&file.metadata, &file.file.metadata()?)
                || !same_file(&file.metadata, &self.open_leaf(&file.name)?.metadata()?)
            {
                return Err(eyre!("retained private artifact identity changed"));
            }
            let (digest, count) = iroha_crypto::sha256_reader_bounded(
                PrivatePositionalReader {
                    file: &file.file,
                    offset: 0,
                },
                file.metadata.len(),
            )?;
            if digest != file.digest
                || count != file.metadata.len()
                || !same_file(&file.metadata, &file.file.metadata()?)
            {
                return Err(eyre!("retained private artifact bytes changed"));
            }
        }
        for (index, directory) in self.directories.iter().enumerate() {
            let parent = directory
                .parent
                .map_or(&self.root.root, |parent| &self.directories[parent].file);
            let named = File::from(
                openat(
                    parent,
                    directory.name.as_str(),
                    OFlags::RDONLY
                        | OFlags::DIRECTORY
                        | OFlags::NOFOLLOW
                        | OFlags::NONBLOCK
                        | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(std::io::Error::from)?,
            );
            if !same_file(&directory.metadata, &directory.file.metadata()?)
                || !same_file(&directory.metadata, &named.metadata()?)
            {
                return Err(eyre!("retained runtime directory identity changed"));
            }
            if self.sealed_namespace {
                Self::directory_entries(
                    &directory.file,
                    self.directories
                        .iter()
                        .filter(|child| child.parent == Some(index))
                        .map(|child| child.name.as_str())
                        .collect(),
                )?;
            }
        }
        if self.sealed_namespace {
            Self::directory_entries(
                &self.root.root,
                self.files
                    .iter()
                    .map(|file| file.name.as_str())
                    .chain(
                        self.directories
                            .iter()
                            .filter(|entry| entry.parent.is_none())
                            .map(|entry| entry.name.as_str()),
                    )
                    .collect(),
            )?;
        }
        for directory in &self.directories {
            if !same_file(&directory.metadata, &directory.file.metadata()?) {
                return Err(eyre!(
                    "retained runtime namespace changed during verification"
                ));
            }
        }
        if !same_file(&root, &self.root.root.metadata()?) {
            return Err(eyre!(
                "retained private namespace changed during verification"
            ));
        }
        self.root.verify_ancestry(&self.path)
    }

    /// Exclusively create, durably write and retain one bounded generated artifact.
    /// Failure preserves surviving bytes; only a successful final caller flush completes publication.
    pub fn write_new(&mut self, name: &str, bytes: &[u8], maximum: u64) -> Result<()> {
        Self::leaf(name)?;
        self.check()?;
        if self.sealed_namespace
            || bytes.is_empty()
            || maximum == 0
            || maximum > 1024 * 1024
            || bytes.len() as u64 > maximum
            || self.files.len() >= MAX_RETAINED_PRIVATE_FILES
            || self
                .total
                .checked_add(bytes.len() as u64)
                .is_none_or(|total| total > MAX_RETAINED_PRIVATE_BYTES)
        {
            return Err(eyre!(
                "retained private generated artifact exceeds its byte bound"
            ));
        }
        let mut file = File::from(
            openat(
                &self.root.root,
                name,
                OFlags::RDWR
                    | OFlags::CREATE
                    | OFlags::EXCL
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::CLOEXEC,
                Mode::from_raw_mode(PRIVATE_FILE_MODE as _),
            )
            .map_err(std::io::Error::from)?,
        );
        file.write_all(bytes)?;
        file.sync_all()?;
        self.root.root.sync_all()?;
        let created = file.metadata()?;
        #[cfg(test)]
        retained_test_event(&self.path, "published", name);
        let index = self.capture(name, maximum, Some(bytes))?;
        if !same_file(&created, &self.files[index].metadata)
            || !same_file(&created, &file.metadata()?)
        {
            return Err(eyre!("retained private publication identity changed"));
        }
        self.check()
    }
}

#[cfg(test)]
thread_local! {
    static RETAINED_TEST_HOOK: std::cell::RefCell<Option<RetainedTestHook>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
struct RetainedTestHook {
    root: PathBuf,
    event: &'static str,
    name: String,
    action: Box<dyn FnOnce()>,
}
#[cfg(test)]
fn retained_test_event(root: &Path, event: &str, name: &str) {
    let hook = RETAINED_TEST_HOOK.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot
            .as_ref()
            .is_some_and(|hook| hook.root == root && hook.event == event && hook.name == name)
        {
            slot.take()
        } else {
            None
        }
    });
    if let Some(hook) = hook {
        (hook.action)();
    }
}
#[cfg(test)]
#[path = "retained_tests.rs"]
mod retained_tests;
