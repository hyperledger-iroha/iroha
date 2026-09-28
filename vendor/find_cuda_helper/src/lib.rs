//! Build hook for `cust` with the workspace runtime-loaded CUDA driver.
//!
//! The upstream wrapper invokes this hook from its build script. Driver ownership
//! and capability discovery live in `cust_raw` and IVM at runtime; a CUDA toolkit
//! and native CUDA linker dependency are never required by this hook.

/// Emit build-script tracking without inspecting toolkit paths or linking CUDA.
pub fn include_cuda() {
    println!("cargo:rerun-if-changed=build.rs");
}
