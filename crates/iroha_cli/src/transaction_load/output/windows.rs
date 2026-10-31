//! Native retained Windows output custody using the shared filesystem owner.
//!
//! Trace publication keeps its original handle through strict sealing and no-replace
//! rename. Journal readback keeps the original writer descriptor. Errors preserve
//! incomplete evidence; no cleanup or recovery follows an untrusted pathname.

use eyre::{Result, ensure, eyre};
use iroha_fs::{
    FileSnapshot, OwnerDirectory, PendingPrivateFile, ReaderDirectory, RetainedFile,
    SealedPrivateFile,
};
use sha2::{Digest as _, Sha256};
use std::{
    cell::{Cell, RefCell},
    ffi::OsString,
    fs::File,
    io::{Read as _, Seek as _, SeekFrom},
    path::{Component, Path, PathBuf},
};

pub(crate) mod canonical_inputs {
    include!("canonical_inputs.rs");
}
use canonical_inputs::RawFileIdentity;

fn parent(path: &Path) -> Result<(OwnerDirectory, OsString)> {
    let normalized: PathBuf = path.components().collect();
    ensure!(
        path.is_absolute()
            && path.as_os_str().as_encoded_bytes().len() <= 4096
            && normalized.as_os_str() == path.as_os_str()
            && path.components().count() <= 64
            && !path
                .components()
                .any(|part| matches!(part, Component::CurDir | Component::ParentDir)),
        "output requires a bounded normalized absolute path"
    );
    let name = path
        .file_name()
        .ok_or_else(|| eyre!("output leaf missing"))?;
    let directory = OwnerDirectory::open(
        path.parent()
            .ok_or_else(|| eyre!("output parent missing"))?,
    )?;
    Ok((directory, name.to_owned()))
}

fn check_maximum(maximum: usize) -> Result<()> {
    ensure!(
        maximum > 0 && maximum <= super::super::MAX_FILE_BYTES,
        "invalid output allocation"
    );
    Ok(())
}

pub(crate) struct JournalOutput {
    directory: OwnerDirectory,
    file: RetainedFile,
    maximum: u64,
}
impl JournalOutput {
    pub(crate) fn create(path: &Path, maximum: usize) -> Result<Self> {
        check_maximum(maximum)?;
        let (directory, _) = parent(path)?;
        let file = RetainedFile::create_new_private(path)?;
        directory.revalidate()?;
        ensure!(
            file.file().metadata()?.len() == 0,
            "new journal is not empty"
        );
        Ok(Self {
            directory,
            file,
            maximum: maximum as u64,
        })
    }
    pub(crate) fn writer_file(&self) -> Result<File> {
        self.directory.revalidate()?;
        self.file.revalidate()?;
        ensure!(
            self.file.file().metadata()?.len() == 0,
            "journal writer was already used"
        );
        Ok(self.file.file().try_clone()?)
    }
    pub(crate) fn seal(self, expected: RawFileIdentity) -> Result<RetainedLoadFile> {
        let file = CompletedFile::Journal(self.file.seal()?);
        let retained = RetainedLoadFile::capture(self.directory, file, self.maximum, None)?;
        ensure!(
            retained.identity()? == expected,
            "journal readback differs from written bytes"
        );
        Ok(retained)
    }
}

pub(crate) struct TraceOutput {
    directory: OwnerDirectory,
    file: PendingPrivateFile,
    stage: OsString,
    destination: OsString,
    maximum: u64,
}
impl TraceOutput {
    pub(crate) fn create(path: &Path, maximum: usize) -> Result<Self> {
        check_maximum(maximum)?;
        let (directory, destination) = parent(path)?;
        let name = destination
            .to_str()
            .ok_or_else(|| eyre!("trace output name must be UTF-8"))?;
        ensure!(
            path.as_os_str().as_encoded_bytes().len() + ".collecting".len() <= 4096,
            "trace stage path exceeds bound"
        );
        let stage = OsString::from(format!("{name}.collecting"));
        let file = directory.create_retained_private(&stage, maximum)?;
        Ok(Self {
            directory,
            file,
            stage,
            destination,
            maximum: maximum as u64,
        })
    }
    pub(crate) fn file_mut(&mut self) -> &mut PendingPrivateFile {
        &mut self.file
    }
    pub(crate) fn publish(self) -> Result<RetainedLoadFile> {
        let sealed = self
            .file
            .seal_read_only()?
            .publish_new_name(&self.destination)?;
        RetainedLoadFile::capture(
            self.directory,
            CompletedFile::Trace(RefCell::new(sealed)),
            self.maximum,
            Some(self.stage),
        )
    }
}

enum CompletedFile {
    Journal(RetainedFile),
    Trace(RefCell<SealedPrivateFile>),
}
impl CompletedFile {
    fn snapshot(&self) -> Result<FileSnapshot> {
        Ok(match self {
            Self::Journal(file) => file.snapshot()?,
            Self::Trace(file) => file.borrow().snapshot()?,
        })
    }
    fn digest(&self, length: u64) -> Result<[u8; 32]> {
        match self {
            Self::Journal(file) => canonical_inputs::raw_digest(file.file(), length),
            Self::Trace(file) => {
                let mut file = file.borrow_mut();
                file.seek(SeekFrom::Start(0))?;
                let mut hasher = Sha256::new();
                let mut buffer = [0u8; 64 * 1024];
                let mut remaining = length;
                while remaining > 0 {
                    let count = usize::try_from(remaining.min(buffer.len() as u64))?;
                    file.read_exact(&mut buffer[..count])?;
                    hasher.update(&buffer[..count]);
                    remaining -= count as u64;
                }
                ensure!(
                    file.read(&mut buffer[..1])? == 0,
                    "retained file gained trailing bytes"
                );
                Ok(hasher.finalize().into())
            }
        }
    }
}

pub(crate) struct RetainedLoadFile {
    directory: OwnerDirectory,
    namespace: ReaderDirectory,
    file: CompletedFile,
    snapshot: FileSnapshot,
    identity: RawFileIdentity,
    stage: Option<OsString>,
    poisoned: Cell<bool>,
}
impl std::fmt::Debug for RetainedLoadFile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RetainedLoadFile { .. }")
    }
}
impl RetainedLoadFile {
    fn capture(
        directory: OwnerDirectory,
        file: CompletedFile,
        maximum: u64,
        stage: Option<OsString>,
    ) -> Result<Self> {
        directory.revalidate()?;
        let namespace = ReaderDirectory::open(directory.path())?;
        let snapshot = file.snapshot()?;
        let length = snapshot.local_parts().2;
        ensure!(
            length > 0 && length <= maximum,
            "completed output is empty or exceeds allocation"
        );
        let identity = RawFileIdentity {
            raw_sha256: file.digest(length)?,
            byte_length: length,
        };
        let retained = Self {
            directory,
            namespace,
            file,
            snapshot,
            identity,
            stage,
            poisoned: Cell::new(false),
        };
        retained.check_metadata()?;
        Ok(retained)
    }
    fn check_metadata(&self) -> Result<()> {
        self.directory.revalidate()?;
        self.namespace.revalidate()?;
        ensure!(
            self.file.snapshot()? == self.snapshot,
            "completed load output full state changed"
        );
        if let Some(stage) = &self.stage {
            ensure!(
                matches!(std::fs::symlink_metadata(self.directory.path().join(stage)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound),
                "trace stage name reappeared after publication"
            );
        }
        self.directory.revalidate()?;
        Ok(())
    }
    pub(crate) fn identity(&self) -> Result<RawFileIdentity> {
        ensure!(
            !self.poisoned.replace(true),
            "load output owner is poisoned"
        );
        self.check_metadata()?;
        ensure!(
            self.file.digest(self.identity.byte_length)? == self.identity.raw_sha256,
            "completed load output bytes changed"
        );
        self.check_metadata()?;
        self.poisoned.set(false);
        Ok(self.identity)
    }
    pub(crate) fn pair_identity(&self, other: &Self) -> Result<(RawFileIdentity, RawFileIdentity)> {
        self.pair_identity_with_midpoint(other, || Ok(()))
    }
    pub(crate) fn pair_identity_with_midpoint(
        &self,
        other: &Self,
        midpoint: impl FnOnce() -> Result<()>,
    ) -> Result<(RawFileIdentity, RawFileIdentity)> {
        let checked = (|| {
            let first_parent = self.namespace.snapshot()?;
            let second_parent = other.namespace.snapshot()?;
            let first = self.identity()?;
            midpoint()?;
            let second = other.identity()?;
            self.check_metadata()?;
            other.check_metadata()?;
            ensure!(
                self.namespace.snapshot()? == first_parent
                    && other.namespace.snapshot()? == second_parent,
                "load output namespace changed during complete pair check"
            );
            Ok((first, second))
        })();
        if checked.is_err() {
            self.poisoned.set(true);
            other.poisoned.set(true);
        }
        checked
    }
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
