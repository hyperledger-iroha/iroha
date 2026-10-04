//! GPU build helper for the FASTPQ prover.
//!
//! Metal uses a separately admitted immutable compiled bundle; ordinary builds
//! never discover or invoke a Metal compiler. This script retains the existing
//! static CUDA path when `fastpq-gpu` is enabled.
// SPDX-License-Identifier: Apache-2.0
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    // Keep Cargo's default whole-package scan disabled for CPU-only builds.
    // Feature changes already select a distinct Cargo build-script unit.
    println!("cargo:rerun-if-changed=build.rs");
    let fastpq_gpu_feature = env::var_os("CARGO_FEATURE_FASTPQ_GPU").is_some();
    if !fastpq_gpu_feature {
        println!("cargo:rustc-cfg=fastpq_cuda_unavailable");
        return;
    }
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        // Metal hosts skip the static CUDA path. Native Metal eligibility is
        // independently determined by the private admitted compiled bundle.
        println!("cargo:rustc-cfg=fastpq_cuda_unavailable");
        return;
    }
    // Only the unchanged CUDA branch discovers a compiler.
    println!("cargo:rerun-if-env-changed=FASTPQ_SKIP_GPU_BUILD");
    println!("cargo:rerun-if-env-changed=CUDA_HOME");
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-changed=cuda/fastpq_cuda.cu");
    let skip_gpu_build = env::var_os("FASTPQ_SKIP_GPU_BUILD").is_some();
    println!("cargo:rerun-if-env-changed=PATH");
    if skip_gpu_build {
        println!("cargo:warning=FASTPQ_SKIP_GPU_BUILD set; CUDA backend disabled.");
        println!("cargo:rustc-cfg=fastpq_cuda_unavailable");
        return;
    }
    if !nvcc_available() {
        println!("cargo:warning=nvcc not found; CUDA backend disabled.");
        println!("cargo:rustc-cfg=fastpq_cuda_unavailable");
        return;
    }
    let cuda_root = locate_cuda_root();
    if let Some(root) = &cuda_root {
        let lib_dir = cuda_lib_dir(root);
        if let Some(path) = lib_dir {
            println!("cargo:rustc-link-search=native={}", path.display());
        }
    }
    // Link against cudart; nvcc will add device runtime automatically.
    println!("cargo:rustc-link-lib=cudart");
    let mut build = cc::Build::new();
    build.cuda(true);
    build.debug(false);
    build
        .file("cuda/fastpq_cuda.cu")
        .flag("-std=c++17")
        .flag("-O3")
        .flag("-lineinfo")
        .flag("-arch=sm_80")
        .flag("-Xptxas=-O3")
        .flag("-Xptxas=-fmad=false")
        .flag("-Xcompiler=-fno-fast-math")
        .flag("-Xcudafe=--display_error_number");
    if let Some(root) = &cuda_root {
        let include_dir = root.join("include");
        if include_dir.exists() {
            build.include(include_dir);
        }
    }
    if let Some(host_compiler) = select_cuda_host_compiler(&target_os) {
        build.ccbin(false);
        build.flag(format!("-ccbin={}", host_compiler.display()));
    } else if target_os == "linux" && !explicit_cxx_configured() {
        build.ccbin(false);
    }
    build.compile("fastpq_cuda");
}
fn nvcc_available() -> bool {
    Command::new("nvcc")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
fn locate_cuda_root() -> Option<PathBuf> {
    env::var_os("CUDA_HOME")
        .or_else(|| env::var_os("CUDA_PATH"))
        .map(PathBuf::from)
        .or_else(|| {
            let default = Path::new("/usr/local/cuda");
            if default.exists() {
                Some(default.to_path_buf())
            } else {
                None
            }
        })
}
fn cuda_lib_dir(root: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let candidate = root.join("lib").join("x64");
        candidate.exists().then_some(candidate)
    }
    #[cfg(not(windows))]
    {
        let candidate = root.join("lib64");
        if candidate.exists() {
            return Some(candidate);
        }
        let alt = root.join("lib");
        alt.exists().then_some(alt)
    }
}
fn select_cuda_host_compiler(target_os: &str) -> Option<PathBuf> {
    if target_os != "linux" || explicit_cxx_configured() {
        return None;
    }
    for candidate in [
        Path::new("/usr/bin/g++-12"),
        Path::new("/usr/local/bin/g++-12"),
        Path::new("/bin/g++-12"),
    ] {
        if candidate.exists() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}
fn explicit_cxx_configured() -> bool {
    env::var_os("CXX").is_some()
        || env::var_os("HOST_CXX").is_some()
        || env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CXX_"))
}
