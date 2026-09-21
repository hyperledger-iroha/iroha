//! Physical archive index custody with observable, nonblocking writer refusal.
//!
//! Archive qualification may hold a reader while authenticating Kura. A carrier
//! that already owns Kura must therefore probe its archive writer without waiting.
//! Every reader and writer signals after releasing the actual lock; the caller
//! drops all other physical guards before awaiting that release and retrying.

use std::{
    fmt,
    ops::{Deref, DerefMut},
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError},
};

/// Local contention names this index's release; poisoning remains a storage error.
#[derive(Debug)]
pub(super) enum ArchiveIndexLockError {
    /// The original physical writer unwound while holding the index.
    Poisoned,
    /// Another physical reader or writer currently prevents publication.
    Busy(concread::release::ReleaseWait),
}

/// One original archive index and its inseparable release notification source.
pub(super) struct ArchiveIndexLock<T> {
    inner: RwLock<T>,
    released: concread::release::ReleaseNotification,
}

/// Read custody signals after unlock, including unwinding without writer poison.
pub(super) struct ArchiveIndexReadGuard<'index, T> {
    inner: concread::release::ReleaseGuard<'index, RwLockReadGuard<'index, T>>,
}

/// Write custody preserves the standard lock's poison behavior on unwind.
pub(super) struct ArchiveIndexWriteGuard<'index, T> {
    inner: concread::release::ReleaseGuard<'index, RwLockWriteGuard<'index, T>>,
}

impl<T> ArchiveIndexLock<T> {
    /// Create the sole physical owner of one archive index.
    pub(super) fn new(value: T) -> Self {
        Self {
            inner: RwLock::new(value),
            released: concread::release::ReleaseNotification::default(),
        }
    }

    fn wrap_read<'index>(
        &'index self,
        guard: RwLockReadGuard<'index, T>,
    ) -> ArchiveIndexReadGuard<'index, T> {
        ArchiveIndexReadGuard {
            inner: self.released.guard(guard),
        }
    }

    fn wrap_write<'index>(
        &'index self,
        guard: RwLockWriteGuard<'index, T>,
    ) -> ArchiveIndexWriteGuard<'index, T> {
        ArchiveIndexWriteGuard {
            inner: self.released.poisoning_guard(guard),
        }
    }

    /// Acquire ordinary read custody, retaining notification until the real unlock.
    pub(super) fn read(&self) -> Result<ArchiveIndexReadGuard<'_, T>, ArchiveIndexLockError> {
        match self.inner.read() {
            Ok(guard) => Ok(self.wrap_read(guard)),
            Err(error) => {
                // A poisoned acquisition still owns a physical guard. Release it
                // through the same notification path rather than exposing it.
                drop(self.wrap_read(error.into_inner()));
                Err(ArchiveIndexLockError::Poisoned)
            }
        }
    }

    /// Acquire an ordinary writer when no enclosing Kura lease is held.
    pub(super) fn write(&self) -> Result<ArchiveIndexWriteGuard<'_, T>, ArchiveIndexLockError> {
        match self.inner.write() {
            Ok(guard) => Ok(self.wrap_write(guard)),
            Err(error) => {
                drop(self.wrap_write(error.into_inner()));
                Err(ArchiveIndexLockError::Poisoned)
            }
        }
    }

    /// Probe publication custody without waiting while retaining other fences.
    ///
    /// Observe before probing so an unlock before the caller's first poll cannot
    /// be lost. A wake grants no lock or archive authority: acquire and recheck on
    /// every retry, including when another reader still holds this same index.
    pub(super) fn try_write(&self) -> Result<ArchiveIndexWriteGuard<'_, T>, ArchiveIndexLockError> {
        // Poison is permanent. Do not turn it into a release dependency if a
        // concurrent erroneous acquisition is briefly dropping its own guard.
        if self.inner.is_poisoned() {
            return Err(ArchiveIndexLockError::Poisoned);
        }
        let wait = self.released.observe();
        match self.inner.try_write() {
            Ok(guard) => Ok(self.wrap_write(guard)),
            Err(TryLockError::WouldBlock) => Err(ArchiveIndexLockError::Busy(wait)),
            Err(TryLockError::Poisoned(error)) => {
                drop(self.wrap_write(error.into_inner()));
                Err(ArchiveIndexLockError::Poisoned)
            }
        }
    }
}

impl<T> Deref for ArchiveIndexReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> Deref for ArchiveIndexWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for ArchiveIndexWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T> fmt::Debug for ArchiveIndexLock<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArchiveIndexLock")
            .finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for ArchiveIndexReadGuard<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArchiveIndexReadGuard")
            .finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for ArchiveIndexWriteGuard<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArchiveIndexWriteGuard")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "archive_index_tests.rs"]
mod tests;
