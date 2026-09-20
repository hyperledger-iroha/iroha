// Retain exact journal and published trace bytes through terminal receipt flush.
use super::canonical_inputs::{RawFileIdentity, raw_digest};
use super::*;
use std::cell::Cell;

/// Mutable journal's original descriptor, parent and independently admitted cap.
pub(crate) struct JournalOutput {
    parent: Parent,
    name: OsString,
    file: File,
    identity: Identity,
    maximum: u64,
}
impl JournalOutput {
    /// Create exactly one fresh private journal with readback access on its own handle.
    pub(crate) fn create(path: &Path, maximum: usize) -> Result<Self> {
        ensure!(
            maximum > 0 && maximum <= super::super::super::MAX_FILE_BYTES,
            "invalid journal allocation"
        );
        let (parent, name) = Parent::capture(path, &mut |_| Ok(()))?;
        let (file, identity) = parent.create_with_access(&name, true, &mut |_| Ok(()))?;
        Ok(Self {
            parent,
            name,
            file,
            identity,
            maximum: maximum as u64,
        })
    }
    /// Clone the original descriptor for the sole bounded writer, without reopening a path.
    pub(crate) fn writer_file(&self) -> Result<File> {
        self.parent
            .check_file(&self.file, &self.name, self.identity, 0)?;
        let file = self.file.try_clone()?;
        self.parent
            .check_file(&file, &self.name, self.identity, 0)?;
        Ok(file)
    }
    /// Seal against the raw digest and length accumulated by the actual writer.
    pub(crate) fn seal(self, expected: RawFileIdentity) -> Result<RetainedLoadFile> {
        let state = self
            .parent
            .check_file(&self.file, &self.name, self.identity, self.maximum)?;
        let retained = RetainedLoadFile::capture(
            self.parent,
            self.name,
            self.file,
            state,
            self.maximum,
            None,
        )?;
        ensure!(
            retained.identity()? == expected,
            "journal readback differs from written bytes"
        );
        Ok(retained)
    }
}

/// Original completed output and its immutable full file state; no path is re-admitted.
pub(crate) struct RetainedLoadFile {
    parent: Parent,
    name: OsString,
    file: File,
    state: FileState,
    identity: RawFileIdentity,
    maximum: u64,
    stage: Option<OsString>,
    poisoned: Cell<bool>,
}
impl std::fmt::Debug for RetainedLoadFile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RetainedLoadFile { .. }")
    }
}
impl RetainedLoadFile {
    pub(super) fn capture(
        parent: Parent,
        name: OsString,
        file: File,
        state: FileState,
        maximum: u64,
        stage: Option<OsString>,
    ) -> Result<Self> {
        ensure!(
            state.size > 0 && state.size <= maximum,
            "empty or oversized completed load output"
        );
        ensure!(
            parent.check_file(&file, &name, state.identity, maximum)? == state,
            "completed load output changed before readback"
        );
        let identity = RawFileIdentity {
            raw_sha256: raw_digest(&file, state.size)?,
            byte_length: state.size,
        };
        let owner = Self {
            parent,
            name,
            file,
            state,
            identity,
            maximum,
            stage,
            poisoned: Cell::new(false),
        };
        owner.identity()?;
        Ok(owner)
    }
    /// Verify both outputs as one stable namespace scan, including the earlier leaf.
    pub(crate) fn pair_identity(&self, other: &Self) -> Result<(RawFileIdentity, RawFileIdentity)> {
        self.pair_identity_with_midpoint(other, || Ok(()))
    }
    /// Check the same bounded pair scan with a deterministic mutation seam.
    pub(crate) fn pair_identity_with_midpoint(
        &self,
        other: &Self,
        mut midpoint: impl FnMut() -> Result<()>,
    ) -> Result<(RawFileIdentity, RawFileIdentity)> {
        let checked = (|| {
            let first_parent = held(self.parent.file())?;
            let second_parent = held(other.parent.file())?;
            let first = self.identity()?;
            midpoint()?;
            let second = other.identity()?;
            // No long reads follow these complete original-file metadata checks.
            self.check_metadata()?;
            other.check_metadata()?;
            ensure!(
                held(self.parent.file())? == first_parent
                    && held(other.parent.file())? == second_parent,
                "load output namespace changed during complete pair check"
            );
            self.parent.check()?;
            other.parent.check()?;
            Ok((first, second))
        })();
        if checked.is_err() {
            self.poisoned.set(true);
            other.poisoned.set(true);
        }
        checked
    }
    fn check_metadata(&self) -> Result<()> {
        ensure!(
            self.parent
                .check_file(&self.file, &self.name, self.state.identity, self.maximum)?
                == self.state,
            "completed load output full state changed"
        );
        if let Some(stage) = &self.stage {
            self.parent.require_absent(stage)?;
        }
        Ok(())
    }
    /// Recheck full metadata, original ancestors, raw bytes and removed stage name.
    pub(crate) fn identity(&self) -> Result<RawFileIdentity> {
        ensure!(
            !self.poisoned.replace(true),
            "load output owner is poisoned"
        );
        ensure!(
            self.parent
                .check_file(&self.file, &self.name, self.state.identity, self.maximum)?
                == self.state,
            "completed load output full state changed"
        );
        ensure!(
            raw_digest(&self.file, self.state.size)? == self.identity.raw_sha256,
            "completed load output bytes changed"
        );
        ensure!(
            self.parent
                .check_file(&self.file, &self.name, self.state.identity, self.maximum)?
                == self.state,
            "completed load output changed during readback"
        );
        if let Some(stage) = &self.stage {
            self.parent.require_absent(stage)?;
        }
        self.poisoned.set(false);
        Ok(self.identity)
    }
}
