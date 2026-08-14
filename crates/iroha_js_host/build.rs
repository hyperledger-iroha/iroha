//! Build script configuring dynamic linker flags for the JS host crate.
fn emit_source_revision_rerun() {
    println!("cargo:rerun-if-env-changed=IROHA_GIT_COMMIT_HASH");
}
#[cfg(target_os = "macos")]
fn main() {
    emit_source_revision_rerun();
    // Allow binaries (including test harnesses) to defer resolving Node N-API symbols
    // until runtime, matching the dynamic lookup behaviour used for the cdylib loader.
    println!("cargo:rustc-link-arg=-undefined");
    println!("cargo:rustc-link-arg=dynamic_lookup");
}
#[cfg(all(target_family = "unix", not(target_os = "macos")))]
fn main() {
    emit_source_revision_rerun();
    // On other Unix targets (e.g., Linux) the default linker rejects unresolved symbols
    // when producing the cdylib for host tests. Relax the requirement so the Node process
    // can satisfy N-API imports at runtime, matching the macOS behaviour above.
    println!("cargo:rustc-link-arg=-Wl,--unresolved-symbols=ignore-all");
}
#[cfg(not(target_family = "unix"))]
fn main() {
    emit_source_revision_rerun();
}
