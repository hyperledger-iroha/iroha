//! Retained command-boundary locks, cancellation, configuration, and executable pins.

use std::{
    ffi::OsString,
    fs::{File, TryLockError},
    io::{self, Read, Seek},
    path::{Path, PathBuf},
    sync::Arc,
};

use iroha_fs::{
    FileIdentity, FileMetadata, FileSnapshot, OwnerDirectory, PrivateDirectory,
    ProcessCommand as Command, ReaderDirectory, RetainedBuildInput, RetainedFile,
};

use super::{Failure, invalid, private_owner};

#[cfg(any(target_os = "linux", target_os = "android"))]
mod cargo_image;

pub(super) struct Lock {
    file: File,
    owner: OwnerDirectory,
    private: PrivateDirectory,
    name: &'static str,
    snapshot: FileSnapshot,
}

impl Lock {
    pub(super) fn acquire(path: &Path, name: &'static str) -> io::Result<Self> {
        let (private, owner) = private_owner(path)?;
        let file = owner.open_lock(name)?;
        let snapshot = FileSnapshot::private_journal(&file)?;
        owner.require_same_lock(name, &file)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "another invocation owns the release lock",
                ));
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
        let result = Self {
            file,
            owner,
            private,
            name,
            snapshot,
        };
        result.revalidate()?;
        Ok(result)
    }

    pub(super) fn revalidate(&self) -> io::Result<()> {
        self.private.revalidate()?;
        self.owner.require_same_lock(self.name, &self.file)?;
        if FileSnapshot::private_journal(&self.file)? != self.snapshot {
            return Err(invalid("release lock original changed"));
        }
        Ok(())
    }
}

pub(super) struct Cancellation {
    directory: PrivateDirectory,
    name: OsString,
}

impl Cancellation {
    pub(super) fn open(path: &Path) -> io::Result<Self> {
        let directory = PrivateDirectory::open_exact(
            path.parent()
                .ok_or_else(|| invalid("cancellation marker has no parent"))?,
        )?;
        let name = path
            .file_name()
            .ok_or_else(|| invalid("cancellation marker has no filename"))?
            .to_owned();
        Ok(Self { directory, name })
    }

    pub(super) fn check(&self) -> Result<(), Failure> {
        const REQUEST: &[u8] = b"{\"reason\":\"operator-request\",\"schema_version\":1}\n";
        let observed = self
            .directory
            .read_optional(&self.name, REQUEST.len() + 1)?;
        self.directory.revalidate()?;
        match observed {
            None => Ok(()),
            Some(bytes) if bytes.as_slice() == REQUEST => Err(Failure::cancelled()),
            Some(_) => {
                Err(invalid("cancellation marker is not the exact canonical request").into())
            }
        }
    }

    pub(super) fn revalidate(&self) -> io::Result<()> {
        self.directory.revalidate()
    }
}

pub(super) struct Tool {
    original: Arc<RetainedBuildInput>,
    snapshot: FileSnapshot,
    evidence: super::provenance::Tool,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    cargo_image: Option<Arc<cargo_image::Image>>,
}

impl Tool {
    pub(super) fn open(path: &Path) -> io::Result<Self> {
        let mut original = RetainedBuildInput::open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            if original.permissions()?.mode() & 0o111 == 0 {
                return Err(invalid("pinned tool is not executable"));
            }
        }
        let snapshot = original.snapshot()?;
        let length = original.len()?;
        let digest = digest_original(&mut original, 2 * 1024 * 1024 * 1024)?;
        original.rewind()?;
        original.revalidate()?;
        if original.snapshot()? != snapshot || length == 0 {
            return Err(invalid("tool original changed while hashing or is empty"));
        }
        Ok(Self {
            original: Arc::new(original),
            snapshot,
            evidence: super::provenance::Tool {
                bytes: length,
                sha256: digest,
            },
            #[cfg(any(target_os = "linux", target_os = "android"))]
            cargo_image: None,
        })
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub(super) fn publish_cargo_image(
        mut self,
        parent: &PrivateDirectory,
        name: &str,
    ) -> io::Result<Self> {
        self.revalidate()?;
        if self.cargo_image.is_some() {
            return Err(invalid("Cargo already has a selected immutable image"));
        }
        let original = Arc::get_mut(&mut self.original)
            .ok_or_else(|| invalid("Cargo image must be published before command binding"))?;
        let image = cargo_image::publish(
            original,
            self.snapshot,
            &self.evidence,
            parent,
            name,
            |_| Ok(()),
        )?;
        self.cargo_image = Some(image);
        self.revalidate()?;
        Ok(self)
    }

    pub(super) fn evidence(&self) -> io::Result<super::provenance::Tool> {
        self.revalidate()?;
        Ok(self.evidence.clone())
    }

    pub(super) fn command(&self) -> io::Result<Command> {
        self.revalidate()?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        if let Some(image) = &self.cargo_image {
            return image.executable.command();
        }
        self.original.command()
    }

    pub(super) fn source_command(&self) -> io::Result<Command> {
        self.revalidate()?;
        self.original.command()
    }

    pub(super) fn bind(&self, command: &mut Command, key: &str) -> io::Result<()> {
        self.revalidate()?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        if let Some(image) = &self.cargo_image {
            command.env_sealed_tool_image(key, &image.executable)?;
            return Ok(());
        }
        command.env_retained_build_input(key, &self.original)?;
        Ok(())
    }

    pub(super) fn require_nested_transcript(
        &self,
        argument: &str,
        expected: &[u8],
        run: impl FnOnce(Command) -> Result<Vec<u8>, Failure>,
    ) -> Result<(), Failure> {
        // This original command owns the fresh CLOEXEC duplicate whose exact
        // path a nested launcher receives. Keep it alive through the probe's
        // original terminal observation. Only argv0 changes: executable lookup
        // remains bound to that original, without a launcher or mutable fallback.
        const KEY: &str = "IROHA_NESTED_TOOL_PREFLIGHT";
        let mut binding = self.command()?;
        self.bind(&mut binding, KEY)?;
        let selected = binding
            .get_envs()
            .find(|(key, _)| *key == KEY)
            .and_then(|(_, value)| value)
            .ok_or_else(|| invalid("retained nested tool binding is absent"))?;
        let mut probe = Command::new(Path::new(selected))?;
        probe.arg(argument);
        let observed = run(probe);
        let custody = self.revalidate();
        drop(binding);
        // Preserve an original command observation error over a later custody
        // refusal; neither may become successful preflight.
        let observed = observed?;
        custody?;
        if observed != expected {
            return Err(invalid(
                "nested original tool transcript differs; unsupported proxy or tool behavior",
            )
            .into());
        }
        Ok(())
    }

    pub(super) fn revalidate(&self) -> io::Result<()> {
        self.original.revalidate()?;
        if self.original.snapshot()? != self.snapshot {
            return Err(invalid("pinned tool changed"));
        }
        #[cfg(any(target_os = "linux", target_os = "android"))]
        if let Some(image) = &self.cargo_image {
            let witness = image.batch.revalidate();
            let executable = image.executable.revalidate();
            witness?;
            executable?;
        }
        Ok(())
    }
}

fn digest_original(reader: &mut impl Read, maximum: u64) -> io::Result<String> {
    use sha2::{Digest as _, Sha256};
    let mut hash = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .filter(|value| *value <= maximum)
            .ok_or_else(|| invalid("original input exceeds its hash ceiling"))?;
        hash.update(&buffer[..count]);
    }
    Ok(hex::encode(hash.finalize()))
}

struct ConfigFile {
    name: &'static str,
    original: Option<(RetainedFile, FileSnapshot)>,
    digest: Option<String>,
}

impl ConfigFile {
    fn open(parent: &ReaderDirectory, name: &'static str) -> io::Result<Self> {
        let mut original = parent
            .open_retained_regular_optional(name)?
            .map(|file| file.snapshot().map(|snapshot| (file, snapshot)))
            .transpose()?;
        let digest = match &mut original {
            Some((file, snapshot)) => {
                let digest = digest_original(file.file_mut(), 1024 * 1024)?;
                file.file_mut().rewind()?;
                file.revalidate()?;
                if file.snapshot()? != *snapshot {
                    return Err(invalid("configuration changed while hashing"));
                }
                Some(digest)
            }
            None => None,
        };
        Ok(Self {
            name,
            original,
            digest,
        })
    }

    fn revalidate(&self, parent: &ReaderDirectory) -> io::Result<()> {
        match &self.original {
            Some((file, snapshot)) => {
                file.revalidate()?;
                if file.snapshot()? != *snapshot {
                    return Err(invalid("Cargo configuration changed"));
                }
            }
            None => {
                if parent.open_retained_regular_optional(self.name)?.is_some() {
                    return Err(invalid("an absent Cargo configuration appeared"));
                }
            }
        }
        parent.revalidate()
    }
}

struct ConfigDirectory {
    parent: ReaderDirectory,
    name: Option<&'static str>,
    directory: Option<ReaderDirectory>,
    files: Vec<ConfigFile>,
}

impl ConfigDirectory {
    fn open(parent: &Path, name: Option<&'static str>) -> io::Result<Self> {
        let parent = ReaderDirectory::open(parent)?;
        let directory = match name {
            Some(name) => parent.open_child_optional(name)?,
            None => Some(ReaderDirectory::open(parent.path())?),
        };
        let files = match &directory {
            Some(directory) => ["config", "config.toml"]
                .into_iter()
                .map(|name| ConfigFile::open(directory, name))
                .collect::<io::Result<Vec<_>>>()?,
            None => Vec::new(),
        };
        Ok(Self {
            parent,
            name,
            directory,
            files,
        })
    }

    fn revalidate(&self) -> io::Result<()> {
        self.parent.revalidate()?;
        if let Some(directory) = &self.directory {
            directory.revalidate()?;
            for file in &self.files {
                file.revalidate(directory)?;
            }
        } else if self
            .parent
            .open_child_optional(
                self.name
                    .ok_or_else(|| invalid("missing configuration directory name"))?,
            )?
            .is_some()
        {
            return Err(invalid("an absent Cargo configuration directory appeared"));
        }
        self.parent.revalidate()
    }
}

pub(super) struct Configuration {
    directories: Vec<ConfigDirectory>,
    source: Arc<ReaderDirectory>,
    cargo_home: ReaderDirectory,
}

impl Configuration {
    pub(super) fn open(source: &Path, cargo_home: &Path) -> io::Result<Self> {
        let mut directories = source
            .ancestors()
            .map(|parent| ConfigDirectory::open(parent, Some(".cargo")))
            .collect::<io::Result<Vec<_>>>()?;
        directories.push(ConfigDirectory::open(cargo_home, None)?);
        let result = Self {
            directories,
            source: Arc::new(ReaderDirectory::open(source)?),
            cargo_home: ReaderDirectory::open(cargo_home)?,
        };
        result.revalidate()?;
        Ok(result)
    }

    pub(super) fn evidence(&self) -> io::Result<Vec<super::provenance::Configuration>> {
        self.revalidate()?;
        self.directories
            .iter()
            .map(|directory| {
                let path = directory.name.map_or_else(
                    || directory.parent.path().to_owned(),
                    |name| directory.parent.path().join(name),
                );
                let path = path
                    .to_str()
                    .ok_or_else(|| invalid("configuration path is not UTF-8"))?
                    .to_owned();
                let digest = |name| {
                    directory
                        .files
                        .iter()
                        .find(|file| file.name == name)
                        .and_then(|file| file.digest.clone())
                };
                Ok(super::provenance::Configuration {
                    path,
                    present: directory.directory.is_some(),
                    config_sha256: digest("config"),
                    config_toml_sha256: digest("config.toml"),
                })
            })
            .collect()
    }

    pub(super) fn bind_current_directory(&self, command: &mut Command) -> io::Result<()> {
        command.current_dir_retained(&self.source)?;
        Ok(())
    }

    pub(super) fn revalidate(&self) -> io::Result<()> {
        self.source.revalidate()?;
        self.cargo_home.revalidate()?;
        for directory in &self.directories {
            directory.revalidate()?;
        }
        Ok(())
    }
}

pub(super) fn disjoint(paths: &[&Path]) -> io::Result<()> {
    // Windows path components are case-insensitive on the qualified NTFS owner. Native
    // directory admission separately rejects reparses; aliases cannot bypass that custody.
    let paths: Vec<_> = paths
        .iter()
        .map(|path| {
            if cfg!(windows) {
                PathBuf::from(path.to_string_lossy().to_lowercase())
            } else {
                (*path).to_owned()
            }
        })
        .collect();
    for (index, left) in paths.iter().enumerate() {
        for right in &paths[index + 1..] {
            if left.starts_with(right) || right.starts_with(left) {
                return Err(invalid(
                    "source, build, artifact and cancellation roots must be disjoint",
                ));
            }
        }
    }
    Ok(())
}

pub(super) struct DisjointRoots {
    originals: Vec<ReaderDirectory>,
    roots: Vec<FileIdentity>,
}

impl DisjointRoots {
    pub(super) fn open(paths: &[&Path]) -> io::Result<Self> {
        let mut originals = Vec::new();
        let mut records = Vec::new();
        for path in paths {
            let mut identities = Vec::new();
            for ancestor in path.ancestors() {
                let original = ReaderDirectory::open(ancestor)?;
                identities.push(original.identity()?);
                originals.push(original);
            }
            records.push(identities);
        }
        for (index, left) in records.iter().enumerate() {
            let root = left
                .first()
                .ok_or_else(|| invalid("missing original root identity"))?;
            for right in &records[index + 1..] {
                let other = right
                    .first()
                    .ok_or_else(|| invalid("missing original root identity"))?;
                if right.contains(root) || left.contains(other) {
                    return Err(invalid(
                        "release roots physically overlap through native directory identities",
                    ));
                }
            }
        }
        let roots = records.iter().map(|identities| identities[0]).collect();
        let result = Self { originals, roots };
        result.revalidate()?;
        Ok(result)
    }

    pub(super) fn revalidate(&self) -> io::Result<()> {
        for original in &self.originals {
            original.revalidate()?;
        }
        Ok(())
    }

    pub(super) fn exclude_marker(&mut self, marker: &Path) -> io::Result<()> {
        let parent = marker
            .parent()
            .ok_or_else(|| invalid("cancellation marker has no parent"))?;
        for ancestor in parent.ancestors() {
            let original = ReaderDirectory::open(ancestor)?;
            if self.roots.contains(&original.identity()?) {
                return Err(invalid(
                    "cancellation marker is inside a source or output root",
                ));
            }
            self.originals.push(original);
        }
        // A private shared ancestor may contain sibling outputs and the marker.
        // Existing directory aliases (including 8.3 spellings) must not select a
        // protected root or its ancestor as the purported marker file.
        match FileMetadata::symlink_metadata(marker) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(invalid("cancellation marker is indirect"));
            }
            Ok(metadata) if metadata.is_dir() => {
                for original in &self.originals {
                    if original.identity()? == metadata.identity() {
                        return Err(invalid("cancellation marker names a protected directory"));
                    }
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        self.revalidate()
    }
}
