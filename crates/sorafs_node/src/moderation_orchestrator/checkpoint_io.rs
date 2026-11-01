//! Original-directory custody for the moderation orchestrator's private checkpoint cache.

use super::ModerationOrchestratorError;
use iroha_fs::{
    AtomicWriteStep, FileIdentity, FileMetadata, FileSnapshot, OwnerDirectory, PublishMode,
    RetainedFile,
};
use std::{
    ffi::OsString,
    io::{self, Read as _},
    path::Path,
};

/// The same cache directory remains owned from admission through every publication and readback.
#[derive(Debug)]
pub(super) struct CheckpointCache {
    directory: OwnerDirectory,
    name: OsString,
}

impl CheckpointCache {
    pub(super) fn open(path: &Path) -> Result<Self, ModerationOrchestratorError> {
        let parent = path.parent().ok_or_else(|| {
            ModerationOrchestratorError::InvalidConfiguration(
                "checkpoint path has no parent".to_owned(),
            )
        })?;
        let name = path.file_name().ok_or_else(|| {
            ModerationOrchestratorError::InvalidConfiguration(
                "checkpoint path has no file name".to_owned(),
            )
        })?;
        let directory = OwnerDirectory::open_or_create(parent).map_err(checkpoint_io)?;
        Ok(Self {
            directory,
            name: name.to_owned(),
        })
    }

    pub(super) fn read(
        &self,
        maximum: u64,
    ) -> Result<Option<Vec<u8>>, ModerationOrchestratorError> {
        self.read_observed(maximum, None, |_| Ok(()))
    }

    fn read_observed(
        &self,
        maximum: u64,
        expected: Option<FileIdentity>,
        after_open: impl FnOnce(&RetainedFile) -> io::Result<()>,
    ) -> Result<Option<Vec<u8>>, ModerationOrchestratorError> {
        let Some(mut original) = self.open_original().map_err(checkpoint_io)? else {
            return Ok(None);
        };
        let before = FileSnapshot::of(original.file(), true).map_err(checkpoint_io)?;
        let result = (|| {
            if let Some(expected) = expected {
                let observed = original.identity().map_err(checkpoint_io)?;
                if observed != expected {
                    return Err(checkpoint_io(io::Error::other(
                        "checkpoint publication identity changed before readback",
                    )));
                }
            }
            let length = original.file().metadata().map_err(checkpoint_io)?.len();
            if length > maximum {
                return Err(checkpoint_bound(maximum));
            }
            after_open(&original).map_err(checkpoint_io)?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(usize::try_from(length).map_err(|_| checkpoint_bound(maximum))?)
                .map_err(|_| checkpoint_io(io::Error::other("checkpoint allocation refused")))?;
            original
                .file_mut()
                .take(maximum.saturating_add(1))
                .read_to_end(&mut bytes)
                .map_err(checkpoint_io)?;
            if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
                return Err(checkpoint_bound(maximum));
            }
            Ok(Some(bytes))
        })();
        // Private leaf and original ancestry custody close even on a bounded-read failure.
        // A changed source never produces accepted bytes or becomes optional absence.
        let after = FileSnapshot::of(original.file(), true).map_err(checkpoint_io)?;
        original.revalidate().map_err(checkpoint_io)?;
        self.directory.revalidate().map_err(checkpoint_io)?;
        if result.is_ok() && before != after {
            return Err(checkpoint_io(io::Error::other(
                "checkpoint changed during bounded read",
            )));
        }
        result
    }

    fn open_original(&self) -> io::Result<Option<RetainedFile>> {
        let result = (|| {
            let original = self.directory.open_retained_regular_optional(&self.name)?;
            if let Some(original) = &original {
                // The parent permits safe public readers; the checkpoint independently
                // requires canonical private current-owner custody on every platform.
                FileSnapshot::of(original.file(), true)?;
                original.revalidate()?;
            }
            Ok(original)
        })();
        self.directory.revalidate()?;
        result
    }

    fn current_snapshot(&self) -> io::Result<Option<FileSnapshot>> {
        let result = (|| {
            self.open_original()?
                .map(|original| {
                    let snapshot = FileSnapshot::of(original.file(), true)?;
                    original.revalidate()?;
                    Ok(snapshot)
                })
                .transpose()
        })();
        self.directory.revalidate()?;
        result
    }

    pub(super) fn write(
        &self,
        bytes: &[u8],
        maximum: u64,
    ) -> Result<(), ModerationOrchestratorError> {
        self.write_observed(bytes, maximum, |_, _| Ok(()))
    }

    fn write_observed(
        &self,
        bytes: &[u8],
        maximum: u64,
        mut observe: impl FnMut(AtomicWriteStep, &Path) -> io::Result<()>,
    ) -> Result<(), ModerationOrchestratorError> {
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
            return Err(checkpoint_bound(maximum));
        }
        let previous = self.current_snapshot().map_err(checkpoint_io)?;
        let mut published = false;
        let mut published_identity = None;
        let result = self.directory.write_atomic_observed(
            &self.name,
            bytes,
            PublishMode::Replace,
            128,
            |step, path| {
                if step == AtomicWriteStep::Published {
                    // Set this before any fallible observation after the native rename.
                    published = true;
                    // Metadata-only observation permits the publisher's still-held Windows
                    // DELETE handle. A streaming reader is admitted after publication closes.
                    published_identity = Some(FileMetadata::symlink_metadata(path)?.identity());
                }
                observe(step, path)?;
                if step == AtomicWriteStep::Publish && self.current_snapshot()? != previous {
                    return Err(io::Error::other(
                        "checkpoint destination changed before publication",
                    ));
                }
                Ok(())
            },
        );
        if let Err(error) = result {
            return Err(if published {
                checkpoint_uncertain(error)
            } else {
                checkpoint_io(error)
            });
        }
        let identity = published_identity.ok_or_else(|| {
            checkpoint_uncertain("checkpoint writer omitted its publication observation")
        })?;
        let persisted = self
            .read_observed(maximum, Some(identity), |_| Ok(()))
            .map_err(checkpoint_uncertain)?;
        if persisted.as_deref() != Some(bytes) {
            return Err(checkpoint_uncertain(
                "checkpoint cache disappeared or changed bytes after publication",
            ));
        }
        Ok(())
    }
}

fn checkpoint_io(error: impl std::fmt::Display) -> ModerationOrchestratorError {
    ModerationOrchestratorError::CheckpointIo(error.to_string())
}

fn checkpoint_uncertain(error: impl std::fmt::Display) -> ModerationOrchestratorError {
    ModerationOrchestratorError::CheckpointDurabilityUncertain(error.to_string())
}

fn checkpoint_bound(maximum: u64) -> ModerationOrchestratorError {
    ModerationOrchestratorError::ResourceExhausted {
        resource: "checkpoint bytes",
        limit: usize::try_from(maximum).unwrap_or(usize::MAX),
    }
}

#[cfg(test)]
#[path = "checkpoint_io/tests.rs"]
mod tests;
