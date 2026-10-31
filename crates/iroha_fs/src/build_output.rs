//! Exact Cargo-output retirement with its admitted initial hardlink count.

use super::*;

impl OwnerDirectory {
    /// Remove one admitted Cargo build-output name while preserving its other hardlinks.
    ///
    /// The caller must retain its exclusive build lock throughout this operation.
    /// Unlike private/runtime removal, this explicit build-cache operation admits
    /// an initial hardlink count greater than one. It freezes the complete original
    /// snapshot, including that count, until removing this name. It never promotes
    /// a retained build-input reader into deletion authority. `false` means only
    /// that the initial original open found no file.
    ///
    /// # Errors
    /// Refuses indirect, unsafe, changed or substituted files/ancestors and reports
    /// native deletion or directory durability failures. An error after deletion
    /// does not imply that the name remains present.
    pub fn remove_build_output(&self, name: impl AsRef<OsStr>) -> io::Result<bool> {
        let name = checked_name(name.as_ref())?;
        self.revalidate()?;
        let result = self.inner.remove_build_output_native(name);
        self.revalidate()?;
        result
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub fn before_named_removal(name: &OsStr) {
    tests::before_named_removal(name);
}
