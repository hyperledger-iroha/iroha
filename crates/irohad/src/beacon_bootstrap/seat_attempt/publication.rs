//! One immutable generated phase and its retained original publication progress.
//!
//! The attempt advances past signing before calling this owner. A failed writer
//! therefore resumes the same bytes and descriptor rather than invoking RNG or
//! signing again. The canonical file publisher is shared with final export.

use super::super::{
    Directory,
    seat_export::{ExportError, FileProgress, publish_file},
};
use iroha_crypto::Hash;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PhaseFile {
    GenerationIntent,
    Publication,
    Deliveries,
    Acceptances,
    CommitmentsInput,
    DeliveriesInput,
    SessionInput,
    CommitmentsProof,
    DeliveriesProof,
    SessionProof,
}
impl PhaseFile {
    fn name(self) -> &'static str {
        match self {
            Self::GenerationIntent => "producer-1-intent.norito",
            Self::Publication => "publication.norito",
            Self::Deliveries => "deliveries.norito",
            Self::Acceptances => "acceptances.norito",
            Self::CommitmentsInput => "input-commitments.norito",
            Self::DeliveriesInput => "input-deliveries.norito",
            Self::SessionInput => "input-final-session.norito",
            Self::CommitmentsProof => "proof-commitments.norito",
            Self::DeliveriesProof => "proof-deliveries.norito",
            Self::SessionProof => "proof-final-session.norito",
        }
    }
    fn private(self) -> bool {
        !matches!(
            self,
            Self::Publication | Self::Deliveries | Self::Acceptances
        )
    }
}

struct Source {
    address: usize,
    length: usize,
    digest: Hash,
}
impl Source {
    fn new(bytes: &[u8]) -> Self {
        Self {
            address: bytes.as_ptr().addr(),
            length: bytes.len(),
            digest: Hash::new(bytes),
        }
    }
    fn matches(&self, bytes: &[u8]) -> bool {
        self.address == bytes.as_ptr().addr()
            && self.length == bytes.len()
            && self.digest == Hash::new(bytes)
    }
}

/// Inline custody for one phase's source identity and original destination fd.
/// The surrounding attempt retains the source bytes until this phase commits.
/// The address is compared only as an identity value and is never dereferenced.
pub(super) struct PhasePublication {
    phase: PhaseFile,
    source: Option<Source>,
    progress: FileProgress,
    terminal_custody_failure: bool,
}
impl PhasePublication {
    pub(super) fn new(phase: PhaseFile) -> Self {
        Self {
            phase,
            source: None,
            progress: FileProgress::default(),
            terminal_custody_failure: false,
        }
    }
    /// Bind the produced source before the first fallible descriptor operation.
    /// Neither refusal nor completion allows another original source to replace it.
    pub(super) fn publish(
        &mut self,
        directory: &Directory,
        bytes: &[u8],
    ) -> Result<(), ExportError> {
        if self.terminal_custody_failure {
            return Err(ExportError::Custody);
        }
        if bytes.is_empty() {
            return Err(ExportError::Phase);
        }
        if let Some(source) = &self.source {
            if !source.matches(bytes) {
                self.terminal_custody_failure = true;
                return Err(ExportError::Custody);
            }
        } else {
            self.source = Some(Source::new(bytes));
        }
        let result = publish_file(
            directory,
            self.phase.name(),
            self.phase.private(),
            bytes,
            &mut self.progress,
        );
        if matches!(result, Err(ExportError::Custody)) {
            self.terminal_custody_failure = true;
        }
        result
    }
    /// Bind the authenticated original source and resume its held durability barrier.
    pub(super) fn restore_complete(
        &mut self,
        directory: &Directory,
        bytes: &[u8],
    ) -> Result<(), ExportError> {
        if self.terminal_custody_failure {
            return Err(ExportError::Custody);
        }
        if bytes.is_empty() {
            return Err(ExportError::Phase);
        }
        if let Some(source) = &self.source {
            if !source.matches(bytes) {
                self.terminal_custody_failure = true;
                return Err(ExportError::Custody);
            }
        } else {
            self.source = Some(Source::new(bytes));
        }
        let result = super::super::seat_export::restore_published_file(
            directory,
            self.phase.name(),
            self.phase.private(),
            bytes,
            &mut self.progress,
        );
        if matches!(result, Err(ExportError::Custody)) {
            self.terminal_custody_failure = true;
        }
        result
    }
    /// Hash only the source whose original file/directory barrier completed.
    /// This records custody, not proof or protocol authorization by itself.
    pub(super) fn complete_hash(&self) -> Result<[u8; 32], ExportError> {
        if self.terminal_custody_failure || !self.progress.complete() {
            return Err(ExportError::Phase);
        }
        self.source
            .as_ref()
            .map(|source| source.digest.into())
            .ok_or(ExportError::Phase)
    }
    pub(super) fn complete(&self) -> bool {
        self.progress.complete()
    }
}

#[cfg(test)]
mod tests;
