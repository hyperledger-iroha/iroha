#![deny(warnings)]
//! Shared build utilities for the Iroha workspace.

use std::error::Error;

/// Emit git and cargo-related information using `vergen`.
///
/// # Errors
/// Fails if `vergen` cannot extract the required metadata.
pub fn emit_git_info() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    use vergen::{CargoBuilder, Emitter as CargoEmitter};
    use vergen_git2::{Emitter as GitEmitter, Git2Builder};

    // Emit only the metadata the workspace exposes today.
    let git_instructions = Git2Builder::default().sha(false).build()?;
    let cargo_instructions = CargoBuilder::default()
        .target_triple(true)
        .features(true)
        .build()?;

    GitEmitter::default()
        .add_instructions(&git_instructions)?
        .emit()?;
    CargoEmitter::default()
        .add_instructions(&cargo_instructions)?
        .emit()?;
    Ok(())
}

/// Warn if mutually exclusive FFI features are enabled simultaneously.
pub fn warn_if_ffi_conflict() {
    let ffi_import = std::env::var_os("CARGO_FEATURE_FFI_IMPORT").is_some();
    let ffi_export = std::env::var_os("CARGO_FEATURE_FFI_EXPORT").is_some();

    warn_if_ffi_conflict_with(ffi_import, ffi_export);
}

fn warn_if_ffi_conflict_with(ffi_import: bool, ffi_export: bool) {
    if ffi_import && ffi_export {
        println!("cargo:warning=Features `ffi_export` and `ffi_import` are mutually exclusive");
        println!("cargo:warning=When both active, `ffi_import` feature takes precedence");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_git_info_runs() {
        emit_git_info().unwrap();
    }

    #[test]
    fn warn_if_ffi_conflict_emits() {
        warn_if_ffi_conflict_with(true, true);
    }
}
