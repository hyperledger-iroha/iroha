//! Shared build script to provide git metadata and feature warnings.
fn main() {
    if let Err(err) = build_support::emit_git_info() {
        eprintln!("cargo:warning=Failed to emit git info: {err}");
    }
    build_support::warn_if_ffi_conflict();
}
