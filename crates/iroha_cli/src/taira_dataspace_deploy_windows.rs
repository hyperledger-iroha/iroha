//! Native Windows private journal custody and original-object publication.

use super::*;
use iroha_fs::{FileSnapshot, MovableFile, PrivateDirectory};
use std::io::{Read, Seek, Write};

#[path = "taira_dataspace_deploy_windows_refresh.rs"]
mod refresh;

impl Journal {
    pub(super) fn open(path: &Path, create: bool) -> Result<Self> {
        let absolute = std::path::absolute(path)?;
        let parent = PrivateDirectory::open(
            absolute
                .parent()
                .ok_or_else(|| eyre!("journal has no parent"))?,
        )?;
        Self::open_native_child(
            &parent,
            absolute
                .file_name()
                .ok_or_else(|| eyre!("journal has no name"))?,
            create,
            false,
        )
    }

    pub(super) fn open_unpublished(path: &Path) -> Result<Self> {
        let absolute = std::path::absolute(path)?;
        let parent = PrivateDirectory::open(
            absolute
                .parent()
                .ok_or_else(|| eyre!("journal has no parent"))?,
        )?;
        Self::open_native_child(
            &parent,
            absolute
                .file_name()
                .ok_or_else(|| eyre!("journal has no name"))?,
            true,
            true,
        )
    }

    pub(super) fn open_native_child(
        parent: &PrivateDirectory,
        name: &std::ffi::OsStr,
        create: bool,
        recover_unplanned: bool,
    ) -> Result<Self> {
        let (directory, created) = if create {
            match parent.create_child(name) {
                Ok(directory) => (directory, true),
                Err(error)
                    if recover_unplanned && error.kind() == std::io::ErrorKind::AlreadyExists =>
                {
                    (parent.open_child(name)?, false)
                }
                Err(error) => return Err(error.into()),
            }
        } else {
            (parent.open_child(name)?, false)
        };
        Self::lock_native(directory, created, recover_unplanned)
    }

    pub(super) fn lock_native(
        directory: PrivateDirectory,
        created: bool,
        recover_unplanned: bool,
    ) -> Result<Self> {
        let (lock, lock_created) = if created {
            (directory.create_lock("lock")?, true)
        } else {
            match directory.open_existing_lock("lock") {
                Ok(lock) => (lock, false),
                Err(error) if recover_unplanned && error.kind() == std::io::ErrorKind::NotFound => {
                    // Lost locks are recoverable only before any binding or evidence exists.
                    for name in directory.entries(100_002)? {
                        require(
                            name.to_str().is_some_and(unsigned_staging_name),
                            "operation journal contains evidence without a durable lock",
                        )?;
                        directory.open_retained_private(&name)?.revalidate()?;
                    }
                    (directory.create_lock("lock")?, true)
                }
                Err(error) => return Err(error.into()),
            }
        };
        lock.try_lock()
            .wrap_err("another process holds this operation journal")?;
        if lock_created {
            directory.sync()?;
        } else {
            directory.revalidate()?;
        }
        let value = Self {
            path: directory.path().to_owned(),
            directory,
            lock_snapshot: FileSnapshot::private_journal(&lock)?,
            _lock: lock,
        };
        value.revalidate()?;
        Ok(value)
    }

    pub(super) fn revalidate(&self) -> Result<()> {
        self.directory.revalidate()?;
        let current = FileSnapshot::private_journal(&self._lock)?;
        let named = self.directory.open_read("lock")?;
        require(
            current == self.lock_snapshot
                && current == FileSnapshot::private_journal(&named)?
                && self._lock.metadata()?.len() == 0,
            "journal lock or its original namespace changed",
        )?;
        self.directory.revalidate()?;
        Ok(())
    }

    pub(super) fn entry_names(&self) -> Result<Vec<std::ffi::OsString>> {
        self.revalidate()?;
        let names = self.directory.entries(100_002)?;
        self.revalidate()?;
        Ok(names)
    }

    pub(super) fn require_private_regular(&self, name: &std::ffi::OsStr) -> Result<()> {
        if name == "lock" {
            return self.revalidate();
        }
        self.directory.open_retained_private(name)?.revalidate()?;
        if name.to_str().is_some_and(refresh::unsigned_control_stage) {
            self.read_native(
                name.to_str().expect("checked UTF-8"),
                refresh::CONTROL_BOUND,
            )?;
        }
        self.revalidate()
    }

    pub(super) fn require_unplanned(&self) -> Result<()> {
        self.require_no_refresh()?;
        for name in self.entry_names()? {
            require(
                name == "lock" || name.to_str().is_some_and(unsigned_staging_name),
                "operation journal contains evidence without a durable plan",
            )?;
            self.require_private_regular(&name)?;
        }
        self.revalidate()
    }

    pub(super) fn require_unsigned_entries(&self, include_plan: bool) -> Result<()> {
        self.require_no_refresh()?;
        for name in self.entry_names()? {
            if name == "preflight-finality" {
                finality::validate_preflight_cache(self)?;
                continue;
            }
            require(
                name.to_str().is_some_and(|name| {
                    matches!(name, "lock" | "definition.json")
                        || (include_plan && name == "plan.json")
                        || unsigned_staging_name(name)
                        || refresh::unsigned_control_stage(name)
                }),
                "operation already contains preparation or deployment evidence",
            )?;
            self.require_private_regular(&name)?;
        }
        require(
            self.read_native("definition.json", MAX_BYTES)?.is_some(),
            "unsigned plan has no retained definition binding",
        )?;
        self.revalidate()
    }

    pub(super) fn require_definition_only(&self) -> Result<()> {
        self.require_unsigned_entries(false)
    }

    pub(super) fn require_unprepared_plan(&self, expected: &PlanV1) -> Result<()> {
        self.require_unsigned_entries(true)?;
        require(
            self.read_native("plan.json", MAX_BYTES)?.as_deref()
                == Some(json::to_vec(expected)?.as_slice()),
            "retained plan changed before unsigned refresh",
        )?;
        self.revalidate()
    }

    pub(super) fn read_native(&self, name: &str, maximum: usize) -> Result<Option<Vec<u8>>> {
        self.revalidate()?;
        let Some(mut original) = self.directory.open_retained_private_optional(name)? else {
            self.revalidate()?;
            return Ok(None);
        };
        let before = original.snapshot()?;
        require(
            original.file().metadata()?.len() <= u64::try_from(maximum)?,
            "journal file exceeds bound",
        )?;
        let mut bytes = Vec::new();
        original
            .file_mut()
            .take(
                u64::try_from(maximum)?
                    .checked_add(1)
                    .ok_or_else(|| eyre!("journal bound overflow"))?,
            )
            .read_to_end(&mut bytes)?;
        require(bytes.len() <= maximum, "journal file exceeds bound")?;
        // Recheck bytes as well as metadata: timestamp collisions are not byte custody.
        let mut offset = 0;
        let mut buffer = [0_u8; 65536];
        for chunk in bytes.chunks(buffer.len()) {
            iroha_fs::read_exact_at(original.file(), &mut buffer[..chunk.len()], offset)?;
            require(
                &buffer[..chunk.len()] == chunk,
                "journal original changed during read",
            )?;
            offset += u64::try_from(chunk.len())?;
        }
        require(
            iroha_fs::read_at(original.file(), &mut [0], offset)? == 0
                && original.snapshot()? == before,
            "journal original changed during read",
        )?;
        original.revalidate()?;
        self.revalidate()?;
        Ok(Some(bytes))
    }

    pub(super) fn read_optional_bounded(
        &self,
        name: &str,
        maximum: usize,
    ) -> Result<Option<Vec<u8>>> {
        self.require_no_refresh()?;
        self.read_native(name, maximum)
    }

    pub(super) fn install_native(&self, name: &str, bytes: &[u8], maximum: usize) -> Result<()> {
        require(bytes.len() <= maximum, "journal output exceeds bound")?;
        self.revalidate()?;
        let temporary = format!(
            ".staging-{name}-{}",
            hex::encode(rand::random::<[u8; 16]>())
        );
        let mut writer = self
            .directory
            .create_retained_private(&temporary, maximum)?;
        writer.write_all(bytes)?;
        let mut original = writer.finish_private()?;
        original.rename_private_no_replace_observed(name, |_| Ok(()))?;
        require(
            read_movable(&mut original, maximum)? == bytes,
            "published journal original changed",
        )?;
        original.revalidate()?;
        self.revalidate()
    }

    pub(super) fn install_bounded(&self, name: &str, bytes: &[u8], maximum: usize) -> Result<()> {
        self.require_no_refresh()?;
        self.install_native(name, bytes, maximum)
    }
}

fn read_movable(original: &mut MovableFile, maximum: usize) -> Result<Vec<u8>> {
    let before = original.snapshot()?;
    original.rewind()?;
    let mut bytes = Vec::new();
    Read::by_ref(original)
        .take(
            u64::try_from(maximum)?
                .checked_add(1)
                .ok_or_else(|| eyre!("journal bound overflow"))?,
        )
        .read_to_end(&mut bytes)?;
    require(
        bytes.len() <= maximum && original.snapshot()? == before,
        "private original exceeds bound or changed",
    )?;
    Ok(bytes)
}
