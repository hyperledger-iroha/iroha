//! Storage operations beneath the one canonical strict-bound progress algorithm.
//!
//! The Windows implementation retains authoritative originals outside the algorithm's
//! local `File` values. Those local values are same-object clones, never reopened
//! substitutes. Existing Unix operations retain their descriptor-relative semantics.

use super::*;
use std::{fs::File, io};

/// The first actual storage failure or canonical protocol classification.
#[derive(Debug)]
pub(super) enum BoundProgressIoFailure {
    /// The initiating ordinary error, including its original source and path.
    Ordinary(Error),
    /// A canonical protocol validator refused the current image.
    Classified(BoundProgressRecoveryFailure),
}

impl BoundProgressIoFailure {
    /// Preserve the existing retryable-versus-invalid protocol distinction.
    pub(super) fn classification(&self) -> BoundProgressRecoveryFailure {
        match self {
            Self::Ordinary(error) => BoundProgressRecoveryFailure::from_kura(error),
            Self::Classified(failure) => *failure,
        }
    }
}

/// Platform storage for the existing strict intent, append and recovery logic.
pub(super) trait BoundProgressIo {
    /// Open an already-admitted original or prove its captured absence.
    fn optional(
        &mut self,
        kura: &Kura,
        namespace: &BoundProgressNamespace,
        path: &Path,
    ) -> Result<Option<File>>;

    /// Open a main original, creating only an explicitly admitted absent slot.
    fn main_file(
        &mut self,
        namespace: &BoundProgressNamespace,
        path: &Path,
        create: bool,
    ) -> io::Result<File>;

    /// Create the intent build into an owner slot before post-create checks.
    fn create_build(&mut self, namespace: &BoundProgressNamespace, path: &Path)
    -> io::Result<File>;

    /// Publish this exact original, retaining an immediate native publication latch.
    fn publish(
        &mut self,
        namespace: &BoundProgressNamespace,
        source: &Path,
        destination: &Path,
        original: &File,
        replace: bool,
    ) -> std::result::Result<(), BoundProgressPromotionError>;

    /// Retire only this owner's exact original and reconcile its close/barrier phase.
    fn retire(&mut self, namespace: &BoundProgressNamespace, path: &Path) -> io::Result<()>;

    /// Retain the first initiating error before later cleanup/classification work.
    fn retain_failure(&mut self, failure: Error);

    /// Retain the first canonical refusal without replacing an earlier ordinary error.
    fn retain_classification(&mut self, failure: BoundProgressRecoveryFailure);

    /// Inspect the first cause before any later diagnostic classification read.
    fn first_failure(&self) -> Option<BoundProgressRecoveryFailure>;

    /// Return the initiating error after the shared operation has finished cleanup.
    #[cfg(windows)]
    fn take_failure(&mut self) -> Option<BoundProgressIoFailure>;

    /// Record an ordinary error before a bool-returning protocol branch exits.
    fn io_failed<T>(&mut self, result: io::Result<T>, path: &Path) -> bool {
        match result {
            Ok(_) => false,
            Err(error) => {
                self.retain_failure(Error::IO(error, path.to_owned()));
                true
            }
        }
    }

    /// Check a current extent without discarding an initiating metadata failure.
    fn has_length(&mut self, file: &File, length: u64, path: &Path) -> bool {
        match file.metadata() {
            Ok(metadata) => metadata.len() == length,
            Err(error) => {
                self.retain_failure(Error::IO(error, path.to_owned()));
                false
            }
        }
    }

    /// Test a recovery name using this storage owner's captured authority.
    fn require_absent(&mut self, namespace: &BoundProgressNamespace, path: &Path)
    -> io::Result<()>;
}

/// Existing descriptor-relative backend; no Windows fallback is provided.
#[cfg(not(windows))]
#[derive(Default)]
pub(super) struct ExistingBoundProgressIo {
    first: Option<BoundProgressIoFailure>,
}

#[cfg(not(windows))]
impl BoundProgressIo for ExistingBoundProgressIo {
    fn optional(
        &mut self,
        kura: &Kura,
        namespace: &BoundProgressNamespace,
        path: &Path,
    ) -> Result<Option<File>> {
        kura.open_optional_bound_progress_file(namespace, path)
    }

    fn main_file(
        &mut self,
        namespace: &BoundProgressNamespace,
        path: &Path,
        create: bool,
    ) -> io::Result<File> {
        Kura::open_direct_sidecar_file_in_namespace(path, create, false, Some(namespace))
    }

    fn create_build(
        &mut self,
        namespace: &BoundProgressNamespace,
        path: &Path,
    ) -> io::Result<File> {
        Kura::create_new_bound_progress_temp(namespace, path)
    }

    fn publish(
        &mut self,
        namespace: &BoundProgressNamespace,
        source: &Path,
        destination: &Path,
        original: &File,
        replace: bool,
    ) -> std::result::Result<(), BoundProgressPromotionError> {
        if replace {
            Kura::promote_bound_progress_temp(namespace, source, destination, original)
        } else {
            Kura::promote_bound_progress_temp_noreplace(namespace, source, destination, original)
        }
    }

    fn retire(&mut self, namespace: &BoundProgressNamespace, path: &Path) -> io::Result<()> {
        Kura::remove_bound_progress_temp_if_present(namespace, path)
    }

    fn retain_failure(&mut self, failure: Error) {
        if self.first.is_none() {
            self.first = Some(BoundProgressIoFailure::Ordinary(failure));
        }
    }

    fn retain_classification(&mut self, failure: BoundProgressRecoveryFailure) {
        if self.first.is_none() {
            self.first = Some(BoundProgressIoFailure::Classified(failure));
        }
    }

    fn first_failure(&self) -> Option<BoundProgressRecoveryFailure> {
        self.first
            .as_ref()
            .map(BoundProgressIoFailure::classification)
    }

    #[cfg(windows)]
    fn take_failure(&mut self) -> Option<BoundProgressIoFailure> {
        self.first.take()
    }

    fn require_absent(
        &mut self,
        _namespace: &BoundProgressNamespace,
        path: &Path,
    ) -> io::Result<()> {
        match std::fs::symlink_metadata(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "progress recovery artifact remains",
            )),
        }
    }
}
