//! Exclusive existing-only custody shared by regenerable cache and durable journal.

use iroha_fs::{FileIdentity, FileSnapshot, PrivateDirectory};
use std::{
    fs::File,
    io::{self, Read as _, Write as _},
    path::Path,
};

pub(super) const LOCK: &str = "owner.lock";
pub(super) const SELECTION: &str = "selection";
const SELECTION_MAX: usize = 1 << 20;

#[derive(Clone, Copy)]
pub(super) enum Mode {
    Initialize,
    Open,
}

pub(super) struct Custody {
    pub(super) directory: PrivateDirectory,
    owner: File,
    identity: FileIdentity,
    selection: FileSnapshot,
}

pub(super) fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "finality storage custody differs",
    )
}

impl Custody {
    pub(super) fn acquire(path: &Path, selection: &[u8], mode: Mode) -> io::Result<Self> {
        if selection.is_empty() || selection.len() > SELECTION_MAX {
            return Err(invalid());
        }
        let directory = PrivateDirectory::open_exact(path)?;
        let owner = match mode {
            Mode::Initialize => {
                if !directory.entries(1)?.is_empty() {
                    return Err(invalid());
                }
                directory.create_lock(LOCK)?
            }
            Mode::Open => directory.open_existing_lock(LOCK)?,
        };
        owner.try_lock().map_err(io::Error::other)?;
        let identity = FileIdentity::of(&owner)?;
        if matches!(mode, Mode::Initialize) {
            if directory.entries(2)? != [std::ffi::OsString::from(LOCK)] {
                return Err(invalid());
            }
            let mut writer =
                directory.create_retained_private("selection.pending", SELECTION_MAX)?;
            writer.write_all(selection)?;
            writer
                .seal_read_only()?
                .publish_new_name(SELECTION)?
                .revalidate()?;
            directory.sync()?;
        }
        let mut selected = directory.open_retained_read_only(SELECTION, SELECTION_MAX)?;
        let before = selected.snapshot()?;
        let mut bytes = Vec::new();
        selected
            .by_ref()
            .take(SELECTION_MAX as u64 + 1)
            .read_to_end(&mut bytes)?;
        selected.revalidate()?;
        if selected.snapshot()? != before || bytes != selection {
            return Err(invalid());
        }
        let custody = Self {
            directory,
            owner,
            identity,
            selection: before,
        };
        custody.guard()?;
        Ok(custody)
    }

    pub(super) fn guard(&self) -> io::Result<()> {
        self.directory.revalidate()?;
        if FileIdentity::of(&self.owner)? != self.identity
            || FileIdentity::of(&self.directory.open_existing_lock(LOCK)?)? != self.identity
        {
            return Err(invalid());
        }
        let selected = self
            .directory
            .open_retained_read_only(SELECTION, SELECTION_MAX)?;
        if selected.snapshot()? != self.selection {
            return Err(invalid());
        }
        selected.revalidate()
    }
}
