//! Shared build script to provide git metadata and feature warnings.
fn main() {
    build_support::emit_git_info();
    build_support::warn_if_ffi_conflict();
}
